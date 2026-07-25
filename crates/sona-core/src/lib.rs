//! # sona-core
//!
//! The headless **perception** core (ADR-0004): a vocal stem (+ BPM) becomes an
//! *annotation JSON* of raw note events — onset/offset in seconds, pitch,
//! per-note confidence (scalar + sub-scores), f0 contour, and energy envelope.
//! The core is invariant to musical settings; *interpretation* (grid anchoring,
//! quantization, correction, MIDI export) is the web review surface's job.
//!
//! The stages, in pipeline order: the [`annotation`] contract (ticket 02) that
//! every stage feeds into, the [`pitch`] stage (ticket 03): the [`PitchTracker`]
//! seam (ADR-0001) and its pYIN default, the [`onset`] stage (ticket 04):
//! energy-novelty onset detection + the RMS envelope, the [`segment`] stage
//! (ticket 05): hybrid offsets + pitch-plateau splitting into note events, and
//! the [`confidence`] stage (ticket 06): the four sub-scores + weakest-link
//! soft-min that turn a segment into an annotation [`Note`]. The [`pipeline`]
//! stage (ticket 07) wires them end to end — a stem (+ BPM) becomes an
//! [`Annotation`] — and [`midi`] (ticket 07) is the optional raw, unquantized
//! `.mid` convenience export derived from it.

pub mod annotation;
pub mod confidence;
pub mod hmm;
pub mod midi;
pub mod onset;
pub mod pipeline;
pub mod pitch;
pub mod prefilter;
pub mod segment;

pub use annotation::{Annotation, EnergyFrame, F0Frame, Note, Producer, SubScores, SCHEMA_VERSION};
pub use confidence::{soft_min, ConfidenceScorer};
pub use midi::{to_smf, write_midi};
pub use onset::{Onset, OnsetDetector, OnsetResult};
pub use pipeline::Transcriber;
pub use pitch::{PYin, PitchTracker};
pub use prefilter::{PitchFrame, PitchPreFilter};
pub use hmm::NoteHmm;
pub use segment::{hz_to_midi, NoteSegment, OffsetInfo, OffsetKind, Segmenter};

/// The crate version, surfaced by the CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// CREPE (neural) pitch backend, ONNX via `ort`. Compiled only with the
/// `crepe` feature; the default pYIN backend needs no ONNX runtime (ADR-0001).
///
/// Empty placeholder — the `crepe` feature already pulls `ort` into the build.
/// The real ONNX-backed `PitchTracker` implementation lands in ticket 11.
#[cfg(feature = "crepe")]
pub mod crepe {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_populated() {
        assert!(!VERSION.is_empty());
    }
}
