# Learned cell stage: export and check

`export_onnx.py CHECKPOINT.pt OUT_DIR` turns a trained checkpoint into the folder sona loads with
`--cell-model OUT_DIR` (`cellstage.onnx` + `cellstage.json`). The weights derive from Pifuyuu's USTs and
from GAME's (CC BY-NC-SA) weights, so OUT_DIR stays local and is never committed.

`sona-eval-cellstage --model OUT_DIR` scores the stage per grid cell on the UST corpus. It reports the
GAME-lead column and the stage, each on sona's estimated grid phase and on the UST's.

Training still goes through the prototype's `build_cells.py` and `train_cells.py`, which are kept beside
the model in `~/.cache/sona/cellstage-D/`. ADR-0026 records the parity checks, and roadmap 1.7 replaces
that path with a Rust feature dump.
