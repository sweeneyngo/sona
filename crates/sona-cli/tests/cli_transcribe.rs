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
fn the_missing_stem_error_names_every_transcribe_option_as_help_does() {
    let help = String::from_utf8(sona().arg("--help").output().expect("run sona").stdout).unwrap();
    let section = help.split("OPTIONS (transcribe):").nth(1).and_then(|s| s.split("OPTIONS (serve):").next()).expect("a transcribe section");
    let options: Vec<&str> = section
        .lines()
        .filter_map(|l| l.strip_prefix("    --").map(|rest| rest.split([' ', '<']).next().unwrap()))
        .collect();
    let parsed = ["bpm", "midi", "grid", "no-salience", "no-spectral-cue", "game", "no-octave-check", "cell-model", "vocals", "bar-one", "bar-one-tracker", "beat-this", "beat-this-threads", "tool-timeout"];
    assert_eq!(options, parsed, "--help lists the options the parser takes");

    let out = sona().args(["transcribe", "--bpm", "120"]).output().expect("run sona");
    assert!(!out.status.success());
    let err = String::from_utf8(out.stderr).unwrap();
    let usage = err.split("USAGE:\n").nth(1).expect("a usage block").trim_end();
    for option in options {
        assert!(usage.contains(&format!("[--{option}")) || usage.contains(&format!(" --{option} ")), "the usage misses --{option}:\n{usage}");
    }
    assert!(help.contains(usage), "the error's usage is --help's:\n{usage}");
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
    let out = sona().args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--no-salience"]).output().expect("run sona");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--grid"));
}

#[test]
fn the_retired_opt_in_flags_are_unknown_options() {
    let wav = tmp("retired_flags.wav");
    write_wav(&wav, &two_note_stem());
    for flag in ["--salience", "--spectral-cue"] {
        let out = sona().args(["transcribe", wav.to_str().unwrap(), "--bpm", "100", "--grid", "16", flag]).output().expect("run sona");
        assert!(!out.status.success(), "{flag}");
        assert!(String::from_utf8_lossy(&out.stderr).contains(&format!("unknown option '{flag}'")), "{flag}");
    }
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

/// A stand-in for `sona-beat-this`: records its arguments in `<path>.calls`, runs `extra`, then writes
/// `downbeats` as its result to the file after `--out`.
#[cfg(unix)]
fn fake_beat_this(name: &str, downbeats: &[f64], extra: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = tmp(name);
    let list = downbeats.iter().map(f64::to_string).collect::<Vec<_>>().join(", ");
    let body = format!(
        "#!/bin/sh\nset -e\necho \"$@\" >> \"$0.calls\"\n{extra}\nwhile [ \"$1\" != \"--out\" ]; do shift; done\nprintf '{{\"beats\": [], \"downbeats\": [{list}], \"beat_this\": \"1.1.0\", \"checkpoint\": \"final0\", \"checkpoint_path\": null}}' > \"$2\"\n"
    );
    std::fs::write(&path, body).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_file(path.with_extension("calls"));
    path
}

#[cfg(unix)]
#[test]
fn beat_this_s_downbeats_put_bar_one_on_the_beat_most_of_them_fall_on() {
    let (mix, vocals) = song_and_vocals(0.1);
    let (mix_wav, vocals_wav) = (tmp("beat_this_mix.wav"), tmp("beat_this_vocals.wav"));
    write_wav(&mix_wav, &mix);
    write_wav(&vocals_wav, &vocals);
    let mut downbeats: Vec<f64> = (0..5).map(|bar| 1.1 + 2.0 * f64::from(bar) + 0.02).collect();
    downbeats.extend([0.62, 4.58]);
    let tool = fake_beat_this("beat_this_fake", &downbeats, "");
    let args = ["transcribe", vocals_wav.to_str().unwrap(), "--bpm", "120", "--grid", "16", "--bar-one", mix_wav.to_str().unwrap(), "--bar-one-tracker", "beat-this", "--beat-this", tool.to_str().unwrap()];
    let out = sona().args(args).output().expect("run sona");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let with: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let b = &with["bar_one"];
    assert!((b["time_s"].as_f64().unwrap() - 0.1 - 1.0).abs() < 0.005, "the first beat is 0.1 s; 5 of 7 downbeats fall 2 beats after it: {b}");
    assert!((b["confidence"].as_f64().unwrap() - 5.0 / 7.0).abs() < 1e-9, "{b}");
    assert_eq!((b["harmonic"].as_bool(), b["source"].as_str()), (Some(true), Some("beat-this")), "{b}");
    assert!((b["harmonic_time_s"].as_f64().unwrap() - 0.1).abs() < 0.005, "the harmony's bar 1 (here its beat fallback) is kept beside it: {b}");
    let settings = &with["provenance"]["settings"];
    assert_eq!((settings["bar_one"].as_str(), settings["bar_one_source"].as_str(), settings["beat_this"].as_str()), (Some("mix"), Some("beat-this"), Some("1.1.0")));
    assert_eq!(settings["beat_this_threads"], "1");
    let calls = std::fs::read_to_string(tool.with_extension("calls")).unwrap();
    assert!(calls.starts_with("--threads 1 --out ") && calls.trim_end().ends_with(mix_wav.to_str().unwrap()), "one thread by default, on the mix: {calls}");
}

#[cfg(unix)]
#[test]
fn beat_this_says_how_to_install_it_and_is_stopped_at_its_time_limit() {
    let (mix, vocals) = song_and_vocals(0.1);
    let (mix_wav, vocals_wav) = (tmp("beat_this_limit_mix.wav"), tmp("beat_this_limit_vocals.wav"));
    write_wav(&mix_wav, &mix);
    write_wav(&vocals_wav, &vocals);
    let run = |extra: &[&str]| {
        let mut args = vec!["transcribe", vocals_wav.to_str().unwrap(), "--bpm", "120", "--bar-one", mix_wav.to_str().unwrap(), "--bar-one-tracker", "beat-this"];
        args.extend(extra);
        sona().args(&args).env("SONA_LEAD_VENV", "/nonexistent/separator-venv").output().expect("run sona")
    };
    let out = run(&[]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("/nonexistent/separator-venv/bin/sona-beat-this") && err.contains("setup-separators.sh beat-this"), "{err}");

    let hung = fake_beat_this("beat_this_hung", &[1.1], "echo 'loading final0' >&2\nexec sleep 30");
    let t0 = std::time::Instant::now();
    let out = run(&["--beat-this", hung.to_str().unwrap(), "--tool-timeout", "1"]);
    assert!(t0.elapsed() < std::time::Duration::from_secs(10), "the hung tool held sona for {:?}", t0.elapsed());
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("stopped after 1 s") && err.contains("loading final0") && err.contains("--tool-timeout"), "{err}");

    let failing = fake_beat_this("beat_this_failing", &[], "echo 'No module named beat_this' >&2\nexit 1");
    let out = run(&["--beat-this", failing.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("No module named beat_this"), "{}", String::from_utf8_lossy(&out.stderr));

    for (extra, expect) in [(vec!["--bar-one-tracker", "madmom"], "harmonic or beat-this"), (vec!["--beat-this-threads", "0"], "at least 1")] {
        let out = run(&extra);
        assert!(String::from_utf8_lossy(&out.stderr).contains(expect), "{extra:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let out = sona().args(["transcribe", vocals_wav.to_str().unwrap(), "--bpm", "120", "--bar-one-tracker", "beat-this"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("add --bar-one"), "{}", String::from_utf8_lossy(&out.stderr));
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
