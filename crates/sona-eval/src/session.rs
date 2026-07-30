//! Loading a saved **review session** into scorable shape.
//!
//! A session JSON (saved by the web review surface) is a labelled example: the
//! producer's original notes (`baseline`, with confidence + sub-scores), the
//! user's corrected notes (`notes`), and the raw perception tracks the core
//! emitted (`f0_contour`, `energy_envelope`). This module turns one into:
//!
//! - a [`Reference`] from the **corrected** notes — the proxy ground truth; and
//! - an [`Estimate`] from the **baseline** notes, carrying each note's confidence
//!   and stage-2 onsets re-picked from the stored envelope.
//!
//! Scoring the estimate against the reference then answers, on real target
//! material: how good was the producer's raw output, and does its confidence flag
//! track the notes the human actually fixed?
//!
//! The corrected notes are a *proxy* ground truth (a human edits under grid- and
//! key-snap, and misses things), so treat the numbers as a strong signal, not an
//! oracle. A session also has no ground-truth f0 — only the producer's own
//! contour — so [`Session::reference`] carries notes but no f0: the frame-level
//! pitch metrics don't apply, the note-level metrics and calibration do.

use std::path::Path;

use serde::Deserialize;
use sona_core::{EnergyFrame, F0Frame, OnsetDetector, SubScores};

use crate::estimate::{EstNote, Estimate};
use crate::reference::{RefF0, RefNote, Reference};
use crate::EvalError;

/// A saved review session, adapted into a reference + estimate pair for scoring.
#[derive(Debug, Clone)]
pub struct Session {
    /// The backend that produced the annotation.
    pub producer: String,
    /// Whether the session carried a producer `baseline`. When false, the estimate
    /// falls back to the corrected notes — scoring it is meaningless (they are
    /// identical) — so callers should skip such sessions.
    pub has_baseline: bool,
    /// Proxy ground truth: the user's corrected notes (no f0).
    pub reference: Reference,
    /// The producer's baseline output: notes + confidence, with stage-2 onsets
    /// re-picked from the stored envelope.
    pub estimate: Estimate,
    /// The baseline notes' per-dimension sub-scores, aligned index-for-index with
    /// [`estimate`](Self::estimate)'s notes. This is what confidence calibration
    /// re-weights — each note pairs its sub-scores with whether it was correct.
    pub baseline_scores: Vec<SubScores>,
    /// The stored energy/RMS envelope, kept for offline replay of stages 3–4.
    pub energy_envelope: Vec<EnergyFrame>,
    /// The stored f0 contour, kept for offline replay of stages 3–4.
    pub f0_contour: Vec<F0Frame>,
}

impl Session {
    /// Load and adapt a session JSON at `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, EvalError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| EvalError::io(path, e))?;
        let raw: RawSession =
            serde_json::from_str(&text).map_err(|e| EvalError::parse(path, e.to_string()))?;
        Ok(Self::from_raw(raw))
    }

    fn from_raw(raw: RawSession) -> Self {
        let reference = Reference {
            notes: raw.notes.iter().map(RawNote::to_ref).collect(),
            f0: Vec::new(), // a session has no ground-truth f0
        };

        let has_baseline = raw.baseline.is_some();
        let baseline = raw.baseline.unwrap_or_else(|| raw.notes.clone());

        let energy_envelope: Vec<EnergyFrame> = raw
            .energy_envelope
            .iter()
            .map(|e| EnergyFrame { time_s: e.time_s, rms: e.rms as f32 })
            .collect();
        let f0_contour: Vec<F0Frame> = raw
            .f0_contour
            .iter()
            .map(|f| F0Frame {
                time_s: f.time_s,
                frequency_hz: f.frequency_hz as f32,
                voicing: f.voicing as f32,
            })
            .collect();

        // Re-derive the stage-2 onsets from the stored envelope, so the onset
        // metric scores detection rather than the note list. Uses the default
        // detector — the same one the pipeline ships.
        let onsets = OnsetDetector::default()
            .onsets_from_envelope(&energy_envelope)
            .iter()
            .map(|o| o.time_s)
            .collect();

        let estimate = Estimate {
            onsets,
            notes: baseline.iter().map(RawNote::to_est).collect(),
            f0: f0_contour
                .iter()
                .map(|f| RefF0 {
                    time_s: f.time_s,
                    frequency_hz: f.frequency_hz,
                    voiced: f.frequency_hz > 0.0,
                })
                .collect(),
        };

        let baseline_scores = baseline.iter().map(RawNote::to_scores).collect();

        Session {
            producer: raw.producer,
            has_baseline,
            reference,
            estimate,
            baseline_scores,
            energy_envelope,
            f0_contour,
        }
    }
}

/// A session note as stored on the wire. Extra fields (sub-scores, `manual`) are
/// ignored; `confidence` defaults to `1.0` for older baselines that lack it.
#[derive(Debug, Clone, Deserialize)]
struct RawNote {
    onset_s: f64,
    offset_s: f64,
    midi_pitch: f64,
    #[serde(default = "one")]
    confidence: f64,
    #[serde(default)]
    sub_scores: RawSubScores,
}

fn one() -> f64 {
    1.0
}

impl RawNote {
    fn midi(&self) -> u8 {
        self.midi_pitch.round().clamp(0.0, 127.0) as u8
    }
    fn to_ref(&self) -> RefNote {
        RefNote { onset_s: self.onset_s, offset_s: self.offset_s, midi_pitch: self.midi() }
    }
    fn to_est(&self) -> EstNote {
        EstNote {
            onset_s: self.onset_s,
            offset_s: self.offset_s,
            midi_pitch: self.midi(),
            confidence: self.confidence as f32,
        }
    }
    fn to_scores(&self) -> SubScores {
        let s = &self.sub_scores;
        SubScores {
            pitch_stability: s.pitch_stability as f32,
            voicing: s.voicing as f32,
            onset_clarity: s.onset_clarity as f32,
            offset_clarity: s.offset_clarity as f32,
        }
    }
}

/// A note's per-dimension sub-scores as stored on the wire. All default to `0.0`
/// so an older baseline that lacks them parses (those notes just carry no signal).
#[derive(Debug, Clone, Default, Deserialize)]
struct RawSubScores {
    #[serde(default)]
    pitch_stability: f64,
    #[serde(default)]
    voicing: f64,
    #[serde(default)]
    onset_clarity: f64,
    #[serde(default)]
    offset_clarity: f64,
}

#[derive(Debug, Deserialize)]
struct RawF0 {
    time_s: f64,
    #[serde(default)]
    frequency_hz: f64,
    #[serde(default)]
    voicing: f64,
}

#[derive(Debug, Deserialize)]
struct RawEnergy {
    time_s: f64,
    #[serde(default)]
    rms: f64,
}

#[derive(Debug, Deserialize)]
struct RawSession {
    #[serde(default = "unknown_producer")]
    producer: String,
    notes: Vec<RawNote>,
    #[serde(default)]
    baseline: Option<Vec<RawNote>>,
    #[serde(default)]
    f0_contour: Vec<RawF0>,
    #[serde(default)]
    energy_envelope: Vec<RawEnergy>,
}

fn unknown_producer() -> String {
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal session: producer emitted three notes, the user corrected the
    // last one's pitch and deleted the middle one, leaving two corrected notes.
    const SESSION: &str = r#"{
        "producer": "pyin",
        "notes": [
            {"onset_s": 0.0, "offset_s": 0.5, "midi_pitch": 60, "confidence": 0.9},
            {"onset_s": 1.2, "offset_s": 1.6, "midi_pitch": 65, "confidence": 0.4}
        ],
        "baseline": [
            {"onset_s": 0.0, "offset_s": 0.5, "midi_pitch": 60, "confidence": 0.9},
            {"onset_s": 0.6, "offset_s": 1.0, "midi_pitch": 62, "confidence": 0.3},
            {"onset_s": 1.2, "offset_s": 1.6, "midi_pitch": 64, "confidence": 0.4}
        ],
        "f0_contour": [],
        "energy_envelope": []
    }"#;

    #[test]
    fn adapts_corrected_notes_to_reference_and_baseline_to_estimate() {
        let s = Session::load_str(SESSION);
        assert!(s.has_baseline);
        assert_eq!(s.producer, "pyin");
        // Reference = the two corrected notes.
        assert_eq!(s.reference.notes.len(), 2);
        assert!(s.reference.f0.is_empty());
        // Estimate = the three producer notes, confidence carried through.
        assert_eq!(s.estimate.notes.len(), 3);
        assert_eq!(s.estimate.notes[1].midi_pitch, 62);
        assert!((s.estimate.notes[2].confidence - 0.4).abs() < 1e-6);
    }

    #[test]
    fn confidence_defaults_when_absent_and_no_baseline_falls_back() {
        let raw = r#"{"producer":"crepe","notes":[{"onset_s":0.0,"offset_s":0.5,"midi_pitch":69}]}"#;
        let s = Session::load_str(raw);
        assert!(!s.has_baseline);
        // No baseline → estimate mirrors the corrected notes.
        assert_eq!(s.estimate.notes.len(), 1);
        assert!((s.estimate.notes[0].confidence - 1.0).abs() < 1e-6);
    }

    impl Session {
        /// Parse a session straight from a JSON string (test helper).
        fn load_str(json: &str) -> Self {
            Session::from_raw(serde_json::from_str(json).expect("valid session json"))
        }
    }
}
