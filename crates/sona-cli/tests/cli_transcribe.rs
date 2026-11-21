//! End to end through the real binary: a WAV stem
//! (+ BPM) produces a complete, valid annotation JSON on stdout, and `--midi`
//! writes a standard MIDI file. A synthesized two-note stem stands in for a real
//! Vocaloid stem (no fixture audio is checked in).

use std::path::{Path, PathBuf};
use std::process::Command;

use sona_core::Annotation;

const SR: u32 = 44_100;

/// Write a mono 16-bit WAV of `samples` to `path`.
fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(v).expect("write sample");
    }
    writer.finalize().expect("finalize wav");
}

/// A vowel-ish tone: fundamental plus two harmonics.
fn tone(freq: f32, dur_s: f32, amp: f32) -> Vec<f32> {
    let n = (dur_s * SR as f32) as usize;
    (0..n)
        .map(|i| {
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / SR as f32;
            amp * (0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin())
        })
        .collect()
}

fn silence(dur_s: f32) -> Vec<f32> {
    vec![0.0f32; (dur_s * SR as f32) as usize]
}

/// A two-note staccato stem (A4 then C5) with clean gaps and a trailing rest.
fn two_note_stem() -> Vec<f32> {
    let mut audio = silence(0.15);
    audio.extend(tone(440.0, 0.3, 0.6));
    audio.extend(silence(0.15));
    audio.extend(tone(523.25, 0.3, 0.6));
    audio.extend(silence(0.15));
    audio
}

fn tmp(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn sona() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sona"))
}

#[test]
fn transcribe_emits_valid_annotation_json() {
    let wav = tmp("two_note.wav");
    write_wav(&wav, &two_note_stem());

    let out = sona()
        .args(["transcribe", wav.to_str().unwrap(), "--bpm", "128"])
        .output()
        .expect("run sona");
    assert!(
        out.status.success(),
        "exit failure: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let annotation: Annotation =
        serde_json::from_slice(&out.stdout).expect("stdout is valid annotation JSON");
    assert_eq!(annotation.bpm, 128.0);
    assert_eq!(annotation.sample_rate, SR);
    assert_eq!(annotation.producer, sona_core::Producer::Pyin);
    assert_eq!(annotation.notes.len(), 2, "notes: {:?}", annotation.notes);
    assert!((annotation.notes[0].midi_pitch as i32 - 69).abs() <= 1);
    assert!((annotation.notes[1].midi_pitch as i32 - 72).abs() <= 1);
    assert!(!annotation.f0_contour.is_empty());
    assert!(!annotation.energy_envelope.is_empty());
}

#[test]
fn midi_flag_writes_a_standard_midi_file() {
    let wav = tmp("midi_stem.wav");
    write_wav(&wav, &two_note_stem());
    let midi = tmp("out.mid");

    let out = sona()
        .args([
            "transcribe",
            wav.to_str().unwrap(),
            "--bpm",
            "120",
            "--midi",
            midi.to_str().unwrap(),
        ])
        .output()
        .expect("run sona");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bytes = std::fs::read(&midi).expect("midi file written");
    assert!(bytes.starts_with(b"MThd"), "standard MIDI header chunk");
    // stdout is still pure JSON even with --midi.
    serde_json::from_slice::<Annotation>(&out.stdout).expect("stdout still JSON");
}

#[test]
fn missing_bpm_is_a_usage_error() {
    let wav = tmp("no_bpm.wav");
    write_wav(&wav, &silence(0.2));

    let out = sona()
        .args(["transcribe", wav.to_str().unwrap()])
        .output()
        .expect("run sona");
    assert!(!out.status.success(), "missing --bpm should fail");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--bpm"),
        "error should mention --bpm"
    );
}

#[test]
fn missing_stem_file_is_an_error() {
    let out = sona()
        .args(["transcribe", "/no/such/stem.wav", "--bpm", "120"])
        .output()
        .expect("run sona");
    assert!(!out.status.success(), "a missing stem should fail");
}

#[test]
fn help_and_version_succeed() {
    for flag in ["--help", "--version"] {
        let out = sona().arg(flag).output().expect("run sona");
        assert!(out.status.success(), "{flag} should exit 0");
        assert!(!out.stdout.is_empty(), "{flag} should print something");
    }
}

#[test]
fn grid_flag_decodes_notes_onto_the_grid() {
    // At 100 BPM a 16th is 150 ms: the stem's notes and gaps are whole cells.
    let wav = tmp("two_note_grid.wav");
    write_wav(&wav, &two_note_stem());
    let out = sona()
        .args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--grid", "16"])
        .output()
        .expect("run sona");
    assert!(out.status.success(), "exit failure: {}", String::from_utf8_lossy(&out.stderr));
    let annotation: Annotation = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(annotation.notes.len(), 2, "notes: {:?}", annotation.notes);
    assert_eq!(annotation.notes[0].midi_pitch, 69);
    assert_eq!(annotation.notes[1].midi_pitch, 72);
    // Every boundary sits on one grid: whole cells apart from the first onset.
    let cell = 0.15;
    let first = annotation.notes[0].onset_s;
    for n in &annotation.notes {
        for t in [n.onset_s, n.offset_s] {
            let k = (t - first) / cell;
            assert!((k - k.round()).abs() < 1e-6, "{t} is off the grid anchored at {first}");
        }
    }
}

#[test]
fn grid_flag_rejects_an_unsupported_division() {
    let wav = tmp("bad_grid.wav");
    write_wav(&wav, &two_note_stem());
    let out = sona()
        .args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--grid", "12"])
        .output()
        .expect("run sona");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--grid"));
}
