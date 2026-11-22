//! The **annotation JSON** — sona's primary artifact and the core↔UI contract
//! (ADR-0004). The perception core emits it; the web review surface interprets
//! it (grid, quantization, correction, MIDI export).
//!
//! Everything here is *raw perception*, invariant to musical settings: note
//! events with onset/offset in **seconds** (not beats — snapping is a UI
//! concern), per-note confidence (a scalar plus per-dimension sub-scores), and
//! two display aids (the f0 contour and the energy/RMS envelope). A matching
//! TypeScript declaration lives at `web/annotation.d.ts`; keep the two in sync.
//!
//! The contract is versioned via [`SCHEMA_VERSION`]; any breaking change to the
//! field set must bump it so a consumer can reject annotations it can't read.

use serde::{Deserialize, Serialize};

/// Version of the annotation JSON schema. Emitted as [`Annotation::version`];
/// bump on any breaking change to the field set so consumers can gate on it.
pub const SCHEMA_VERSION: u32 = 1;

/// A complete annotation: the perception core's output for one vocal stem.
///
/// Timing is in seconds throughout (raw, unquantized). `bpm` is carried only so
/// the UI can seed its grid — the core never uses it (ADR-0004).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    /// Schema version; see [`SCHEMA_VERSION`].
    pub version: u32,
    /// Audio sample rate of the analysed stem, in Hz.
    pub sample_rate: u32,
    /// User-supplied tempo (single fixed BPM, v1). Passed through for the UI's
    /// grid; the core does no beat tracking.
    pub bpm: f64,
    /// Total stem duration, in seconds.
    pub duration_s: f64,
    /// Which backend/adapter produced this annotation.
    pub producer: Producer,
    /// Detected note events, in onset order.
    pub notes: Vec<Note>,
    /// Per-frame fundamental-frequency contour (display aid).
    pub f0_contour: Vec<F0Frame>,
    /// Per-frame energy/RMS envelope (display aid).
    pub energy_envelope: Vec<EnergyFrame>,
}

/// A single detected note. Onset/offset are raw seconds; the UI hard-snaps them
/// to a grid at interpretation time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// Note onset, in seconds from the start of the stem.
    pub onset_s: f64,
    /// Note offset, in seconds from the start of the stem.
    pub offset_s: f64,
    /// MIDI pitch number (0–127); 69 = A4 = 440 Hz.
    pub midi_pitch: u8,
    /// Combined confidence scalar in `0.0..=1.0`, the weakest-link soft-min of
    /// [`sub_scores`](Note::sub_scores). Drives the review-surface heatmap.
    pub confidence: f32,
    /// The per-dimension scores the scalar is composed from — let the UI show
    /// *why* a note is flagged (e.g. "⚠ uncertain offset").
    pub sub_scores: SubScores,
}

/// Per-dimension confidence sub-scores, each in `0.0..=1.0`. A note is only as
/// trustworthy as its shakiest dimension (weakest-link soft-min → the scalar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubScores {
    /// How steady the f0 was across the note (vibrato-tolerant).
    pub pitch_stability: f32,
    /// Voicing strength vs. separation bleed / noise floor.
    pub voicing: f32,
    /// Sharpness of the onset transient.
    pub onset_clarity: f32,
    /// Steepness of the offset energy drop (sharp = high, slow tail = low).
    pub offset_clarity: f32,
}

/// One frame of the f0 contour (display aid). `voicing` is the per-frame
/// voiced/unvoiced confidence in `0.0..=1.0`; `frequency_hz` is unspecified
/// (typically `0.0`) where a frame is unvoiced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct F0Frame {
    /// Frame time, in seconds from the start of the stem.
    pub time_s: f64,
    /// Estimated fundamental frequency at this frame, in Hz.
    pub frequency_hz: f32,
    /// Voiced/unvoiced confidence at this frame, in `0.0..=1.0`.
    pub voicing: f32,
}

/// One frame of the energy/RMS envelope (display aid).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyFrame {
    /// Frame time, in seconds from the start of the stem.
    pub time_s: f64,
    /// Root-mean-square amplitude at this frame.
    pub rms: f32,
}

/// The backend or adapter that produced an annotation. Serialized in
/// kebab-case (`"pyin"`, `"crepe"`, `"basic-pitch"`, `"game"`) so the wire form matches
/// the names used across the spec and CONTEXT.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Producer {
    /// The default DSP pYIN backend.
    Pyin,
    /// The CREPE neural backend (ONNX via `ort`).
    Crepe,
    /// basic-pitch, joined to the comparison via an adapter.
    BasicPitch,
    /// openvpi GAME note extraction (the optional `game` feature, ADR-0017).
    Game,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but structurally complete annotation used by the round-trip and
    /// fixture tests.
    fn sample_annotation() -> Annotation {
        Annotation {
            version: SCHEMA_VERSION,
            sample_rate: 44_100,
            bpm: 128.0,
            duration_s: 1.5,
            producer: Producer::Pyin,
            notes: vec![Note {
                onset_s: 0.25,
                offset_s: 0.5,
                midi_pitch: 69,
                confidence: 0.82,
                sub_scores: SubScores {
                    pitch_stability: 0.95,
                    voicing: 0.88,
                    onset_clarity: 0.9,
                    offset_clarity: 0.82,
                },
            }],
            f0_contour: vec![
                F0Frame { time_s: 0.25, frequency_hz: 440.0, voicing: 0.9 },
                F0Frame { time_s: 0.26, frequency_hz: 441.2, voicing: 0.88 },
            ],
            energy_envelope: vec![
                EnergyFrame { time_s: 0.25, rms: 0.12 },
                EnergyFrame { time_s: 0.26, rms: 0.14 },
            ],
        }
    }

    #[test]
    fn round_trips_through_json() {
        let original = sample_annotation();
        let json = serde_json::to_string(&original).expect("serialize");
        let decoded: Annotation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, decoded);
    }

    #[test]
    fn producer_serializes_kebab_case() {
        assert_eq!(serde_json::to_string(&Producer::Pyin).unwrap(), "\"pyin\"");
        assert_eq!(serde_json::to_string(&Producer::Crepe).unwrap(), "\"crepe\"");
        assert_eq!(
            serde_json::to_string(&Producer::BasicPitch).unwrap(),
            "\"basic-pitch\""
        );
    }
}
