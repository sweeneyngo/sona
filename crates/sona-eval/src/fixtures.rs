//! **Golden fixtures** — a handful of short, committed clips with hand-designed
//! ground truth (ADR-0011). Unlike the Vocadito corpus (large, git-ignored,
//! fetched), these live in the repo and load in ordinary `cargo test`, so they
//! are the fast, offline regression signal.
//!
//! Each fixture is a `<name>.wav` beside a `<name>.ref.json` ([`Reference`]).
//! Both are generated from one [`crate::synth::FixtureSpec`] by the
//! `sona-eval-gen-fixtures` binary, so they stay consistent.

use std::path::{Path, PathBuf};

use crate::error::EvalError;
use crate::reference::Reference;
use crate::synth::fixture_specs;
use crate::wav;

/// A loaded fixture: its audio and ground-truth reference.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// Fixture name (file stem).
    pub name: String,
    /// Mono samples in `-1.0..=1.0`.
    pub samples: Vec<f32>,
    /// Audio sample rate, in Hz.
    pub sample_rate: u32,
    /// Hand-designed ground truth.
    pub reference: Reference,
}

impl Fixture {
    /// Stem duration, in seconds.
    pub fn duration_s(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate.max(1) as f64
    }
}

/// The directory holding the committed fixtures (`crates/sona-eval/fixtures`).
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The names of the committed fixtures, in a stable order.
pub fn fixture_names() -> Vec<String> {
    fixture_specs().iter().map(|s| s.name.to_string()).collect()
}

/// Load one fixture by name from [`fixtures_dir`].
pub fn load(name: &str) -> Result<Fixture, EvalError> {
    load_from(&fixtures_dir(), name)
}

/// Load every committed fixture, in [`fixture_names`] order.
pub fn load_all() -> Result<Vec<Fixture>, EvalError> {
    fixture_names().iter().map(|n| load(n)).collect()
}

/// Load a fixture by name from an explicit directory (used by tests).
pub fn load_from(dir: &Path, name: &str) -> Result<Fixture, EvalError> {
    let wav_path = dir.join(format!("{name}.wav"));
    let ref_path = dir.join(format!("{name}.ref.json"));

    let (samples, sample_rate) = wav::read_mono(&wav_path)?;
    let json = std::fs::read_to_string(&ref_path).map_err(|e| EvalError::io(&ref_path, e))?;
    let reference: Reference = serde_json::from_str(&json)
        .map_err(|e| EvalError::parse(&ref_path, e.to_string()))?;

    Ok(Fixture { name: name.to_string(), samples, sample_rate, reference })
}

#[cfg(test)]
mod tests {
    use super::*;

    // These load the *committed* fixtures, so they run in normal `cargo test`
    // (offline, fast) and would fail if the generator output drifted or wasn't
    // regenerated.

    #[test]
    fn all_committed_fixtures_load() {
        let fixtures = load_all().expect("committed fixtures load");
        assert_eq!(fixtures.len(), fixture_names().len());
        for f in &fixtures {
            assert!(!f.samples.is_empty(), "{} has samples", f.name);
            assert!(!f.reference.notes.is_empty(), "{} has reference notes", f.name);
            assert!(!f.reference.f0.is_empty(), "{} has reference f0", f.name);
        }
    }

    #[test]
    fn committed_reference_matches_the_spec() {
        // Guards against a stale regeneration: the committed .ref.json must match
        // what the current spec produces. Compared with a tolerance, not
        // bit-exactly — serde_json's default float parser can round-trip an f64
        // within ~1 ULP, which is meaningless at the harness's ±50 ms / cent
        // tolerances but would break `assert_eq`.
        for spec in fixture_specs() {
            let loaded = load(spec.name).expect("load fixture");
            assert_references_close(&loaded.reference, &spec.reference(), spec.name);
        }
    }

    /// Assert two references match to a tolerance that ignores float round-trip
    /// noise but catches real staleness (wrong counts, pitches, or timings).
    fn assert_references_close(a: &Reference, b: &Reference, ctx: &str) {
        assert_eq!(a.notes.len(), b.notes.len(), "{ctx}: note count is stale — rerun gen-fixtures");
        for (x, y) in a.notes.iter().zip(&b.notes) {
            assert_eq!(x.midi_pitch, y.midi_pitch, "{ctx}: note pitch stale");
            assert!((x.onset_s - y.onset_s).abs() < 1e-6, "{ctx}: note onset stale");
            assert!((x.offset_s - y.offset_s).abs() < 1e-6, "{ctx}: note offset stale");
        }
        assert_eq!(a.f0.len(), b.f0.len(), "{ctx}: f0 length is stale — rerun gen-fixtures");
        for (x, y) in a.f0.iter().zip(&b.f0) {
            assert!((x.time_s - y.time_s).abs() < 1e-6, "{ctx}: f0 time stale");
            assert!((x.frequency_hz - y.frequency_hz).abs() < 1e-2, "{ctx}: f0 freq stale");
            assert_eq!(x.voiced, y.voiced, "{ctx}: f0 voicing stale");
        }
    }
}
