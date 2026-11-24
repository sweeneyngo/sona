"""Per-song edit-cost class counts of the chosen phrase fit, for checking the Rust port exactly."""
import json, os
import lib07 as L
from sonaphrase_sweep import fit, songs

P0 = dict(beta=0.5, zero=2.0, skip=3.0, kmax=8, kind_aware=True, flat=1.0, tail=2)
res = {}
for out in ("lead", "stage"):
    for s in songs:
        a = L.score(s, fit(s, out, P0))
        res[f"{out}/{s['id']}"] = [a["counts"][k] for k in L.ec.CLASSES]
json.dump(res, open(os.path.join(L.WORK, "proto_counts.json"), "w"), indent=1)
for k, v in res.items():
    print(k, v)
