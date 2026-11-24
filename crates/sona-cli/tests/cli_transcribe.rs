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

#[test]
fn grid_mode_with_refinements_off_still_decodes_the_melody() {
    let wav = tmp("two_note_salience.wav");
    write_wav(&wav, &two_note_stem());
    let out = sona()
        .args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--grid", "16", "--no-salience", "--no-spectral-cue"])
        .output()
        .expect("run sona");
    assert!(out.status.success(), "exit failure: {}", String::from_utf8_lossy(&out.stderr));
    let annotation: Annotation = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let pitches: Vec<u8> = annotation.notes.iter().map(|n| n.midi_pitch).collect();
    assert_eq!(pitches, vec![69, 72], "notes: {:?}", annotation.notes);
}

#[test]
fn grid_refinements_without_grid_are_a_usage_error() {
    let wav = tmp("salience_no_grid.wav");
    write_wav(&wav, &two_note_stem());
    let out = sona().args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--salience"]).output().expect("run sona");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--grid"));
}

#[test]
fn the_cell_stage_needs_game_and_the_vocals_stem() {
    let wav = tmp("cells_args.wav");
    write_wav(&wav, &two_note_stem());
    let w = wav.to_str().unwrap();
    for (extra, expect) in [
        (vec!["--grid", "16", "--cell-model", "/nonexistent"], "--cell-model needs --game"),
        (vec!["--grid", "16", "--game", "/nonexistent", "--cell-model", "/nonexistent"], "--vocals"),
        (vec!["--grid", "16", "--vocals", w], "--vocals is only used with --cell-model"),
    ] {
        let mut args = vec!["transcribe", w, "--bpm", "100"];
        args.extend(extra);
        let out = sona().args(&args).output().expect("run sona");
        assert!(!out.status.success(), "{args:?}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(expect), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

/// 12 s at 120 BPM: a sung tone on every 8th from `first_s`, and a mix with a drum hit on every beat.
fn song_and_vocals(first_s: f32) -> (Vec<f32>, Vec<f32>) {
    let n = 12 * SR as usize;
    let mut vocals = vec![0.0f32; n];
    let mut mix = vec![0.0f32; n];
    let mut seed = 3u32;
    for e in 0..46 {
        let at = ((first_s + e as f32 * 0.25) * SR as f32) as usize;
        for (i, s) in tone(if e % 4 < 2 { 440.0 } else { 523.25 }, 0.2, 0.4).into_iter().enumerate() {
            vocals[at + i] += s * (-(i as f32) / SR as f32 * 12.0).exp();
        }
        if e % 2 == 0 {
            for i in 0..(0.06 * SR as f32) as usize {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
                mix[at + i] += 0.8 * (-(i as f32) / SR as f32 * 60.0).exp() * noise;
            }
        }
    }
    mix.iter_mut().zip(&vocals).for_each(|(m, v)| *m += v);
    (mix, vocals)
}

#[test]
fn bar_one_is_estimated_only_when_asked_for() {
    let (mix, vocals) = song_and_vocals(0.1);
    let (mix_wav, vocals_wav) = (tmp("bar_one_mix.wav"), tmp("bar_one_vocals.wav"));
    write_wav(&mix_wav, &mix);
    write_wav(&vocals_wav, &vocals);
    let v = vocals_wav.to_str().unwrap();
    let run = |extra: &[&str]| -> serde_json::Value {
        let mut args = vec!["transcribe", v, "--bpm", "120", "--grid", "16"];
        args.extend(extra);
        let out = sona().args(&args).output().expect("run sona");
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    let plain = run(&[]);
    assert!(plain.get("bar_one").is_none(), "{}", plain["bar_one"]);
    assert!(plain["provenance"]["settings"].get("bar_one").is_none());
    let with = run(&["--bar-one", mix_wav.to_str().unwrap()]);
    let b = &with["bar_one"];
    let t = b["time_s"].as_f64().unwrap();
    assert!((t - 0.1).abs() < 0.005, "6 bars are too few to vote on the bar, so bar 1 is the first beat: {b}");
    assert_eq!(b["harmonic"], false, "{b}");
    assert_eq!(b["confidence"].as_f64(), Some(0.0), "{b}");
    assert_eq!(with["provenance"]["settings"]["bar_one"], "mix");
    assert_eq!(with["notes"], plain["notes"], "the notes don't depend on bar 1");
}

#[test]
fn bar_one_needs_the_mix_at_the_vocals_rate() {
    let vocals = tmp("bar_one_rate_vocals.wav");
    write_wav(&vocals, &two_note_stem());
    let mix = tmp("bar_one_rate_mix.wav");
    let spec = hound::WavSpec { channels: 1, sample_rate: 22_050, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(&mix, spec).unwrap();
    for _ in 0..22_050 {
        w.write_sample(0i16).unwrap();
    }
    w.finalize().unwrap();
    let out = sona().args(["transcribe", vocals.to_str().unwrap(), "--bpm", "120", "--bar-one", mix.to_str().unwrap()]).output().expect("run sona");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("22050 Hz"), "{}", String::from_utf8_lossy(&out.stderr));
}
