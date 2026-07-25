//! **Confidence calibration** — the metric ADR-0011 calls arguably the most
//! important, and the one raw F1 can't express.
//!
//! sona's thesis (ADR-0004/0008) is "emit every note, flag the uncertain ones."
//! That is only worth anything if the flag *tracks reality*: low-confidence notes
//! should be the wrong ones. So we bucket estimated notes by their confidence
//! scalar and, per bucket, report the **note error rate** — the fraction of that
//! bucket's notes that are *not* a correct (COnP) transcription. Well-calibrated
//! output has error rate falling as confidence rises.
//!
//! "Correct" reuses the COnP matching from [`super::note`]: an estimated note is
//! correct iff the maximum onset+pitch matching pairs it with some reference note.

use super::note::{match_notes, NoteParams};
use crate::estimate::EstNote;
use crate::reference::RefNote;

/// One confidence bucket's tally.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfidenceBucket {
    /// Inclusive lower edge of the confidence range.
    pub lo: f64,
    /// Exclusive upper edge (inclusive for the top bucket, so 1.0 lands).
    pub hi: f64,
    /// Estimated notes whose confidence fell in this bucket.
    pub count: usize,
    /// Fraction of this bucket's notes that were *not* a correct (COnP) note.
    /// `0.0` for an empty bucket.
    pub error_rate: f64,
}

/// The confidence-calibration table: one [`ConfidenceBucket`] per equal-width
/// slice of `[0, 1]`, low confidence first.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfidenceCalibration {
    /// Buckets in ascending confidence order.
    pub buckets: Vec<ConfidenceBucket>,
}

/// Bucket the estimate's notes by confidence into `n_buckets` equal-width bins
/// over `[0, 1]` and report each bin's note error rate against the reference.
///
/// A note is "correct" iff it appears in the COnP (onset + pitch) maximum
/// matching. `n_buckets` is clamped to at least 1.
pub fn confidence_calibration(
    reference: &[RefNote],
    estimate: &[EstNote],
    params: &NoteParams,
    n_buckets: usize,
) -> ConfidenceCalibration {
    let n_buckets = n_buckets.max(1);

    // Which estimated notes are correct under COnP (onset + pitch, no offset).
    let mut correct = vec![false; estimate.len()];
    for (_ref_i, est_i) in match_notes(reference, estimate, params, false) {
        correct[est_i] = true;
    }

    // Tally (count, errors) per bucket.
    let mut counts = vec![0usize; n_buckets];
    let mut errors = vec![0usize; n_buckets];
    for (i, note) in estimate.iter().enumerate() {
        let b = bucket_index(note.confidence, n_buckets);
        counts[b] += 1;
        if !correct[i] {
            errors[b] += 1;
        }
    }

    let width = 1.0 / n_buckets as f64;
    let buckets = (0..n_buckets)
        .map(|b| ConfidenceBucket {
            lo: b as f64 * width,
            hi: (b + 1) as f64 * width,
            count: counts[b],
            error_rate: if counts[b] == 0 {
                0.0
            } else {
                errors[b] as f64 / counts[b] as f64
            },
        })
        .collect();

    ConfidenceCalibration { buckets }
}

/// Bucket a confidence in `[0, 1]` into `[0, n)`. Values are clamped, and the top
/// edge (1.0) falls into the last bucket rather than overflowing.
fn bucket_index(confidence: f32, n_buckets: usize) -> usize {
    let c = confidence.clamp(0.0, 1.0) as f64;
    ((c * n_buckets as f64) as usize).min(n_buckets - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rn(onset: f64, midi: u8) -> RefNote {
        RefNote { onset_s: onset, offset_s: onset + 0.4, midi_pitch: midi }
    }
    fn en(onset: f64, midi: u8, confidence: f32) -> EstNote {
        EstNote { onset_s: onset, offset_s: onset + 0.4, midi_pitch: midi, confidence }
    }

    #[test]
    fn well_calibrated_flag_puts_errors_in_low_buckets() {
        // Two reference notes. Two high-confidence estimates match them; two
        // low-confidence estimates are spurious (wrong pitch, far away).
        let reference = vec![rn(0.0, 60), rn(1.0, 62)];
        let estimate = vec![
            en(0.0, 60, 0.9),  // correct, high conf -> top bucket
            en(1.0, 62, 0.85), // correct, high conf -> top bucket
            en(3.0, 72, 0.1),  // wrong, low conf -> bottom bucket
            en(4.0, 74, 0.15), // wrong, low conf -> bottom bucket
        ];
        let cal = confidence_calibration(&reference, &estimate, &NoteParams::default(), 5);
        assert_eq!(cal.buckets.len(), 5);
        // Bottom bucket [0.0–0.2): both notes, both wrong -> error 1.0.
        assert_eq!(cal.buckets[0].count, 2);
        assert_eq!(cal.buckets[0].error_rate, 1.0);
        // Top bucket [0.8–1.0]: both notes, both correct -> error 0.0.
        assert_eq!(cal.buckets[4].count, 2);
        assert_eq!(cal.buckets[4].error_rate, 0.0);
        // Middle buckets empty.
        for b in &cal.buckets[1..4] {
            assert_eq!(b.count, 0);
            assert_eq!(b.error_rate, 0.0);
        }
    }

    #[test]
    fn confidence_of_one_lands_in_top_bucket() {
        let reference = vec![rn(0.0, 60)];
        let estimate = vec![en(0.0, 60, 1.0)];
        let cal = confidence_calibration(&reference, &estimate, &NoteParams::default(), 4);
        assert_eq!(cal.buckets[3].count, 1);
        assert_eq!(cal.buckets[3].error_rate, 0.0);
    }

    #[test]
    fn edges_and_ranges_are_reported() {
        let cal = confidence_calibration(&[], &[], &NoteParams::default(), 4);
        let edges: Vec<(f64, f64)> = cal.buckets.iter().map(|b| (b.lo, b.hi)).collect();
        assert_eq!(edges, vec![(0.0, 0.25), (0.25, 0.5), (0.5, 0.75), (0.75, 1.0)]);
        for b in &cal.buckets {
            assert_eq!(b.count, 0);
            assert_eq!(b.error_rate, 0.0);
        }
    }

    #[test]
    fn n_buckets_clamped_to_one() {
        let reference = vec![rn(0.0, 60)];
        let estimate = vec![en(0.0, 60, 0.5), en(9.0, 72, 0.5)];
        let cal = confidence_calibration(&reference, &estimate, &NoteParams::default(), 0);
        assert_eq!(cal.buckets.len(), 1);
        assert_eq!(cal.buckets[0].count, 2);
        assert!((cal.buckets[0].error_rate - 0.5).abs() < 1e-12); // one of two wrong
    }
}
