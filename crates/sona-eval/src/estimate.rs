//! The **estimate** — a producer's transcription of one stem, in the shape the
//! metrics score against a [`Reference`]. It mirrors [`Reference`] but carries
//! the two things ground truth doesn't:
//!
//! - a **per-note confidence** scalar (`0.0..=1.0`), because "does the flag
//!   predict the wrong notes?" is a first-class metric (ADR-0011); and
//! - f0 in the same [`RefF0`] frame shape, so the pitch metrics compare like
//!   with like.
//!
//! [`Reference`]: crate::reference::Reference

use crate::reference::RefF0;

/// A producer's transcription of one stem, carrying each scored stage's output:
/// the **stage-2 onset times**, the **stage-3 notes** (with confidence), and the
/// **f0 contour**.
///
/// The onsets are kept separately from the notes on purpose. Scoring detected
/// onsets (stage 2) against reference note onsets, *independently* of the notes
/// segmentation produced (stage 3), is what lets the harness attribute a missing
/// note to the stage that owns it — the standing question ADR-0011 exists to
/// answer. Deriving onsets from the note list instead would collapse the two
/// stages into one number.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    /// Detected onset times (stage 2), in seconds — the raw onset detector's
    /// output, *not* the notes' onsets. Empty means "no stage-2 signal supplied".
    pub onsets: Vec<f64>,
    /// Estimated note events (stage 3), in onset order.
    pub notes: Vec<EstNote>,
    /// Estimated frame-level f0 contour (same shape as the reference's).
    pub f0: Vec<RefF0>,
}

impl Estimate {
    /// Adapt a pipeline [`Annotation`] into a scorable estimate.
    ///
    /// Notes carry through with their combined confidence scalar. For the f0
    /// contour, a frame is taken as **voiced iff it carries a frequency**
    /// (`frequency_hz > 0`) — that is exactly how the tracker encodes its own
    /// voicing decision (pYIN emits `0.0` Hz for frames it called unvoiced, per
    /// the [`F0Frame`] contract), so no separate voicing threshold is imposed
    /// here. The frames stay on the producer's time grid; align them to a
    /// reference grid with [`crate::align::resample_f0_onto`] before scoring
    /// pitch.
    ///
    /// **[`onsets`](Estimate::onsets) is left empty**: the annotation contract
    /// (ADR-0004) carries notes and f0 for the UI, not the raw stage-2 onsets, so
    /// they can't be recovered here. A caller that wants the onset-stage metric
    /// must set [`onsets`](Estimate::onsets) from the onset detector directly.
    ///
    /// [`Annotation`]: sona_core::Annotation
    /// [`F0Frame`]: sona_core::F0Frame
    pub fn from_annotation(annotation: &sona_core::Annotation) -> Self {
        Estimate {
            onsets: Vec::new(),
            notes: annotation
                .notes
                .iter()
                .map(|n| EstNote {
                    onset_s: n.onset_s,
                    offset_s: n.offset_s,
                    midi_pitch: n.midi_pitch,
                    confidence: n.confidence,
                })
                .collect(),
            f0: annotation
                .f0_contour
                .iter()
                .map(|f| RefF0 {
                    time_s: f.time_s,
                    frequency_hz: f.frequency_hz,
                    voiced: f.frequency_hz > 0.0,
                })
                .collect(),
        }
    }
}

/// An estimated note: onset/offset in seconds, integer MIDI pitch, and the
/// producer's confidence scalar. The confidence is what the calibration metric
/// buckets on — it is the only field a [`RefNote`] lacks.
///
/// [`RefNote`]: crate::reference::RefNote
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EstNote {
    /// Onset, in seconds from the start of the stem.
    pub onset_s: f64,
    /// Offset, in seconds from the start of the stem.
    pub offset_s: f64,
    /// MIDI pitch number (0–127); 69 = A4 = 440 Hz.
    pub midi_pitch: u8,
    /// Combined confidence scalar in `0.0..=1.0` (the pipeline's soft-min).
    pub confidence: f32,
}
