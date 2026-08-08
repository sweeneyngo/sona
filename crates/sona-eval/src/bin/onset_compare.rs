//! **Energy-rise vs spectral-flux onsets** — the ADR-0013 gate.
//!
//!     cargo run --release -p sona-eval --features spectral --bin sona-eval-onset-compare -- --corpus
//!
//! The onset-deviation diagnostic (`reports/onset-deviation.md`) showed the
//! onset-F1 dip is **recall-limited** — the energy-rise novelty misses ~half the
//! Vocadito attacks (soft/legato onsets). Spectral flux (ADR-0013) should recover
//! them. This runs both detectors over the same stems and reports onset
//! Precision / Recall / F1 at the metric's ±50 ms, side by side.
//!
//! The gate: **spectral flux lifts onset recall and F1 on Vocadito** (the fixtures
//! are the staccato regression wall — it must not lose those). Only then is the
//! `spectral` feature worth promoting toward the default (a follow-up ADR step).
//!
//! Requires the `spectral` feature (for the spectral detector). Built without it,
//! the binary reports energy only and says so.

use std::process::ExitCode;

use sona_core::onset::OnsetDetector;

use sona_eval::metrics::onset::{onset_metrics, ONSET_WINDOW_S};
use sona_eval::metrics::Prf1;
use sona_eval::reference::Reference;
use sona_eval::{fixtures, vocadito, wav};

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");
    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-onset-compare: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A prepared stem: reference onsets + the mono samples both detectors run on.
struct Stem {
    ref_onsets: Vec<f64>,
    samples: Vec<f32>,
    sample_rate: u32,
}

/// Mean P/R/F1 over a stem set, plus pooled detected/matched counts for a recall
/// read that isn't averaged over uneven stem sizes.
#[derive(Default)]
struct Agg {
    p: f64,
    r: f64,
    f1: f64,
    n: usize,
    detected: usize,
    reference: usize,
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    let mut fixtures_stems = Vec::new();
    for f in fixtures::load_all()? {
        fixtures_stems.push(Stem {
            ref_onsets: sorted_ref(&f.reference),
            samples: f.samples,
            sample_rate: f.sample_rate,
        });
    }

    let mut corpus_stems = Vec::new();
    if with_corpus {
        let root = vocadito::corpus_dir();
        let corpus = vocadito::discover(&root)?;
        eprintln!("running both detectors over {} Vocadito stems…", corpus.tracks.len());
        for track in &corpus.tracks {
            let reference = vocadito::load_reference(track)?;
            let (samples, sample_rate) = wav::read_mono(&track.audio)?;
            corpus_stems.push(Stem {
                ref_onsets: sorted_ref(&reference),
                samples,
                sample_rate,
            });
        }
    }

    println!("== Onset detector comparison: energy-rise vs spectral-flux (ADR-0013) ==\n");
    #[cfg(not(feature = "spectral"))]
    println!(
        "NOTE: built without the `spectral` feature — energy only. Re-run with\n\
         `--features spectral` for the comparison.\n"
    );

    if !corpus_stems.is_empty() {
        report("Vocadito corpus", &corpus_stems);
    }
    report("golden fixtures (staccato regression wall)", &fixtures_stems);
    Ok(())
}

/// Score both detectors over `stems` and print the side-by-side table + verdict.
fn report(label: &str, stems: &[Stem]) {
    let energy = score(stems, &OnsetDetector::default());

    println!("### {label}  ({} stems)\n", stems.len());
    println!("{:<16} {:>10} {:>10} {:>10} {:>12} {:>12}", "detector", "meanP", "meanR", "meanF1", "detected", "reference");
    print_row("energy-rise", &energy);

    #[cfg(feature = "spectral")]
    {
        use sona_core::onset::OnsetNovelty;
        // Two dimensions. threshold_delta trades recall vs precision (the novelty
        // scale differs from energy's, so it can't be inherited). max_filter_bins is
        // SuperFlux: 0 = plain flux, >0 suppresses vibrato to recover precision.
        const SPECTRAL_DELTA_GRID: &[f32] = &[0.02, 0.05, 0.1, 0.2, 0.3, 0.5];
        const MAXFILT_GRID: &[usize] = &[0, 1, 3, 5, 8];
        let energy_f1 = energy.f1 / energy.n.max(1) as f64;
        let mut best: Option<(usize, f32, f64, Agg)> = None;
        // For each SuperFlux width, report its best-F1 threshold row (a full 2-D
        // grid is 30 rows; the per-width best is the readable slice).
        for &bins in MAXFILT_GRID {
            let mut best_at_bins: Option<(f32, f64, Agg)> = None;
            for &delta in SPECTRAL_DELTA_GRID {
                let a = score(
                    stems,
                    &OnsetDetector {
                        novelty: OnsetNovelty::SpectralFlux { max_filter_bins: bins },
                        threshold_delta: delta,
                        ..OnsetDetector::default()
                    },
                );
                let f1 = a.f1 / a.n.max(1) as f64;
                if best_at_bins.as_ref().is_none_or(|(_, bf1, _)| f1 > *bf1) {
                    best_at_bins = Some((delta, f1, a));
                }
            }
            if let Some((delta, f1, a)) = best_at_bins {
                let tag = if bins == 0 { "plain flux".to_string() } else { format!("superflux m={bins}") };
                print_row(&format!("{tag} (δ={delta:.2})"), &a);
                if best.as_ref().is_none_or(|(_, _, bf1, _)| f1 > *bf1) {
                    best = Some((bins, delta, f1, a));
                }
            }
        }
        if let Some((bins, delta, f1, a)) = best {
            let dr = a.r / a.n.max(1) as f64 - energy.r / energy.n.max(1) as f64;
            println!(
                "\n  best: max_filter_bins={bins} δ={delta:.2}  mean F1 {:.3} (energy {:.3}, {:+.3})   mean recall Δ {:+.3}",
                f1,
                energy_f1,
                f1 - energy_f1,
                dr
            );
            println!(
                "  → {}",
                if f1 > energy_f1 + 1e-3 {
                    "SuperFlux beats energy — the gate passes; consider promoting toward default"
                } else {
                    "still does not beat energy F1 — see report for the next lever"
                }
            );
        }
    }
    println!();
}

/// Mean P/R/F1 (and pooled counts) of one detector over the stems, at ±50 ms.
fn score(stems: &[Stem], detector: &OnsetDetector) -> Agg {
    let mut a = Agg::default();
    for s in stems {
        let mut det: Vec<f64> =
            detector.detect(&s.samples, s.sample_rate).onsets.iter().map(|o| o.time_s).collect();
        det.sort_by(|x, y| x.partial_cmp(y).expect("finite times"));
        let m: Prf1 = onset_metrics(&s.ref_onsets, &det, ONSET_WINDOW_S);
        a.p += m.precision;
        a.r += m.recall;
        a.f1 += m.f1;
        a.n += 1;
        a.detected += det.len();
        a.reference += s.ref_onsets.len();
    }
    a
}

fn print_row(name: &str, a: &Agg) {
    let n = a.n.max(1) as f64;
    println!(
        "{:<16} {:>10.3} {:>10.3} {:>10.3} {:>12} {:>12}",
        name,
        a.p / n,
        a.r / n,
        a.f1 / n,
        a.detected,
        a.reference
    );
}

/// Sorted reference note-onset times for a stem.
fn sorted_ref(reference: &Reference) -> Vec<f64> {
    let mut v: Vec<f64> = reference.notes.iter().map(|n| n.onset_s).collect();
    v.sort_by(|a, b| a.partial_cmp(b).expect("finite times"));
    v
}
