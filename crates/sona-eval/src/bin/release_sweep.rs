//! **Note-offset Release sweep** — the payoff gate for [ADR-0012].
//!
//!     cargo run --release -p sona-eval --bin sona-eval-release-sweep -- --corpus
//!
//! [ADR-0012] adds a `Release` sub-state so a note's offset lands at the energy
//! fall. Its defaults are eye-plausible placeholders; this binary is the gate
//! that decides whether they earn their place, on the same corpus and machine.
//!
//! The comparison is a clean A/B on the *identical* pipeline: the only thing that
//! changes is whether the decay path is enabled. The **baseline** is the ADR-0010
//! model — Release disabled via `release_drop_threshold > 1` — scored here, now,
//! rather than trusting a previously recorded number. Each grid point then enables
//! the decay path and re-scores.
//!
//! The gate (per ADR-0012), all three:
//!  1. corpus **COnPOff-F1 improves** over the Release-off baseline,
//!  2. corpus **COnP-F1 does not regress** (offset changes must not cost onsets),
//!  3. the **staccato golden fixture** does not regress.
//!
//! Upstream stages (pYIN, energy onsets, envelope) don't depend on the segmenter
//! params, so they are computed once per stem and cached; only the segmenter +
//! confidence re-run per grid point.
//!
//! [ADR-0012]: ../../../docs/adr/0012-note-offset-release-state.md

use std::process::ExitCode;

use sona_core::hmm::HmmParams;
use sona_core::onset::OnsetDetector;
use sona_core::pitch::{PYin, PitchTracker};
use sona_core::{ConfidenceScorer, NoteHmm, Segmenter};

use sona_eval::metrics::{self, MetricParams};
use sona_eval::reference::Reference;
use sona_eval::{fixtures, resample_f0_onto, vocadito, wav, EstNote, Estimate, RefF0};

/// A `release_drop_threshold` above any attainable `drop_steepness` (which is
/// clamped to `0..=1`), so no frame is ever flagged — the ADR-0010 baseline.
const RELEASE_DISABLED_THRESHOLD: f32 = 2.0;

/// Energy-fall bonus grid (nats) — how strongly a steep fall pulls the decode
/// into `Release`. Includes the placeholder default (6.0).
const RELEASE_BONUS_GRID: &[f32] = &[3.0, 6.0, 9.0, 12.0];
/// Energy-fall steepness threshold grid — how sharp a drop must be to flag a
/// release. Lower fires more readily. Includes the placeholder default (0.5).
const RELEASE_THRESHOLD_GRID: &[f32] = &[0.35, 0.5, 0.65];

fn main() -> ExitCode {
    let with_corpus = std::env::args().any(|a| a == "--corpus");
    match run(with_corpus) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-release-sweep: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One stem with its upstream stages already computed — the segmenter consumes
/// only these, so they are cached across the whole grid.
struct Stem {
    name: String,
    contour: Vec<sona_core::F0Frame>,
    onsets: Vec<sona_core::Onset>,
    envelope: Vec<sona_core::EnergyFrame>,
    onset_times: Vec<f64>,
    reference: Reference,
    f0_on_ref: Vec<RefF0>,
}

/// Mean note-F1 over a set of stems.
#[derive(Clone, Copy, Default)]
struct NoteF1 {
    conp: f64,
    conpoff: f64,
}

/// Build a `NoteHmm` with the decay path either disabled (baseline) or set to a
/// grid point.
fn segmenter(release: Option<(f32, f32)>) -> Segmenter {
    match release {
        None => NoteHmm { release_drop_threshold: RELEASE_DISABLED_THRESHOLD, ..NoteHmm::default() },
        Some((bonus, threshold)) => NoteHmm {
            params: HmmParams { release_offset_bonus: bonus, ..HmmParams::default() },
            release_drop_threshold: threshold,
            ..NoteHmm::default()
        },
    }
}

fn run(with_corpus: bool) -> Result<(), sona_eval::EvalError> {
    let tracker = PYin::default();
    let detector = OnsetDetector::default();
    let params = MetricParams::default();
    let scorer = ConfidenceScorer::default();

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

    // --- Baseline: Release disabled, scored here on this corpus --------------
    let base_seg = segmenter(None);
    let base_corpus = mean_note_f1(&corpus_cache, &base_seg, &scorer, &params);
    let base_stac = score_stem(&fixtures_cache[staccato], &base_seg, &scorer, &params);

    println!("== Note-offset Release sweep vs. Release-off baseline (ADR-0012) ==\n");
    if corpus_cache.is_empty() {
        println!("(no corpus loaded — run with --corpus for the real gate; fixtures alone can't judge it)\n");
    }
    println!(
        "baseline (Release OFF):  corpus COnP-F1 {:.3}  COnPOff-F1 {:.3}   |   staccato COnP-F1 {:.3}  COnPOff-F1 {:.3}\n",
        base_corpus.conp, base_corpus.conpoff, base_stac.conp, base_stac.conpoff
    );

    // --- Sweep the release knobs --------------------------------------------
    println!(
        "{:>6} {:>10}   {:>11} {:>13} {:>10}   {:>13} {:>15}   {:>7}",
        "bonus", "threshold", "corpusCOnP", "corpusCOnPOff", "ΔCOnPOff", "staccatoCOnP", "staccatoCOnPOff", "gate"
    );
    let mut best: Option<(f32, f32, NoteF1)> = None;
    for &bonus in RELEASE_BONUS_GRID {
        for &threshold in RELEASE_THRESHOLD_GRID {
            let seg = segmenter(Some((bonus, threshold)));
            let corpus = mean_note_f1(&corpus_cache, &seg, &scorer, &params);
            let stac = score_stem(&fixtures_cache[staccato], &seg, &scorer, &params);
            let delta_conpoff = corpus.conpoff - base_corpus.conpoff;

            // Gate: COnPOff up, COnP not down, staccato held (allow float noise).
            let improves_offsets = !corpus_cache.is_empty() && corpus.conpoff > base_corpus.conpoff + 1e-6;
            let holds_conp = corpus.conp >= base_corpus.conp - 1e-6;
            let holds_staccato =
                stac.conp >= base_stac.conp - 1e-6 && stac.conpoff >= base_stac.conpoff - 1e-6;
            let pass = improves_offsets && holds_conp && holds_staccato;

            println!(
                "{:>6.1} {:>10.2}   {:>11.3} {:>13.3} {:>+10.3}   {:>13.3} {:>15.3}   {:>7}",
                bonus,
                threshold,
                corpus.conp,
                corpus.conpoff,
                delta_conpoff,
                stac.conp,
                stac.conpoff,
                if pass { "PASS" } else { "—" }
            );

            if pass {
                match best {
                    Some((_, _, b)) if b.conpoff >= corpus.conpoff => {}
                    _ => best = Some((bonus, threshold, corpus)),
                }
            }
        }
    }

    println!();
    match best {
        Some((bonus, threshold, f1)) => println!(
            "BEST passing setting: release_offset_bonus={bonus:.1} release_drop_threshold={threshold:.2}  →  corpus COnPOff-F1 {:.3} (baseline {:.3}, {:+.3}), COnP-F1 {:.3} (baseline {:.3})",
            f1.conpoff, base_corpus.conpoff, f1.conpoff - base_corpus.conpoff, f1.conp, base_corpus.conp
        ),
        None => println!(
            "NO release setting improved corpus COnPOff-F1 while holding COnP-F1 and the staccato fixture.\nThe decay path stays available but its default should be conservative (or disabled) pending the target-domain corpus — see ADR-0012."
        ),
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
