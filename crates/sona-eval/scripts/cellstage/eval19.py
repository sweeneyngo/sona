"""Score cv19.py's out-of-sample logits: pooled cost/min, clean measures, missing, cell accuracy, and paired
song-bootstrap Δ against a base recipe.

usage: eval19.py CELLS_DIR CV_DIR BASE NAME[@switch|@cf] ...
  NAME@0.5  decode with lib19.viterbi at that switch cost; NAME@cf  the switch cost chosen cross-fitted
  (for each fold, the value that minimises the other folds' pooled cost/min, from SWITCHES).
  ORACLE=1 adds the label oracles (no model: what the stage's choice space allows)."""
import glob, json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
import lib19 as L, model19 as M

cells, cv, base = sys.argv[1], sys.argv[2], sys.argv[3]
SWITCHES = [0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0]
ids = sorted(os.path.basename(f).split(".")[0] for f in glob.glob(f"{cells}/*.onset.json"))
D = {s: L.load_song(cells, s, "onset") for s in ids}


def rows_of(name, switch):
    out = []
    acc = [0, 0]
    for s in ids:
        d = D[s]
        lg = np.load(f"{cv}/{name}/{s}.npy")
        cl = L.viterbi(M.log_softmax_masked(lg, d["cands"]), d, switch)
        out.append(L.row(d, L.cells_to_notes(cl, d)))
        pred = np.where(cl == 0, -1, d["cands"][np.arange(len(cl)), cl])
        acc[0] += int((pred[d["span"]] == d["ref"][d["span"]]).sum())
        acc[1] += int(d["span"].sum())
    return out, 100 * acc[0] / acc[1]


def cross_fitted(name):
    folds = json.load(open(f"{cv}/{name}/recipe.json"))["folds"]
    grid = SWITCHES
    by = {sw: rows_of(name, sw)[0] for sw in grid}
    pos = {s: i for i, s in enumerate(ids)}
    rows, chosen = [None] * len(ids), []
    for te in folds:
        rest = [pos[s] for s in ids if s not in te]
        best = min(grid, key=lambda sw: L.per_min(L.pool([by[sw][i] for i in rest])))
        chosen.append(best)
        for s in te:
            rows[pos[s]] = by[best][pos[s]]
    return rows, chosen


def parse(spec):
    name, _, dec = spec.partition("@")
    if dec == "cf":
        rows, chosen = cross_fitted(name)
        return rows, f"switch cross-fitted {chosen}", None
    rows, acc = rows_of(name, float(dec or 0))
    return rows, f"switch {dec or 0}", acc


B, _, bacc = parse(base)
pb = L.pool(B)
print(f"base {base}: {L.per_min(pb):.1f} cost/min, clean {L.clean(pb):.1f}%, missing {pb['counts']['missing']}, cells {bacc:.2f}%")
print("  classes: " + " ".join(f"{k} {v * L.ec.W[k]}" for k, v in pb["counts"].items()))
for spec in sys.argv[4:]:
    R, how, acc = parse(spec)
    print(L.fmt(spec, B, R) + (f"  cells {acc:.2f}%" if acc is not None else f"  ({how})"))
    pr = L.pool(R)
    print("    Δ units by class: " + " ".join(f"{k} {(pr['counts'][k] - pb['counts'][k]) * L.ec.W[k]:+d}" for k in L.ec.CLASSES))
if os.environ.get("ORACLE"):
    for kind in ("first", "set"):
        rows = []
        for s in ids:
            d = D[s]
            d["G"] = L.good_sets(d)
            cl = np.zeros(d["n"], np.int64)
            for c in range(d["n"]):
                if kind == "first":
                    y = d["y"][c]
                    cl[c] = y if y >= 0 else 0
                else:
                    g = np.nonzero(d["G"][c])[0]
                    if g.size:
                        cl[c] = cl[c - 1] if c > 0 and d["G"][c, cl[c - 1]] else g[0]
                    else:
                        cl[c] = 0
            # outside the UST's span the oracle takes the base's choice
            lg = np.load(f"{cv}/{base.partition('@')[0]}/{s}.npy")
            arg = L.choose(lg, d["cands"])
            cl[~d["span"]] = arg[~d["span"]]
            rows.append(L.row(d, L.cells_to_notes(cl, d)))
        print(L.fmt(f"oracle labels ({kind})", B, rows))
        pr = L.pool(rows)
        print("    Δ units by class: " + " ".join(f"{k} {(pr['counts'][k] - pb['counts'][k]) * L.ec.W[k]:+d}" for k in L.ec.CLASSES))
