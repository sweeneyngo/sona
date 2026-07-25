# Baseline — ADR-0007 heuristic pipeline

The gate for the note-HMM work (ticket 03). Every Phase-B change is measured
against these numbers; the note-HMM must beat note-F1 on expressive vocals
**without** regressing the staccato fixture.

- **Producer:** current ADR-0007 pipeline — pYIN → energy onsets → heuristic
  segmentation → confidence (`Transcriber::default()`, `PYin::default()`).
- **Metrics:** ticket-02 families, `mir_eval` conventions (±50 ms onset, ±50-cent
  pitch, offset `max(20 % dur, 50 ms)`).
- **Data:** the four committed golden fixtures. **Vocadito was not run** in this
  environment — the corpus is git-ignored and needs the fetch script; the harness
  scores it under `--corpus` once `SONA_VOCADITO_DIR` is populated.
- **Reproduce:** `cargo run -p sona-eval --bin sona-eval-report` (add `--corpus`).

## Golden fixtures

| fixture  | RPA   | RCA   | Vx-Rec | Vx-FA | onset F1 | COnP F1 | COnPOff F1 |
|----------|-------|-------|--------|-------|----------|---------|------------|
| legato   | 0.925 | 0.925 | 0.936  | 0.000 | **0.000**| **0.000**| **0.000** |
| staccato | 0.874 | 0.874 | 0.874  | 0.086 | 1.000    | 1.000   | 1.000      |
| vibrato  | 0.942 | 0.942 | 0.942  | 0.111 | 1.000    | 1.000   | 1.000      |
| bleed    | 0.961 | 0.961 | 0.961  | 0.016 | 1.000    | 1.000   | 1.000      |

Confidence calibration: every emitted note landed in the top bucket at error
0.000 (the pipeline is confident and, where it emits notes, correct on these
fixtures) — so the calibration signal is uninformative here and needs Vocadito
to exercise the low-confidence buckets.

## The standing question: onsets (stage 2) or segmentation (stage 3)?

**Answer (on the fixtures): missing notes are an onset-recall failure, not
over-merging in segmentation.**

The onset metric here scores the **raw onset detector's output** (stage 2)
against the reference note onsets — read straight from `OnsetDetector::detect`,
*not* derived from the pipeline's post-segmentation notes. That is what makes the
onset-vs-note split able to separate the two stages at all: if it read note
onsets, both columns below would be the same list and could never disagree.

Two lines of evidence:

1. **`note-drop = onset-recall − COnP-recall = 0.000 on every fixture.** Every
   onset the detector found became a matching note; no notes are lost between
   stage 2 and stage 3. (Over-merging would show as onset-recall *above*
   note-recall — a positive drop — which never occurs.)

   | stem     | onset-R (stage 2) | COnP-R (stage 3) | note-drop |
   |----------|-------------------|------------------|-----------|
   | legato   | 0.000             | 0.000            | 0.000     |
   | staccato | 1.000             | 1.000            | 0.000     |
   | vibrato  | 1.000             | 1.000            | 0.000     |
   | bleed    | 1.000             | 1.000            | 0.000     |
   | **mean** | 0.750             | 0.750            | 0.000     |

2. **The one fixture that loses notes (legato) loses them at stage 2.** pYIN
   tracks its pitch fine (RPA 0.925), but the detector's **onset-recall is 0.000**
   — it fires no onset within ±50 ms of either reference note onset on the
   smoothly-connected legato phrase — so segmentation has nothing to turn into
   notes and the pipeline emits zero. The failure is measured *at* the onset
   stage, not inferred from the empty note list.

Caveat: these are synthetic fixtures, and only one (legato) actually loses notes,
so the segmentation-over-merge failure mode is present in the data but never
*triggered* here. Vocadito would exercise it on real expressive vocals. The
mechanism the harness now measures is sound — a `note-drop` near zero rules
segmentation *out* as the cause of *missing* notes — and it points Phase B at how
notes are *born* (onset recall), not how they're merged.

## Harness fidelity (`mir_eval` 0.8.2, offline)

The pitch metric was cross-checked against `mir_eval` on these real pipeline
estimates:

- **Metric alone** (mir_eval fed sona's already-aligned estimate): **0.0 abs
  diff** on all four fixtures — RPA/RCA/VR/VFA match exactly.
- **Resampler + metric** (mir_eval resampling the raw estimate itself): agrees to
  **≤ 0.078**, always with mir_eval scoring *higher*. The gap is the deliberate
  tail-handling choice (`align::resample_f0_onto`): pYIN's 2048-sample window
  can't reach the last ~8 frames of each stem, and where `mir_eval` extrapolates
  the last pitch across them, sona marks them unvoiced. Our choice is uniformly
  **stricter** (it can only lower pitch scores, never inflate them) and is applied
  identically to every producer, so baseline comparisons stay fair.
