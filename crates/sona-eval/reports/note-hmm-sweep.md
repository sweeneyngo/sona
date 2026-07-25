# Note-HMM sweep vs. ADR-0007 baseline (ticket 08)

The Phase-B payoff gate. The note-HMM (ADR-0010) replaces the ADR-0007 heuristic
segmentation core; this is the measurement that proves it beats the baseline
before the heuristic is retired.

- **Gate (the spec):** beat baseline note-F1 on the **expressive** corpus
  (Vocadito) **without regressing** the staccato golden fixture.
- **Reproduce:** `cargo run --release -p sona-eval --bin sona-eval-sweep -- --corpus`
- **Data:** the 40 Vocadito stems + the four committed golden fixtures.
- **Metric:** ticket-02 note-F1, `mir_eval` conventions (COnP = onset+pitch;
  COnPOff = +offset), same as [baseline.md](baseline.md).
- Upstream (pYIN, energy onsets) is identical to the baseline and independent of
  the HMM knobs, so it is computed once per stem; only the segmenter + confidence
  re-run per grid point.

## Result: the note-HMM wins across the board

| setting (stable_self / λ) | corpus COnP-F1 | corpus COnPOff-F1 | staccato COnP | staccato COnPOff |
|---|---|---|---|---|
| **baseline (ADR-0007 heuristic)** | **0.515** | **0.348** | 1.000 | 1.000 |
| 0.90 / 1.5 | 0.627 | 0.487 | 1.000 | 1.000 |
| 0.90 / 2.5 | 0.628 | 0.489 | 1.000 | 1.000 |
| 0.90 / 4.0 | 0.629 | 0.490 | 1.000 | 1.000 |
| **0.95 / 1.5** | **0.633** | **0.495** | 1.000 | 1.000 |
| **0.95 / 2.5** | **0.633** | **0.494** | 1.000 | 1.000 |
| **0.95 / 4.0** | **0.633** | **0.494** | 1.000 | 1.000 |
| 0.98 / 1.5 | 0.624 | 0.486 | 1.000 | 1.000 |
| 0.98 / 2.5 | 0.627 | 0.489 | 1.000 | 1.000 |
| 0.98 / 4.0 | 0.624 | 0.485 | 1.000 | 1.000 |
| 0.99 / 1.5 | 0.622 | 0.485 | 1.000 | 1.000 |
| 0.99 / 2.5 | 0.620 | 0.482 | 1.000 | 1.000 |
| 0.99 / 4.0 | 0.612 | 0.476 | 1.000 | 1.000 |
| 0.995 / 1.5 | 0.613 | 0.477 | 1.000 | 1.000 |
| 0.995 / 2.5 | 0.607 | 0.472 | 1.000 | 1.000 |
| 0.995 / 4.0 | 0.609 | 0.474 | 1.000 | 1.000 |

**Every** setting cleared the gate: it beat the baseline COnP-F1 on the corpus
and held the staccato fixture at 1.000. `stable_self = 0.95` is the interior
optimum; λ is essentially flat. (This grid was run at `attack_self = 0.6`,
`pitch_trust = 1.0`; the two are refined below.)

## Secondary sweep: `attack_self` × `pitch_trust` (held at stable_self=0.95, λ=2.5)

ADR-0010 gives `attack_self ≈ 0.9` and trust exponent `τ ≈ 0.1` as literature
starting points and is explicit that the model **"is not eye-tunable and must be
gated by measurement."** So these two were swept too — with the ADR nominals *in
the grid* — rather than set by eye.

| attack_self / pitch_trust | corpus COnP-F1 | corpus COnPOff-F1 | staccato COnP | gate |
|---|---|---|---|---|
| 0.50 / 0.10 | 0.479 | 0.348 | 1.000 | — |
| **0.50 / 1.00** | **0.646** | **0.508** | 1.000 | PASS |
| 0.50 / 2.00 | 0.640 | 0.495 | 1.000 | PASS |
| 0.60 / 1.00 | 0.633 | 0.494 | 1.000 | PASS |
| 0.75 / 1.00 | 0.540 | 0.412 | 1.000 | PASS |
| 0.90 / 0.10 *(ADR nominal)* | 0.366 | 0.260 | 1.000 | — |
| 0.90 / 1.00 | 0.405 | 0.286 | 1.000 | — |
| 0.90 / 2.00 | 0.439 | 0.309 | 1.000 | — |

(full 4×4 grid in the binary output.)

**ADR-0010's nominal values fail the gate.** `attack_self = 0.9` scores
0.37–0.44 corpus COnP-F1 — *below* the 0.515 heuristic baseline — at every trust
value: a high Attack self-prob leaves the decode stuck in Attack (its emission
coincides with Stable on a clean on-pitch contour), so note boundaries blur.
`τ = 0.1` likewise lands at/below the baseline (0.37–0.48): the emission is too
flat for the pitch to place a boundary. The measured optimum is **`attack_self =
0.5`, `pitch_trust = 1.0`** → corpus COnP-F1 **0.646**, COnPOff-F1 **0.508** — the
best of the whole study.

## Chosen default (all four, measurement-gated)

`stable_self = 0.95`, `interval_lambda = 2.5`, `attack_self = 0.5`,
`pitch_trust = 1.0` → corpus **COnP-F1 0.646 (+0.131 / +25% over baseline)**,
**COnPOff-F1 0.508 (+0.160 / +46%)**, staccato fixture held at 1.000.

## What the numbers say

1. **The HMM beats the baseline decisively, and offset accuracy improves most.**
   At the chosen default: COnP-F1 **0.515 → 0.646** (+0.131, +25%) and COnPOff-F1
   **0.348 → 0.508** (+0.160, +46%). The baseline named offset placement the
   single weakest link (COnPOff ≪ COnP); the Viterbi note model closes most of
   that gap, exactly where ADR-0010 predicted a note model would pay off.

2. **The self-transition has a clear interior optimum.** note-F1 rises from
   0.90, peaks at **0.95**, then falls as the prior gets over-confident (0.99,
   0.995) and starts merging real notes — Ycart's caution made visible. This is
   the one knob ADR-0010 promised, and it behaves like one.

3. **The interval-prior λ barely matters here.** note-F1 is flat (±0.001) across
   λ ∈ {1.5, 2.5, 4.0} at the optimum. Consistent with the baseline's finding
   that octave errors are rare on this corpus (RCA ≈ RPA): the interval prior's
   value is in **boundary placement**, not octave fixing. λ is kept at a middle
   2.5 so a meaningful octave penalty remains for material Vocadito doesn't cover
   (the interval prior is still what lets the UI-side slope/MAD hacks be removed).

## Decision

The note-HMM is the default segmenter (already wired behind the `Segmenter`
seam, ticket 07). On this sign-off:

- **Defaults set from the sweep** (all four measurement-gated, not by eye):
  `stable_self = 0.95`, `interval_lambda = 2.5`, `attack_self = 0.5`,
  `pitch_trust = 1.0`. ADR-0010's nominal `attack_self`/`τ` were measured to fail
  the gate; the ADR's own rule — "not eye-tunable, must be gated by measurement"
  — is why they were swept rather than assumed.
- **UI perception hacks removed** (`web/lib/view.ts` slope filter + MAD outlier
  trim): the interval prior rejects octave artefacts at the source, so the
  interpretation layer no longer needs to (restores the ADR-0004 boundary).
- **The ADR-0007 heuristic segmenter is retired.** The baseline it produced is
  preserved here and in [baseline.md](baseline.md).
