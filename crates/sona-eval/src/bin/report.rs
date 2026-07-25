//! The evaluation report harness.
//!
//!     cargo run -p sona-eval --bin sona-eval-report            # fixtures only
//!     cargo run -p sona-eval --bin sona-eval-report -- --corpus # + Vocadito
//!
//! It prints a **full per-stage report** — RPA/RCA, onset-F1, note-F1
//! (COnP/COnPOff), and a confidence-calibration table — for each golden fixture
//! (ticket 02). With `--corpus` it also loads the Vocadito corpus from
//! `SONA_VOCADITO_DIR` (or the crate-local `data/vocadito`) and **fails loudly**
//! if it isn't there.
//!
//! Until a real producer is wired in (ticket 03), each fixture is scored against
//! the **identity estimate** (the reference echoed back), so every stage reads a
//! perfect 1.0 — the report proves the metric plumbing renders end to end, not
//! that anything transcribes yet.

use std::process::ExitCode;

use sona_eval::fixtures;
use sona_eval::metrics;
use sona_eval::vocadito;
use sona_eval::Estimate;

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

    println!("== Per-stage metrics (estimate = identity; real producer in ticket 03) ==");
    let params = metrics::MetricParams::default();
    for f in &loaded {
        let estimate = Estimate::from_reference(&f.reference);
        let report = metrics::evaluate(&f.reference, &estimate, &params);
        print!("{}", report.format(&f.name));
    }
    println!();

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

    Ok(())
}
