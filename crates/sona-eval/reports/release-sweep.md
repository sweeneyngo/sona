# Note-offset Release sweep (ADR-0012 gate)

The Release sub-state and this sweep's binary were removed in audit A07 (2026-11-24); this report is the record. Restore them from the parent of the A07 change to re-run.

Run: `cargo run --release -p sona-eval --bin sona-eval-release-sweep -- --corpus` (bin removed in audit A07, 2026-11-24; restore it from the commit before A07 to rerun)
Corpus: Vocadito, 40 stems (Zenodo 5578807). A/B on the identical pipeline — the only variable is whether the decay path (`Stable → Release`) is enabled.

## Baseline (Release OFF)

Scored here, this run — not a trusted prior number. It **reproduces the recorded ADR-0010 result exactly**, which validates the A/B:

| | corpus COnP-F1 | corpus COnPOff-F1 | staccato COnP | staccato COnPOff |
|---|---|---|---|---|
| Release OFF | **0.646** | **0.508** | 1.000 | 1.000 |

## Sweep (Release ON) — `release_offset_bonus × release_drop_threshold`

| bonus | threshold | corpus COnP | corpus COnPOff | ΔCOnPOff | staccato COnP | staccato COnPOff | gate |
|---|---|---|---|---|---|---|---|
| 3  | 0.35 | 0.597 | 0.417 | −0.091 | 1.000 | 1.000 | — |
| 3  | 0.50 | 0.634 | 0.474 | −0.034 | 1.000 | 1.000 | — |
| 3  | 0.65 | 0.647 | 0.495 | −0.013 | 1.000 | 1.000 | — |
| 6  | 0.35 | 0.589 | 0.408 | −0.099 | 1.000 | 1.000 | — |
| 6  | 0.50 | 0.632 | 0.478 | −0.030 | 1.000 | 1.000 | — |
| 6  | 0.65 | 0.652 | 0.503 | −0.005 | 1.000 | 1.000 | — |
| 9  | 0.35 | 0.587 | 0.405 | −0.102 | 1.000 | 1.000 | — |
| 9  | 0.50 | 0.635 | 0.480 | −0.028 | 1.000 | 1.000 | — |
| 9  | 0.65 | 0.654 | 0.506 | −0.001 | 1.000 | 1.000 | — |
| 12 | 0.35 | 0.587 | 0.405 | −0.103 | 1.000 | 1.000 | — |
| 12 | 0.50 | 0.635 | 0.481 | −0.027 | 1.000 | 1.000 | — |
| 12 | 0.65 | 0.656 | 0.508 | −0.000 | 1.000 | 1.000 | — |

## Verdict: gate FAILS on Vocadito → ship Release OFF by default

No setting improves corpus COnPOff-F1. The least-aggressive (bonus 12 / threshold 0.65) is **neutral** on offsets (Δ −0.000) and mildly helps COnP (+0.010); every more-eager setting **hurts** offsets, down to 0.417. The staccato fixture holds 1.000 everywhere.

This is a real negative result on real expressive singing — not a distribution to explain away. The mechanism can only tighten an offset where a note ends in a **sharp RMS fall**, and most singing doesn't: expressive offsets are soft decays, breaths, or pitch-transitions with no steep fall. So the flag either never fires (no help) or fires on mid-note dynamics / vibrato dips and clips real notes (the harm above). That is an intrinsic ceiling of an energy-fall-driven release, not a Vocadito quirk.

**Decision (ADR-0012):** the Release path ships **disabled** (`NoteHmm::release_drop_threshold = f32::INFINITY`), so default behaviour is exactly the proven ADR-0010. It ships off because the evidence says a pure-DSP energy-fall release is not the offset fix — no favorable-corpus rescue is assumed. The capability + knobs are retained and unit-tested (harmless off; may still help genuinely sharp-cutoff input); enabling is a one-line threshold flip (`~0.65`), with the measured least-harmful params (bonus 12) as the enable defaults.

Monotonicity note: as the threshold falls (more frames flagged) both metrics degrade smoothly, confirming the harm is over-eager release-flagging — the flag catching things that aren't note ends.
