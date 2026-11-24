//! **Correction cost** — how much manual fixing a producer's output needed.
//!
//! sona is judged by ear: the yardstick is "how much did a human have to
//! correct?" (see `CONTEXT.md`). The review surface records that as the diff
//! between a producer's original notes (the *baseline*) and the user's edited
//! set, saved into each session JSON. This module recomputes that diff offline
//! so a run of saved sessions can be aggregated per producer.
//!
//! It is the Rust twin of `web/lib/correction.ts` and matches it note-for-note:
//! notes are paired by **onset proximity, ignoring pitch** (so a pitch
//! *correction* stays one fix, not a delete + an add), then per-field thresholds
//! decide whether onset/offset actually moved. Timing corrections larger than
//! the match window read as a delete + an add. The onset/offset change
//! thresholds are the ±50 ms the note metric uses (ADR-0011).
//!
//! [`edit_cost`] scores a transcription against a UST on grid cells instead, weighted by how hard each
//! error is to fix in OpenUtau. The two agree on the shared case file `fixtures/correction/cases.json`
//! for the classes both have (extra = deleted, missing = added, pitch, length = offset moved), which
//! `web/lib/correction.test.ts` asserts too.

use serde::Deserialize;

pub mod edit_cost;
pub mod lyric_placement;

/// A note as stored in a session file: raw seconds, MIDI pitch. Extra fields
/// (confidence, sub-scores, `manual`) are ignored.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct EditNote {
    pub onset_s: f64,
    pub offset_s: f64,
    pub midi_pitch: f64,
}

/// Thresholds governing how baseline and final notes are paired and compared.
#[derive(Debug, Clone, Copy)]
pub struct MatchParams {
    /// Max onset separation, in seconds, for two notes to pair as "the same note".
    pub match_window_s: f64,
    /// Onset shift, in seconds, beyond which a matched note counts as onset-corrected.
    pub onset_tol_s: f64,
    /// Offset shift, in seconds, beyond which a matched note counts as offset-corrected.
    pub offset_tol_s: f64,
}

impl Default for MatchParams {
    fn default() -> Self {
        MatchParams { match_window_s: 0.15, onset_tol_s: 0.05, offset_tol_s: 0.05 }
    }
}

/// Per-category correction counts plus their sum.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CorrectionCost {
    /// Notes the user added (misses).
    pub added: usize,
    /// Notes the user deleted (false positives / bleed).
    pub deleted: usize,
    /// Matched notes whose MIDI pitch changed.
    pub pitch_fixed: usize,
    /// Matched notes whose onset moved beyond the tolerance.
    pub onset_moved: usize,
    /// Matched notes whose offset moved beyond the tolerance.
    pub offset_moved: usize,
    /// Sum of the categories above (equal weights until calibrated).
    pub total: usize,
}

/// The correction cost of turning `baseline` into `final_`. A single matched
/// note can count in several categories at once (e.g. re-pitched *and* re-timed).
pub fn correction_cost(baseline: &[EditNote], final_: &[EditNote], p: &MatchParams) -> CorrectionCost {
    // All feasible (baseline, final) pairs within the match window, nearest first.
    let mut candidates: Vec<(f64, usize, usize)> = Vec::new();
    for (i, b) in baseline.iter().enumerate() {
        for (j, f) in final_.iter().enumerate() {
            let dist = (b.onset_s - f.onset_s).abs();
            if dist <= p.match_window_s {
                candidates.push((dist, i, j));
            }
        }
    }
    // Nearest first; break ties by index so the pairing is deterministic.
    candidates.sort_by(|a, b| {
        a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))
    });

    let mut used_b = vec![false; baseline.len()];
    let mut used_f = vec![false; final_.len()];
    let mut pitch_fixed = 0;
    let mut onset_moved = 0;
    let mut offset_moved = 0;
    for (_, i, j) in candidates {
        if used_b[i] || used_f[j] {
            continue;
        }
        used_b[i] = true;
        used_f[j] = true;
        let (b, f) = (&baseline[i], &final_[j]);
        if b.midi_pitch.round() as i64 != f.midi_pitch.round() as i64 {
            pitch_fixed += 1;
        }
        if (b.onset_s - f.onset_s).abs() > p.onset_tol_s {
            onset_moved += 1;
        }
        if (b.offset_s - f.offset_s).abs() > p.offset_tol_s {
            offset_moved += 1;
        }
    }
    let deleted = used_b.iter().filter(|u| !**u).count();
    let added = used_f.iter().filter(|u| !**u).count();
    let total = added + deleted + pitch_fixed + onset_moved + offset_moved;
    CorrectionCost { added, deleted, pitch_fixed, onset_moved, offset_moved, total }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(onset_s: f64, offset_s: f64, midi_pitch: f64) -> EditNote {
        EditNote { onset_s, offset_s, midi_pitch }
    }

    #[test]
    fn identical_sets_cost_nothing() {
        let notes = [n(0.0, 0.5, 60.0), n(0.6, 1.0, 62.0)];
        assert_eq!(correction_cost(&notes, &notes, &MatchParams::default()), CorrectionCost::default());
    }

    #[test]
    fn deletion_and_addition_are_counted() {
        let p = MatchParams::default();
        let base = [n(0.0, 0.5, 60.0), n(2.0, 2.5, 72.0)];
        let del = correction_cost(&base, &base[..1], &p);
        assert_eq!((del.deleted, del.added, del.total), (1, 0, 1));
        let add = correction_cost(&base[..1], &base, &p);
        assert_eq!((add.deleted, add.added, add.total), (0, 1, 1));
    }

    #[test]
    fn re_pitched_note_is_one_fix_not_delete_plus_add() {
        let c = correction_cost(&[n(0.0, 0.5, 60.0)], &[n(0.0, 0.5, 61.0)], &MatchParams::default());
        assert_eq!((c.pitch_fixed, c.added, c.deleted, c.total), (1, 0, 0, 1));
    }

    #[test]
    fn onset_move_is_counted_only_past_the_tolerance() {
        let p = MatchParams::default();
        // 80 ms > 50 ms tol, within the 150 ms window → paired and counted.
        let moved = correction_cost(&[n(0.0, 0.5, 60.0)], &[n(0.08, 0.5, 60.0)], &p);
        assert_eq!((moved.onset_moved, moved.total), (1, 1));
        // 30 ms < tol → unchanged.
        let still = correction_cost(&[n(0.0, 0.5, 60.0)], &[n(0.03, 0.5, 60.0)], &p);
        assert_eq!(still.total, 0);
    }

    #[test]
    fn large_timing_move_reads_as_delete_plus_add() {
        // 300 ms > 150 ms window: the notes don't pair.
        let c = correction_cost(&[n(0.0, 0.5, 60.0)], &[n(0.3, 0.8, 60.0)], &MatchParams::default());
        assert_eq!((c.deleted, c.added, c.onset_moved, c.total), (1, 1, 0, 2));
    }

    #[test]
    fn matching_is_one_to_one_under_contention() {
        // Two final notes near one baseline note: the nearer pairs, the other adds.
        let c = correction_cost(
            &[n(0.0, 0.5, 60.0)],
            &[n(0.0, 0.5, 60.0), n(0.03, 0.5, 60.0)],
            &MatchParams::default(),
        );
        assert_eq!((c.added, c.deleted, c.total), (1, 0, 1));
    }

    #[derive(Deserialize)]
    struct SharedCase {
        name: String,
        spacing_s: f64,
        anchor_s: f64,
        ust: Vec<(f64, f64, f64)>,
        output: Vec<(f64, f64, f64)>,
        correction: SharedCorrection,
        edit_cost: SharedEditCost,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct SharedCorrection {
        added: usize,
        deleted: usize,
        pitch_fixed: usize,
        onset_moved: usize,
        offset_moved: usize,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct SharedEditCost {
        extra: usize,
        missing: usize,
        pitch: usize,
        length: usize,
    }

    #[test]
    fn both_scorers_agree_with_correction_ts_on_every_shared_case() {
        use edit_cost::{edit_cost, CellGrid, ErrorClass};
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/correction/cases.json");
        let cases: Vec<SharedCase> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert!(cases.len() >= 6);
        let notes = |v: &[(f64, f64, f64)]| v.iter().map(|&(a, b, p)| n(a, b, p)).collect::<Vec<_>>();
        for c in &cases {
            let (ust, out) = (notes(&c.ust), notes(&c.output));
            let k = correction_cost(&out, &ust, &MatchParams::default());
            let got = SharedCorrection { added: k.added, deleted: k.deleted, pitch_fixed: k.pitch_fixed, onset_moved: k.onset_moved, offset_moved: k.offset_moved };
            assert_eq!(got, c.correction, "{}: correction", c.name);
            let e = edit_cost(&ust, &out, &CellGrid { spacing_s: c.spacing_s, anchor_s: c.anchor_s, cells_per_measure: 16 });
            let got = SharedEditCost { extra: e.count(ErrorClass::Extra), missing: e.count(ErrorClass::Missing), pitch: e.count(ErrorClass::Pitch), length: e.count(ErrorClass::Length) };
            assert_eq!(got, c.edit_cost, "{}: edit cost", c.name);
            assert_eq!((got.extra, got.missing, got.pitch, got.length), (k.deleted, k.added, k.pitch_fixed, k.offset_moved), "{}: shared classes", c.name);
            assert_eq!(e.cost(), (k.deleted + 2 * k.added + k.pitch_fixed + k.offset_moved) as u64, "{}: no other class", c.name);
        }
    }

    #[test]
    fn empty_sides() {
        let p = MatchParams::default();
        assert_eq!(correction_cost(&[], &[], &p).total, 0);
        assert_eq!(correction_cost(&[], &[n(0.0, 0.5, 60.0)], &p).added, 1);
        assert_eq!(correction_cost(&[n(0.0, 0.5, 60.0)], &[], &p).deleted, 1);
    }
}
