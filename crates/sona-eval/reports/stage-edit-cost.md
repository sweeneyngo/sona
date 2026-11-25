# The learned cell stage trained for edit cost (utau-edit-cost 19)

Date: 2026-11-24. Songs: every spent split's songs that `sona-eval-cellstage` scores (dev 9, holdout 9,
holdout2 10, holdout3 10 = 38 songs, 104.5 min), read with `--reread` and logged as diagnosis. **holdout4 was not
read.** Revised weights (`spec.md`): off-time start or end 4, near-miss pitch 3, octave 1, extra 1, missing 2,
chopped/joined 4. All numbers are out of sample: each song is scored by a model that never saw it.

Selection rule (committed in the ticket before any CV run, 9950cf4; K = 10 in 07fd0f8; C named in 2398d64
before the final run): C is kept, opt-in, only if, pooled over the 38 songs of the final CV, the paired
song-bootstrap 95% upper bound of Δ cost/min against B (the current recipe retrained under the same folds) is
below 0, missing does not rise (or the bound holds with missing at 4), clean measures do not fall, and any
decoding parameter is chosen cross-fitted.

## Answer

- **C passes the rule.** Final CV (10 folds by song, seed 1, each song scored by the one fold model that never
  saw it, through the Rust scorer `sona-eval-cellstage --edit-cost --fold-models`):

  | recipe (38 songs, out of sample) | cost/min | Δ [95%] | clean measures | Δ [95%] | missing (Δ) | @4 upper | cells |
  |---|---|---|---|---|---|---|---|
  | **B**: current recipe retrained (true phase, CE, argmax) | 275.8 | | 14.1% | | 1 297 | | 83.28% |
  | B, seed 1 (noise floor) | 274.9 | −0.9 [−6.2, +4.1] | 14.2% | +0.1 [−0.9, +0.9] | 1 228 (−69) | +2.9 | 83.28% |
  | **C**: onset phase + residuals, edge-weighted CE + pairwise term, edge-cost decoding (cross-fitted) | **254.1** | **−21.7 [−31.7, −12.1]** | **16.4%** | **+2.3 [+1.1, +3.7]** | 1 444 (+147) | **−8.4** | 83.30%* |
  | *ablations (Python scorer, identical to Rust on B and C)* | | | | | | | |
  | C's training, argmax decoding | 262.9 | −12.9 [−20.7, −5.5] | 16.3% | +2.2 [+1.0, +3.5] | 1 276 (−21) | −6.7 | 83.32% |
  | B's training, edge-cost decoding (cross-fitted) | 262.2 | −13.7 [−20.5, −6.4] | 14.9% | +0.8 [+0.2, +1.3] | 1 459 (+162) | −3.8 | |

  \* at switch 1.5, which the cross-fitting chose on 9 of 10 folds (2.0 on the other).

  1. Upper bound −12.1 < 0. 2. Missing rises by 147 notes, and the bound stays below 0 with missing at 4
  (−8.4). 3. Clean measures rise (+2.3). 4. The switch cost is cross-fitted. **C is kept, opt-in**: a model
  directory whose `cellstage.json` sets `lead_residuals` and `switch_cost`; the shipped default is unchanged
  until holdout4.
- **Where the cost goes** (units, C − B): chopped −1 880, length −1 172, pitch −261, shift −232, extra −129,
  moved −72, octave +32, missing +294, joined +1 148. The decoding removes flicker (chopped, extra notes) at
  the price of merging some real repeats and dropping some short notes; the training removes part of the
  boundary jitter (length −716 and chopped −800 at argmax) without dropping notes (missing −21).
- **Per split** (B → C): dev 322.3 → 291.7, holdout 226.6 → 205.3, holdout2 301.9 → 279.2, holdout3 249.7 →
  237.6; clean measures rise on all four. 33 of 38 songs get cheaper; of the 5 that get dearer, the worst
  rises by 74.0 cost/min, the next by 39.5.
- **The residual feature alone is small.** Onset-phase training with the residuals moves −2.3 [−7.2, +2.5]
  (exploration); the gain comes from charging the loss for what the edit cost charges (pairwise agreement
  inside a UST note, edge cells weighted) and from decoding with a cost per edge. The residual stays in C
  because the rule was run with it; whether it earns its 8 columns is not separately confirmed.
- **The shipped stage's in-sample numbers overstated it.** Retrained out of sample, the current recipe costs
  322.3/min on dev, against 272.2 for the shipped model that trained on dev. The honest baseline for any stage
  change is B, not 272.2.
- **Next**: C goes into the holdout4 bundle as the stage (with its own rule there); the label oracle
  (−101 to −116 on E) says the stage's choice space still holds most of the cost, and joined notes (+1 148
  here) are now its largest class. The candidate model trained on all 38 songs is local only (below).

## The candidate model (local only)

C's recipe trained on all 38 songs (`train_all19.py`), exported with switch cost 1.5 (the value the
cross-fitting chose on 9 of 10 folds): a local folder outside the repository, never committed (its weights
derive from Pifuyuu's USTs and GAME). It loads through the same `CellModel` path the CLI and server use
(`--cell-model DIR`); on dev it runs end to end (in sample, so its score there means nothing). The shipped
model folder (`~/.cache/sona/cellstage-D/`) is untouched.

## What else these numbers could be measuring

- **Selection on the exploration.** C was picked among six training recipes and eight decodings on the same
  38 songs (different folds). Its exploration number (−17.2) is below the final one (−21.7), and its two
  closest rivals were within noise, so the gain is not a pick of the luckiest; but the final interval is still
  on songs the choice saw. holdout4 is the confirmation.
- **Seed noise.** B retrained with another seed moves −0.9 [−6.2, +4.1]: a fifth of C's gain.
- **Dropped notes.** Missing rises by 147 (C) and the decoding alone by 162; with missing weighted 4 the
  upper bound is −8.4. C's training alone lowers missing (−21).
- **A tuned decoder.** The switch cost is chosen per fold from the other folds' out-of-sample predictions
  (1.5 on 9/10); the models that produced those predictions had seen the fold's songs in training, so the
  choice is not perfectly blind, but the value is stable across folds and the neighbouring values (1–2) give
  similar gains on E.
- **Phase.** C trains on the onset phase that ships, B on the UST's (the current recipe). C's argmax gain
  could partly be phase; the decoding part (−13.7 on B alone) is not.
- **Outlier songs.** 33 of 38 improve; the largest single gain is −119.9, so the pooled result does not rest
  on one song, and the dearer songs are reported above.
- **Scorer artefacts.** The Rust and Python scorers agree to the unit on B and C, Python reproduces the
  shipped stage's Rust counts on 38/38 songs, and Rust's decoder gives Python's notes.

## Exploration (fold assignment E: 4 folds by song, permutation seed 2; Python scorer = Rust's)

Used freely to choose C, so these numbers carry selection (six training recipes × eight decodings looked at).
B here is the current recipe under E: 273.7 cost/min, clean 14.8%, missing 1 214, cells 83.38%.

| recipe | argmax Δ [95%] | clean Δ | missing Δ | decoded, cross-fitted Δ [95%] (switch) | clean Δ | missing Δ (@4 upper) |
|---|---|---|---|---|---|---|
| B (current recipe) | 0 | 0 | 0 | −13.6 [−21.2, −6.0] (2, 2, 2, 2) | +0.6 | +199 (−2.7) |
| RO: onset phase + residuals | −2.3 [−7.2, +2.5] | +0.2 | −27 | −13.9 [−21.1, −6.3] (2, 2, 2, 2) | +1.0 | +167 (−2.3) |
| W: RO + edge cells weighted 3 | −2.4 [−7.2, +2.9] | +1.5 | −29 | −12.7 [−18.0, −7.5] (2, 1, 2, 1) | +1.7 | +136 (−3.8) |
| S: RO + source-aware set targets | +28.6 [+19.7, +38.5] | −3.3 | −22 | (0 chosen) | | |
| P: RO + 0.5 × pairwise same-note term | −13.4 [−20.2, −7.7] | +0.9 | −24 | −19.3 [−26.7, −12.3] (1, 1, 1, 1) | +1.3 | +62 (−10.7) |
| P1: RO + 1.0 × pairwise | −12.3 [−19.0, −6.2] | +0.7 | −11 | −17.4 [−25.6, −9.6] (1, 1, 1, 1.5) | +1.1 | +84 (−8.2) |
| **PW: P + edge weights 3 (= C)** | −10.4 [−14.8, −6.2] | +1.0 | −55 | −17.2 [−23.6, −10.9] (2, 1, 1.5, 1.5) | +1.4 | +56 (−9.4) |

C was PW rather than P: within noise of each other on cost, PW drops fewer notes (+56 against +62 decoded, −55
against −24 at argmax) and keeps more clean measures, which are the rule's other two criteria; and it is the
loss that charges edges as the edit cost does.

Decoding variants (Python only): on B, a lower cost for rest edges is worse (switch 2 with rest edges free:
+4.8; rest edges 0.5: −6.5, against −13.6 for one cost of 2); on RO, a separate cost for same-pitch restarts
(0 to 1) gives −11.2 to −11.5 against −13.9. One cost per edge stays.

Label oracles (no model; the cells take the UST's choice where a candidate has it, on E's songs): the
prototype's first-candidate labels −101.1 [−118.9, −83.9]; source-aware labels (a candidate whose own note
splits where the UST's does) −116.0 [−135.5, −97.6]. Both raise missing (+518, +504: cells with no candidate
at the UST's pitch become rests).

## Verification

- **Default unchanged.** `sona-eval-cellstage --edit-cost` with the shipped model prints the same report
  before and after the change (dev 272.2, `diff` of the two outputs). The re-exported shipped checkpoint
  writes a byte-identical `cellstage.json`.
- **Python = Rust.** The shipped checkpoint on the Rust rows gives exactly the stage notes Rust wrote, and
  `lib19`'s scoring gives the Rust report's class counts, on 38 of 38 songs (`parity19.py`). Rust's
  `decode_classes` gives `lib19.viterbi`'s notes on 9 of 9 dev songs at switch 2 (`parity_decode19.py`).
  `export_onnx.py` checks ONNX Runtime against PyTorch on every export (≤ 1e-5).
- **Tests through the public interface** (`crates/sona-core/tests/`):
  - `stage_residuals.rs` (4): no extras gives the prototype's features exactly; the residuals follow them in
    each row and change no candidate; each edge carries the lean of the raw edge rounded onto it (onset and
    offset, both edges of a cell, an unvoiced note carries nothing); a note the snap moved is clamped at 2.
  - `stage_decode.rs` (5): an unsure one-cell flicker is smoothed away, a sure change stays, a same-pitch
    restart counts as an edge, switch 0 is the masked argmax, and on random small cases the decoding scores
    as well as the best of all 6ⁿ choices under the stated objective (brute force).
  - `sona-eval-cellstage`'s `held_out_model`: each song goes to the one fold model that never saw it; a song
    every model saw, or more than one left out, is refused.
- **Mutants caught**: residual sign flipped, end edge dropped, offsets read as onsets, offset residual taken
  from the onset, clamp widened, extras on by default (`stage_residuals`); source restarts ignored, rest→rest
  charged, unmasked candidates, broken backtrack (`stage_decode`); any unseen fold accepted (`held_out_model`).
  One mutant survives: ties to the higher class instead of the lower. It is equivalent under the objective
  (both are optimal); the tie order only matters for Python parity, which the parity run checks.

## Reproduce

```sh
export ORT_DYLIB_PATH=… SONA_GAME_DIR=… SONA_DEV_CACHE=… SONA_UST_DIR=… SONA_UST_STEMS_DIR=…
cargo build --release -p sona-eval --features game --bin sona-eval-cellstage
BIN=target/release/sona-eval-cellstage CELLS=target/stage19-cells     # UST-derived: keep out of the repo
$BIN --model ~/.cache/sona/cellstage-D --edit-cost --dump-cells $CELLS
for s in holdout holdout2 holdout3; do
  $BIN --model ~/.cache/sona/cellstage-D --edit-cost --dump-cells $CELLS --split $s --reread --reason "…"
done
cd crates/sona-eval/scripts/cellstage      # PY = a Python with torch, numpy, onnx, onnxruntime
$PY -I parity19.py $CELLS ~/.cache/sona/cellstage-D/cellstage_D.pt REPORT…
# exploration (fold assignment E)
$PY -I cv19.py $CELLS $CV_E 4 2 B: RO:phase=onset,width=1054 W:phase=onset,width=1054,w_edge=3 \
    S:phase=onset,width=1054,loss=set P:phase=onset,width=1054,w_pair=0.5 P1:phase=onset,width=1054,w_pair=1.0 \
    PW:phase=onset,width=1054,w_pair=0.5,w_edge=3
ORACLE=1 $PY -I eval19.py $CELLS $CV_E B RO W S P P1 PW B@cf RO@cf W@cf P@cf P1@cf PW@cf
# final CV (fold assignment F)
export PY BIN CELLS CV=target/stage19-cv/F
./final19.sh train B: Bs1:seed=1 C:phase=onset,width=1054,w_pair=0.5,w_edge=3
$PY -I eval19.py $CELLS $CV B C@cf Bs1 B@cf C        # prints the cross-fitted switch per fold
./final19.sh export B 0,0,0,0,0,0,0,0,0,0; ./final19.sh export Bs1 0,0,0,0,0,0,0,0,0,0
./final19.sh export C 1.5,1.5,1.5,1.5,1.5,1.5,2.0,1.5,1.5,1.5   # the per-fold values eval19 printed
for r in B Bs1 C; do ./final19.sh score $r; done
rows() { echo "$CV/$1/rust_dev.txt,$CV/$1/rust_holdout.txt,$CV/$1/rust_holdout2.txt,$CV/$1/rust_holdout3.txt"; }
$PY -I rustcv19.py $CELLS "$(rows B)" "C=$(rows C)" "Bs1=$(rows Bs1)"
$PY -I splits19.py $CELLS "$(rows B)" "$(rows C)"
# the candidate model (local only)
$PY -I train_all19.py $CELLS MODEL_DIR/cellstage_19C.pt phase=onset,width=1054,w_pair=0.5,w_edge=3
$PY -I export_onnx.py MODEL_DIR/cellstage_19C.pt MODEL_DIR 1.5
```
