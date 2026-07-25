//! Note transcription metrics — `mir_eval.transcription`, in pure Rust.
//!
//! Precision / Recall / F1 over note *events*, at two strictnesses (ADR-0011):
//! - **COnP** (Correct Onset + Pitch): onset within ±50 ms and pitch within
//!   ±50 cents.
//! - **COnPOff** (+ Offset): additionally, the offset within `max(20 % of the
//!   reference note's duration, 50 ms)`.
//!
//! As with onsets, "correct" is the size of a maximum matching under those
//! constraints, so an estimate can't be double-counted against two references.
//!
//! Pitches here are integer MIDI. `mir_eval` measures pitch distance in cents
//! from Hz (`1200·|log2(ref)−log2(est)|`); for two MIDI integers that is exactly
//! `100·|Δsemitone|`, so at a 50-cent tolerance only an exact pitch match passes
//! — which is what we compute directly, no Hz round-trip.

use super::matching::{matched_prf1, maximum_matching};
use super::Prf1;
use crate::estimate::EstNote;
use crate::reference::RefNote;

/// Tolerances for note matching. Defaults are `mir_eval`'s (ADR-0011).
#[derive(Debug, Clone, Copy)]
pub struct NoteParams {
    /// Onset tolerance, in seconds.
    pub onset_tolerance_s: f64,
    /// Pitch tolerance, in cents.
    pub pitch_tolerance_cents: f64,
    /// Fraction of the reference note's duration allowed as offset slack.
    pub offset_ratio: f64,
    /// Floor on the offset tolerance, in seconds.
    pub offset_min_tolerance_s: f64,
}

impl Default for NoteParams {
    fn default() -> Self {
        NoteParams {
            onset_tolerance_s: 0.05,
            pitch_tolerance_cents: 50.0,
            offset_ratio: 0.2,
            offset_min_tolerance_s: 0.05,
        }
    }
}

/// Decimals `mir_eval` rounds onset/offset distances to before comparing, so a
/// distance of exactly the tolerance isn't rejected by float noise (`N_DECIMALS`).
const N_DECIMALS: i32 = 4;

fn round_dist(x: f64) -> f64 {
    let scale = 10f64.powi(N_DECIMALS);
    (x * scale).round() / scale
}

/// COnP: Precision / Recall / F1 with onset + pitch constraints (offset ignored).
pub fn con_p(reference: &[RefNote], estimate: &[EstNote], params: &NoteParams) -> Prf1 {
    prf1(reference, estimate, params, false)
}

/// COnPOff: COnP plus the offset constraint.
pub fn con_p_off(reference: &[RefNote], estimate: &[EstNote], params: &NoteParams) -> Prf1 {
    prf1(reference, estimate, params, true)
}

fn prf1(
    reference: &[RefNote],
    estimate: &[EstNote],
    params: &NoteParams,
    with_offset: bool,
) -> Prf1 {
    matched_prf1(reference, estimate, |r, e| {
        feasible_pair(r, e, params, with_offset)
    })
}

/// The maximum matching of reference→estimate notes under the given constraints,
/// as `(ref_idx, est_idx)` pairs. Exposed so the confidence-calibration metric
/// can ask "which estimated notes are correct (COnP)?" without re-deriving it.
pub fn match_notes(
    reference: &[RefNote],
    estimate: &[EstNote],
    params: &NoteParams,
    with_offset: bool,
) -> Vec<(usize, usize)> {
    let feasible: Vec<Vec<usize>> = reference
        .iter()
        .map(|r| {
            estimate
                .iter()
                .enumerate()
                .filter(|(_, e)| feasible_pair(r, e, params, with_offset))
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    maximum_matching(&feasible)
}

/// Whether reference note `r` and estimate note `e` satisfy every active
/// constraint (onset, pitch, and — when `with_offset` — offset).
fn feasible_pair(r: &RefNote, e: &EstNote, params: &NoteParams, with_offset: bool) -> bool {
    let onset_ok = round_dist((r.onset_s - e.onset_s).abs()) <= params.onset_tolerance_s;

    // Integer MIDI: cent distance is 100 cents per semitone.
    let pitch_cents = 100.0 * (r.midi_pitch as f64 - e.midi_pitch as f64).abs();
    let pitch_ok = pitch_cents <= params.pitch_tolerance_cents;

    let offset_ok = if with_offset {
        let ref_dur = r.offset_s - r.onset_s;
        let tol = (params.offset_ratio * ref_dur).max(params.offset_min_tolerance_s);
        round_dist((r.offset_s - e.offset_s).abs()) <= tol
    } else {
        true
    };

    onset_ok && pitch_ok && offset_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rn(onset: f64, offset: f64, midi: u8) -> RefNote {
        RefNote { onset_s: onset, offset_s: offset, midi_pitch: midi }
    }
    fn en(onset: f64, offset: f64, midi: u8) -> EstNote {
        EstNote { onset_s: onset, offset_s: offset, midi_pitch: midi, confidence: 1.0 }
    }

    #[test]
    fn exact_match_is_perfect() {
        let reference = vec![rn(0.0, 0.5, 60), rn(0.6, 1.0, 62)];
        let estimate = vec![en(0.0, 0.5, 60), en(0.6, 1.0, 62)];
        let p = con_p(&reference, &estimate, &NoteParams::default());
        assert_eq!((p.precision, p.recall, p.f1), (1.0, 1.0, 1.0));
        let po = con_p_off(&reference, &estimate, &NoteParams::default());
        assert_eq!((po.precision, po.recall, po.f1), (1.0, 1.0, 1.0));
    }

    #[test]
    fn wrong_pitch_never_matches() {
        // Off by a semitone (100 cents > 50) -> no COnP match.
        let reference = vec![rn(0.0, 0.5, 60)];
        let estimate = vec![en(0.0, 0.5, 61)];
        assert_eq!(con_p(&reference, &estimate, &NoteParams::default()).f1, 0.0);
    }

    #[test]
    fn onset_outside_window_misses() {
        // 60 ms late -> onset miss.
        let reference = vec![rn(0.0, 0.5, 60)];
        let estimate = vec![en(0.06, 0.5, 60)];
        assert_eq!(con_p(&reference, &estimate, &NoteParams::default()).recall, 0.0);
    }

    #[test]
    fn offset_forgiven_by_conp_but_not_conpoff() {
        // Right onset+pitch, but offset is way off. 1 s note -> offset tol =
        // max(0.2*1.0, 0.05) = 200 ms; estimate offset is 400 ms early -> COnP
        // hits, COnPOff misses.
        let reference = vec![rn(0.0, 1.0, 60)];
        let estimate = vec![en(0.0, 0.6, 60)];
        let params = NoteParams::default();
        assert_eq!(con_p(&reference, &estimate, &params).f1, 1.0);
        assert_eq!(con_p_off(&reference, &estimate, &params).f1, 0.0);
    }

    #[test]
    fn short_note_uses_the_50ms_offset_floor() {
        // 100 ms note -> 0.2*0.1 = 20 ms < 50 ms floor, so tol = 50 ms.
        // Offset 40 ms off -> within floor -> COnPOff hits.
        let reference = vec![rn(0.0, 0.1, 60)];
        let estimate = vec![en(0.0, 0.14, 60)];
        assert_eq!(con_p_off(&reference, &estimate, &NoteParams::default()).f1, 1.0);
    }

    #[test]
    fn extra_estimate_lowers_precision_only() {
        // One good note + one spurious note: recall 1/1, precision 1/2.
        let reference = vec![rn(0.0, 0.5, 60)];
        let estimate = vec![en(0.0, 0.5, 60), en(2.0, 2.5, 72)];
        let p = con_p(&reference, &estimate, &NoteParams::default());
        assert!((p.recall - 1.0).abs() < 1e-12);
        assert!((p.precision - 0.5).abs() < 1e-12);
    }

    #[test]
    fn matching_is_one_to_one_under_contention() {
        // Two estimates both near the one reference note -> only one can match.
        let reference = vec![rn(0.0, 0.5, 60)];
        let estimate = vec![en(0.0, 0.5, 60), en(0.03, 0.5, 60)];
        let m = match_notes(&reference, &estimate, &NoteParams::default(), false);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn empty_sides_are_zero() {
        let params = NoteParams::default();
        assert_eq!(con_p(&[], &[en(0.0, 0.5, 60)], &params).f1, 0.0);
        assert_eq!(con_p(&[rn(0.0, 0.5, 60)], &[], &params).f1, 0.0);
    }
}
