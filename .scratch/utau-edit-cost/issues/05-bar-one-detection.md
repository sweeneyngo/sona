# 05 — Find beat 1 of the measure

Status: needs-triage
Type: prototype
Blocked by: audit A02 (export frame), 10

## What

Only if 03 shows bar offsets are common. Estimate which beat starts the measure (downbeat) from the mix
and/or the vocal, and shift sona's tick 0 to it (roadmap 2.3).

- Oracle first: frame pass rate if the bar offset were always right.
- Prototype on dumped intermediates (dev), then port; the port must reproduce the prototype exactly.
- Opt-in until holdout4 confirms it, under a rule committed before the read.

## Comments

- 2026-11-24 (after A03, `crates/sona-eval/reports/bar-one.md`): scope sharpened. (1) **Beat:** the vocals'
  cell line nearest `grid_phase(mix, 60/BPM)`; matches the drums' beat on 9/9 dev songs. (2) **Bar:** the
  beat (mod 4) with the most harmonic change in chroma(mix − vocals); a prototype only, no ground truth,
  weak on yamineko, fukagyakuteki, atamaro, amaten and the 236–250 BPM songs. (3) Tick 0 = that bar line
  at or before 0 s; the export starts the audio that much after tick 0 (A02's frame). Opt-in; confirm on
  holdout4 against **ear-labelled** beat and bar 1, not the UST's grid.
