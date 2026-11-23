//! The learned cell stage (ADR-0017, ported in ADR-0026): for every grid cell, a small bidirectional
//! GRU chooses rest or one of five candidate pitches, from GAME's encoder features on the vocals and
//! lead stems, GAME's notes on both, sona's grid notes, the pitch track and the octave check. This file
//! builds the per-cell features exactly as the Python prototype that was confirmed on holdout3 did
//! (`crates/sona-eval/scripts/cellstage/`), and turns the per-cell choices back into notes. Running the
//! model needs ONNX Runtime and lives with GAME (`game::CellModel`).

/// A GAME note as the stage sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageNote {
    pub onset_s: f64,
    pub offset_s: f64,
    pub midi: f32,
    pub voiced: bool,
}

/// GAME encoder output for one chunk: `frames` rows of `x_seg ‖ x_est`, 10 ms apart from `t0`.
#[derive(Debug, Clone, PartialEq)]
pub struct EncoderChunk {
    pub t0: f64,
    pub frames: usize,
    pub dim: usize,
    pub data: Vec<f32>,
}

/// GAME's encoder frame step.
pub const FRAME_S: f64 = 0.01;

/// Everything the features are built from. `lead_notes` are octave-corrected; `lead_ratios` are the
/// octave check's per-note log odd/even harmonic ratios, aligned with `lead_notes`.
#[derive(Debug, Clone)]
pub struct StageInputs<'a> {
    pub spacing: f64,
    pub anchor: f64,
    pub lead_notes: &'a [StageNote],
    pub lead_ratios: &'a [f32],
    pub vocals_notes: &'a [StageNote],
    /// sona's own grid-mode notes on the vocals stem: (onset, offset, MIDI).
    pub sona_notes: &'a [(f64, f64, i32)],
    pub lead_frames: &'a [EncoderChunk],
    pub vocals_frames: &'a [EncoderChunk],
    /// The vocals stem's pitch track: (time, Hz; 0 when unvoiced).
    pub f0: &'a [(f64, f64)],
    /// The vocals stem's energy envelope: (time, RMS).
    pub rms: &'a [(f64, f64)],
}

/// The candidates of one cell: rest, GAME on the lead, GAME on the vocals, sona, and the lead an octave
/// up and down; -1 where there is none.
pub type Candidates = [i32; 6];

/// A note snapped onto the grid: (onset, offset, MIDI).
type Snapped = (f64, f64, i32);

fn pyround(x: f64) -> f64 {
    x.round_ties_even()
}

/// GAME's tuning offset for a song: the circular mean of its voiced notes' fractional pitch.
fn song_tune(notes: &[StageNote]) -> f64 {
    let (mut x, mut y, mut any) = (0.0, 0.0, false);
    for n in notes.iter().filter(|n| n.voiced) {
        let frac = f64::from(n.midi) - f64::from(n.midi).round_ties_even();
        x += (std::f64::consts::TAU * frac).cos();
        y += (std::f64::consts::TAU * frac).sin();
        any = true;
    }
    if any {
        -y.atan2(x) / std::f64::consts::TAU
    } else {
        0.0
    }
}

/// GAME notes on sona's grid phase, tuned, monophonic: the prototype's `snap_notes`.
pub fn snap_notes(notes: &[StageNote], spacing: f64, anchor: f64) -> Vec<Snapped> {
    let tune = song_tune(notes);
    let mut out: Vec<Snapped> = notes
        .iter()
        .filter(|n| n.voiced)
        .map(|n| {
            let on = anchor + pyround((n.onset_s - anchor) / spacing) * spacing;
            let mut off = anchor + pyround((n.offset_s - anchor) / spacing) * spacing;
            if off <= on + 1e-9 {
                off = on + spacing;
            }
            (on, off, pyround(f64::from(n.midi) + tune) as i32)
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    for i in 1..out.len() {
        if out[i].0 < out[i - 1].1 - 1e-9 {
            if out[i].0 > out[i - 1].0 + 1e-9 {
                out[i - 1].1 = out[i].0;
            } else {
                out[i].0 = out[i - 1].1;
                out[i].1 = out[i].1.max(out[i].0 + spacing);
            }
        }
    }
    out
}

/// Per cell: the pitch under its centre, the centre's position in that note and the note's length, in
/// cells, and the note's index (`usize::MAX` where there is none).
struct OnGrid {
    pitch: Vec<i32>,
    pos: Vec<f64>,
    len: Vec<f64>,
    note: Vec<usize>,
}

fn on_grid(notes: &[Snapped], spacing: f64, anchor: f64, k0: i64, n: usize) -> OnGrid {
    let mut g = OnGrid { pitch: vec![-1; n], pos: vec![-1.0; n], len: vec![0.0; n], note: vec![usize::MAX; n] };
    for c in 0..n {
        let t = anchor + ((k0 + c as i64) as f64 + 0.5) * spacing;
        let i = notes.partition_point(|x| x.0 <= t);
        if i > 0 {
            let (on, off, p) = notes[i - 1];
            if on <= t && t < off {
                g.pitch[c] = p;
                g.pos[c] = (t - on) / spacing;
                g.len[c] = (off - on) / spacing;
                g.note[c] = i - 1;
            }
        }
    }
    g
}

/// The mean encoder frame over [a, b): the prototype's `cell_feat`.
fn cell_frames(chunks: &[EncoderChunk], a: f64, b: f64, out: &mut Vec<f64>) {
    let dim = chunks.first().map_or(0, |c| c.dim);
    let start = out.len();
    out.resize(start + dim, 0.0);
    let Some(ch) = chunks.iter().find(|c| c.t0 <= a && a < c.t0 + c.frames as f64 * FRAME_S) else { return };
    let i = ((a - ch.t0) / FRAME_S) as usize;
    let j = (i + 1).max(ch.frames.min(((b - ch.t0) / FRAME_S).ceil() as usize));
    let rows = (j - i) as f32;
    for d in 0..dim {
        let mut s = 0.0f32;
        for r in i..j {
            s += ch.data[r * dim + d];
        }
        out[start + d] = f64::from(s / rows);
    }
}

fn searchsorted(ts: &[(f64, f64)], t: f64) -> usize {
    ts.partition_point(|x| x.0 < t)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn hz_to_midi(hz: f64) -> f64 {
    if hz > 0.0 {
        69.0 + 12.0 * (hz.max(1e-9) / 440.0).log2()
    } else {
        0.0
    }
}

/// Which source note each candidate of a cell comes from (`usize::MAX` for none), so the notes the stage
/// writes can split where the chosen source starts a new note on the same pitch.
pub type Sources = [usize; 6];

/// The cells' features (row-major, `n` × width) and candidates, for cells `k0 .. k0 + n`.
pub fn cell_features(inp: &StageInputs<'_>, k0: i64, n: usize) -> (Vec<f32>, Vec<Candidates>) {
    let (x, cands, _) = cell_features_with_sources(inp, k0, n);
    (x, cands)
}

pub fn cell_features_with_sources(inp: &StageInputs<'_>, k0: i64, n: usize) -> (Vec<f32>, Vec<Candidates>, Vec<Sources>) {
    let (sp, anc) = (inp.spacing, inp.anchor);
    let lead = on_grid(&snap_notes(inp.lead_notes, sp, anc), sp, anc, k0, n);
    let voc = on_grid(&snap_notes(inp.vocals_notes, sp, anc), sp, anc, k0, n);
    let mut sona: Vec<Snapped> = inp.sona_notes.to_vec();
    sona.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    let son = on_grid(&sona, sp, anc, k0, n);

    let voiced_ratios: Vec<f64> = inp.lead_notes.iter().zip(inp.lead_ratios).filter(|(n, _)| n.voiced).map(|(_, &r)| f64::from(r)).collect();
    let song_ratio = if voiced_ratios.is_empty() { 0.0 } else { median(&mut voiced_ratios.clone()) };
    let ratio_at = |t: f64| -> f64 {
        let i = inp.lead_notes.partition_point(|x| x.onset_s <= t);
        if i == 0 {
            return 0.0;
        }
        let nt = inp.lead_notes[i - 1];
        if nt.onset_s <= t && t < nt.offset_s && nt.voiced {
            f64::from(inp.lead_ratios[i - 1])
        } else {
            0.0
        }
    };

    let mut x: Vec<f64> = Vec::new();
    let mut cands = Vec::with_capacity(n);
    let mut sources = Vec::with_capacity(n);
    for c in 0..n {
        let k = k0 + c as i64;
        let a = anc + k as f64 * sp;
        let b = a + sp;
        let l = lead.pitch[c];
        let cp: Candidates = [-1, l, voc.pitch[c], son.pitch[c], if l >= 0 { l + 12 } else { -1 }, if l >= 0 { l - 12 } else { -1 }];
        cands.push(cp);
        let li = lead.note[c];
        sources.push([usize::MAX, li, voc.note[c], son.note[c], li, li]);
        cell_frames(inp.vocals_frames, a, b, &mut x);
        cell_frames(inp.lead_frames, a, b, &mut x);
        let (i0, i1) = (searchsorted(inp.f0, a), searchsorted(inp.f0, b));
        let f0: Vec<f64> = inp.f0[i0..i1].iter().map(|f| hz_to_midi(f.1)).collect();
        let voiced = if f0.is_empty() { 0.0 } else { f0.iter().filter(|&&m| m > 0.0).count() as f64 / f0.len() as f64 };
        let mut pos: Vec<f64> = f0.iter().copied().filter(|&m| m > 0.0).collect();
        let med = if pos.is_empty() { 0.0 } else { median(&mut pos) };
        let r = &inp.rms[searchsorted(inp.rms, a)..searchsorted(inp.rms, b)];
        let log_rms = if r.is_empty() { -14.0 } else { (r.iter().map(|e| e.1).sum::<f64>() / r.len() as f64 + 1e-6).ln() };
        x.extend([voiced, med.ln_1p(), log_rms]);
        x.extend(cp[1..4].iter().map(|&p| f64::from(u8::from(p >= 0))));
        x.extend(cp[1..4].iter().map(|&p| if p >= 0 && med > 0.0 { med - f64::from(p) } else { 0.0 }));
        let both = |p: i32, q: i32| f64::from(u8::from(p == q && p >= 0));
        x.extend([both(cp[1], cp[2]), both(cp[1], cp[3]), both(cp[2], cp[3])]);
        x.extend([lead.pos[c], lead.len[c], voc.pos[c], voc.len[c], son.pos[c], son.len[c]]);
        x.extend([k.rem_euclid(4) as f64, k.rem_euclid(16) as f64 / 16.0]);
        x.extend([ratio_at(a + sp / 2.0), song_ratio]);
    }
    (x.into_iter().map(|v| v as f32).collect(), cands, sources)
}

/// The candidate each cell takes (0 = rest) from the model's logits, choosing only among the candidates
/// that exist: the prototype's masked argmax.
pub fn choose_class(logits: &[f32], cands: &[Candidates]) -> Vec<usize> {
    let nc = cands.first().map_or(6, |c| c.len());
    cands
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let row = &logits[i * nc..(i + 1) * nc];
            (0..nc).filter(|&j| j == 0 || cp[j] >= 0).max_by(|&a, &b| row[a].total_cmp(&row[b]).then(b.cmp(&a))).unwrap_or(0)
        })
        .collect()
}

/// Notes from the cells' choices: a note runs while the pitch holds, and splits where the pitch changes,
/// at a rest, or where the chosen source itself starts a new note on the same pitch.
pub fn cells_to_notes(classes: &[usize], cands: &[Candidates], sources: &[Sources], k0: i64, spacing: f64, anchor: f64) -> Vec<(f64, f64, i32)> {
    let mut out: Vec<(f64, f64, i32)> = Vec::new();
    let mut prev: Option<(usize, i32)> = None;
    for (c, &class) in classes.iter().enumerate() {
        let pitch = if class == 0 { -1 } else { cands[c][class] };
        let start = anchor + (k0 + c as i64) as f64 * spacing;
        let continues = match prev {
            Some((pc, pp)) => pitch >= 0 && pitch == pp && !(pc == class && sources[c][class] != sources[c - 1][class]),
            None => false,
        };
        if pitch < 0 {
            prev = None;
            continue;
        }
        if continues {
            if let Some(last) = out.last_mut() {
                last.1 = start + spacing;
            }
        } else {
            out.push((start, start + spacing, pitch));
        }
        prev = Some((class, pitch));
    }
    out
}

/// The pitch each cell sings (-1 for rest): [`choose_class`] mapped to its candidate.
pub fn choose(logits: &[f32], cands: &[Candidates]) -> Vec<i32> {
    let nc = cands.first().map_or(6, |c| c.len());
    cands
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let row = &logits[i * nc..(i + 1) * nc];
            let best = (0..nc).filter(|&j| j == 0 || cp[j] >= 0).max_by(|&a, &b| row[a].total_cmp(&row[b]).then(b.cmp(&a))).unwrap_or(0);
            if best == 0 {
                -1
            } else {
                cp[best]
            }
        })
        .collect()
}

/// Round to half precision and back: the prototype cached encoder features as float16.
pub fn to_f16_and_back(v: f32) -> f32 {
    let bits = v.to_bits();
    let sign = bits & 0x8000_0000;
    let exp = ((bits >> 23) & 0xff) as i32;
    let man = bits & 0x7f_ffff;
    if exp == 0xff {
        return v;
    }
    let e = exp - 127;
    if e > 15 {
        return f32::from_bits(sign | 0x7f80_0000);
    }
    if e < -24 {
        return f32::from_bits(sign);
    }
    // Keep 10 mantissa bits (fewer below the normal range), rounding to nearest, ties to even.
    let keep = if e >= -14 { 10 } else { (10 - (-14 - e)).max(0) };
    let drop = 23 - keep;
    let full = man | 0x80_0000;
    let half_ulp = 1u32 << (drop - 1);
    let mask = (1u32 << drop) - 1;
    let rem = full & mask;
    let mut kept = full >> drop;
    if rem > half_ulp || (rem == half_ulp && kept & 1 == 1) {
        kept += 1;
    }
    let value = (kept as f64) * 2f64.powi(e - keep);
    let rounded = value as f32;
    f32::from_bits(sign | rounded.to_bits())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(on: f64, off: f64, midi: f32) -> StageNote {
        StageNote { onset_s: on, offset_s: off, midi, voiced: true }
    }

    #[test]
    fn snapping_uses_bankers_rounding_tunes_and_stays_monophonic() {
        let notes = [note(0.05, 0.25, 60.2), note(0.24, 0.5, 62.2), StageNote { voiced: false, ..note(0.5, 0.6, 0.0) }];
        let s = snap_notes(&notes, 0.1, 0.0);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].2, s[1].2), (60, 62), "a 0.2-semitone sharp song is tuned back");
        assert!((s[0].0 - 0.0).abs() < 1e-9, "0.5 cells rounds to the even cell");
        assert!(s[0].1 <= s[1].0 + 1e-9, "no overlap");
    }

    #[test]
    fn notes_hold_while_the_pitch_does_and_split_where_the_source_restarts() {
        let cands: Vec<Candidates> = vec![[-1, 60, -1, 60, 72, 48]; 6];
        let sources: Vec<Sources> = vec![[usize::MAX, 0, usize::MAX, 0, 0, 0], [usize::MAX, 0, usize::MAX, 0, 0, 0], [usize::MAX, 1, usize::MAX, 0, 1, 1], [usize::MAX, 1, usize::MAX, 0, 1, 1], [usize::MAX, 1, usize::MAX, 0, 1, 1], [usize::MAX, 1, usize::MAX, 0, 1, 1]];
        // lead, lead (new lead note: split), lead, rest, sona, lead (switching source on one pitch: no split)
        let classes = [1, 1, 1, 0, 3, 1];
        let notes = cells_to_notes(&classes, &cands, &sources, 10, 0.5, 0.1);
        assert_eq!(notes, vec![(5.1, 6.1, 60), (6.1, 6.6, 60), (7.1, 8.1, 60)]);
    }

    #[test]
    fn halves_round_to_even_like_python() {
        // 0.25 / 0.5 and 0.75 / 0.5 are exactly half a cell: Python's round() goes to the even cell.
        let s = snap_notes(&[note(0.25, 0.75, 60.0)], 0.5, 0.0);
        assert_eq!((s[0].0, s[0].1), (0.0, 1.0));
        let up = snap_notes(&[note(0.75, 1.25, 60.0)], 0.5, 0.0);
        assert_eq!(up[0].0, 1.0);
    }

    #[test]
    fn frames_on_a_cell_boundary_belong_to_the_cell_they_start() {
        // A pitch frame exactly at a cell's start counts in that cell, not the one before (numpy's
        // left-side searchsorted).
        let f0 = [(0.0, 0.0), (0.5, 440.0), (1.0, 0.0)];
        let rms = [(0.0, 1.0), (0.5, 1.0)];
        let chunks: [EncoderChunk; 0] = [];
        let inp = StageInputs { spacing: 0.5, anchor: 0.0, lead_notes: &[], lead_ratios: &[], vocals_notes: &[], sona_notes: &[], lead_frames: &chunks, vocals_frames: &chunks, f0: &f0, rms: &rms };
        let (x, _) = cell_features(&inp, 0, 2);
        let width = x.len() / 2;
        assert_eq!((x[0], x[width]), (0.0, 1.0), "voiced share of cell 0 and cell 1");
    }

    #[test]
    fn half_precision_matches_ieee() {
        for (v, want) in [(1.0f32, 1.0f32), (0.1, 0.099_975_586), (65504.0, 65504.0), (1e-8, 0.0), (-2.5, -2.5), (std::f32::consts::PI, 3.140_625)] {
            assert_eq!(to_f16_and_back(v), want, "{v}");
        }
    }

    #[test]
    fn choose_only_among_existing_candidates() {
        let cands = [[-1, 60, -1, 62, 72, 48], [-1, -1, -1, -1, -1, -1]];
        let logits = [0.0, 1.0, 9.0, 2.0, 0.5, 0.1, 0.0, 5.0, 5.0, 5.0, 5.0, 5.0];
        assert_eq!(choose(&logits, &cands), vec![62, -1]);
    }

    /// `SONA_CELLSTAGE_PARITY=DIR cargo test -p sona-core -- --ignored prototype_parity`, where DIR holds
    /// one folder per song written by the prototype-side dumper (its exact inputs and features).
    #[test]
    #[ignore = "needs the prototype's dumped inputs (local corpus data)"]
    fn prototype_parity() {
        let root = std::env::var("SONA_CELLSTAGE_PARITY").expect("SONA_CELLSTAGE_PARITY");
        let mut songs: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("inputs.json").is_file()).collect();
        songs.sort();
        assert!(!songs.is_empty());
        for dir in songs {
            let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join("inputs.json")).unwrap()).unwrap();
            let notes = |k: &str| -> Vec<StageNote> {
                j[k].as_array().unwrap().iter().map(|n| StageNote { onset_s: n[0].as_f64().unwrap(), offset_s: n[1].as_f64().unwrap(), midi: n[2].as_f64().unwrap() as f32, voiced: n[3].as_bool().unwrap() }).collect()
            };
            let pairs = |k: &str| -> Vec<(f64, f64)> { j[k].as_array().unwrap().iter().map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap())).collect() };
            let f32s = |name: &str| -> Vec<f32> { std::fs::read(dir.join(name)).unwrap().chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect() };
            let chunks = |stem: &str| -> Vec<EncoderChunk> {
                let data = f32s(&format!("{stem}_frames.bin"));
                let mut at = 0;
                j["chunks"][stem].as_array().unwrap().iter().map(|c| {
                    let (frames, dim) = (c["frames"].as_u64().unwrap() as usize, c["dim"].as_u64().unwrap() as usize);
                    let chunk = EncoderChunk { t0: c["t0"].as_f64().unwrap(), frames, dim, data: data[at..at + frames * dim].to_vec() };
                    at += frames * dim;
                    chunk
                }).collect()
            };
            let (lead, vocals) = (notes("lead_notes"), notes("vocals_notes"));
            let ratios: Vec<f32> = j["lead_ratios"].as_array().unwrap().iter().map(|r| r.as_f64().unwrap() as f32).collect();
            let sona: Vec<(f64, f64, i32)> = j["sona_notes"].as_array().unwrap().iter().map(|n| (n[0].as_f64().unwrap(), n[1].as_f64().unwrap(), n[2].as_i64().unwrap() as i32)).collect();
            let (lf, vf, f0, rms) = (chunks("lead"), chunks("vocals"), pairs("f0"), pairs("rms"));
            let inp = StageInputs { spacing: j["spacing"].as_f64().unwrap(), anchor: j["anchor"].as_f64().unwrap(), lead_notes: &lead, lead_ratios: &ratios, vocals_notes: &vocals, sona_notes: &sona, lead_frames: &lf, vocals_frames: &vf, f0: &f0, rms: &rms };
            let (k0, n, width) = (j["k0"].as_i64().unwrap(), j["n"].as_u64().unwrap() as usize, j["width"].as_u64().unwrap() as usize);
            let (x, cands) = cell_features(&inp, k0, n);
            let want_x = f32s("x.bin");
            let want_c: Vec<i32> = std::fs::read(dir.join("cand.bin")).unwrap().chunks_exact(2).map(|b| i32::from(i16::from_le_bytes([b[0], b[1]]))).collect();
            assert_eq!(x.len(), want_x.len(), "{dir:?}: feature count");
            let got_c: Vec<i32> = cands.iter().flatten().copied().collect();
            let cand_diff = got_c.iter().zip(&want_c).filter(|(a, b)| a != b).count();
            let mut worst = vec![0f32; width];
            for (i, (a, b)) in x.iter().zip(&want_x).enumerate() {
                let d = (a - b).abs() / (1.0 + b.abs());
                worst[i % width] = worst[i % width].max(d);
            }
            let bad: Vec<(usize, f32)> = worst.iter().copied().enumerate().filter(|(_, d)| *d > 1e-6).collect();
            eprintln!("{:?}: {n} cells, candidate mismatches {cand_diff}, worst relative feature error {:.2e}, columns over 1e-6: {:?}", dir.file_name().unwrap(), worst.iter().copied().fold(0f32, f32::max), &bad[..bad.len().min(12)]);
            assert_eq!(cand_diff, 0, "{dir:?}: candidates");
            assert!(bad.is_empty(), "{dir:?}: features");
        }
    }
}
