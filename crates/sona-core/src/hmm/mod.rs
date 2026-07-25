//! **Note-HMM segmentation** — a Tony-style note-model HMM over the pitch
//! contour (ADR-0010), the Phase-B replacement for the ADR-0007 heuristic
//! segmenter. It reads the same three raw-perception tracks the heuristic did —
//! the f0 contour, the energy onsets, and the RMS envelope — and produces the
//! same [`NoteSegment`](crate::segment::NoteSegment)s behind the same
//! [`Segmenter`](crate::segment::Segmenter) seam, so everything downstream
//! (confidence, CLI, annotation JSON) is untouched.
//!
//! The stage is three pieces, one per ticket:
//!
//! 1. [`model`] (ticket 05) — the lattice: pitch states × Attack/Stable/Silent
//!    sub-states, a Gaussian emission, and left-to-right transitions with an
//!    interval prior.
//! 2. [`decode`] (ticket 06) — Viterbi over the lattice, with detected energy
//!    onsets fed in as a per-frame Attack prior so re-articulated same-pitch
//!    notes split.
//! 3. [`notes`] (ticket 07) — the decoded state path becomes `NoteSegment`s:
//!    alpha-trimmed final pitch, short-note pruning, same-pitch merging, and the
//!    offset drop-steepness the confidence stage still reads.

pub mod decode;
pub mod model;
pub mod notes;

pub use decode::{viterbi_path, DecodeInput};
pub use model::{HmmModel, HmmParams, ObsFrame, Sub};
pub use notes::NoteHmm;
