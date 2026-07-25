//! # sona-eval
//!
//! The offline **evaluation harness** (ADR-0011): it measures the perception
//! pipeline against labeled vocal data. It is the *ruler*, not the product —
//! nothing here ships to users, and it lives outside `sona-core` so dataset and
//! metric code never leak into the headless core.
//!
//! Two data sources both load into one common [`reference::Reference`]:
//! - [`fixtures`] — a few short, committed clips with hand-designed ground truth
//!   (the fast, offline regression signal).
//! - [`vocadito`] — the Vocadito corpus (large, git-ignored, fetched by
//!   `scripts/fetch-vocadito.sh`).
//!
//! [`synth`] is the shared source of truth from which the golden fixtures'
//! audio *and* ground truth are generated, and [`wav`] is the fixture WAV I/O.
//!
//! An [`Estimate`] (a producer's transcription) is scored against a
//! [`reference::Reference`] by the per-stage [`metrics`] (RPA/RCA, onset-F1,
//! note-F1 COnP/COnPOff, confidence calibration) — reimplemented in pure Rust
//! and cross-checked against `mir_eval` (ticket 02).

pub mod error;
pub mod estimate;
pub mod fixtures;
pub mod metrics;
pub mod reference;
pub mod synth;
pub mod vocadito;
pub mod wav;

pub use error::EvalError;
pub use estimate::{EstNote, Estimate};
pub use fixtures::Fixture;
pub use metrics::{evaluate, MetricParams, StageReport};
pub use reference::{RefF0, RefNote, Reference};
pub use vocadito::Corpus;
