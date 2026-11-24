# 12 — Start, end and pitch errors on single notes

Status: claimed
Type: research
Blocked by: 07

## What

03: errors on single notes (length, shift, pitch) are 19–26% of GAME lead's cost. Each is cheap to fix
in OpenUtau (weight 1), but there are many. Break them down by cause (snapping to the wrong cell, a late
consonant onset, a release, an octave) after 07 changes the note boundaries, since 07 moves them.

## Selection rule (declared 2026-11-24, before measuring)

Work on the default output (07 left the default boundaries unchanged). Dev only for decisions; the spent splits
(holdout, holdout2, holdout3) may be re-read from 03's dumps for diagnosis only, logged in the ledger as a
re-analysis. holdout4 / holdout4-tempo are not touched.

- Baseline to reproduce first: dev 180.2 (stage, in-sample) / 188.7 (GAME lead, the out-of-sample reference) /
  338.4 (pYIN) cost/min.
- For each candidate cause (wrong-cell snap / late consonant onset, release / offset placement, min-length or
  merge rules, octave, semitone from vibrato centre or scoop, the UST author's conventions): measure the cost an
  oracle fix removes and whether evidence available at transcription time predicts it.
- **Port** a mechanism only if dev GAME-lead pooled cost/min drops with a paired song-bootstrap 95% interval
  (10 000 resamples) on the difference that excludes 0, the stage's pooled cost/min does not rise, and neither
  output's pooled clean-measure share falls.
- A port is opt-in (default output byte-identical), reproduces the prototype's per-song class counts exactly,
  has behaviour tests through the public interface and a mutant shown to be caught.
- Leave chopped/joined to 07/11. Negative results are recorded with their numbers.

## Comments
