//! The cell stage's opt-in rounding-residual features (utau-edit-cost 19): a model trained without them gets
//! exactly the prototype's features, and one trained with them sees, at every cell edge, how far GAME lead's
//! raw note edge sat from the cell it was rounded to.
use sona_core::cellstage::{cell_features, cell_features_with, EncoderChunk, FeatureSet, StageInputs, StageNote, RESIDUAL_WIDTH};

const SPACING: f64 = 0.1;

fn note(onset_s: f64, offset_s: f64, voiced: bool) -> StageNote {
    StageNote { onset_s, offset_s, midi: 60.0, voiced }
}

struct Case {
    lead: Vec<StageNote>,
    ratios: Vec<f32>,
    frames: Vec<EncoderChunk>,
    f0: Vec<(f64, f64)>,
    rms: Vec<(f64, f64)>,
    sona: Vec<(f64, f64, i32)>,
}

impl Case {
    /// Lead note A: raw 0.04–0.31 s, rounded to cells 0–3 (onset 0.4 cell early of its raw time, offset 0.1
    /// cell). Note B: raw 0.36–0.60 s, rounded to cells 4–6 (onset 0.4 cell late, offset exact). An unvoiced
    /// stretch after it.
    fn new() -> Self {
        let lead = vec![note(0.04, 0.31, true), note(0.36, 0.60, true), note(0.60, 0.68, false)];
        let frames = vec![EncoderChunk { t0: 0.0, frames: 80, dim: 2, data: (0..160).map(|i| i as f32 * 0.01).collect() }];
        let f0 = (0..80).map(|i| (i as f64 * 0.01, 261.6)).collect();
        let rms = (0..80).map(|i| (i as f64 * 0.01, 0.3)).collect();
        Self { ratios: vec![0.1; lead.len()], lead, frames, f0, rms, sona: vec![(0.0, 0.3, 60), (0.4, 0.6, 60)] }
    }

    fn inputs(&self) -> StageInputs<'_> {
        StageInputs {
            spacing: SPACING,
            anchor: 0.0,
            lead_notes: &self.lead,
            lead_ratios: &self.ratios,
            vocals_notes: &self.lead,
            sona_notes: &self.sona,
            lead_frames: &self.frames,
            vocals_frames: &self.frames,
            f0: &self.f0,
            rms: &self.rms,
        }
    }
}

const CELLS: usize = 8;

#[test]
fn a_model_without_the_extras_reads_exactly_the_prototypes_features() {
    let case = Case::new();
    let plain = cell_features(&case.inputs(), 0, CELLS);
    let default = cell_features_with(&case.inputs(), 0, CELLS, FeatureSet::default());
    assert_eq!(plain, default);
    assert!(!FeatureSet::default().lead_residuals);
}

#[test]
fn the_residuals_follow_the_prototypes_features_in_each_row() {
    let case = Case::new();
    let (plain, cands, sources) = cell_features(&case.inputs(), 0, CELLS);
    let (extra, cands_x, sources_x) = cell_features_with(&case.inputs(), 0, CELLS, FeatureSet { lead_residuals: true });
    let width = plain.len() / CELLS;
    assert_eq!(extra.len(), CELLS * (width + RESIDUAL_WIDTH));
    for c in 0..CELLS {
        assert_eq!(&extra[c * (width + RESIDUAL_WIDTH)..c * (width + RESIDUAL_WIDTH) + width], &plain[c * width..(c + 1) * width], "cell {c}");
    }
    assert_eq!((cands, sources), (cands_x, sources_x), "the extras change no candidate");
}

/// Each cell's residual block: start edge (onset here?, its lean; offset here?, its lean), then the end edge.
fn residuals(case: &Case) -> Vec<[f32; RESIDUAL_WIDTH]> {
    let (x, _, _) = cell_features_with(&case.inputs(), 0, CELLS, FeatureSet { lead_residuals: true });
    let width = x.len() / CELLS;
    (0..CELLS).map(|c| x[(c + 1) * width - RESIDUAL_WIDTH..(c + 1) * width].try_into().expect("block")).collect()
}

#[test]
fn each_edge_carries_how_far_the_lead_notes_raw_edge_sat_from_it() {
    let got = residuals(&Case::new());
    let none = [0.0; 4];
    let onset = |r: f32| [1.0, r, 0.0, 0.0];
    let offset = |r: f32| [0.0, 0.0, 1.0, r];
    let cell = |start: [f32; 4], end: [f32; 4]| -> [f32; RESIDUAL_WIDTH] { [start, end].concat().try_into().expect("8") };
    let want = [
        cell(onset(0.4), none),    // A starts on edge 0, its raw onset 0.4 cell later
        cell(none, none),          //
        cell(none, offset(0.1)),   // A ends on edge 3, raw 0.1 cell later
        cell(offset(0.1), onset(-0.4)), // B starts on edge 4, raw 0.4 cell earlier
        cell(onset(-0.4), none),
        cell(none, offset(0.0)),   // B ends on edge 6, exactly
        cell(offset(0.0), none),   // the unvoiced note after it carries nothing
        cell(none, none),
    ];
    for (c, (g, w)) in got.iter().zip(&want).enumerate() {
        for (a, b) in g.iter().zip(w) {
            assert!((a - b).abs() < 1e-5, "cell {c}: got {g:?}, want {w:?}");
        }
    }
}

#[test]
fn a_note_cut_short_by_its_neighbour_reports_at_most_two_cells() {
    // Both start on cell 0; the snap moves the second to the first's end (cell 10), 10 cells from its raw onset.
    let mut case = Case::new();
    case.lead = vec![note(0.0, 1.0, true), note(0.01, 1.4, true)];
    case.ratios = vec![0.1; 2];
    let (x, _, _) = cell_features_with(&case.inputs(), 0, 12, FeatureSet { lead_residuals: true });
    let width = x.len() / 12;
    let cell9 = &x[10 * width - RESIDUAL_WIDTH..10 * width];
    assert_eq!(&cell9[4..8], &[1.0, -2.0, 1.0, 0.0], "on edge 10: the moved onset, clamped, and the first note's exact end");
}
