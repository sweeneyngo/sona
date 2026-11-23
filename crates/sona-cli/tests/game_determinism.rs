//! The same stem gives the same GAME notes in every process, even with several running at once (ADR-0023).
//! `ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo test -p sona-cli --features game --test game_determinism -- --ignored`
//! A synthetic melody is clean enough that thread-dependent rounding rarely flips a boundary; set
//! `SONA_DETERMINISM_STEM` to a real 44.1 kHz vocal stem (local, never committed) to test that too.
use std::process::Command;

fn melody_wav(path: &std::path::Path) {
    let sr = 44_100u32;
    let spec = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut phase = 0.0f64;
    for (k, m) in [64.0, 67.0, 69.0, 67.0, 64.0, 62.0, 60.0, 62.0, 64.0, 69.0, 71.0, 72.0].iter().cycle().take(36).enumerate() {
        for i in 0..(0.3 * f64::from(sr)) as usize {
            let t = i as f64 / f64::from(sr);
            let f = 440.0 * 2f64.powf((m - 69.0 + 0.3 * (std::f64::consts::TAU * 5.5 * t).sin()) / 12.0);
            phase += std::f64::consts::TAU * f / f64::from(sr);
            let gap = if k % 4 == 3 && t > 0.22 { 0.0 } else { 1.0 };
            w.write_sample((gap * 0.3 * (phase.sin() + 0.4 * (2.0 * phase).sin()) * f64::from(i16::MAX)) as i16).unwrap();
        }
    }
    w.finalize().unwrap();
}

#[test]
#[ignore = "needs ONNX Runtime and a downloaded GAME release"]
fn the_same_stem_gives_the_same_notes_in_every_process() {
    let dir = std::env::var("SONA_GAME_DIR").expect("SONA_GAME_DIR");
    let synthetic = std::env::temp_dir().join(format!("sona-game-determinism-{}.wav", std::process::id()));
    let wav = match std::env::var_os("SONA_DETERMINISM_STEM") {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            melody_wav(&synthetic);
            synthetic.clone()
        }
    };
    let run = || {
        let out = Command::new(env!("CARGO_BIN_EXE_sona"))
            .args(["transcribe"])
            .arg(&wav)
            .args(["--bpm", "123", "--grid", "16", "--game", &dir])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        v["notes"].as_array().unwrap().iter().map(|n| (n["onset_s"].to_string(), n["offset_s"].to_string(), n["midi_pitch"].to_string())).collect::<Vec<_>>()
    };
    let first = run();
    assert!(first.len() > 5, "{} notes", first.len());
    let runs: Vec<_> = std::thread::scope(|s| (0..3).map(|_| s.spawn(run)).collect::<Vec<_>>().into_iter().map(|h| h.join().unwrap()).collect());
    for r in runs {
        assert_eq!(r, first);
    }
    let _ = std::fs::remove_file(&synthetic);
}
