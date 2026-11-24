//! The **annotation JSON** — sona's primary artifact and the core↔UI contract
//! (ADR-0004). The perception core emits it; the web review surface interprets
//! it (grid, quantization, correction, MIDI export).
//!
//! Everything here is *raw perception*, invariant to musical settings: note
//! events with onset/offset in **seconds** (not beats — snapping is a UI
//! concern), per-note confidence (a scalar plus per-dimension sub-scores), and
//! two display aids (the f0 contour and the energy/RMS envelope). The matching
//! TypeScript types live in `web/lib/annotation.ts`; keep the two in sync (no test
//! checks it yet).
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
    /// What produced this annotation (ADR-0024). Optional, so older annotations still load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    /// Bar 1 estimated from the music (utau-edit-cost 05): only with the opt-in (`sona transcribe
    /// --bar-one MIX`, `sona serve` `bar_one=1`); absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar_one: Option<BarOneEstimate>,
}

/// Where bar 1 falls in the music, in stem seconds (`sona_core::bar_one`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BarOneEstimate {
    /// The first bar line at or after 0 s, on a grid line.
    pub time_s: f64,
    /// Share of the song's 8-bar blocks whose harmonic vote agrees, in `0..=1`.
    pub confidence: f64,
    /// `true` when `time_s` is the harmonic bar line; `false` when the harmony was too weak and it is
    /// the first beat.
    pub harmonic: bool,
}

/// Everything needed to answer "what produced this?": the build, the exact input and model files, the
/// settings, and, for `sona serve`, the job that ran it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub sona_version: String,
    /// `git describe` of the build, with `-dirty` for uncommitted changes; `None` outside a checkout.
    pub sona_commit: Option<String>,
    pub created_unix_s: u64,
    pub input: FileFingerprint,
    /// Every setting that changes the notes, by name, in a stable order.
    pub settings: std::collections::BTreeMap<String, String>,
    /// Model files the notes came from (empty for the pure-DSP path).
    #[serde(default)]
    pub models: Vec<FileFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<JobRecord>,
}

/// A file by name, size and SHA-256 (hex), so it can be checked with `sha256sum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    /// What the file is for, e.g. `input`, `game/segmenter`, `separator/lead`.
    pub role: String,
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// A `sona serve` job: the uploaded file, how long each step took, and the external tools it used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub upload: FileFingerprint,
    pub steps: Vec<StepTime>,
    /// Tool name → version, e.g. `demucs` → `4.0.1`; `unknown` when it couldn't be read.
    pub tools: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub models: Vec<FileFingerprint>,
    /// Where the separated stems came from (ADR-0030); absent when the job separated nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stems: Option<StemSource>,
}

/// A job's separated stems, by stem-cache key: the SHA-256 of the upload's hash and every separator
/// version and setting that shapes the stems (ADR-0030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StemSource {
    pub key: String,
    /// `true` when this job took the stems from the cache instead of running the separators.
    pub reused: bool,
    /// The job that ran the separators.
    pub separated_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepTime {
    pub step: String,
    pub seconds: f64,
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
            provenance: None,
            bar_one: None,
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
    fn bar_one_is_absent_from_the_json_unless_estimated() {
        let plain = serde_json::to_value(sample_annotation()).unwrap();
        assert!(plain.get("bar_one").is_none(), "{plain}");
        let with = Annotation { bar_one: Some(BarOneEstimate { time_s: 0.42, confidence: 0.75, harmonic: true }), ..sample_annotation() };
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(json["bar_one"], serde_json::json!({ "time_s": 0.42, "confidence": 0.75, "harmonic": true }));
        assert_eq!(serde_json::from_value::<Annotation>(json).unwrap(), with);
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
