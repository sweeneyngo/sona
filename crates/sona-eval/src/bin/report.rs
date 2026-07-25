//! The evaluation report harness — the **baseline** for the note-HMM work.
//!
//!     cargo run -p sona-eval --bin sona-eval-report            # fixtures only
//!     cargo run -p sona-eval --bin sona-eval-report -- --corpus # + Vocadito
//!
//! It runs the **current ADR-0007 pipeline** (pYIN → energy onsets → heuristic
//! segmentation → confidence) over each stem and scores its output with the
//! ticket-02 metrics, printing a full per-stage table — RPA/RCA, onset-F1,
//! note-F1 (COnP/COnPOff), confidence calibration — plus an onset-vs-segmentation
//! attribution summary. These numbers are the baseline every Phase-B change must
//! beat (ticket 03).
//!
//! With `--corpus` it also scores the Vocadito corpus from `SONA_VOCADITO_DIR`
//! (or the crate-local `data/vocadito`) and **fails loudly** if it isn't there;
//! the corpus is git-ignored, so a fresh checkout runs fixtures only.

use std::process::ExitCode;

use sona_core::{Producer, PYin, Transcriber};
use sona_eval::metrics::{self, MetricParams, StageReport};
use sona_eval::reference::Reference;
use sona_eval::{fixtures, resample_f0_onto, vocadito, wav, Estimate};

/// BPM handed to the pipeline. The core does no beat tracking and the metrics
/// ignore it (they work in seconds), so any value does — it only seeds the UI
/// grid in a real annotation.
const BPM: f64 = 120.0;

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
    let transcriber = Transcriber::default();
    let tracker = PYin::default();
    let params = MetricParams::default();

    println!("== Baseline: ADR-0007 pipeline (pYIN, default Transcriber) ==\n");

    println!("-- Golden fixtures --\n");
    let mut fixture_reports = Vec::new();
    for f in fixtures::load_all()? {
        let report = score_stem(&transcriber, &tracker, &f.samples, f.sample_rate, &f.reference, &params);
        print!("{}", report.format(&f.name));
        fixture_reports.push((f.name.clone(), report));
    }
    print_attribution("fixtures", &fixture_reports);

    if with_corpus {
        let root = vocadito::corpus_dir();
        println!("\n-- Vocadito corpus ({}) --\n", root.display());
        let corpus = vocadito::discover(&root)?; // fails loudly if absent
        let mut corpus_reports = Vec::new();
        for track in &corpus.tracks {
            let reference = vocadito::load_reference(track)?;
            let (samples, sample_rate) = wav::read_mono(&track.audio)?;
            let report = score_stem(&transcriber, &tracker, &samples, sample_rate, &reference, &params);
            print!("{}", report.format(&format!("vocadito_{}", track.id)));
            corpus_reports.push((track.id.clone(), report));
        }
        print_attribution("vocadito", &corpus_reports);
    } else {
        println!("\n(run with --corpus to also score Vocadito from SONA_VOCADITO_DIR)");
    }

    Ok(())
}

/// Transcribe one mono stem with the real pipeline and score it against its
/// reference.
///
/// Two things are stitched into the estimate that `Estimate::from_annotation`
/// can't supply on its own:
/// - the **stage-2 onset times**, read straight from the onset detector, so the
///   onset metric scores stage 2 — not the post-segmentation notes (the
///   annotation contract doesn't carry raw onsets). The detector is
///   deterministic, so calling it here reproduces what `transcribe` used.
/// - the **f0 resampled onto the reference frame grid**, since the pitch metric
///   compares frame for frame.
fn score_stem(
    transcriber: &Transcriber,
    tracker: &PYin,
    samples: &[f32],
    sample_rate: u32,
    reference: &Reference,
    params: &MetricParams,
) -> StageReport {
    let annotation = transcriber.transcribe(tracker, samples, sample_rate, BPM, Producer::Pyin);
    let mut estimate = Estimate::from_annotation(&annotation);

    estimate.onsets = transcriber
        .onset
        .detect(samples, sample_rate)
        .onsets
        .iter()
        .map(|o| o.time_s)
        .collect();

    let ref_times: Vec<f64> = reference.f0.iter().map(|f| f.time_s).collect();
    estimate.f0 = resample_f0_onto(&ref_times, &estimate.f0);
    metrics::evaluate(reference, &estimate, params)
}

/// Print the onset-vs-segmentation attribution: onset recall (stage 2) beside
/// note COnP recall (stage 3). A high onset recall with a much lower note recall
/// points at segmentation (over-merging), not onset detection — the standing
/// question ticket 03 must answer.
fn print_attribution(label: &str, reports: &[(String, StageReport)]) {
    if reports.is_empty() {
        return;
    }
    println!("\n  attribution ({label}) — onset recall (stage 2) vs note recall (stage 3):");
    println!("  {:<16}  {:>10}  {:>10}  {:>9}", "stem", "onset-R", "COnP-R", "note-drop");
    let (mut sum_onset_r, mut sum_note_r) = (0.0, 0.0);
    for (name, r) in reports {
        let drop = r.onset.recall - r.note_conp.recall;
        println!(
            "  {:<16}  {:>10.3}  {:>10.3}  {:>9.3}",
            name, r.onset.recall, r.note_conp.recall, drop
        );
        sum_onset_r += r.onset.recall;
        sum_note_r += r.note_conp.recall;
    }
    let n = reports.len() as f64;
    println!(
        "  {:<16}  {:>10.3}  {:>10.3}  {:>9.3}",
        "MEAN",
        sum_onset_r / n,
        sum_note_r / n,
        (sum_onset_r - sum_note_r) / n
    );
}
