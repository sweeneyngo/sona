//! The cell stage's opt-in decoding with a cost per note edge (utau-edit-cost 19): a flicker the model is
//! unsure of goes, a change it is sure of stays, and the result is the best-scoring choice there is.
use sona_core::cellstage::{cells_to_notes, choose_class, decode_classes, Candidates, Sources};

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

const SP: f64 = 0.1;

fn notes(classes: &[usize], cands: &[Candidates], sources: &[Sources]) -> Vec<(f64, f64, i32)> {
    cells_to_notes(classes, cands, sources, 0, SP, 0.0)
}

/// Logits that give class `j` probability `p` and spread the rest evenly over the other existing classes.
fn row(cands: &Candidates, j: usize, p: f64) -> [f32; 6] {
    let others = (0..6).filter(|&k| k != j && (k == 0 || cands[k] >= 0)).count() as f64;
    std::array::from_fn(|k| if k == j { p.ln() as f32 } else { ((1.0 - p) / others).ln() as f32 })
}

#[test]
fn a_one_cell_flicker_the_model_is_unsure_of_is_smoothed_away() {
    // Lead says 60 throughout, sona says 62; the model leans to sona on cell 2 only, and barely.
    let cands: Vec<Candidates> = vec![[-1, 60, -1, 62, 72, 48]; 5];
    let sources: Vec<Sources> = vec![[None, Some(0), None, Some(0), Some(0), Some(0)]; 5];
    let logits: Vec<f32> = (0..5).flat_map(|c| if c == 2 { row(&cands[c], 3, 0.5) } else { row(&cands[c], 1, 0.9) }).collect();
    assert_eq!(notes(&choose_class(&logits, &cands), &cands, &sources).len(), 3, "the argmax keeps the blip");
    let smoothed = notes(&decode_classes(&logits, &cands, &sources, 1.0), &cands, &sources);
    assert_eq!(smoothed.len(), 1);
    assert_eq!(smoothed[0].2, 60);
    assert!((smoothed[0].1 - 0.5).abs() < 1e-9, "one note over all five cells: {smoothed:?}");
}

#[test]
fn a_change_the_model_is_sure_of_stays() {
    let cands: Vec<Candidates> = vec![[-1, 60, -1, 62, 72, 48]; 6];
    let sources: Vec<Sources> = vec![[None, Some(0), None, Some(0), Some(0), Some(0)]; 6];
    let logits: Vec<f32> = (0..6).flat_map(|c| if c >= 3 { row(&cands[c], 3, 0.95) } else { row(&cands[c], 1, 0.95) }).collect();
    let got = notes(&decode_classes(&logits, &cands, &sources, 1.0), &cands, &sources);
    assert_eq!(got.iter().map(|n| n.2).collect::<Vec<_>>(), vec![60, 62]);
}

#[test]
fn a_new_note_on_the_same_pitch_is_an_edge_too() {
    // Two candidates on 60: the lead restarts its note at cell 3, the vocals hold one note. The model barely
    // prefers the lead, so the argmax splits; with an edge cost it holds the note.
    let cands: Vec<Candidates> = vec![[-1, 60, 60, -1, 72, 48]; 6];
    let sources: Vec<Sources> = (0..6).map(|c| [None, Some(usize::from(c >= 3)), Some(0), None, Some(0), Some(0)]).collect();
    let logits: Vec<f32> = (0..6).flat_map(|_| [-3.0, 0.1, 0.0, 0.0, -3.0, -3.0]).collect();
    assert_eq!(notes(&choose_class(&logits, &cands), &cands, &sources).len(), 2);
    assert_eq!(notes(&decode_classes(&logits, &cands, &sources, 1.0), &cands, &sources).len(), 1);
}

#[test]
fn no_edge_cost_is_the_masked_argmax() {
    let mut rng = Rng(0x19_0001);
    for _ in 0..200 {
        let n = 1 + (rng.unit() * 40.0) as usize;
        let (cands, sources, logits) = random_case(&mut rng, n);
        assert_eq!(decode_classes(&logits, &cands, &sources, 0.0), choose_class(&logits, &cands));
    }
}

fn random_case(rng: &mut Rng, n: usize) -> (Vec<Candidates>, Vec<Sources>, Vec<f32>) {
    let cands: Vec<Candidates> = (0..n).map(|_| std::array::from_fn(|j| if j == 0 || rng.unit() < 0.3 { -1 } else { 60 + (rng.unit() * 3.0) as i32 })).collect();
    let sources: Vec<Sources> = (0..n).map(|c| std::array::from_fn(|j| (j > 0).then(|| c / (1 + (rng.unit() * 3.0) as usize)))).collect();
    let logits = (0..n * 6).map(|_| (rng.unit() * 6.0 - 3.0) as f32).collect();
    (cands, sources, logits)
}

/// The requirement, independent of the decoder: the cells' log-probabilities over the existing candidates,
/// minus the cost for every inner cell boundary where `cells_to_notes` starts or ends a note.
fn objective(classes: &[usize], logits: &[f32], cands: &[Candidates], sources: &[Sources], cost: f64) -> f64 {
    let mut total = 0.0;
    for (c, &k) in classes.iter().enumerate() {
        let row = &logits[c * 6..(c + 1) * 6];
        let valid: Vec<usize> = (0..6).filter(|&j| j == 0 || cands[c][j] >= 0).collect();
        if !valid.contains(&k) {
            return f64::NEG_INFINITY;
        }
        let z: f64 = valid.iter().map(|&j| f64::from(row[j]).exp()).sum();
        total += f64::from(row[k]) - z.ln();
    }
    let n = classes.len() as i64;
    let mut edges: Vec<i64> = notes(classes, cands, sources).iter().flat_map(|&(a, b, _)| [(a / SP).round() as i64, (b / SP).round() as i64]).filter(|&e| e > 0 && e < n).collect();
    edges.sort_unstable();
    edges.dedup();
    total - cost * edges.len() as f64
}

#[test]
fn the_decoding_is_the_best_choice_there_is() {
    let mut rng = Rng(0x19_0002);
    for case in 0..150 {
        let n = 1 + (rng.unit() * 5.0) as usize;
        let (cands, sources, logits) = random_case(&mut rng, n);
        let cost = rng.unit() * 3.0;
        let got = decode_classes(&logits, &cands, &sources, cost);
        let mut best = f64::NEG_INFINITY;
        for code in 0..6usize.pow(n as u32) {
            let classes: Vec<usize> = (0..n).map(|c| code / 6usize.pow(c as u32) % 6).collect();
            best = best.max(objective(&classes, &logits, &cands, &sources, cost));
        }
        let mine = objective(&got, &logits, &cands, &sources, cost);
        assert!((mine - best).abs() < 1e-9, "case {case}: decoded {got:?} scores {mine}, the best is {best}");
    }
}
