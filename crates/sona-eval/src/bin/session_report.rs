//! Score saved review sessions — the producer's raw output vs. what the human kept.
//!
//!     cargo run -p sona-eval --bin sona-eval-session -- <session.json | dir>
//!
//! For each session it treats the **corrected notes as ground truth** and the
//! **baseline (producer) notes as the estimate**, then reports note-F1
//! (COnP/COnPOff), stage-2 onset-F1 (onsets re-picked from the stored envelope),
//! and the confidence-calibration table — does a low flag actually mark the notes
//! the user fixed? The aggregate pools calibration across every session, the view
//! that matters for tuning the flag.
//!
//! No audio is needed: everything is recomputed from the tracks the session
//! already carries. Sessions without a `baseline` are skipped (nothing to score).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sona_eval::metrics::confidence::{confidence_calibration, ConfidenceCalibration};
use sona_eval::metrics::note::{con_p, con_p_off};
use sona_eval::metrics::onset::{onset_metrics, ONSET_WINDOW_S};
use sona_eval::metrics::{NoteParams, Prf1};
use sona_eval::{EvalError, Session};

/// Equal-width confidence buckets for the calibration table.
const N_BUCKETS: usize = 5;

fn main() -> ExitCode {
    let input = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: sona-eval-session <session.json | dir-of-sessions>");
            return ExitCode::FAILURE;
        }
    };
    match run(&input) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-session: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Running totals for the aggregate view.
#[derive(Default)]
struct Aggregate {
    scored: usize,
    skipped: usize,
    sum_conp_f1: f64,
    sum_conpoff_f1: f64,
    /// Pooled per-bucket (count, errors), summed across sessions.
    bucket_count: Vec<usize>,
    bucket_err: Vec<usize>,
}

impl Aggregate {
    fn add_calibration(&mut self, cal: &ConfidenceCalibration) {
        if self.bucket_count.is_empty() {
            self.bucket_count = vec![0; cal.buckets.len()];
            self.bucket_err = vec![0; cal.buckets.len()];
        }
        for (i, b) in cal.buckets.iter().enumerate() {
            // error_rate * count is an exact integer error count; recover it.
            self.bucket_count[i] += b.count;
            self.bucket_err[i] += (b.error_rate * b.count as f64).round() as usize;
        }
    }
}

fn run(input: &Path) -> Result<(), EvalError> {
    let params = NoteParams::default();
    let mut agg = Aggregate::default();

    println!("== Session scoring: baseline (producer) vs. corrected (human) ==\n");

    for path in session_paths(input)? {
        let session = Session::load(&path)?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

        if !session.has_baseline {
            println!("── {name} ({}) — skipped: no baseline\n", session.producer);
            agg.skipped += 1;
            continue;
        }

        let (reference, estimate) = (&session.reference, &session.estimate);
        let ref_onsets: Vec<f64> = reference.notes.iter().map(|n| n.onset_s).collect();
        let onset = onset_metrics(&ref_onsets, &estimate.onsets, ONSET_WINDOW_S);
        let conp = con_p(&reference.notes, &estimate.notes, &params);
        let conpoff = con_p_off(&reference.notes, &estimate.notes, &params);
        let cal = confidence_calibration(&reference.notes, &estimate.notes, &params, N_BUCKETS);

        println!("── {name} ({}) ──", session.producer);
        println!(
            "  {:>7} notes, {:>3} kept   onsets {}   COnP {}   COnPOff {}",
            estimate.notes.len(),
            reference.notes.len(),
            fmt_prf1(&onset),
            fmt_prf1(&conp),
            fmt_prf1(&conpoff),
        );
        print_calibration(&cal, "  ");
        println!();

        agg.scored += 1;
        agg.sum_conp_f1 += conp.f1;
        agg.sum_conpoff_f1 += conpoff.f1;
        agg.add_calibration(&cal);
    }

    print_aggregate(&agg);
    Ok(())
}

fn print_aggregate(agg: &Aggregate) {
    println!("== Aggregate ==\n");
    println!("  sessions scored: {}   skipped (no baseline): {}", agg.scored, agg.skipped);
    if agg.scored == 0 {
        return;
    }
    let n = agg.scored as f64;
    println!(
        "  mean COnP-F1 {:.3}   mean COnPOff-F1 {:.3}",
        agg.sum_conp_f1 / n,
        agg.sum_conpoff_f1 / n
    );
    println!("\n  pooled confidence calibration (note error rate per bucket):");
    let width = 1.0 / agg.bucket_count.len().max(1) as f64;
    for (i, (&count, &err)) in agg.bucket_count.iter().zip(&agg.bucket_err).enumerate() {
        let rate = if count == 0 { 0.0 } else { err as f64 / count as f64 };
        println!(
            "    [{:.2}–{:.2})  n {:>4}  error {:.3}",
            i as f64 * width,
            (i + 1) as f64 * width,
            count,
            rate
        );
    }
    println!("\n  Well-calibrated: error rate falls as confidence rises.");
}

fn print_calibration(cal: &ConfidenceCalibration, indent: &str) {
    print!("{indent}calibration:");
    for b in &cal.buckets {
        print!("  [{:.1}–{:.1}) n{} e{:.2}", b.lo, b.hi, b.count, b.error_rate);
    }
    println!();
}

fn fmt_prf1(p: &Prf1) -> String {
    format!("P{:.2} R{:.2} F1{:.2}", p.precision, p.recall, p.f1)
}

/// A single session file, or every `*.json` directly under a directory (sorted).
fn session_paths(input: &Path) -> Result<Vec<PathBuf>, EvalError> {
    if !input.is_dir() {
        return Ok(vec![input.to_path_buf()]);
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(input).map_err(|e| EvalError::io(input, e))? {
        let path = entry.map_err(|e| EvalError::io(input, e))?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}
