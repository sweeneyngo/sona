//! Give an ONNX model's random nodes a fixed `seed` attribute, in memory. GAME's
//! D3PM segmenter samples with an unseeded `RandomUniformLike`, so without this
//! the same song gives different notes from run to run (ADR-0023).

pub const RANDOM_OPS: [&[u8]; 4] = [b"RandomUniformLike", b"RandomUniform", b"RandomNormalLike", b"RandomNormal"];
const MODEL_GRAPH: u32 = 7;
const GRAPH_NODE: u32 = 1;
const NODE_OP_TYPE: u32 = 4;
const NODE_ATTRIBUTE: u32 = 5;
const ATTR_NAME: u32 = 1;
const ATTR_F: u32 = 2;
const ATTR_TYPE: u32 = 20;
const ATTR_TYPE_FLOAT: u64 = 1;
const LEN: u8 = 2;

/// The model with every top-level random node seeded, and how many there were.
pub fn seed_random_nodes(model: &[u8], seed: f32) -> Result<(Vec<u8>, usize), String> {
    let mut seeded = 0;
    let out = rewrite(model, |field, body| {
        if field != MODEL_GRAPH {
            return Ok(None);
        }
        let graph = rewrite(body, |field, node| {
            if field != GRAPH_NODE || !is_random(node)? {
                return Ok(None);
            }
            // Each node its own seed: two same-shaped random nodes with one seed would draw identical streams.
            let node_seed = seed + seeded as f32;
            seeded += 1;
            Ok(Some(with_seed(node, node_seed)?))
        })?;
        Ok(Some(graph))
    })?;
    Ok((out, seeded))
}

fn is_random(node: &[u8]) -> Result<bool, String> {
    let mut op = None;
    for f in fields(node) {
        let f = f?;
        if f.number == NODE_OP_TYPE && f.wire == LEN {
            op = Some(f.body);
        }
    }
    Ok(op.is_some_and(|op| RANDOM_OPS.contains(&op)))
}

fn with_seed(node: &[u8], seed: f32) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(node.len() + 16);
    for f in fields(node) {
        let f = f?;
        if f.number == NODE_ATTRIBUTE && f.wire == LEN && attr_name(f.body)? == Some(b"seed".as_slice()) {
            continue;
        }
        out.extend_from_slice(f.raw);
    }
    let mut attr = Vec::new();
    put_len(&mut attr, ATTR_NAME, b"seed");
    put_key(&mut attr, ATTR_F, 5);
    attr.extend_from_slice(&seed.to_le_bytes());
    put_key(&mut attr, ATTR_TYPE, 0);
    put_varint(&mut attr, ATTR_TYPE_FLOAT);
    put_len(&mut out, NODE_ATTRIBUTE, &attr);
    Ok(out)
}

/// The attribute's name; like protobuf parsers, the last value of a repeated scalar field wins.
fn attr_name(attr: &[u8]) -> Result<Option<&[u8]>, String> {
    let mut name = None;
    for f in fields(attr) {
        let f = f?;
        if f.number == ATTR_NAME && f.wire == LEN {
            name = Some(f.body);
        }
    }
    Ok(name)
}

fn rewrite(msg: &[u8], mut edit: impl FnMut(u32, &[u8]) -> Result<Option<Vec<u8>>, String>) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(msg.len() + 64);
    for f in fields(msg) {
        let f = f?;
        match if f.wire == LEN { edit(f.number, f.body)? } else { None } {
            Some(body) => put_len(&mut out, f.number, &body),
            None => out.extend_from_slice(f.raw),
        }
    }
    Ok(out)
}

struct Field<'a> {
    number: u32,
    wire: u8,
    body: &'a [u8],
    raw: &'a [u8],
}

fn fields(msg: &[u8]) -> impl Iterator<Item = Result<Field<'_>, String>> {
    let mut pos = 0;
    std::iter::from_fn(move || {
        if pos >= msg.len() {
            return None;
        }
        let start = pos;
        let parsed = (|| {
            let key = varint(msg, &mut pos)?;
            let number = u32::try_from(key >> 3).map_err(|_| "field number out of range")?;
            let wire = (key & 7) as u8;
            let body = match wire {
                0 => {
                    varint(msg, &mut pos)?;
                    &msg[pos..pos]
                }
                1 => skip(msg, &mut pos, 8)?,
                2 => {
                    let n = usize::try_from(varint(msg, &mut pos)?).map_err(|_| "length out of range")?;
                    skip(msg, &mut pos, n)?
                }
                5 => skip(msg, &mut pos, 4)?,
                w => return Err(format!("unsupported protobuf wire type {w}")),
            };
            Ok(Field { number, wire, body, raw: &msg[start..pos] })
        })();
        if parsed.is_err() {
            pos = msg.len();
        }
        Some(parsed)
    })
}

fn varint(buf: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *buf.get(*pos).ok_or("truncated protobuf")?;
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("varint too long".into())
}

fn skip<'a>(buf: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let end = pos.checked_add(n).filter(|&e| e <= buf.len()).ok_or("truncated protobuf")?;
    let s = &buf[*pos..end];
    *pos = end;
    Ok(s)
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn put_key(out: &mut Vec<u8>, field: u32, wire: u8) {
    put_varint(out, (u64::from(field) << 3) | u64::from(wire));
}

fn put_len(out: &mut Vec<u8>, field: u32, body: &[u8]) {
    put_key(out, field, LEN);
    put_varint(out, body.len() as u64);
    out.extend_from_slice(body);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(op: &str, attrs: &[Vec<u8>]) -> Vec<u8> {
        let mut n = Vec::new();
        put_len(&mut n, 1, b"in");
        put_len(&mut n, 2, b"out");
        put_len(&mut n, NODE_OP_TYPE, op.as_bytes());
        for a in attrs {
            put_len(&mut n, NODE_ATTRIBUTE, a);
        }
        n
    }

    fn float_attr(name: &str, v: f32) -> Vec<u8> {
        let mut a = Vec::new();
        put_len(&mut a, ATTR_NAME, name.as_bytes());
        put_key(&mut a, ATTR_F, 5);
        a.extend_from_slice(&v.to_le_bytes());
        put_key(&mut a, ATTR_TYPE, 0);
        put_varint(&mut a, ATTR_TYPE_FLOAT);
        a
    }

    fn model(nodes: &[Vec<u8>]) -> Vec<u8> {
        let mut g = Vec::new();
        for n in nodes {
            put_len(&mut g, GRAPH_NODE, n);
        }
        put_len(&mut g, 2, b"graph-name");
        let mut m = Vec::new();
        put_key(&mut m, 1, 0);
        put_varint(&mut m, 8);
        put_len(&mut m, MODEL_GRAPH, &g);
        put_len(&mut m, 14, b"opset");
        m
    }

    fn nodes_of(m: &[u8]) -> Vec<Vec<u8>> {
        let g = fields(m).map(Result::unwrap).find(|f| f.number == MODEL_GRAPH).unwrap().body.to_vec();
        fields(&g).map(Result::unwrap).filter(|f| f.number == GRAPH_NODE).map(|f| f.body.to_vec()).collect()
    }

    fn seeds_of(node: &[u8]) -> Vec<f32> {
        fields(node)
            .map(Result::unwrap)
            .filter(|f| f.number == NODE_ATTRIBUTE && attr_name(f.body).unwrap() == Some(b"seed".as_slice()))
            .map(|f| {
                let v = fields(f.body).map(Result::unwrap).find(|g| g.number == ATTR_F).unwrap();
                f32::from_le_bytes(v.raw[v.raw.len() - 4..].try_into().unwrap())
            })
            .collect()
    }

    #[test]
    fn seeds_the_random_node_and_leaves_everything_else_byte_identical() {
        let add = node("Add", &[]);
        let rand = node("RandomUniformLike", &[float_attr("high", 1.0)]);
        let m = model(&[add.clone(), rand.clone()]);
        let (out, n) = seed_random_nodes(&m, 7.0).unwrap();
        assert_eq!(n, 1);
        let ns = nodes_of(&out);
        assert_eq!(ns[0], add);
        assert_eq!(seeds_of(&ns[1]), vec![7.0]);
        assert!(ns[1].starts_with(&rand), "the node's own fields are kept, in order");
        let (again, _) = seed_random_nodes(&m, 7.0).unwrap();
        assert_eq!(again, out);
    }

    #[test]
    fn gives_each_random_node_its_own_seed() {
        let m = model(&[node("RandomUniformLike", &[]), node("Add", &[]), node("RandomNormalLike", &[])]);
        let (out, n) = seed_random_nodes(&m, 5.0).unwrap();
        assert_eq!(n, 2);
        let ns = nodes_of(&out);
        assert_eq!((seeds_of(&ns[0]), seeds_of(&ns[2])), (vec![5.0], vec![6.0]));
    }

    #[test]
    fn replaces_an_existing_seed_rather_than_adding_a_second() {
        let m = model(&[node("RandomNormal", &[float_attr("seed", 3.0), float_attr("scale", 2.0)])]);
        let (out, n) = seed_random_nodes(&m, 1.0).unwrap();
        assert_eq!(n, 1);
        assert_eq!(seeds_of(&nodes_of(&out)[0]), vec![1.0]);
    }

    #[test]
    fn leaves_a_model_without_random_nodes_byte_identical() {
        let m = model(&[node("Add", &[]), node("Mul", &[float_attr("seed", 3.0)])]);
        let (out, n) = seed_random_nodes(&m, 1.0).unwrap();
        assert_eq!((out, n), (m, 0));
    }

    #[test]
    fn refuses_a_truncated_model() {
        let m = model(&[node("RandomUniformLike", &[])]);
        assert!(seed_random_nodes(&m[..m.len() - 3], 1.0).is_err());
    }
}

#[cfg(test)]
mod real_model {
    #[test]
    #[ignore = "needs a downloaded GAME release"]
    fn seeds_the_real_game_segmenter() {
        let dir = std::env::var("SONA_GAME_DIR").expect("SONA_GAME_DIR");
        let raw = std::fs::read(std::path::Path::new(&dir).join("segmenter.onnx")).unwrap();
        let (out, n) = super::seed_random_nodes(&raw, 1.0).unwrap();
        assert_eq!(n, 1);
        if let Ok(p) = std::env::var("SONA_SEEDED_OUT") {
            std::fs::write(p, out).unwrap();
        }
    }
}
