//! The evaluation report harness.
//!
//!     cargo run -p sona-eval --bin sona-eval-report            # fixtures only
//!     cargo run -p sona-eval --bin sona-eval-report -- --corpus # + Vocadito
//!
//! **This scaffold verifies that data loads** (ticket 01). The metrics — RPA/RCA,
//! onset-F1, note-F1 (COnP/COnPOff), confidence calibration — arrive in ticket
//! 02, at which point this grows a per-stage score table. With `--corpus` it
//! loads the Vocadito corpus from `SONA_VOCADITO_DIR` (or the crate-local
//! `data/vocadito`) and **fails loudly** if it isn't there.

use std::process::ExitCode;

use sona_eval::fixtures;
use sona_eval::vocadito;

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");

    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-report: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    println!("== Golden fixtures ==");
    let loaded = fixtures::load_all()?;
    println!("{:<10}  {:>8}  {:>7}  {:>9}", "fixture", "dur (s)", "notes", "f0 frames");
    for f in &loaded {
        println!(
            "{:<10}  {:>8.2}  {:>7}  {:>9}",
            f.name,
            f.duration_s(),
            f.reference.notes.len(),
            f.reference.f0.len()
        );
    }
    println!("{} fixtures loaded.\n", loaded.len());

    if with_corpus {
        let root = vocadito::corpus_dir();
        println!("== Vocadito corpus ({}) ==", root.display());
        let corpus = vocadito::discover(&root)?; // fails loudly if absent
        let mut with_notes = 0;
        let mut with_f0 = 0;
        for track in &corpus.tracks {
            if track.notes_a1.is_some() || track.notes_a2.is_some() {
                with_notes += 1;
            }
            if track.f0.is_some() {
                with_f0 += 1;
            }
        }
        println!(
            "{} tracks discovered ({with_notes} with notes, {with_f0} with f0).",
            corpus.tracks.len()
        );
    } else {
        println!("(run with --corpus to also load Vocadito from SONA_VOCADITO_DIR)");
    }

    println!("\nMetrics land in ticket 02; this scaffold only verifies loading.");
    Ok(())
}
