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

## Selection rule (declared 2026-11-24, before the Rust sweep)

Dev only (9 songs); no holdout of any kind is read. Revised weights (`spec.md`). Baseline reproduced first:
GAME lead + octave **303.0** cost/min (out of sample, decides), learned stage **272.2** (in-sample, shown).
Candidates read only audio evidence (GAME's raw notes on the lead and vocals, sona's grid notes, the pitch
track, the vocals' or lead stem's energy) and the grid; the UST is read only by the oracles and the scoring.

A candidate is ported (opt-in) only if, on dev GAME lead:

1. the paired song-bootstrap (10 000 resamples) 95% upper bound of Δ cost/min is below 0;
2. clean measures do not fall (point estimate Δ ≥ 0);
3. it does not win by dropping notes: the missing count does not rise, or, if it does, the upper bound of
   Δ cost/min stays below 0 with missing re-weighted from 2 to 4 (a dropped note then costs as much as an
   off-time one);
4. a tuned parameter is chosen leave-one-song-out, and the leave-one-song-out result also has its upper
   bound below 0.

If it raises the stage's cost (point estimate), it is for GAME notes only, as 12's trim is. Among candidates
that pass, the lowest GAME-lead cost/min is ported; passing candidates are then measured together. A port
reproduces the prototype's per-song class counts exactly, leaves the default run byte-identical, and has
behaviour tests through the public interface with a mutant shown to be caught. Negative results are recorded
with their numbers; knobs that earn nothing are deleted.

## Comments
