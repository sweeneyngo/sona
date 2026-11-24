//! The **annotation JSON** — sona's primary artifact and the core↔UI contract
//! (ADR-0004). The perception core emits it; the web review surface interprets
//! it (grid, quantization, correction, and the UST, USTX and MIDI exports).
//!
//! Everything here is *raw perception*, invariant to musical settings: note
//! events with onset/offset in **seconds** (not beats — snapping is a UI
//! concern), per-note confidence (a scalar plus per-dimension sub-scores), and
//! two display aids (the f0 contour and the energy/RMS envelope). The matching
//! TypeScript types live in `web/lib/annotation.ts`. The two are held together by
//! `tests/fixtures/annotation-full.json`: a test here writes it with every optional
//! field, and `web/lib/annotation.test.ts` reads it into the TypeScript types.
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
///
/// From the harmony (ADR-0031) unless `source` names a downbeat tracker (ADR-0036); the harmonic
/// estimate's JSON has no `source` or `harmonic_time_s`, as before.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BarOneEstimate {
    /// The first bar line at or after 0 s, on a grid line.
    pub time_s: f64,
    /// From the harmony: the share of the song's 8-bar blocks whose vote agrees. From a tracker: the
    /// share of its downbeats that voted for this bar line. In `0..=1`.
    pub confidence: f64,
    /// `true` when `time_s` is a bar line the source found; `false` when it fell back to the first beat
    /// (the harmony too weak, or the tracker gave no downbeats).
    pub harmonic: bool,
    /// The downbeat tracker the bar came from; absent for the harmony.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<BarOneSource>,
    /// With a tracker: where the harmony puts bar 1 (ADR-0031's rule), so the app can flag a disagreement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harmonic_time_s: Option<f64>,
}

/// A source of bar 1 other than the harmony.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BarOneSource {
    /// Beat This! (CPJKU), run as a user-installed tool.
    BeatThis,
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
    /// The separation the job asked for: `none`, `vocals` or `lead` (ADR-0032); absent in older records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separation: Option<String>,
    /// Where the separated stems came from (ADR-0030); absent when the job separated nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stems: Option<StemSource>,
    /// The song's lyric as the job was sent it, one sung line per line (lyrics-first 03); absent for a
    /// job sent without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyric: Option<String>,
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
    /// `true` when the job sent no audio but named its upload by hash, which the server took from the
    /// stem cache (ADR-0032).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub by_hash: bool,
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
    /// [`sub_scores`](Note::sub_scores); for the learned cell stage's notes, the
    /// stage's own chance that the note needs no split or merge (ADR-0033).
    /// Drives the review-surface heatmap.
    pub confidence: f32,
    /// The per-dimension scores the scalar is composed from — let the UI show
    /// *why* a note is flagged (e.g. "⚠ uncertain offset").
    pub sub_scores: SubScores,
    /// For the learned cell stage's notes only: why the stage doubts the note (ADR-0034). Absent for
    /// every other producer and in older annotations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_doubt: Option<StageDoubt>,
}

/// What lowers a learned cell stage note's confidence (ADR-0033), split into its two causes, each in
/// log-odds and `>= 0`: how far the note's length and the model's uncertainty over its cells each pull it
/// below a one-cell note the model is sure of. The larger one is the reason the editor gives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StageDoubt {
    /// From the note's length: long notes are where several syllables hide.
    pub length: f32,
    /// From the model's top class probability over the note's cells: unsure cells are where notes get cut.
    pub unsure: f32,
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
                stage_doubt: None,
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
        let with = Annotation { bar_one: Some(BarOneEstimate { time_s: 0.42, confidence: 0.75, harmonic: true, source: None, harmonic_time_s: None }), ..sample_annotation() };
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(json["bar_one"], serde_json::json!({ "time_s": 0.42, "confidence": 0.75, "harmonic": true }));
        assert_eq!(serde_json::from_value::<Annotation>(json).unwrap(), with);
    }

    #[test]
    fn a_tracker_s_bar_one_names_its_source_and_the_harmony_s_bar() {
        let tracked = BarOneEstimate { time_s: 0.9, confidence: 0.97, harmonic: true, source: Some(BarOneSource::BeatThis), harmonic_time_s: Some(0.42) };
        let with = Annotation { bar_one: Some(tracked), ..sample_annotation() };
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(json["bar_one"], serde_json::json!({ "time_s": 0.9, "confidence": 0.97, "harmonic": true, "source": "beat-this", "harmonic_time_s": 0.42 }));
        assert_eq!(serde_json::from_value::<Annotation>(json).unwrap(), with);
    }

    #[test]
    fn stage_doubt_is_absent_from_the_json_unless_the_stage_made_the_note() {
        let plain = serde_json::to_value(sample_annotation()).unwrap();
        assert!(plain["notes"][0].get("stage_doubt").is_none(), "{plain}");
        let mut with = sample_annotation();
        with.notes[0].stage_doubt = Some(StageDoubt { length: 0.75, unsure: 0.5 });
        let json = serde_json::to_value(&with).unwrap();
        assert_eq!(json["notes"][0]["stage_doubt"], serde_json::json!({ "length": 0.75, "unsure": 0.5 }));
        assert_eq!(serde_json::from_value::<Annotation>(json).unwrap(), with);
    }

    /// Every field the contract has, optional ones included.
    fn full_annotation() -> Annotation {
        let file = |role: &str, name: &str, bytes: u64, sha: char| FileFingerprint { role: role.into(), name: name.into(), bytes, sha256: sha.to_string().repeat(64) };
        let settings = [("bpm", "174"), ("engine", "game+cells"), ("grid", "24")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        Annotation {
            producer: Producer::Game,
            bpm: 174.0,
            provenance: Some(Provenance {
                sona_version: "0.1.0".into(),
                sona_commit: Some("v0.1.0-12-gabcdef0".into()),
                created_unix_s: 1_764_000_000,
                input: file("input", "lead.wav", 88_244, 'a'),
                settings,
                models: vec![file("game/encoder", "encoder.onnx", 1_024, 'b')],
                job: Some(JobRecord {
                    id: "j1".into(),
                    upload: file("upload", "song.mp3", 4_096, 'c'),
                    steps: vec![StepTime { step: "transcribing".into(), seconds: 1.5 }],
                    tools: [("demucs".to_string(), "4.0.1".to_string())].into(),
                    models: vec![file("separator/lead", "UVR.onnx", 2_048, 'd')],
                    separation: Some("lead".into()),
                    stems: Some(StemSource { key: "e".repeat(64), reused: true, separated_by: "j0".into(), by_hash: true }),
                    lyric: Some("あめが ふる\nかぜが ふく".into()),
                }),
            }),
            bar_one: Some(BarOneEstimate { time_s: 0.42, confidence: 0.75, harmonic: true, source: Some(BarOneSource::BeatThis), harmonic_time_s: Some(0.17) }),
            notes: sample_annotation().notes.into_iter().map(|n| Note { stage_doubt: Some(StageDoubt { length: 0.75, unsure: 0.5 }), ..n }).collect(),
            ..sample_annotation()
        }
    }

    /// The contract fixture the web reads (`web/lib/annotation.test.ts`); `UPDATE_GOLDEN=1` rewrites it.
    #[test]
    fn full_annotation_matches_the_contract_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/annotation-full.json");
        let written = serde_json::to_string_pretty(&full_annotation()).unwrap() + "\n";
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, &written).unwrap();
        }
        let fixture = std::fs::read_to_string(&path).expect("the contract fixture");
        assert_eq!(fixture, written, "rewrite it with UPDATE_GOLDEN=1, then run the web's annotation test");
        assert_eq!(serde_json::from_str::<Annotation>(&fixture).unwrap(), full_annotation());
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
