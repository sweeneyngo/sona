# 18 — Off-time starts and ends under the revised weights

Status: claimed
Type: prototype
Blocked by: —

## What

The user revised the weights (2026-11-24, `spec.md`): an off-time start or end costs 4, a near-miss pitch 3.
Rescored on dev, timing (length + shift + moved) is 45% of GAME lead's 303.0 cost/min and 40% of the
stage's 272.2. Oracles (`scripts/edit-cost-taxonomy/taxo.py`, new weights): fixing every length pair cuts
44% (lead) / 39% (stage); every shift/moved pair 24% / 27%.

1. Taxonomy of the timing errors on both outputs: start vs end, direction, size in cells, context (legato
   boundary, before a rest, after a rest, phrase start), note length, and which source the stage took
   the boundary from. Oracles per slice (for example, only one-cell errors; only ends before a rest).
2. Prototype fixes from audio evidence and nothing that reads the UST: energy offsets (12's release trim
   on the stage, retuned for the new weights), onsets on the lead stem, agreement between GAME and the
   stage's boundaries, or anything the taxonomy points to.

Lyric-based timing (the aligner's mora onsets) belongs to lyrics-first 16; leave it out.

## Acceptance

- Selection rule committed before the Rust sweep.
- A report with the taxonomy, oracles and candidates. Show the missing-note count beside each cost,
  since an off-time note (4) now costs more than deleting and redrawing it (3).
- Default unchanged; anything that ships is opt-in until holdout4.

## Comments
