//! Frame-level pitch metrics — `mir_eval.melody`, reimplemented in pure Rust.
//!
//! Four numbers, all over the reference's voiced frames at a ±50-cent tolerance
//! (ADR-0011):
//! - **Raw Pitch Accuracy** — fraction of ref-voiced frames whose estimated
//!   pitch is within tolerance.
//! - **Raw Chroma Accuracy** — same, but octave errors are forgiven, isolating
//!   the octave mistakes ADR-0010's interval prior targets.
//! - **Voicing Recall** — fraction of ref-voiced frames the estimate calls voiced.
//! - **Voicing False Alarm** — fraction of ref-*unvoiced* frames the estimate
//!   wrongly calls voiced.
//!
//! ## Alignment contract
//!
//! `mir_eval` resamples the estimate onto the reference time grid before scoring.
//! We factor that step out: [`pitch_metrics`] assumes **the two contours already
//! share a time grid** (equal length, frame `i` of each is the same instant) and
//! scores frame-by-frame. That is exactly `mir_eval`'s identity path (equal time
//! bases skip interpolation), so the numbers match; resampling a producer's hop
//! onto the reference hop is the caller's job (ticket 03).

use crate::reference::RefF0;

/// The four frame-level pitch scores, each in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchMetrics {
    /// Raw Pitch Accuracy: ref-voiced frames pitched within tolerance.
    pub raw_pitch_accuracy: f64,
    /// Raw Chroma Accuracy: like RPA, ignoring octave errors.
    pub raw_chroma_accuracy: f64,
    /// Voicing Recall: ref-voiced frames the estimate also calls voiced.
    pub voicing_recall: f64,
    /// Voicing False Alarm: ref-unvoiced frames the estimate calls voiced.
    pub voicing_false_alarm: f64,
}

/// `mir_eval`'s base frequency for the Hz↔cent map (`melody.hz2cents`). The
/// single home for the convention, shared with the f0 resampler
/// ([`crate::align`]) so the alignment and the metric can't drift apart.
pub const CENT_BASE_HZ: f64 = 10.0;
/// Default pitch tolerance, in cents (a quarter tone).
pub const CENT_TOLERANCE: f64 = 50.0;

/// Frequency in cents relative to [`CENT_BASE_HZ`]; `0` for an absent (0 Hz)
/// pitch, matching `mir_eval.hz2cents` (which leaves zeros in place).
pub fn hz_to_cents(freq_hz: f32) -> f64 {
    if freq_hz == 0.0 {
        0.0
    } else {
        1200.0 * ((freq_hz as f64).abs() / CENT_BASE_HZ).log2()
    }
}

/// Inverse of [`hz_to_cents`] for a nonzero pitch — reconstruct Hz from cents.
pub fn cents_to_hz(cents: f64) -> f32 {
    (CENT_BASE_HZ * 2f64.powf(cents / 1200.0)) as f32
}

/// Score an estimated f0 contour against a reference one, both on the same time
/// grid (see the module's alignment contract). Panics if the lengths differ.
pub fn pitch_metrics(reference: &[RefF0], estimate: &[RefF0]) -> PitchMetrics {
    assert_eq!(
        reference.len(),
        estimate.len(),
        "pitch_metrics needs aligned contours (resample the estimate onto the \
         reference grid first)"
    );

    PitchMetrics {
        raw_pitch_accuracy: raw_pitch_accuracy(reference, estimate, CENT_TOLERANCE, false),
        raw_chroma_accuracy: raw_pitch_accuracy(reference, estimate, CENT_TOLERANCE, true),
        voicing_recall: voicing_recall(reference, estimate),
        voicing_false_alarm: voicing_false_alarm(reference, estimate),
    }
}

/// A frame counts as voiced iff it is flagged voiced *and* carries a real
/// frequency — `mir_eval.freq_to_voicing` forces voicing to 0 where freq is 0.
fn is_voiced(frame: &RefF0) -> bool {
    frame.voiced && frame.frequency_hz != 0.0
}

/// `sum(est_voiced over ref-voiced frames) / sum(ref-voiced)`. `mir_eval` returns
/// 1.0 when the reference has no voiced frames (nothing to recall).
fn voicing_recall(reference: &[RefF0], estimate: &[RefF0]) -> f64 {
    let ref_voiced = reference.iter().filter(|f| is_voiced(f)).count();
    if ref_voiced == 0 {
        return 1.0;
    }
    let hits = reference
        .iter()
        .zip(estimate)
        .filter(|(r, e)| is_voiced(r) && is_voiced(e))
        .count();
    hits as f64 / ref_voiced as f64
}

/// `sum(est_voiced over ref-*unvoiced* frames) / sum(ref-unvoiced)`. `mir_eval`
/// returns 0.0 when the reference has no unvoiced frames (no chance to false-alarm).
fn voicing_false_alarm(reference: &[RefF0], estimate: &[RefF0]) -> f64 {
    let ref_unvoiced = reference.iter().filter(|f| !is_voiced(f)).count();
    if ref_unvoiced == 0 {
        return 0.0;
    }
    let alarms = reference
        .iter()
        .zip(estimate)
        .filter(|(r, e)| !is_voiced(r) && is_voiced(e))
        .count();
    alarms as f64 / ref_unvoiced as f64
}

/// Raw Pitch Accuracy, or — with `chroma` set — Raw Chroma Accuracy.
///
/// Mirrors `mir_eval.melody.raw_pitch_accuracy`: the denominator is the count of
/// ref-voiced frames; the numerator counts ref-voiced frames where **both** the
/// reference and estimate carry a pitch (nonzero cents) and the cent difference
/// is within tolerance. Voicing *estimation* is ignored — an estimate's pitch is
/// judged even on frames it called unvoiced, as long as it reported a frequency.
/// For chroma, the difference is first folded onto the nearest octave.
fn raw_pitch_accuracy(
    reference: &[RefF0],
    estimate: &[RefF0],
    cent_tolerance: f64,
    chroma: bool,
) -> f64 {
    let ref_voiced = reference.iter().filter(|f| is_voiced(f)).count();
    if ref_voiced == 0 {
        return 0.0;
    }

    let correct = reference
        .iter()
        .zip(estimate)
        .filter(|(r, _)| is_voiced(r))
        .filter(|(r, e)| {
            let ref_cent = hz_to_cents(r.frequency_hz);
            let est_cent = hz_to_cents(e.frequency_hz);
            // Both must carry a pitch, else the frame is skipped (mir_eval's
            // `nonzero_freqs` mask).
            if ref_cent == 0.0 || est_cent == 0.0 {
                return false;
            }
            let mut diff = (ref_cent - est_cent).abs();
            if chroma {
                let octave = 1200.0 * (diff / 1200.0 + 0.5).floor();
                diff = (diff - octave).abs();
            }
            diff < cent_tolerance
        })
        .count();

    correct as f64 / ref_voiced as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(time_s: f64, hz: f32) -> RefF0 {
        RefF0 { time_s, frequency_hz: hz, voiced: true }
    }
    fn u(time_s: f64) -> RefF0 {
        RefF0 { time_s, frequency_hz: 0.0, voiced: false }
    }

    #[test]
    fn identical_contours_score_perfectly() {
        let contour = vec![v(0.0, 440.0), u(0.01), v(0.02, 220.0)];
        let m = pitch_metrics(&contour, &contour);
        assert_eq!(m.raw_pitch_accuracy, 1.0);
        assert_eq!(m.raw_chroma_accuracy, 1.0);
        assert_eq!(m.voicing_recall, 1.0);
        assert_eq!(m.voicing_false_alarm, 0.0);
    }

    #[test]
    fn octave_error_misses_pitch_but_hits_chroma() {
        // Estimate is one octave high on both voiced frames: RPA 0, RCA 1.
        let reference = vec![v(0.0, 220.0), v(0.01, 440.0)];
        let estimate = vec![v(0.0, 440.0), v(0.01, 880.0)];
        let m = pitch_metrics(&reference, &estimate);
        assert_eq!(m.raw_pitch_accuracy, 0.0);
        assert_eq!(m.raw_chroma_accuracy, 1.0);
    }

    #[test]
    fn half_the_voiced_frames_within_tolerance() {
        // Frame 0 spot on; frame 1 off by ~78 cents (> 50) -> wrong. RPA = 1/2.
        let reference = vec![v(0.0, 440.0), v(0.01, 440.0)];
        let estimate = vec![v(0.0, 440.0), v(0.01, 460.0)];
        let m = pitch_metrics(&reference, &estimate);
        assert!((m.raw_pitch_accuracy - 0.5).abs() < 1e-12);
    }

    #[test]
    fn tolerance_is_about_a_quarter_tone() {
        // Just inside the ±50-cent window counts; well outside doesn't. (Exactly
        // 50 cents is unrepresentable from f32 Hz, so we don't assert on it —
        // there mir_eval's `<` and our `<` would agree on whatever f32 rounds to.)
        let reference = vec![v(0.0, 440.0)];
        let inside = vec![v(0.0, 440.0 * 2f32.powf(45.0 / 1200.0))]; // +45 cents
        let outside = vec![v(0.0, 440.0 * 2f32.powf(55.0 / 1200.0))]; // +55 cents
        assert_eq!(pitch_metrics(&reference, &inside).raw_pitch_accuracy, 1.0);
        assert_eq!(pitch_metrics(&reference, &outside).raw_pitch_accuracy, 0.0);
    }

    #[test]
    fn voicing_recall_and_false_alarm() {
        // Ref: voiced, voiced, unvoiced, unvoiced.
        // Est: voiced, unvoiced, voiced, unvoiced.
        let reference = vec![v(0.0, 440.0), v(0.01, 440.0), u(0.02), u(0.03)];
        let estimate = vec![v(0.0, 440.0), u(0.01), v(0.02, 440.0), u(0.03)];
        let m = pitch_metrics(&reference, &estimate);
        assert!((m.voicing_recall - 0.5).abs() < 1e-12); // 1 of 2 voiced kept
        assert!((m.voicing_false_alarm - 0.5).abs() < 1e-12); // 1 of 2 unvoiced alarmed
    }

    #[test]
    fn no_voiced_reference_frames() {
        let reference = vec![u(0.0), u(0.01)];
        let estimate = vec![v(0.0, 440.0), u(0.01)];
        let m = pitch_metrics(&reference, &estimate);
        assert_eq!(m.raw_pitch_accuracy, 0.0); // undefined -> 0
        assert_eq!(m.voicing_recall, 1.0); // nothing to recall -> 1
        assert_eq!(m.voicing_false_alarm, 0.5); // 1 of 2 unvoiced alarmed
    }

    #[test]
    #[should_panic(expected = "aligned contours")]
    fn mismatched_lengths_panic() {
        pitch_metrics(&[v(0.0, 440.0)], &[]);
    }
}
