//! **Per-stage metrics** (ADR-0011, ticket 02) — the numbers that say *which
//! stage* owns an error, reimplemented in pure Rust so results compare to the
//! `mir_eval` literature.
//!
//! One submodule per metric family:
//! - [`pitch`] — frame-level RPA / RCA / voicing (`mir_eval.melody`).
//! - [`onset`] — onset P/R/F1 at ±50 ms (`mir_eval.onset`).
//! - [`note`] — note P/R/F1 as COnP / COnPOff (`mir_eval.transcription`).
//! - [`confidence`] — does the per-note confidence flag predict wrong notes?
//!
//! [`matching`] is the maximum-bipartite-matching core shared by onset and note
//! scoring. [`evaluate`] runs every family for one stem and returns a
//! [`StageReport`]; [`StageReport::format`] renders the table the report binary
//! prints.
//!
//! ### Validated against `mir_eval`
//!
//! The formulas were cross-checked once, offline, against `mir_eval` 0.8.2 — see
//! ticket 02's comments in `.scratch/note-hmm-transcription/issues/02-metrics.md`
//! for the agreement. The committed unit tests are hand-worked small cases; they
//! don't require Python.

pub mod confidence;
pub mod matching;
pub mod note;
pub mod onset;
pub mod pitch;

use crate::estimate::Estimate;
use crate::reference::Reference;

pub use confidence::{ConfidenceBucket, ConfidenceCalibration};
pub use note::NoteParams;
pub use pitch::PitchMetrics;

/// Precision / Recall / F1 — the shape both onset and note scoring return.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prf1 {
    /// Precision: matched / estimated.
    pub precision: f64,
    /// Recall: matched / reference.
    pub recall: f64,
    /// F1: harmonic mean of precision and recall.
    pub f1: f64,
}

impl Prf1 {
    /// Build P/R/F1 from a match count and the two population sizes, following
    /// `mir_eval.util.f_measure` (F1 is 0 when both P and R are 0).
    pub fn from_counts(matched: usize, n_reference: usize, n_estimate: usize) -> Self {
        let precision = if n_estimate == 0 { 0.0 } else { matched as f64 / n_estimate as f64 };
        let recall = if n_reference == 0 { 0.0 } else { matched as f64 / n_reference as f64 };
        let f1 = if precision == 0.0 && recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        Prf1 { precision, recall, f1 }
    }
}

/// Tolerances and knobs for a full [`evaluate`] pass. Defaults are `mir_eval`'s
/// (ADR-0011); a caller sweeping presets can vary them.
#[derive(Debug, Clone, Copy)]
pub struct MetricParams {
    /// Onset-detection tolerance, in seconds.
    pub onset_window_s: f64,
    /// Note onset/pitch/offset tolerances.
    pub note: NoteParams,
    /// Number of equal-width confidence buckets for calibration.
    pub confidence_buckets: usize,
}

impl Default for MetricParams {
    fn default() -> Self {
        MetricParams {
            onset_window_s: onset::ONSET_WINDOW_S,
            note: NoteParams::default(),
            confidence_buckets: 5,
        }
    }
}

/// Every per-stage metric for one stem.
#[derive(Debug, Clone)]
pub struct StageReport {
    /// Frame-level pitch metrics.
    pub pitch: PitchMetrics,
    /// Onset detection P/R/F1.
    pub onset: Prf1,
    /// Note P/R/F1, COnP (onset + pitch).
    pub note_conp: Prf1,
    /// Note P/R/F1, COnPOff (onset + pitch + offset).
    pub note_conpoff: Prf1,
    /// Confidence calibration table.
    pub confidence: ConfidenceCalibration,
}

/// Score an [`Estimate`] against a [`Reference`], every stage at once.
///
/// Onset times are taken from the two note lists' onsets — the onset stage feeds
/// segmentation, so a note's onset *is* the onset estimate for this purpose. The
/// f0 contours must already share a time grid (see [`pitch`]'s alignment
/// contract); [`pitch::pitch_metrics`] panics otherwise.
pub fn evaluate(reference: &Reference, estimate: &Estimate, params: &MetricParams) -> StageReport {
    let ref_onsets: Vec<f64> = reference.notes.iter().map(|n| n.onset_s).collect();
    let est_onsets: Vec<f64> = estimate.notes.iter().map(|n| n.onset_s).collect();

    StageReport {
        pitch: pitch::pitch_metrics(&reference.f0, &estimate.f0),
        onset: onset::onset_metrics(&ref_onsets, &est_onsets, params.onset_window_s),
        note_conp: note::con_p(&reference.notes, &estimate.notes, &params.note),
        note_conpoff: note::con_p_off(&reference.notes, &estimate.notes, &params.note),
        confidence: confidence::confidence_calibration(
            &reference.notes,
            &estimate.notes,
            &params.note,
            params.confidence_buckets,
        ),
    }
}

impl StageReport {
    /// Render the full per-stage table for one stem, headed by `name`.
    pub fn format(&self, name: &str) -> String {
        use std::fmt::Write;
        let mut s = String::new();
        let p = &self.pitch;

        let _ = writeln!(s, "── {name} ──");
        let _ = writeln!(
            s,
            "  pitch    RPA {:.3}  RCA {:.3}  Vx-Recall {:.3}  Vx-FalseAlarm {:.3}",
            p.raw_pitch_accuracy, p.raw_chroma_accuracy, p.voicing_recall, p.voicing_false_alarm
        );
        let _ = writeln!(
            s,
            "  onsets   P {:.3}  R {:.3}  F1 {:.3}",
            self.onset.precision, self.onset.recall, self.onset.f1
        );
        let _ = writeln!(
            s,
            "  COnP     P {:.3}  R {:.3}  F1 {:.3}",
            self.note_conp.precision, self.note_conp.recall, self.note_conp.f1
        );
        let _ = writeln!(
            s,
            "  COnPOff  P {:.3}  R {:.3}  F1 {:.3}",
            self.note_conpoff.precision, self.note_conpoff.recall, self.note_conpoff.f1
        );
        let _ = writeln!(s, "  confidence calibration (note error rate per bucket):");
        if self.confidence.buckets.is_empty() {
            let _ = writeln!(s, "    (no estimated notes)");
        } else {
            for b in &self.confidence.buckets {
                let _ = writeln!(
                    s,
                    "    [{:.2}–{:.2})  n {:>3}  error {:.3}",
                    b.lo, b.hi, b.count, b.error_rate
                );
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prf1_from_counts_matches_mir_eval_semantics() {
        let p = Prf1::from_counts(1, 2, 2);
        assert!((p.precision - 0.5).abs() < 1e-12);
        assert!((p.recall - 0.5).abs() < 1e-12);
        assert!((p.f1 - 0.5).abs() < 1e-12);

        // Both zero -> F1 defined as 0, not NaN.
        let z = Prf1::from_counts(0, 3, 3);
        assert_eq!(z.f1, 0.0);

        // Empty populations -> zeros, no divide-by-zero.
        assert_eq!(Prf1::from_counts(0, 0, 0).f1, 0.0);
    }
}
