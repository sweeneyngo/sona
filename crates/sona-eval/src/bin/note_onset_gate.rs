//! **Note-level onset gate** — does the onset choice help *notes*, not just onsets?
//!
//!     cargo run --release -p sona-eval --features spectral --bin sona-eval-note-onset-gate -- --corpus
//!
//! Onset-F1 (`sona-eval-onset-compare`) is a proxy. Onsets feed the note-HMM's
//! same-pitch splitter, where the two error types are asymmetric: a **missed**
//! onset merges two notes (a lost boundary → likely a COnP miss), while a **false**
//! onset is an over-split the HMM's interval prior + `merge_same_pitch` can absorb.
//! So SuperFlux's higher recall at slightly lower onset-F1 might still *raise*
//! end-to-end note-F1. This runs the full pipeline with each onset source and
//! scores **note COnP / COnPOff** — the metric that actually decides promotion.
//!
//! Sources compared: energy (default), two SuperFlux thresholds, and an
//! **energy⊕SuperFlux ensemble** (energy's onsets ∪ SuperFlux onsets energy
//! missed). Everything upstream/independent of the onset set (pYIN, envelope, the
//! resampled f0) is cached once per stem; only the onset lists, the note-HMM, and
//! confidence re-run per source.
//!
//! Gate: an onset source **beats energy on corpus COnP-F1 without regressing the
//! staccato fixture** → promotion signal. Else energy stays default.

use std::process::ExitCode;

use sona_core::onset::{OnsetDetector, OnsetNovelty};
use sona_core::pitch::{PYin, PitchTracker};
use sona_core::{ConfidenceScorer, NoteHmm, Onset, Segmenter};

use sona_eval::metrics::{self, MetricParams};
use sona_eval::reference::Reference;
use sona_eval::{fixtures, resample_f0_onto, vocadito, wav, EstNote, Estimate, RefF0};

/// SuperFlux frequency max-filter half-width used throughout (the gate's best).
const SUPERFLUX_BINS: usize = 3;
/// Dedup window (s) for the ensemble: a SuperFlux onset within this of an energy
/// onset is considered the same attack and not added again.
const ENSEMBLE_DEDUP_S: f64 = 0.05;

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");
    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-note-onset-gate: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One stem with everything that doesn't depend on the onset choice cached: the
/// contour, the (shared) energy envelope, the reference, the resampled f0, and the
/// candidate onset sets.
struct Stem {
    name: String,
    contour: Vec<sona_core::F0Frame>,
    envelope: Vec<sona_core::EnergyFrame>,
    reference: Reference,
    f0_on_ref: Vec<RefF0>,
    /// Named onset sets to score: (label, onsets).
    onset_sets: Vec<(&'static str, Vec<Onset>)>,
}

#[derive(Clone, Copy, Default)]
struct NoteF1 {
    conp: f64,
    conpoff: f64,
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    let tracker = PYin::default();
    let params = MetricParams::default();
    let scorer = ConfidenceScorer::default();

    let mut fixtures_cache = Vec::new();
    for f in fixtures::load_all()? {
        fixtures_cache.push(prepare(&tracker, &f.name, &f.samples, f.sample_rate, f.reference));
    }
    let staccato = fixtures_cache
        .iter()
        .position(|s| s.name == "staccato")
        .expect("the staccato golden fixture must exist");

    let mut corpus_cache = Vec::new();
    if with_corpus {
        let root = vocadito::corpus_dir();
        let corpus = vocadito::discover(&root)?;
        eprintln!("preparing {} Vocadito stems (pYIN + onsets, once)…", corpus.tracks.len());
        for track in &corpus.tracks {
            let reference = vocadito::load_reference(track)?;
            let (samples, sr) = wav::read_mono(&track.audio)?;
            corpus_cache.push(prepare(&tracker, &format!("vocadito_{}", track.id), &samples, sr, reference));
        }
    }

    // The onset-source labels, in the order prepare() built them.
    let labels: Vec<&'static str> = fixtures_cache
        .first()
        .map(|s| s.onset_sets.iter().map(|(l, _)| *l).collect())
        .unwrap_or_default();

    println!("== Note-level onset gate: note-F1 by onset source (ADR-0013 / ticket 04) ==\n");
    if corpus_cache.is_empty() {
        println!("(no corpus loaded — run with --corpus for the real gate)\n");
    }

    let corpus_scores: Vec<(&str, NoteF1)> = labels
        .iter()
        .map(|&l| (l, mean_note_f1(&corpus_cache, l, &scorer, &params)))
        .collect();
    let staccato_scores: Vec<(&str, NoteF1)> = labels
        .iter()
        .map(|&l| (l, score_stem(&fixtures_cache[staccato], l, &scorer, &params)))
        .collect();

    let energy_conp = corpus_scores.first().map(|(_, f)| f.conp).unwrap_or(0.0);
    let energy_stac_conp = staccato_scores.first().map(|(_, f)| f.conp).unwrap_or(0.0);

    println!(
        "{:<22} {:>12} {:>15} {:>14} {:>17} {:>7}",
        "onset source", "corpusCOnP", "corpusCOnPOff", "staccatoCOnP", "staccatoCOnPOff", "gate"
    );
    for ((label, corpus), (_, stac)) in corpus_scores.iter().zip(&staccato_scores) {
        let beats = !corpus_cache.is_empty() && corpus.conp > energy_conp + 1e-6;
        let holds_staccato = stac.conp >= energy_stac_conp - 1e-6;
        let pass = *label != "energy" && beats && holds_staccato;
        println!(
            "{:<22} {:>12.3} {:>15.3} {:>14.3} {:>17.3} {:>7}",
            label,
            corpus.conp,
            corpus.conpoff,
            stac.conp,
            stac.conpoff,
            if *label == "energy" { "(base)" } else if pass { "PASS" } else { "—" }
        );
    }

    println!(
        "\nEnergy baseline: corpus COnP-F1 {energy_conp:.3}. A source PASSES only if it beats that\n\
         and holds the staccato fixture. Onset-F1 said energy won; COnP is the real test."
    );
    Ok(())
}

/// Cache pYIN + the shared envelope + the candidate onset sets for one stem.
fn prepare(tracker: &PYin, name: &str, samples: &[f32], sample_rate: u32, reference: Reference) -> Stem {
    let contour = tracker.track(samples, sample_rate);

    let energy_det = OnsetDetector::default();
    let energy_res = energy_det.detect(samples, sample_rate);
    let envelope = energy_res.energy_envelope.clone();
    let energy_onsets = energy_res.onsets;

    let superflux = |delta: f32| -> Vec<Onset> {
        OnsetDetector {
            novelty: OnsetNovelty::SpectralFlux { max_filter_bins: SUPERFLUX_BINS },
            threshold_delta: delta,
            ..OnsetDetector::default()
        }
        .detect(samples, sample_rate)
        .onsets
    };
    let sf_aggressive = superflux(0.02); // high recall
    let sf_conservative = superflux(0.10); // higher precision
    let ensembled = ensemble(&energy_onsets, &sf_aggressive, ENSEMBLE_DEDUP_S);

    let ref_times: Vec<f64> = reference.f0.iter().map(|f| f.time_s).collect();
    let est_f0: Vec<RefF0> = contour
        .iter()
        .map(|f| RefF0 { time_s: f.time_s, frequency_hz: f.frequency_hz, voiced: f.frequency_hz > 0.0 })
        .collect();
    let f0_on_ref = resample_f0_onto(&ref_times, &est_f0);

    Stem {
        name: name.to_string(),
        contour,
        envelope,
        reference,
        f0_on_ref,
        onset_sets: vec![
            ("energy", energy_onsets),
            ("superflux δ=0.02", sf_aggressive),
            ("superflux δ=0.10", sf_conservative),
            ("ensemble energy⊕sf", ensembled),
        ],
    }
}

/// Energy onsets ∪ SuperFlux onsets that no energy onset is within `dedup_s` of.
fn ensemble(energy: &[Onset], spectral: &[Onset], dedup_s: f64) -> Vec<Onset> {
    let mut out = energy.to_vec();
    for s in spectral {
        if !energy.iter().any(|e| (e.time_s - s.time_s).abs() < dedup_s) {
            out.push(s.clone());
        }
    }
    out.sort_by(|a, b| a.time_s.partial_cmp(&b.time_s).expect("finite onset times"));
    out
}

/// Full-pipeline note-F1 for one named onset source on one stem.
fn score_stem(stem: &Stem, label: &str, scorer: &ConfidenceScorer, params: &MetricParams) -> NoteF1 {
    let onsets = &stem
        .onset_sets
        .iter()
        .find(|(l, _)| *l == label)
        .expect("known onset source")
        .1;
    let seg: Segmenter = NoteHmm::default();
    let segments = seg.segment(onsets, &stem.contour, &stem.envelope);
    let notes = scorer.score_all(&segments, &stem.envelope);
    let estimate = Estimate {
        onsets: onsets.iter().map(|o| o.time_s).collect(),
        notes: notes
            .iter()
            .map(|n| EstNote {
                onset_s: n.onset_s,
                offset_s: n.offset_s,
                midi_pitch: n.midi_pitch,
                confidence: n.confidence,
            })
            .collect(),
        f0: stem.f0_on_ref.clone(),
    };
    let report = metrics::evaluate(&stem.reference, &estimate, params);
    NoteF1 { conp: report.note_conp.f1, conpoff: report.note_conpoff.f1 }
}

/// Mean note-F1 over a stem set for one onset source.
fn mean_note_f1(stems: &[Stem], label: &str, scorer: &ConfidenceScorer, params: &MetricParams) -> NoteF1 {
    if stems.is_empty() {
        return NoteF1::default();
    }
    let mut acc = NoteF1::default();
    for stem in stems {
        let f = score_stem(stem, label, scorer, params);
        acc.conp += f.conp;
        acc.conpoff += f.conpoff;
    }
    let n = stems.len() as f64;
    NoteF1 { conp: acc.conp / n, conpoff: acc.conpoff / n }
}
