# Baseline — ADR-0007 heuristic pipeline

The gate for the note-HMM work (ticket 03). Every Phase-B change is measured
against these numbers; the note-HMM must beat note-F1 on expressive vocals
**without** regressing the staccato fixture.

- **Producer:** current ADR-0007 pipeline — pYIN → energy onsets → heuristic
  segmentation → confidence (`Transcriber::default()`, `PYin::default()`).
- **Metrics:** ticket-02 families, `mir_eval` conventions (±50 ms onset, ±50-cent
  pitch, offset `max(20 % dur, 50 ms)`).
- **Data:** the 40 **Vocadito** stems (git-ignored, fetched via
  `scripts/fetch-vocadito.sh`) plus the four committed golden fixtures.
- **Reproduce:** `cargo run -p sona-eval --bin sona-eval-report` (add `--corpus`).

## Vocadito corpus (40 real vocal stems) — the primary gate

| metric | mean | min | max |
|--------|------|-----|-----|
| RPA | 0.858 | 0.601 | 0.978 |
| RCA | 0.863 | 0.667 | 0.978 |
| Voicing recall | 0.887 | 0.703 | 0.984 |
| Voicing false-alarm | 0.063 | 0.001 | 0.188 |
| **onset F1** | **0.565** | 0.103 | 0.795 |
| **COnP F1** (note: onset+pitch) | **0.515** | 0.161 | 0.824 |
| **COnPOff F1** (+offset) | **0.348** | 0.097 | 0.612 |

This is the number Phase B must beat: **COnP-F1 0.515 / COnPOff-F1 0.348** on
Vocadito, without regressing the staccato fixture (below).

Three things the corpus shows that the synthetic fixtures could not:

1. **Note quality is the bottleneck, and offset most of all.** Nearly half of
   true notes are missed or mis-transcribed (COnP-F1 0.515); requiring the offset
   too nearly halves the score again (COnPOff-F1 0.348). Offset precision is the
   single weakest link.
2. **Pitch tracking is not the problem, and octave errors barely exist.** RPA is
   0.858 and RCA is only 0.006 above it — pYIN is rarely an octave out on this
   corpus. ADR-0010's octave-error motivation for the interval prior is *weakly*
   supported by this baseline; the interval prior's value here is more likely in
   note **boundary** placement than in fixing octave slips.
3. **Missing notes are *not* lost to segmentation over-merging.** `note-drop =
   onset-recall − COnP-recall` is **negative** (mean −0.101; negative on 33 of 40
   stems, positive on only 4, all ≤ 0.08). Note recall (0.542) *exceeds* raw
   onset recall (0.441): the pipeline produces more correct notes than the energy
   onset detector produces onsets, because the plateau segmenter also births notes
   from the **pitch contour**, not only from detected onsets. So the energy onset
   detector is the *weaker* note-birth signal, and segmentation is net-additive —
   it is not throwing notes away by merging them.

## Golden fixtures (fast regression signal)

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

**Answer (on the corpus): missing notes are *not* lost to segmentation
over-merging. Segmentation is net-additive; the ceiling on note quality is the
accuracy of the pitch-contour → note step (and, hardest of all, offset placement)
— not onset recall, and not pitch tracking.**

The onset metric scores the **raw onset detector** (stage 2) against reference
note onsets — read straight from `OnsetDetector::detect`, *not* the
post-segmentation notes — so `note-drop = onset-recall − COnP-recall` compares two
independent note-birth signals.

- On the **corpus**, `note-drop` is **negative** (mean −0.101). Note recall
  (0.542) exceeds onset recall (0.441): the plateau segmenter births notes from
  the pitch contour, so it recovers notes the energy onset detector never flags.
  Over-merging (which would show a *positive* drop) appears on only 4 of 40 stems
  and never above 0.08. Segmentation is therefore *not* where notes vanish.
- The **fixtures** looked like the opposite (`note-drop = 0.000`, legato losing
  everything at onset-recall 0.000) — but that was an artifact of synthetic
  cleanliness: their notes happen to be born one-to-one from onsets, so the two
  signals can't diverge. The corpus is the trustworthy read; the fixtures remain
  the fast regression wall, not the diagnostic.

So Phase B (ADR-0010's note-HMM, replacing the plateau segmenter) is aimed at the
right stage — it upgrades the pitch-contour → note step that is already doing the
load-bearing work. The largest single headroom is **offset accuracy**
(COnPOff-F1 0.348 vs COnP-F1 0.515); better note-boundary placement is where a
Viterbi note model should pay off most. The energy onset detector's weakness
(onset-F1 0.565) is a real but *secondary* finding — worth its own look, but not
the binding constraint on note recall, because segmentation already compensates.

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
