//! # sona-core
//!
//! The headless **perception** core (ADR-0004): a vocal stem (+ BPM) becomes an
//! *annotation JSON* of raw note events — onset/offset in seconds, pitch,
//! per-note confidence (scalar + sub-scores), f0 contour, and energy envelope.
//! The core is invariant to musical settings; *interpretation* (grid anchoring,
//! quantization, correction, MIDI export) is the web review surface's job.
//!
//! The remaining pipeline modules — segmentation, confidence, emit — land in
//! subsequent tickets. What exists today is the [`annotation`] contract
//! (ticket 02) that every stage feeds into, the [`pitch`] stage (ticket 03):
//! the [`PitchTracker`] seam (ADR-0001) and its pYIN default, and the [`onset`]
//! stage (ticket 04): energy-novelty onset detection + the RMS envelope.

pub mod annotation;
pub mod onset;
pub mod pitch;

pub use annotation::{Annotation, EnergyFrame, F0Frame, Note, Producer, SubScores, SCHEMA_VERSION};
pub use onset::{Onset, OnsetDetector, OnsetResult};
pub use pitch::{PYin, PitchTracker};

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
