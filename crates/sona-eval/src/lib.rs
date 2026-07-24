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
//! **Scope of this scaffold (ticket 01):** load data and fail loudly when the
//! corpus is absent. The metrics (RPA/RCA, onset-F1, note-F1, confidence
//! calibration) land in ticket 02.

pub mod error;
pub mod fixtures;
pub mod reference;
pub mod synth;
pub mod vocadito;
pub mod wav;

pub use error::EvalError;
pub use fixtures::Fixture;
pub use reference::{RefF0, RefNote, Reference};
pub use vocadito::Corpus;
