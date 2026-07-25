# 08 — Retune against the harness, beat the baseline, remove UI hacks

Status: done
Blocked by: 07
Phase: B (note-HMM) — **the payoff**

Prove it and clean up.

- Sweep the **self-transition probability** (the one main knob) and interval-penalty λ against Vocadito; watch note-F1 recall/precision so an over-confident prior doesn't silently drop real notes (Ycart's caution).
- Compare to the **03 baseline**. Requirement: **beat baseline note-F1 on expressive vocals without regressing the staccato golden fixture.** If it can't, record why and keep the heuristic path.
- On sign-off, **remove the UI-side slope filter and MAD outlier trim** from `web/lib/view.ts` (the interval prior subsumes them; restores the ADR-0004 boundary), and retire the old heuristic segmenter.
- Consider named presets (legato/staccato) only if the sweep shows one setting can't serve both — backed by F1, not vibes.

Done when: a committed report shows the HMM beating baseline under the stated condition; UI hacks removed; defaults set from the sweep.

## Comments

**Done.** Added `sona-eval-sweep` (caches pYIN + onsets once per stem, re-runs only the segmenter + confidence per grid point) and swept `stable_self` × `interval_lambda` over the 40 Vocadito stems + fixtures. Result: the note-HMM beat the ADR-0007 baseline at **every** grid point while holding the staccato fixture at 1.000. Report: [note-hmm-sweep.md](../../../crates/sona-eval/reports/note-hmm-sweep.md).

- **Shipped default (all four measurement-gated):** `stable_self = 0.95`, `interval_lambda = 2.5`, `attack_self = 0.5`, `pitch_trust = 1.0` → corpus COnP-F1 **0.515 → 0.646** (+25%), COnPOff-F1 **0.348 → 0.508** (+46%). The self-transition shows a clear interior optimum (peaks at 0.95, over-merges above it — Ycart's caution visible); λ is insensitive (octave errors are rare on this corpus). A **secondary sweep** proved ADR-0010's nominal `attack_self=0.9` / `τ=0.1` actually *fail the gate* (below baseline), so — per the ADR's own "not eye-tunable, must be measured" rule — they were set from the harness (0.5 / 1.0 is the study's best), not by eye.
- **UI hacks removed** (`web/lib/view.ts` `pitchSlopeStPerSec` slope filter + `pitchOutlierFlags` MAD trim, and the whole "hide outliers" UI): the interval prior rejects octave artefacts at the source, restoring the ADR-0004 core/UI boundary. `web` typechecks + 44 vitest tests green; verified in-browser (no console errors, outlier UI gone).
- **Heuristic retired:** `Segmenter` is now a type alias for the note-HMM; the ADR-0007 `HeuristicSegmenter` and its plateau code are deleted. Its baseline is preserved in the two reports.
