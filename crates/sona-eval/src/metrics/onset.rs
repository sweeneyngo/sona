//! Onset detection metrics — `mir_eval.onset.f_measure`, in pure Rust.
//!
//! Precision / Recall / F1 over onset *times*: an estimated onset is correct if
//! it falls within `window` of a reference onset (ADR-0011 uses ±50 ms), with
//! each reference and estimate onset used at most once. That last clause is why
//! this is a maximum bipartite matching, not a per-onset nearest-neighbour count
//! — two estimates crowding one reference onset can only score one hit.

use super::matching::matched_prf1;
use super::Prf1;

/// Default onset tolerance, in seconds (±50 ms).
pub const ONSET_WINDOW_S: f64 = 0.05;

/// Precision / Recall / F1 of estimated onsets against reference onsets, counting
/// an estimate correct within `window_s`. Both lists are onset times in seconds.
///
/// An onset is feasible for a reference by interval containment
/// (`est-w <= ref <= est+w`), which is how `mir_eval` tests it (via searchsorted)
/// rather than `|ref-est| <= w`; the two agree mathematically but differ at float
/// boundaries (e.g. `|1.0-1.05|` rounds just over 0.05, yet 1.0 is inside
/// `[1.05-0.05, 1.05+0.05]`). Following `mir_eval`'s form lets an onset exactly at
/// the window edge still count.
pub fn onset_metrics(reference: &[f64], estimate: &[f64], window_s: f64) -> Prf1 {
    matched_prf1(reference, estimate, |&r, &e| {
        r >= e - window_s && r <= e + window_s
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_alignment() {
        let m = onset_metrics(&[0.0, 1.0, 2.0], &[0.0, 1.0, 2.0], ONSET_WINDOW_S);
        assert_eq!((m.precision, m.recall, m.f1), (1.0, 1.0, 1.0));
    }

    #[test]
    fn within_and_outside_window() {
        // est[0] is 40 ms late (hit), est[1] is 60 ms late (miss).
        let m = onset_metrics(&[0.0, 1.0], &[0.04, 1.06], ONSET_WINDOW_S);
        assert!((m.precision - 0.5).abs() < 1e-12);
        assert!((m.recall - 0.5).abs() < 1e-12);
    }

    #[test]
    fn two_estimates_one_reference_scores_one_hit() {
        // Both estimates are within 50 ms of the single reference onset, but the
        // matching allows only one hit -> precision 1/2, recall 1/1.
        let m = onset_metrics(&[1.0], &[0.98, 1.02], ONSET_WINDOW_S);
        assert!((m.precision - 0.5).abs() < 1e-12);
        assert!((m.recall - 1.0).abs() < 1e-12);
    }

    #[test]
    fn boundary_is_inclusive() {
        // Exactly 50 ms away must count (mir_eval onset uses `<=`).
        let m = onset_metrics(&[1.0], &[1.05], ONSET_WINDOW_S);
        assert_eq!(m.recall, 1.0);
    }

    #[test]
    fn empty_sides_are_zero() {
        assert_eq!(onset_metrics(&[], &[0.0], ONSET_WINDOW_S).f1, 0.0);
        assert_eq!(onset_metrics(&[0.0], &[], ONSET_WINDOW_S).f1, 0.0);
    }
}
