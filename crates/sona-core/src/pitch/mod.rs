//! **Pitch tracking** — the first stage of the perception pipeline (ADR-0001).
//!
//! A [`PitchTracker`] turns mono audio into a per-frame f0 contour with voicing
//! confidence — `audio → (f0 contour, voicing)` and *nothing musical baked in*.
//! Notes, onsets, grid, and BPM are later stages' and the UI's concern; a
//! tracker only perceives pitch. That narrow seam is what lets pYIN (the DSP
//! default, [`pyin::PYin`]) and CREPE (neural, ticket 11) be swapped and
//! compared behind one interface.
//!
//! The output is a [`Vec<F0Frame>`] — exactly the annotation JSON's
//! `f0_contour` field (ADR-0004), so a tracker's result drops straight into an
//! [`Annotation`](crate::annotation::Annotation).

pub mod pyin;

pub use pyin::PYin;

use crate::annotation::F0Frame;

/// A pluggable pitch-tracking backend (ADR-0001).
///
/// The contract is deliberately minimal: given mono `samples` at `sample_rate`,
/// produce a per-frame f0 contour with voicing. Frame timing, hop size, and any
/// smoothing are the implementation's business; the caller only sees the
/// resulting [`F0Frame`]s. Implementations must be pure perception — no note
/// segmentation, no grid, no BPM.
pub trait PitchTracker {
    /// Estimate the f0 contour of `samples` (mono, `-1.0..=1.0`) at
    /// `sample_rate` Hz. Frames are returned in time order; a frame with
    /// `frequency_hz == 0.0` and low `voicing` marks an unvoiced region.
    fn track(&self, samples: &[f32], sample_rate: u32) -> Vec<F0Frame>;
}

/// Sliding-window median filter over an f0 track, **skipping non-positive
/// samples** (unvoiced / missing frames encoded as `0.0`).
///
/// This is the contour-stage guard the spec calls for: an isolated octave error
/// or jitter spike is a lone outlier among its neighbours, so the window median
/// replaces it with the surrounding pitch. Unvoiced frames don't drag voiced
/// medians because they're excluded from the window; a window with no voiced
/// sample stays `0.0`.
///
/// `window` is the full width in frames, clamped to an odd number `>= 1`.
pub fn median_filter_positive(values: &[f32], window: usize) -> Vec<f32> {
    let width = window.max(1) | 1; // force odd so the window is symmetric
    let half = width / 2;
    let mut out = vec![0.0f32; values.len()];
    let mut scratch: Vec<f32> = Vec::with_capacity(width);

    for (i, slot) in out.iter_mut().enumerate() {
        scratch.clear();
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(values.len());
        for &v in &values[lo..hi] {
            if v > 0.0 && v.is_finite() {
                scratch.push(v);
            }
        }
        if !scratch.is_empty() {
            scratch.sort_by(|a, b| a.partial_cmp(b).expect("no NaNs: filtered above"));
            *slot = scratch[scratch.len() / 2];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_filter_replaces_isolated_octave_error() {
        // A single doubled ("octave up") frame in an otherwise steady 440 Hz
        // contour is a classic YIN error the median must erase.
        let track = [440.0, 441.0, 880.0, 439.0, 440.0];
        let filtered = median_filter_positive(&track, 5);
        assert!((filtered[2] - 440.0).abs() < 2.0, "got {}", filtered[2]);
    }

    #[test]
    fn median_filter_skips_unvoiced_zeros() {
        // Zeros (unvoiced) must not pull a voiced frame's median toward 0.
        let track = [0.0, 0.0, 440.0, 441.0, 442.0];
        let filtered = median_filter_positive(&track, 5);
        assert!(
            filtered[2] >= 440.0 && filtered[2] <= 442.0,
            "got {}",
            filtered[2]
        );
    }

    #[test]
    fn median_filter_leaves_all_unvoiced_at_zero() {
        let track = [0.0, 0.0, 0.0];
        assert_eq!(median_filter_positive(&track, 3), vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn median_window_is_forced_odd() {
        // width 4 -> 5; center frame sees two neighbours each side.
        let track = [10.0, 10.0, 100.0, 10.0, 10.0];
        assert!((median_filter_positive(&track, 4)[2] - 10.0).abs() < f32::EPSILON);
    }
}
