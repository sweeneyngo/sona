//! **Signed onset-deviation histogram** — a cheap infra check before committing
//! to spectral-flux onsets.
//!
//!     cargo run --release -p sona-eval --bin sona-eval-onset-deviation -- --corpus
//!
//! Onset recall is a measured bottleneck (F1 ≈ 0.565 on Vocadito). Two very
//! different causes give the same F1 dip, and the fixes are opposite:
//!
//!  - a **systematic timing bias** (frame-centering / latency off-by-one) — the
//!    detected onsets are all shifted a few ms from the truth. A constant
//!    correction fixes it for free; no new detector needed.
//!  - **genuinely hard onsets** — soft/legato attacks the energy-rise novelty
//!    never sees. Only a better onset signal (spectral flux) helps.
//!
//! This bins the signed deviation `detected − reference` over matched pairs. A
//! distribution **centred on zero** ⇒ hard onsets (spectral flux is justified); a
//! distribution **biased off zero** ⇒ an infra shift to correct first. The pairing
//! reuses the metric's exact maximum-cardinality matcher so the matched set is the
//! one the onset-F1 actually scores.

use std::process::ExitCode;

use sona_core::onset::OnsetDetector;

use sona_eval::metrics::matching::maximum_matching;
use sona_eval::reference::Reference;
use sona_eval::{fixtures, vocadito, wav};

/// Matching window (seconds). Wider than the metric's ±50 ms so a real bias near
/// the metric window's edge is not clipped — the point is to *see* the bias, not
/// to reproduce the F1's matched count exactly.
const MATCH_WINDOW_S: f64 = 0.100;
/// Histogram bin width (seconds). ~one pYIN/onset hop (256/44100 ≈ 5.8 ms), so an
/// off-by-one-frame latency lands in a distinct bin from zero.
const BIN_S: f64 = 0.005;
/// Half-range of the histogram (seconds); deviations beyond fall in the tails.
const HALF_RANGE_S: f64 = 0.060;
/// A |median| below this (≈ one hop) reads as "no meaningful bias".
const BIAS_EPS_S: f64 = 0.006;
/// The onset-F1 tolerance (mirrors `metrics::onset::ONSET_WINDOW_S`). A bias
/// smaller than this is real but does *not* cost F1 hits — correcting it is a
/// placement/correctness win, not an F1 win.
const METRIC_WINDOW_S: f64 = 0.050;
/// Below this matched-fraction, the onset dip is recall-dominated (missed
/// attacks), which no constant correction can recover — a better onset signal can.
const RECALL_FLOOR: f64 = 0.75;

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");
    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-onset-deviation: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    let detector = OnsetDetector::default();

    let mut deviations: Vec<f64> = Vec::new();
    let mut n_ref = 0usize;
    let mut n_det = 0usize;

    // Fixtures always; corpus when asked. Deviations pool across all stems.
    let mut stems: Vec<(String, Vec<f64>, Vec<f64>)> = Vec::new();
    for f in fixtures::load_all()? {
        stems.push(collect(&detector, &f.name, &f.samples, f.sample_rate, &f.reference));
    }
    if with_corpus {
        let root = vocadito::corpus_dir();
        let corpus = vocadito::discover(&root)?;
        eprintln!("running onset detection over {} Vocadito stems…", corpus.tracks.len());
        for track in &corpus.tracks {
            let reference = vocadito::load_reference(track)?;
            let (samples, sr) = wav::read_mono(&track.audio)?;
            stems.push(collect(&detector, &format!("vocadito_{}", track.id), &samples, sr, &reference));
        }
    }

    for (_, ref_onsets, det_onsets) in &stems {
        n_ref += ref_onsets.len();
        n_det += det_onsets.len();
        for d in match_deviations(ref_onsets, det_onsets) {
            deviations.push(d);
        }
    }

    println!("== Signed onset-deviation histogram (detected − reference) ==\n");
    if !with_corpus {
        println!("(fixtures only — run with --corpus for the real signal)\n");
    }
    println!(
        "stems {}   reference onsets {}   detected onsets {}   matched pairs {} (±{:.0} ms window)\n",
        stems.len(),
        n_ref,
        n_det,
        deviations.len(),
        MATCH_WINDOW_S * 1000.0
    );

    if deviations.is_empty() {
        println!("no matched pairs — nothing to say about bias");
        return Ok(());
    }

    deviations.sort_by(|a, b| a.partial_cmp(b).expect("finite deviations"));
    let n = deviations.len() as f64;
    let mean = deviations.iter().sum::<f64>() / n;
    let median = deviations[deviations.len() / 2];
    let var = deviations.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();

    print_histogram(&deviations);

    println!(
        "\nmean {:+.1} ms   median {:+.1} ms   std {:.1} ms",
        mean * 1000.0,
        median * 1000.0,
        std * 1000.0
    );

    // Two independent questions, two independent answers.
    // (1) Recall: are enough onsets even detected to match?
    let match_rate = deviations.len() as f64 / n_ref.max(1) as f64;
    // (2) Bias: is the matched set systematically shifted, and does the shift
    //     exceed the F1 tolerance (so correcting it would actually move F1)?
    let biased = median.abs() >= BIAS_EPS_S;
    let bias_costs_f1 = median.abs() >= METRIC_WINDOW_S;

    println!(
        "\nmatched {} / {} reference onsets = {:.0}% (within ±{:.0} ms)",
        deviations.len(),
        n_ref,
        match_rate * 100.0,
        MATCH_WINDOW_S * 1000.0
    );

    println!("\nVERDICT");
    // Recall verdict — the dominant lever for a low match rate.
    if match_rate < RECALL_FLOOR {
        println!(
            "  · RECALL-limited: {:.0}% of reference onsets have no detection within ±{:.0} ms — the\n\
             \x20   energy-rise novelty simply misses roughly half the attacks (soft/legato onsets). No\n\
             \x20   constant correction recovers a missed onset; a better onset signal (spectral flux) can.\n\
             \x20   → this justifies the spectral-flux move.",
            (1.0 - match_rate) * 100.0,
            MATCH_WINDOW_S * 1000.0
        );
    } else {
        println!("  · Recall is adequate ({:.0}% matched) — the dip is not dominated by missed onsets.", match_rate * 100.0);
    }
    // Bias verdict — separate; here it is real but within tolerance.
    if !biased {
        println!("  · No timing bias (|median| < {:.0} ms ≈ one hop).", BIAS_EPS_S * 1000.0);
    } else if bias_costs_f1 {
        println!(
            "  · TIMING BIAS costs F1: median {:+.1} ms exceeds the ±{:.0} ms tolerance. A constant\n\
             \x20   correction of {:+.1} ms is a near-free F1 fix — apply and re-score before anything else.",
            median * 1000.0,
            METRIC_WINDOW_S * 1000.0,
            -median * 1000.0
        );
    } else {
        println!(
            "  · Small timing bias present (median {:+.1} ms ≈ {:.0} hops early) but WITHIN the ±{:.0} ms\n\
             \x20   tolerance, so it does not cost onset-F1. Worth a constant {:+.1} ms correction for onset\n\
             \x20   *placement* accuracy (tighter note starts), but it is not the lever for the 0.565.",
            median * 1000.0,
            (median.abs() / (256.0 / 44_100.0)).round(),
            METRIC_WINDOW_S * 1000.0,
            -median * 1000.0
        );
    }
    Ok(())
}

/// Detect onsets for one stem and return `(name, sorted reference onsets, sorted
/// detected onsets)`.
fn collect(
    detector: &OnsetDetector,
    name: &str,
    samples: &[f32],
    sample_rate: u32,
    reference: &Reference,
) -> (String, Vec<f64>, Vec<f64>) {
    let result = detector.detect(samples, sample_rate);
    let mut det: Vec<f64> = result.onsets.iter().map(|o| o.time_s).collect();
    det.sort_by(|a, b| a.partial_cmp(b).expect("finite times"));
    let mut refs: Vec<f64> = reference.notes.iter().map(|n| n.onset_s).collect();
    refs.sort_by(|a, b| a.partial_cmp(b).expect("finite times"));
    (name.to_string(), refs, det)
}

/// Signed deviations `detected − reference` over a maximum-cardinality matching
/// within [`MATCH_WINDOW_S`]. Reuses the metric's matcher so a reference onset is
/// paired to at most one detected onset (no double-counting a cluster).
fn match_deviations(ref_onsets: &[f64], det_onsets: &[f64]) -> Vec<f64> {
    let adjacency: Vec<Vec<usize>> = ref_onsets
        .iter()
        .map(|&r| {
            det_onsets
                .iter()
                .enumerate()
                .filter(|(_, &d)| (d - r).abs() <= MATCH_WINDOW_S)
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    maximum_matching(&adjacency)
        .into_iter()
        .map(|(r, d)| det_onsets[d] - ref_onsets[r])
        .collect()
}

/// Print a text histogram of the (sorted) deviations, one row per [`BIN_S`] bin
/// across ±[`HALF_RANGE_S`], with `<` / `>` tail rows.
fn print_histogram(sorted: &[f64]) {
    let n_bins = (2.0 * HALF_RANGE_S / BIN_S).round() as usize;
    let mut bins = vec![0usize; n_bins];
    let mut under = 0usize;
    let mut over = 0usize;
    for &d in sorted {
        if d < -HALF_RANGE_S {
            under += 1;
        } else if d >= HALF_RANGE_S {
            over += 1;
        } else {
            let idx = ((d + HALF_RANGE_S) / BIN_S).floor() as usize;
            bins[idx.min(n_bins - 1)] += 1;
        }
    }
    let peak = bins.iter().copied().chain([under, over]).max().unwrap_or(1).max(1);
    let bar = |count: usize| "█".repeat((count * 40 / peak).max(usize::from(count > 0)));

    println!("   deviation (ms)   count");
    println!("      < {:+5.0}   {:>5}  {}", -HALF_RANGE_S * 1000.0, under, bar(under));
    for (i, &count) in bins.iter().enumerate() {
        let lo = (-HALF_RANGE_S + i as f64 * BIN_S) * 1000.0;
        println!("   {:+5.0}..{:+5.0}   {:>5}  {}", lo, lo + BIN_S * 1000.0, count, bar(count));
    }
    println!("      ≥ {:+5.0}   {:>5}  {}", HALF_RANGE_S * 1000.0, over, bar(over));
}
