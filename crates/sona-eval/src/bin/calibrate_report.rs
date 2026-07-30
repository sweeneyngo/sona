//! Confidence-calibration analysis across saved sessions.
//!
//!     cargo run -p sona-eval --bin sona-eval-calibrate -- <dir-of-sessions>
//!
//! Pools every baseline note from every session, labels it correct/wrong by COnP
//! (onset + pitch) against that session's corrected notes, and asks two questions
//! the calibration *table* can't:
//!
//! 1. **Per-sub-score AUC** — how well does each dimension (and the current
//!    confidence scalar) separate correct notes from wrong ones? `0.5` is a coin
//!    flip; higher is more predictive.
//! 2. **Best re-weight** — a grid search over convex weights on the four
//!    sub-scores, **trained on some sessions and scored on held-out others**, so
//!    the winning weights aren't overfit to one stem. Compared against the current
//!    confidence's AUC on the same held-out set.
//!
//! With one session there is nothing to hold out, so it splits notes evenly and
//! says so loudly — a within-stem number that flatters itself. The point of this
//! bin is to be re-run as sessions accumulate.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sona_eval::metrics::note::match_notes;
use sona_eval::metrics::NoteParams;
use sona_eval::{EvalError, Session};

const FEATURES: [&str; 4] = ["pitch_stability", "voicing", "onset_clarity", "offset_clarity"];
/// Convex-weight grid tried per dimension in the re-weight search.
const LEVELS: [f64; 6] = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];

/// One pooled note: its four sub-scores, the shipped confidence, whether it was a
/// correct (COnP) transcription, and which session it came from (for the split).
struct Sample {
    feats: [f64; 4],
    confidence: f64,
    correct: bool,
    session: usize,
}

fn main() -> ExitCode {
    let input = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: sona-eval-calibrate <dir-of-sessions>");
            return ExitCode::FAILURE;
        }
    };
    match run(&input) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-calibrate: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &Path) -> Result<(), EvalError> {
    let params = NoteParams::default();
    let mut samples: Vec<Sample> = Vec::new();
    let (mut n_sessions, mut skipped) = (0usize, 0usize);

    for path in session_paths(input)? {
        let session = Session::load(&path)?;
        if !session.has_baseline {
            skipped += 1;
            continue;
        }
        // COnP (onset + pitch) correctness for each baseline (estimate) note.
        let mut correct = vec![false; session.estimate.notes.len()];
        for (_ref_i, est_i) in
            match_notes(&session.reference.notes, &session.estimate.notes, &params, false)
        {
            correct[est_i] = true;
        }
        for (i, ss) in session.baseline_scores.iter().enumerate() {
            samples.push(Sample {
                feats: [
                    ss.pitch_stability as f64,
                    ss.voicing as f64,
                    ss.onset_clarity as f64,
                    ss.offset_clarity as f64,
                ],
                confidence: session.estimate.notes[i].confidence as f64,
                correct: correct[i],
                session: n_sessions,
            });
        }
        n_sessions += 1;
    }

    println!("== Confidence calibration analysis ({}) ==\n", input.display());
    if samples.is_empty() {
        println!("  no scorable sessions found (need a `baseline` with sub_scores).");
        return Ok(());
    }

    let n_correct = samples.iter().filter(|s| s.correct).count();
    println!(
        "  sessions: {n_sessions} (with baseline), skipped {skipped}\n  \
         notes pooled: {}   correct: {n_correct}   wrong: {}",
        samples.len(),
        samples.len() - n_correct
    );

    // --- Per-feature AUC over all pooled notes ---
    println!("\n  per-feature AUC (pooled, 0.5 = no signal):");
    for (k, name) in FEATURES.iter().enumerate() {
        let a = auc(samples.iter().map(|s| (s.feats[k], s.correct)));
        println!("    {name:<16} {a:.3}");
    }
    let conf_auc_all = auc(samples.iter().map(|s| (s.confidence, s.correct)));
    println!("    {:<16} {:.3}", "confidence(now)", conf_auc_all);

    // --- Train/test split, then a weight search on train scored on test ---
    // With ≥2 sessions, hold out whole sessions (alternate) so the test measures
    // generalisation to an unseen stem. With one, fall back to a note-parity split
    // — a within-stem number that flatters itself.
    let (train, test): (Vec<&Sample>, Vec<&Sample>) = if n_sessions >= 2 {
        println!(
            "\n  split: by session — train {} / test {} sessions",
            (0..n_sessions).filter(|i| i % 2 == 0).count(),
            (0..n_sessions).filter(|i| i % 2 == 1).count()
        );
        samples.iter().partition(|s| s.session % 2 == 0)
    } else {
        println!("\n  split: by note, WITHIN-STEM (only 1 session) — weak, collect more");
        let train = samples.iter().step_by(2).collect();
        let test = samples.iter().skip(1).step_by(2).collect();
        (train, test)
    };

    let (best_w, best_train_auc) = best_weights(&train);
    let test_auc = auc(test.iter().map(|s| (weighted(&s.feats, &best_w), s.correct)));
    let conf_test_auc = auc(test.iter().map(|s| (s.confidence, s.correct)));

    println!("\n  best re-weight (trained on train, scored on held-out test):");
    print!("    weights ");
    for (k, name) in FEATURES.iter().enumerate() {
        print!(" {}={:.1}", short(name), best_w[k]);
    }
    println!("\n    AUC   train {best_train_auc:.3}   test {test_auc:.3}");
    println!("    vs current confidence AUC   test {conf_test_auc:.3}");

    println!("\n  Higher AUC = the flag better separates correct from wrong notes.");
    if n_sessions < 5 {
        println!("  ⚠ only {n_sessions} session(s): treat the weights as directional, not final.");
    }
    Ok(())
}

/// Grid-search convex weights over the four sub-scores; return the weights with
/// the highest AUC on `train` and that AUC.
fn best_weights(train: &[&Sample]) -> ([f64; 4], f64) {
    let mut best = ([1.0, 0.0, 0.0, 0.0], 0.0);
    for &a in &LEVELS {
        for &b in &LEVELS {
            for &c in &LEVELS {
                for &d in &LEVELS {
                    let w = [a, b, c, d];
                    if a + b + c + d == 0.0 {
                        continue;
                    }
                    let score = auc(train.iter().map(|s| (weighted(&s.feats, &w), s.correct)));
                    if score > best.1 {
                        best = (w, score);
                    }
                }
            }
        }
    }
    best
}

/// Weighted mean of a note's sub-scores under weights `w`.
fn weighted(feats: &[f64; 4], w: &[f64; 4]) -> f64 {
    let sum: f64 = w.iter().sum();
    if sum == 0.0 {
        return 0.0;
    }
    feats.iter().zip(w).map(|(f, wi)| f * wi).sum::<f64>() / sum
}

/// AUC (Mann–Whitney) via average ranks, tie-aware. `0.5` when a class is empty.
fn auc(data: impl Iterator<Item = (f64, bool)>) -> f64 {
    let mut v: Vec<(f64, bool)> = data.collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = v.len();
    let n_pos = v.iter().filter(|x| x.1).count();
    let n_neg = n - n_pos;
    if n_pos == 0 || n_neg == 0 {
        return 0.5;
    }
    // Sum of average ranks (1-indexed) of the positive class.
    let mut sum_pos_ranks = 0.0;
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j + 1 < n && v[j + 1].0 == v[i].0 {
            j += 1;
        }
        let avg_rank = ((i + 1) + (j + 1)) as f64 / 2.0;
        for item in &v[i..=j] {
            if item.1 {
                sum_pos_ranks += avg_rank;
            }
        }
        i = j + 1;
    }
    (sum_pos_ranks - (n_pos * (n_pos + 1) / 2) as f64) / (n_pos * n_neg) as f64
}

fn short(name: &str) -> &str {
    match name {
        "pitch_stability" => "ps",
        "voicing" => "v",
        "onset_clarity" => "onc",
        "offset_clarity" => "ofc",
        other => other,
    }
}

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
