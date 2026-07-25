//! **Note-HMM parameter sweep** (ticket 08) — the payoff gate for Phase B.
//!
//!     cargo run --release -p sona-eval --bin sona-eval-sweep -- --corpus
//!
//! Sweeps the note-HMM's two levers — the **Stable self-transition** (the one
//! main knob, ADR-0010) and the **interval-prior λ** — over Vocadito + the golden
//! fixtures, and prints note-F1 (COnP / COnPOff) for each setting beside the
//! **ADR-0007 heuristic baseline** computed in the same run on the same machine.
//!
//! The gate (ticket 08 / the spec): **beat baseline note-F1 on the expressive
//! corpus without regressing the staccato golden fixture.** The upstream stages
//! (pYIN, energy onsets) don't depend on the HMM parameters, so they are run once
//! per stem and cached; only the segmenter + confidence re-run per grid point.

use std::process::ExitCode;

use sona_core::hmm::HmmParams;
use sona_core::onset::OnsetDetector;
use sona_core::pitch::{PYin, PitchTracker};
use sona_core::{ConfidenceScorer, NoteHmm, Segmenter};

use sona_eval::metrics::{self, MetricParams};
use sona_eval::reference::Reference;
use sona_eval::{fixtures, resample_f0_onto, vocadito, wav, EstNote, Estimate, RefF0};

/// Stable self-transition values to sweep (the main knob). Higher merges more.
const STABLE_SELF_GRID: &[f32] = &[0.90, 0.95, 0.98, 0.99, 0.995];
/// Interval-prior λ values to sweep (semitones). Smaller makes jumps costlier.
const LAMBDA_GRID: &[f32] = &[1.5, 2.5, 4.0];

/// The `stable_self` / λ the secondary sweep holds fixed (the main sweep's best).
const FIXED_STABLE_SELF: f32 = 0.95;
const FIXED_LAMBDA: f32 = 2.5;
/// ADR-0010 leaves `attack_self` (nominal 0.9) and the trust exponent `τ`
/// (nominal 0.1) as literature starting points and is explicit that the model is
/// "not eye-tunable and must be gated by measurement". So they get their own
/// sweep — held at the main sweep's best `stable_self`/λ — rather than being set
/// by eye. Values include the ADR nominals so the choice is measured, not asserted.
const ATTACK_SELF_GRID: &[f32] = &[0.5, 0.6, 0.75, 0.9];
const PITCH_TRUST_GRID: &[f32] = &[0.1, 0.5, 1.0, 2.0];

/// The recorded ADR-0007 heuristic baseline the note-HMM had to beat — the
/// numbers from `reports/baseline.md` (ticket 03). The heuristic segmenter itself
/// was retired on the ticket-08 sign-off, so the gate compares against these
/// fixed historical numbers rather than re-running it.
const BASE_CORPUS_CONP_F1: f64 = 0.515;
const BASE_CORPUS_CONPOFF_F1: f64 = 0.348;
const BASE_STACCATO_CONP_F1: f64 = 1.000;
const BASE_STACCATO_CONPOFF_F1: f64 = 1.000;

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");
    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-sweep: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One stem with its upstream stages already computed — the HMM/heuristic
/// consume only these, so they are cached across the whole grid.
struct Stem {
    name: String,
    contour: Vec<sona_core::F0Frame>,
    onsets: Vec<sona_core::Onset>,
    envelope: Vec<sona_core::EnergyFrame>,
    /// Detected onset *times* (stage-2 signal for the onset metric — unused by the
    /// note metric, but kept so the estimate is complete).
    onset_times: Vec<f64>,
    reference: Reference,
    /// Estimate f0 already resampled onto the reference frame grid (constant
    /// across the sweep, since pitch tracking doesn't depend on the segmenter).
    f0_on_ref: Vec<RefF0>,
}

/// Mean note-F1 over a set of stems.
#[derive(Clone, Copy, Default)]
struct NoteF1 {
    conp: f64,
    conpoff: f64,
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    let tracker = PYin::default();
    let detector = OnsetDetector::default();
    let params = MetricParams::default();

    // --- Cache the upstream stages once per stem -----------------------------
    let mut fixtures_cache = Vec::new();
    for f in fixtures::load_all()? {
        fixtures_cache.push(prepare(&tracker, &detector, &f.name, &f.samples, f.sample_rate, f.reference));
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
            corpus_cache.push(prepare(
                &tracker,
                &detector,
                &format!("vocadito_{}", track.id),
                &samples,
                sr,
                reference,
            ));
        }
    }

    let scorer = ConfidenceScorer::default();

    println!("== Note-HMM sweep vs. ADR-0007 baseline (reports/baseline.md) ==\n");
    println!(
        "baseline (retired heuristic):  corpus COnP-F1 {BASE_CORPUS_CONP_F1:.3}  COnPOff-F1 {BASE_CORPUS_CONPOFF_F1:.3}   |   staccato COnP-F1 {BASE_STACCATO_CONP_F1:.3}  COnPOff-F1 {BASE_STACCATO_CONPOFF_F1:.3}\n"
    );
    if corpus_cache.is_empty() {
        println!("(no corpus loaded — run with --corpus for the real gate; fixtures alone can't judge it)\n");
    }

    // --- Sweep the grid ------------------------------------------------------
    println!(
        "{:>11} {:>7}   {:>12} {:>12}   {:>13} {:>13}   {:>7}",
        "stable_self", "lambda", "corpusCOnP", "corpusCOnPOff", "staccatoCOnP", "staccatoCOnPOff", "gate"
    );
    let mut best: Option<(f32, f32, NoteF1)> = None;
    for &stable_self in STABLE_SELF_GRID {
        for &lambda in LAMBDA_GRID {
            let seg: Segmenter = NoteHmm {
                params: HmmParams { stable_self, interval_lambda: lambda, ..HmmParams::default() },
                ..NoteHmm::default()
            };
            let corpus = mean_note_f1(&corpus_cache, &seg, &scorer, &params);
            let stac = score_stem(&fixtures_cache[staccato], &seg, &scorer, &params);

            // Gate: beat the baseline on the expressive corpus (COnP-F1) without
            // regressing the staccato fixture (allow a hair of float noise).
            let beats = !corpus_cache.is_empty() && corpus.conp > BASE_CORPUS_CONP_F1;
            let holds_staccato = stac.conp >= BASE_STACCATO_CONP_F1 - 1e-6;
            let pass = beats && holds_staccato;

            println!(
                "{:>11.3} {:>7.1}   {:>12.3} {:>12.3}   {:>13.3} {:>13.3}   {:>7}",
                stable_self,
                lambda,
                corpus.conp,
                corpus.conpoff,
                stac.conp,
                stac.conpoff,
                if pass { "PASS" } else { "—" }
            );

            if pass {
                match best {
                    Some((_, _, b)) if b.conp >= corpus.conp => {}
                    _ => best = Some((stable_self, lambda, corpus)),
                }
            }
        }
    }

    println!();
    match best {
        Some((ss, lam, f1)) => println!(
            "BEST passing setting: stable_self={ss:.3} lambda={lam:.1}  →  corpus COnP-F1 {:.3} (baseline {:.3}), COnPOff-F1 {:.3} (baseline {:.3})",
            f1.conp, BASE_CORPUS_CONP_F1, f1.conpoff, BASE_CORPUS_CONPOFF_F1
        ),
        None => println!(
            "NO setting beat the baseline COnP-F1 without holding the staccato fixture. The heuristic path stays the default; see the report for why."
        ),
    }

    // --- Secondary sweep: attack_self × pitch_trust (ADR-0010 nominals included) ---
    println!(
        "\n== attack_self × pitch_trust  (held at stable_self={FIXED_STABLE_SELF:.2}, λ={FIXED_LAMBDA:.1}) ==\n"
    );
    println!(
        "{:>11} {:>11}   {:>12} {:>12}   {:>13}   {:>7}",
        "attack_self", "pitch_trust", "corpusCOnP", "corpusCOnPOff", "staccatoCOnP", "gate"
    );
    for &attack_self in ATTACK_SELF_GRID {
        for &pitch_trust in PITCH_TRUST_GRID {
            let seg: Segmenter = NoteHmm {
                params: HmmParams {
                    stable_self: FIXED_STABLE_SELF,
                    interval_lambda: FIXED_LAMBDA,
                    attack_self,
                    pitch_trust,
                    ..HmmParams::default()
                },
                ..NoteHmm::default()
            };
            let corpus = mean_note_f1(&corpus_cache, &seg, &scorer, &params);
            let stac = score_stem(&fixtures_cache[staccato], &seg, &scorer, &params);
            let pass = !corpus_cache.is_empty()
                && corpus.conp > BASE_CORPUS_CONP_F1
                && stac.conp >= BASE_STACCATO_CONP_F1 - 1e-6;
            println!(
                "{:>11.2} {:>11.2}   {:>12.3} {:>12.3}   {:>13.3}   {:>7}",
                attack_self,
                pitch_trust,
                corpus.conp,
                corpus.conpoff,
                stac.conp,
                if pass { "PASS" } else { "—" }
            );
        }
    }
    Ok(())
}

/// Run pYIN + onsets once and package a cached [`Stem`].
fn prepare(
    tracker: &PYin,
    detector: &OnsetDetector,
    name: &str,
    samples: &[f32],
    sample_rate: u32,
    reference: Reference,
) -> Stem {
    let contour = tracker.track(samples, sample_rate);
    let onset_result = detector.detect(samples, sample_rate);
    let onset_times = onset_result.onsets.iter().map(|o| o.time_s).collect();

    let ref_times: Vec<f64> = reference.f0.iter().map(|f| f.time_s).collect();
    let est_f0: Vec<RefF0> = contour
        .iter()
        .map(|f| RefF0 { time_s: f.time_s, frequency_hz: f.frequency_hz, voiced: f.frequency_hz > 0.0 })
        .collect();
    let f0_on_ref = resample_f0_onto(&ref_times, &est_f0);

    Stem {
        name: name.to_string(),
        contour,
        onsets: onset_result.onsets,
        envelope: onset_result.energy_envelope,
        onset_times,
        reference,
        f0_on_ref,
    }
}

/// Note-F1 of one segmenter on one stem.
fn score_stem(stem: &Stem, seg: &Segmenter, scorer: &ConfidenceScorer, params: &MetricParams) -> NoteF1 {
    let segments = seg.segment(&stem.onsets, &stem.contour, &stem.envelope);
    let notes = scorer.score_all(&segments, &stem.envelope);
    let estimate = Estimate {
        onsets: stem.onset_times.clone(),
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

/// Mean note-F1 over a stem set (`0.0` on an empty set).
fn mean_note_f1(stems: &[Stem], seg: &Segmenter, scorer: &ConfidenceScorer, params: &MetricParams) -> NoteF1 {
    if stems.is_empty() {
        return NoteF1::default();
    }
    let mut acc = NoteF1::default();
    for stem in stems {
        let f = score_stem(stem, seg, scorer, params);
        acc.conp += f.conp;
        acc.conpoff += f.conpoff;
    }
    let n = stems.len() as f64;
    NoteF1 { conp: acc.conp / n, conpoff: acc.conpoff / n }
}
