# Spec: Note-HMM segmentation + evaluation harness

Status: ready-for-review

## Goal

Two linked pieces of work, sequenced so the second is provable:

1. **Evaluation harness** — measure transcription quality against labeled vocal data, per stage. ([ADR-0011](../../docs/adr/0011-evaluation-harness-vocadito-metrics.md))
2. **Note-HMM segmentation** — replace the ADR-0007 offset/plateau heuristics with a Tony-style note-HMM over the pitch contour. ([ADR-0010](../../docs/adr/0010-note-hmm-segmentation.md))

The order is load-bearing: the note-HMM's central knob (self-transition probability) is not eye-tunable, and its failure mode (real notes vanishing) is invisible without a recall number. So the harness lands first, records a baseline for the current heuristic segmenter, and every HMM change is judged against it.

See [CONTEXT.md](../../CONTEXT.md), the two ADRs above, and [ADR-0005](../../docs/adr/0005-pyin-hand-rolled-median-filtered.md)/[ADR-0006](../../docs/adr/0006-energy-based-onset-detection.md) for the stages being built on. All work is pure Rust — no ML runtime (ADR-0005).

## Non-goals

- No CREPE / basic-pitch comparison work here (spec tickets 11–12 own that).
- No change to the annotation JSON contract (ADR-0004) or the `Segmenter`/`NoteSegment` seam — the HMM is an internal replacement behind it.
- No new heavy dependencies; metrics are reimplemented in pure Rust in a separate `sona-eval` crate.

## Sequencing

- **Phase A — harness (01–03):** scaffold, metrics, baseline report. Gate.
- **Phase B — note-HMM (04–08):** pitch pre-filter, HMM model, Viterbi + energy split, post-processing + wiring, retune vs baseline.
- **Phase C — confidence (09):** derive sub-scores from HMM posteriors.

Phase B cannot start landing until 03 records a baseline. Phase C follows B.

## Done when

- The harness prints per-stage RPA/RCA, onset-F1, note-F1 (COnP/COnPOff), and a confidence-calibration table on Vocadito + golden fixtures.
- The note-HMM beats the recorded ADR-0007 baseline on note-F1 for expressive vocals **without** regressing the staccato golden fixture.
- The UI-side slope filter and MAD outlier trim are removed (subsumed by the HMM interval prior).
