"""Song-level cross-validation of cell-stage recipes on `--dump-cells` rows, scored with the revised-weight
edit cost (lib19 = the Rust scorer, checked by parity19.py).

usage: cv19.py CELLS_DIR OUT_DIR FOLDS PERM_SEED RECIPE [RECIPE ...]
  RECIPE = name:key=value,...   keys: phase (true|onset), width (1046|1054), loss (first|set), w_edge, w_pair,
           epochs, hid, seed
Writes OUT_DIR/<recipe>/<song>.npy (out-of-sample logits on the onset phase, what ships) and, with SAVE=1,
OUT_DIR/<recipe>/fold<k>.pt checkpoints (UST-derived: keep them local)."""
import glob, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, torch
import lib19 as L, model19 as M

cells, out, K, perm_seed = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
recipes = []
for spec in sys.argv[5:]:
    name, _, kv = spec.partition(":")
    cfg = dict(phase="true", width=1046, loss="first", w_edge=1, w_pair=0, epochs=15, hid=96, seed=0)
    for item in filter(None, kv.split(",")):
        k, v = item.split("=")
        cfg[k] = v if k in ("phase", "loss") else float(v) if "." in v else int(v)
    recipes.append((name, cfg))
ids = sorted(os.path.basename(f).split(".")[0] for f in glob.glob(f"{cells}/*.onset.json"))
only = os.environ.get("SONGS")
if only:
    ids = [i for i in ids if i in only.split(",")]
order = np.random.RandomState(perm_seed).permutation(len(ids))
folds = [[ids[i] for i in order[k::K]] for k in range(K)]
os.makedirs(out, exist_ok=True)
json.dump(folds, open(f"{out}/folds.json", "w"))
data = {}
for phase in {c["phase"] for _, c in recipes} | {"onset"}:
    data[phase] = {}
    for sid in ids:
        d = L.load_song(cells, sid, phase)
        d["G"] = L.good_sets(d)
        data[phase][sid] = d
for name, cfg in recipes:
    od = f"{out}/{name}"
    os.makedirs(od, exist_ok=True)
    json.dump(dict(cfg=cfg, folds=folds), open(f"{od}/recipe.json", "w"))
    for k, te in enumerate(folds):
        if all(os.path.exists(f"{od}/{s}.npy") for s in te):
            continue
        t0 = time.time()
        ck = M.train([data[cfg["phase"]][s] for s in ids if s not in te], cfg)
        for s in te:
            np.save(f"{od}/{s}.npy", M.logits(ck, data["onset"][s]))
        if os.environ.get("SAVE"):
            torch.save(ck, f"{od}/fold{k}.pt")
        print(f"{name} fold {k + 1}/{K} ({len(te)} songs) {time.time() - t0:.0f}s", flush=True)
