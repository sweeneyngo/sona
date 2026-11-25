# 19 — Train the cell stage on edit cost

Status: ready-for-agent
Type: prototype
Blocked by: 18

## What

18 found that one-cell jitter at legato boundaries dominates the timing cost, that no audio source says
which way it goes, and that GAME's own rounding residual does (a boundary whose raw time is 0.4–0.5 cell
from its cell is wrong 40% of the time, and 84% of those lean toward the UST). A better choice among the
sources the stage already sees has a ceiling of −41.7 cost/min (in sample). The stage was trained for
cell accuracy, which charges a one-cell boundary slip as one cell; the user's revised weights charge it
up to 12.

Retrain the stage (`~/.cache/sona/cellstage-D/build_cells.py`, `train_cells.py`; ADR-0026) with:
- a loss or decoding that charges boundary errors at their edit cost (start/end off 4, near-miss pitch
  3, octave 1, chopped/joined 4, missing 2, extra 1);
- GAME lead's rounding residual at each edge as a feature;
- song-level cross-validation over every spent split (dev, holdout, holdout2, holdout3), scoring each fold
  with the revised-weight edit cost; the current recipe retrained under the same folds is the baseline.

## Acceptance

- Selection rule committed before the CV run.
- CV table: pooled cost/min, clean measures, missing count, paired song-bootstrap intervals against the
  current recipe under the same folds.
- A winning model stays opt-in (`--model` dir) until the holdout4 read; weights stay local.

## Comments
