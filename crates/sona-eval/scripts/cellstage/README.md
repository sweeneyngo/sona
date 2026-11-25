# Learned cell stage: export, check and retrain

`export_onnx.py CHECKPOINT.pt OUT_DIR [SWITCH_COST]` turns a trained checkpoint into the folder sona loads
with `--cell-model OUT_DIR` (`cellstage.onnx` + `cellstage.json`), and checks ONNX Runtime against PyTorch.
The weights derive from Pifuyuu's USTs and from GAME's (CC BY-NC-SA) weights, so OUT_DIR stays local and is
never committed.

`sona-eval-cellstage --model OUT_DIR` scores the stage per grid cell on the UST corpus. It reports the
GAME-lead column and the stage, each on sona's estimated grid phase and on the UST's.

## Retraining from sona's own features (utau-edit-cost 19)

The shipped model was trained by the prototype's `build_cells.py` and `train_cells.py` (kept beside it in
`~/.cache/sona/cellstage-D/`; ADR-0026). These scripts train from rows the Rust feature builder writes, so a
model reads exactly what sona gives it:

1. `sona-eval-cellstage --model DIR --edit-cost --dump-cells CELLS [--split NAME --reread --reason …]` writes
   each song's features on the onset phase (what ships) and on the UST's phase: the prototype's 1046 columns,
   then GAME lead's 8 rounding-residual columns (`FeatureSet::lead_residuals`), with candidates, sources and
   the lead UST's notes. UST-derived: keep CELLS out of the repo.
2. `parity19.py CELLS CHECKPOINT REPORT…`: the PyTorch checkpoint on those rows gives the notes Rust wrote,
   and `lib19`'s scoring (`edit-cost-taxonomy/ec.py`) gives the Rust report's per-song counts.
3. `cv19.py CELLS OUT FOLDS SEED RECIPE…` trains song-level folds (`model19.py`; the defaults are the
   prototype's recipe) and saves out-of-sample logits; `eval19.py` scores them, with
   `NAME@SWITCH` for decoding with a cost per note edge (`decode_classes`) and `NAME@cf` for the cost chosen
   cross-fitted.
4. `final19.sh train|export|score` runs the final CV: ten fold models per recipe, exported with their
   switch cost, scored by the Rust scorer with `--fold-models` (each song by the one model that never saw it);
   `rustcv19.py` pools those reports into the CV table and `splits19.py` breaks it down per split and song.
   `parity_decode19.py` checks Rust's decoding against `lib19.viterbi`.
5. `train_all19.py CELLS OUT.pt RECIPE` trains one model on every song, to export for use.

The report is `crates/sona-eval/reports/stage-edit-cost.md`.
