//! # sona-core
//!
//! The headless **perception** core (ADR-0004): a vocal stem (+ BPM) becomes an
//! *annotation JSON* of raw note events — onset/offset in seconds, pitch,
//! per-note confidence (scalar + sub-scores), f0 contour, and energy envelope.
//! The core is invariant to musical settings; *interpretation* (grid anchoring,
//! quantization, correction, MIDI export) is the web review surface's job.
//!
//! This crate is currently a scaffold (ticket 01). The pipeline modules —
//! pitch tracking, onset detection, segmentation, confidence, emit — land in
//! subsequent tickets. The [`PitchTracker`] seam (ADR-0001) is the first of
//! those and is intentionally not defined here yet.

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
