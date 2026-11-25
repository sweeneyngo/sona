"""Train one cell-stage model on every song of a `--dump-cells` directory (the model to export for use).

usage: train_all19.py CELLS_DIR CHECKPOINT.pt RECIPE      (RECIPE as in cv19.py, without the name)
Then: export_onnx.py CHECKPOINT.pt OUT_DIR SWITCH_COST. Both derive from the UST: keep them local."""
import glob, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import torch
import lib19 as L, model19 as M

cells, out, spec = sys.argv[1], sys.argv[2], sys.argv[3]
cfg = dict(phase="true", width=1046, loss="first", w_edge=1, w_pair=0, epochs=15, hid=96, seed=0)
for item in filter(None, spec.split(",")):
    k, v = item.split("=")
    cfg[k] = v if k in ("phase", "loss") else float(v) if "." in v else int(v)
ids = sorted(os.path.basename(f).split(".")[0] for f in glob.glob(f"{cells}/*.onset.json"))
songs = [L.load_song(cells, s, cfg["phase"]) for s in ids]
for d in songs:
    d["G"] = L.good_sets(d)
torch.save(M.train(songs, cfg), out)
print(f"trained on {len(ids)} songs: {out}")
