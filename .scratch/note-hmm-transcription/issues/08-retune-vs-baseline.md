# 08 — Retune against the harness, beat the baseline, remove UI hacks

Status: ready
Blocked by: 07
Phase: B (note-HMM) — **the payoff**

Prove it and clean up.

- Sweep the **self-transition probability** (the one main knob) and interval-penalty λ against Vocadito; watch note-F1 recall/precision so an over-confident prior doesn't silently drop real notes (Ycart's caution).
- Compare to the **03 baseline**. Requirement: **beat baseline note-F1 on expressive vocals without regressing the staccato golden fixture.** If it can't, record why and keep the heuristic path.
- On sign-off, **remove the UI-side slope filter and MAD outlier trim** from `web/lib/view.ts` (the interval prior subsumes them; restores the ADR-0004 boundary), and retire the old heuristic segmenter.
- Consider named presets (legato/staccato) only if the sweep shows one setting can't serve both — backed by F1, not vibes.

Done when: a committed report shows the HMM beating baseline under the stated condition; UI hacks removed; defaults set from the sweep.
