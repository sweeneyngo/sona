"""Nested leave-one-song-out for the detector's restart merges: (config, quantile) and the threshold itself
are chosen on 8 songs and applied to the 9th."""
import itertools, sys
import numpy as np
import lib07 as L
import det

songs = L.load()
CFGS = [dict(feat="mfcc", w=6, alpha=0.0, lead=0.0), dict(feat="mfcc", w=6, alpha=1.0, lead=0.25)]
QS = [0.02, 0.05, 0.10, 0.20]
for out in ("lead", "stage"):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    G = {s["id"]: L.to_grid(s, s["outputs"][out]) for s in songs}
    RS = {ci: {s["id"]: det.restart_scores(s, G[s["id"]], c) for s in songs} for ci, c in enumerate(CFGS)}

    def merged(ci, th, s):
        g = G[s["id"]]
        return L.score(s, L.from_grid(s, L.merge(g, {i for i, v in RS[ci][s["id"]].items() if v < th and g[i - 1][2] == g[i][2]})))

    oof = []
    for k, s in enumerate(songs):
        train = [o for o in songs if o["id"] != s["id"]]
        best = None
        for ci, q in itertools.product(range(len(CFGS)), QS):
            th = float(np.quantile([v for o in train for v in RS[ci][o["id"]].values()], q))
            c = L.cpm([merged(ci, th, o) for o in train])
            if best is None or c < best[0]:
                best = (c, ci, th, q)
        oof.append(merged(best[1], best[2], s))
        print(f"  {out} held out {s['id']:28} cfg {best[1]} q {best[3]}", flush=True)
    L.report(f"{out} merges, nested LOSO choice", songs, base, oof)
