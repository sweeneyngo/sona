//! Tempo estimation against the UST corpus's exact tempos (ADR-0022).
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-tempo            # the dev songs
//! cargo run --release -p sona-eval --bin sona-eval-tempo -- --test --reason "…" [--reread]
//! ```
//! `--test` reads every held-out split (logged in the holdout ledger). Audio is the full mix,
//! `data/pifuyuu-audio/<id>.wav`. A hit is within 2% of the UST tempo.
use std::process::ExitCode;
use sona_core::tempo::estimate_bpm;
use sona_eval::holdout::authorize;
use sona_eval::ust_corpus::{manifest_path, Manifest};
use sona_eval::wav;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() / b <= 0.02
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let test = args.iter().any(|a| a == "--test");
    let reread = args.iter().any(|a| a == "--reread");
    let reason = args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).map(String::as_str);
    let manifest = match Manifest::load(&manifest_path()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let splits: Vec<String> = if test {
        let mut s: Vec<String> = manifest.songs.iter().filter_map(|s| s.split.clone()).filter(|s| s != "dev").collect();
        s.sort();
        s.dedup();
        s
    } else {
        vec!["dev".into()]
    };
    if test {
        for split in &splits {
            if let Err(e) = authorize(&manifest, split, "sona-eval-tempo", reason, reread) {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let (mut n, mut exact, mut octave, mut half) = (0, 0, 0, 0);
    for song in manifest.songs.iter().filter(|s| s.split.as_ref().is_some_and(|x| splits.contains(x))) {
        let Some(truth) = song.bpm_from_ust else { continue };
        let (x, sr) = match wav::read_mono(&audio.join(format!("{}.wav", song.id))) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}: {e}", song.id);
                return ExitCode::FAILURE;
            }
        };
        let got = estimate_bpm(&x, sr).unwrap_or(f64::NAN);
        let verdict = if near(got, truth) {
            "exact"
        } else if near(got / 2.0, truth) {
            "double"
        } else if near(got * 2.0, truth) {
            "HALF"
        } else {
            "other"
        };
        n += 1;
        exact += usize::from(verdict == "exact");
        octave += usize::from(verdict != "other");
        half += usize::from(verdict == "HALF");
        println!("{:28} {:>8} UST {truth:6.1}  est {got:6.1}  {verdict}", song.id, song.split.as_deref().unwrap_or(""));
    }
    let pct = |k: usize| 100.0 * k as f64 / n.max(1) as f64;
    println!("\n{n} songs: exact {exact} ({:.1}%) · within an octave {octave} ({:.1}%) · half-tempo errors {half}", pct(exact), pct(octave));
    ExitCode::SUCCESS
}
