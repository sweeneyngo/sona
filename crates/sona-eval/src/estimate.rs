//! The **estimate** — a producer's transcription of one stem, in the shape the
//! metrics (ticket 02) score against a [`Reference`]. It mirrors [`Reference`]
//! but carries the two things ground truth doesn't:
//!
//! - a **per-note confidence** scalar (`0.0..=1.0`), because "does the flag
//!   predict the wrong notes?" is a first-class metric (ADR-0011); and
//! - f0 in the same [`RefF0`] frame shape, so the pitch metrics compare like
//!   with like.
//!
//! Ticket 02 defines the type and the metrics that consume it; wiring a real
//! producer (the pipeline) into an `Estimate` is ticket 03's baseline report.
//!
//! [`Reference`]: crate::reference::Reference

use crate::reference::{RefF0, Reference};

/// A producer's transcription of one stem: estimated notes (with confidence)
/// and an estimated f0 contour.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    /// Estimated note events, in onset order.
    pub notes: Vec<EstNote>,
    /// Estimated frame-level f0 contour (same shape as the reference's).
    pub f0: Vec<RefF0>,
}

impl Estimate {
    /// The **identity estimate**: the reference echoed back, every note at full
    /// confidence. A perfect transcription — scoring it yields all-1.0 metrics.
    ///
    /// It exists so the report harness has *something* to score before a real
    /// producer is wired in (ticket 03), and as an upper-bound sanity check that
    /// the metrics reward a perfect answer. It is not a producer.
    pub fn from_reference(reference: &Reference) -> Self {
        Estimate {
            notes: reference
                .notes
                .iter()
                .map(|n| EstNote {
                    onset_s: n.onset_s,
                    offset_s: n.offset_s,
                    midi_pitch: n.midi_pitch,
                    confidence: 1.0,
                })
                .collect(),
            f0: reference.f0.clone(),
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
