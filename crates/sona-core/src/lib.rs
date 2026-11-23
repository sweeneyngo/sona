//! # sona-core
//!
//! The headless **perception** core (ADR-0004): a vocal stem (+ BPM) becomes an
//! *annotation JSON* of raw note events — onset/offset in seconds, pitch,
//! per-note confidence (scalar + sub-scores), f0 contour, and energy envelope.
//! The core is invariant to musical settings; *interpretation* (grid anchoring,
//! quantization, correction, MIDI export) is the web review surface's job.
//!
//! The stages, in pipeline order: the [`annotation`] contract that every stage
//! feeds into, the [`pitch`] stage: the [`PitchTracker`] seam (ADR-0001) and its
//! pYIN default, the [`onset`] stage: energy-novelty onset detection + the RMS
//! envelope, the [`segment`] stage: note segmentation into note events, and the
//! [`confidence`] stage: the four sub-scores + weakest-link soft-min that turn a
//! segment into an annotation [`Note`]. The [`pipeline`] stage wires them end to
//! end — a stem (+ BPM) becomes an [`Annotation`] — and [`midi`] is the optional
//! raw, unquantized `.mid` convenience export derived from it.

pub mod annotation;
pub mod confidence;
#[cfg(feature = "game")]
pub mod game;
pub mod grid;
pub mod hmm;
pub mod midi;
pub mod onnx_seed;
pub mod onset;
pub mod pipeline;
pub mod pitch;
pub mod prefilter;
pub mod segment;
pub mod timbre;
#[cfg(feature = "spectral")]
pub mod tempo;

pub use annotation::{Annotation, EnergyFrame, F0Frame, Note, Producer, SubScores, SCHEMA_VERSION};
pub use confidence::{soft_min, ConfidenceScorer};
pub use midi::{to_smf, write_midi};
pub use onset::{Onset, OnsetDetector, OnsetResult};
pub use pipeline::Transcriber;
pub use pitch::{HarmonicSalience, PYin, PitchCandidate, PitchTracker};
pub use prefilter::{PitchFrame, PitchPreFilter};
pub use grid::{CandidateEvidence, CueModel, GridDecoder, GridMode, GridSpec, SpectralCue};
pub use timbre::TimbreFrames;
pub use hmm::NoteHmm;
pub use segment::{hz_to_midi, NoteSegment, OffsetInfo, OffsetKind, Segmenter};

/// The crate version, surfaced by the CLI.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// CREPE (neural) pitch backend, ONNX via `ort`. Compiled only with the
/// `crepe` feature; the default pYIN backend needs no ONNX runtime (ADR-0001).
///
/// Empty placeholder — the `crepe` feature already pulls `ort` into the build.
#[cfg(feature = "crepe")]
pub mod crepe {}

// The CLI prints VERSION; an empty one is a build error, not a test failure.
const _: () = assert!(!VERSION.is_empty());
