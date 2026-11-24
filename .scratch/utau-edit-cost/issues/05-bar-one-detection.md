# 05 — Find beat 1 of the measure

Status: needs-triage
Type: prototype
Blocked by: 03, audit A02 (export frame), audit A03 (what bar 1 means)

## What

Only if 03 shows bar offsets are common. Estimate which beat starts the measure (downbeat) from the mix
and/or the vocal, and shift sona's tick 0 to it (roadmap 2.3).

- Oracle first: frame pass rate if the bar offset were always right.
- Prototype on dumped intermediates (dev), then port; the port must reproduce the prototype exactly.
- Opt-in until holdout4 confirms it, under a rule committed before the read.

## Comments
