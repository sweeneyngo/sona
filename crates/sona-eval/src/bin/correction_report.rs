//! Correction-cost report — the by-ear scoreboard, aggregated per producer.
//!
//!     cargo run -p sona-eval --bin sona-eval-correction -- <dir>
//!
//! Reads every `*.json` session saved from the review surface in `<dir>`, diffs
//! each one's edited notes against its `baseline` (the producer's original
//! output), and prints per-session correction costs plus a per-producer mean.
//! That mean is the practical yardstick from `CONTEXT.md` — "how much manual
//! correction did the output need" — so pYIN, CREPE, and basic-pitch can be
//! compared over the same stems.
//!
//! Files without a `notes` array are skipped with a note (the directory may hold
//! MIDI exports or other JSON); a session without a `baseline` field is treated
//! as unedited (cost 0), matching how the review surface falls back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use sona_eval::correction::{correction_cost, CorrectionCost, EditNote, MatchParams};
use sona_eval::EvalError;

/// One saved session, as far as the correction report cares.
#[derive(Debug, Deserialize)]
struct Session {
    /// The backend that produced the annotation; defaults when absent.
    #[serde(default = "unknown_producer")]
    producer: String,
    /// The user's edited note set.
    notes: Vec<EditNote>,
    /// The producer's original notes. Absent on an unedited annotation.
    #[serde(default)]
    baseline: Option<Vec<EditNote>>,
}

fn unknown_producer() -> String {
    "unknown".to_string()
}

fn main() -> ExitCode {
    let dir = match std::env::args().nth(1) {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("usage: sona-eval-correction <dir-of-session-json>");
            return ExitCode::FAILURE;
        }
    };
    match run(&dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-correction: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A producer's running totals across the sessions it produced.
#[derive(Default)]
struct Tally {
    sessions: usize,
    sum: CostSums,
}

/// Sums of each cost category, so a mean can be taken at the end.
#[derive(Default)]
struct CostSums {
    added: usize,
    deleted: usize,
    pitch_fixed: usize,
    onset_moved: usize,
    offset_moved: usize,
    total: usize,
}

impl CostSums {
    fn add(&mut self, c: &CorrectionCost) {
        self.added += c.added;
        self.deleted += c.deleted;
        self.pitch_fixed += c.pitch_fixed;
        self.onset_moved += c.onset_moved;
        self.offset_moved += c.offset_moved;
        self.total += c.total;
    }
}

fn run(dir: &Path) -> Result<(), EvalError> {
    let params = MatchParams::default();
    // BTreeMap keeps the per-producer summary in stable, sorted order.
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();

    println!("== Correction cost per session ({}) ==\n", dir.display());
    println!(
        "  {:<28}  {:<12}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}",
        "session", "producer", "total", "add", "del", "pitch", "onset", "offset"
    );

    let mut any = false;
    for path in session_files(dir)? {
        let text = std::fs::read_to_string(&path).map_err(|e| EvalError::io(&path, e))?;
        let session: Session = match serde_json::from_str(&text) {
            Ok(s) => s,
            // Not a session file (no `notes` array, or malformed) — skip it.
            Err(e) => {
                eprintln!("  (skipping {}: {e})", path.display());
                continue;
            }
        };
        any = true;

        // No baseline → the annotation was never edited; the producer's notes are
        // their own baseline, so the cost is zero.
        let baseline = session.baseline.as_deref().unwrap_or(&session.notes);
        let cost = correction_cost(baseline, &session.notes, &params);

        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        println!(
            "  {:<28}  {:<12}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}",
            truncate(name, 28),
            truncate(&session.producer, 12),
            cost.total,
            cost.added,
            cost.deleted,
            cost.pitch_fixed,
            cost.onset_moved,
            cost.offset_moved,
        );

        let tally = tallies.entry(session.producer).or_default();
        tally.sessions += 1;
        tally.sum.add(&cost);
    }

    if !any {
        println!("\n  (no session JSON found in {})", dir.display());
        return Ok(());
    }

    println!("\n== Mean correction cost per producer ==\n");
    println!(
        "  {:<12}  {:>4}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7}",
        "producer", "n", "total", "add", "del", "pitch", "onset", "offset"
    );
    for (producer, t) in &tallies {
        let n = t.sessions as f64;
        println!(
            "  {:<12}  {:>4}  {:>7.2}  {:>7.2}  {:>7.2}  {:>7.2}  {:>7.2}  {:>7.2}",
            truncate(producer, 12),
            t.sessions,
            t.sum.total as f64 / n,
            t.sum.added as f64 / n,
            t.sum.deleted as f64 / n,
            t.sum.pitch_fixed as f64 / n,
            t.sum.onset_moved as f64 / n,
            t.sum.offset_moved as f64 / n,
        );
    }
    println!("\n  Lower is better — fewer manual corrections per stem.");
    Ok(())
}

/// The `*.json` files directly under `dir`, sorted for a stable report.
fn session_files(dir: &Path) -> Result<Vec<PathBuf>, EvalError> {
    let entries = std::fs::read_dir(dir).map_err(|e| EvalError::io(dir, e))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| EvalError::io(dir, e))?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Clip a string to `max` chars so the columns stay aligned.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
