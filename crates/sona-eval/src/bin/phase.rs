//! Grid-phase estimation against the UST corpus's alignment (ADR-0027): the error of each estimator as
//! a fraction of a grid cell, circular, with the UST's exact tempo.
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-phase -- [--sona] [--split NAME --reason "…" [--reread]]
//! ```
//! `--sona` also scores the phase of sona's grid notes on the vocals stem (slow: it runs pYIN).
use std::process::ExitCode;
use sona_core::tempo::grid_phase;
use sona_core::{GridMode, PYin, Producer, Transcriber};
use sona_eval::holdout::authorize;
use sona_eval::quantize::grid_spacing_seconds;
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest, SongUsts};
use sona_eval::{ust, wav};

/// Signed circular difference in cells, in (-0.5, 0.5].
fn cells_off(est: f64, truth: f64, spacing: f64) -> f64 {
    let d = (est - truth).rem_euclid(spacing) / spacing;
    if d > 0.5 {
        d - 1.0
    } else {
        d
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let with_sona = args.iter().any(|a| a == "--sona");
    let split = flag("--split").unwrap_or_else(|| "dev".into());
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let splits: Vec<String> = if split == "test" {
        let mut s: Vec<String> = manifest.songs.iter().filter_map(|s| s.split.clone()).filter(|s| s != "dev").collect();
        s.sort();
        s.dedup();
        s
    } else {
        vec![split.clone()]
    };
    if split != "dev" {
        for sp in &splits {
            authorize(&manifest, sp, "sona-eval-phase", flag("--reason").as_deref(), args.iter().any(|a| a == "--reread"))?;
        }
    }
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let mut errs: Vec<Vec<f64>> = vec![Vec::new(); 3];
    println!("{:28} {:>8} {:>8} {:>8}   (cells off the UST grid)", "song", "mix", "vocals", "sona");
    for song in manifest.songs.iter().filter(|s| s.split.as_ref().is_some_and(|x| splits.contains(x))) {
        let (Some(offset), Some(bpm)) = (song.alignment_offset_s, song.bpm_from_ust) else { continue };
        if !song.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) {
            continue;
        }
        if std::env::var("SONA_ONLY").is_ok_and(|only| !only.split(',').any(|o| o == song.id)) {
            continue;
        }
        let division = song.grid_division.unwrap_or(16);
        let spacing = grid_spacing_seconds(bpm, f64::from(division));
        let truth = offset.rem_euclid(spacing);
        let mut row = [f64::NAN; 3];
        if let Ok((mix, sr)) = wav::read_mono(&audio.join(format!("{}.wav", song.id))) {
            row[0] = grid_phase(&mix, sr, spacing).map_or(f64::NAN, |p| cells_off(p, truth, spacing));
        }
        if let Ok((vocals, sr)) = wav::read_mono(&stems.join(&song.id).join("vocals.wav")) {
            row[1] = grid_phase(&vocals, sr, spacing).map_or(f64::NAN, |p| cells_off(p, truth, spacing));
            if with_sona {
                let _ = SongUsts::load(song, &cache);
                let t = Transcriber { grid: Some(GridMode { division, ..GridMode::sixteenths() }), ..Transcriber::default() };
                let a = t.transcribe(&PYin::default(), &vocals, sr, bpm, Producer::Pyin);
                let notes: Vec<(f64, f64, i32)> = a.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
                if !notes.is_empty() {
                    row[2] = cells_off(sona_core_phase(&notes, spacing), truth, spacing);
                }
            }
        }
        println!("{:28} {:>+8.3} {:>+8.3} {:>+8.3}", song.id, row[0], row[1], row[2]);
        for (e, v) in errs.iter_mut().zip(row) {
            if v.is_finite() {
                e.push(v);
            }
        }
    }
    let summary = |e: &[f64]| -> String {
        if e.is_empty() {
            return "—".into();
        }
        let mut a: Vec<f64> = e.iter().map(|v| v.abs()).collect();
        a.sort_by(f64::total_cmp);
        let within = a.iter().filter(|&&v| v <= 0.1).count();
        let mut s: Vec<f64> = e.to_vec();
        s.sort_by(f64::total_cmp);
        format!("median |err| {:.3} · signed median {:+.3} · within 0.1 cell {}/{}", a[a.len() / 2], s[s.len() / 2], within, a.len())
    };
    for (name, e) in ["mix", "vocals", "sona"].iter().zip(&errs) {
        println!("{name:7} {}", summary(e));
    }
    Ok(())
}

/// The phase of sona's grid notes (the cell stage's estimate before ADR-0027).
fn sona_core_phase(notes: &[(f64, f64, i32)], spacing: f64) -> f64 {
    let first = notes[0].0.rem_euclid(spacing);
    if spacing - first < 1e-6 * spacing {
        0.0
    } else {
        first
    }
}
