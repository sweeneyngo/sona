# 19 — Train the cell stage on edit cost

Status: claimed
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

### Selection rule (committed before any CV run; lead agent, 2026-11-24)

Data: every spent split's songs that `sona-eval-cellstage` scores (dev 9, holdout 9, holdout2 10, holdout3 10 =
38), read through `--split NAME --reread --reason …` (logged in `corpus/holdout-reads.jsonl` as diagnosis).
holdout4 is never read. Training rows are the Rust feature builder's own (`--dump-cells`), so a trained model
reads exactly what sona gives it.

- **Exploration CV** (fold assignment E: 4 folds by song, permutation seed 2): free to compare recipes and pick
  one. Its numbers are exploration only.
- **Final CV** (fold assignment F: K folds by song, permutation seed 1, K stated below before the run), run
  once for:
  - **B**, the current recipe retrained under F: the prototype's 1046 features, CE on the first candidate
    with the UST's pitch, the UST's true phase, HID 96, 15 epochs, seed 0, masked argmax;
  - **C**, the one candidate recipe, named below before the run.
- Each fold's model scores only its held-out songs, through the Rust scorer
  (`sona-eval-cellstage --edit-cost --fold-models DIR`, the stage on the onset phase, which ships).
- **C is kept (opt-in model dir) only if**, pooled over the 38 songs:
  1. the paired song-bootstrap (10 000 resamples) 95% upper bound of Δ cost/min (C − B) is below 0;
  2. the missing count does not rise, or the upper bound stays below 0 with missing re-weighted to 4;
  3. clean measures do not fall (point Δ ≥ 0);
  4. any decoding parameter C has is chosen cross-fitted (for each fold, by the other folds' out-of-sample
     cost), never on the fold it scores.
- Noise floor: B retrained with seed 1 under F, against B, reported beside the result.
- Cell accuracy is reported, not a criterion. The shipped default stays the current model either way; a C
  that passes stays opt-in until holdout4.
- **K = 10** (timing: one epoch on 20 songs takes 12 s on 8 threads, so a 15-epoch model on ~34 songs takes
  about 5 min; leave-one-song-out would need 38 models per recipe, ≈ 3.5 h each, and B + C + B's seed check
  ≈ 10 h. Ten folds take ≈ 50 min per recipe.)
- **C, named after the exploration CV and before the final CV run**: the same network trained on the onset
  phase (what ships) with GAME lead's 8 residual features (1054 per cell), loss = cross-entropy with cells
  next to a UST note edge weighted 3, plus 0.5 × a pairwise term (−log of the chance two neighbouring cells
  of one UST note agree on pitch); decoded with `decode_classes` at a switch cost chosen cross-fitted from
  {0, 0.25, 0.5, 1, 1.5, 2, 3, 4} (export: `switch_cost`, `lead_residuals`). Exploration (4 folds, seed 2,
  38 songs, vs B under the same folds; 6 recipes × decodings looked at, so its numbers carry selection):
  −18.0 [−23.7, −12.7] cost/min at switch 1.0, clean +1.6, missing +18.
