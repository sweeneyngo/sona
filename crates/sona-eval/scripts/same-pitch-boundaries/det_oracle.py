"""07 step 2/3 oracles: detector placement given the true count; thresholded splits; merges."""
import sys
import numpy as np
import lib07 as L
import det
from oracle1a_lib import counts_per_note

songs = L.load()
CFGS = [dict(feat="mfcc", w=6, alpha=0.0, lead=0.0), dict(feat="mfcc", w=6, alpha=1.0, lead=0.25)]


def top_cuts(row, k):
    best = sorted(row, key=lambda kv: -kv[1])[: k - 1]
    return sorted(c for c, _ in best)


def run(out, cfg):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    print(f"\n== {out} {cfg}: baseline {L.cpm(base):.1f}", flush=True)
    G = {s["id"]: L.to_grid(s, s["outputs"][out]) for s in songs}
    NOV = {s["id"]: det.novelty(s["id"], cfg) for s in songs}
    SC = {s["id"]: det.grid_scores(s, G[s["id"]], cfg, NOV[s["id"]]) for s in songs}
    RS = {s["id"]: det.restart_scores(s, G[s["id"]], cfg, NOV[s["id"]]) for s in songs}
    CN = {s["id"]: counts_per_note(s, G[s["id"]]) for s in songs}

    def show(name, gl):
        notes = [L.from_grid(s, gl[s["id"]]) for s in songs]
        rows = [L.score(s, n) for s, n in zip(songs, notes)]
        eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
        return L.report(name, songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")

    show("true count, detector place", {sid: L.split_at(G[sid], {i: top_cuts(SC[sid][i], len(c)) for i, c in enumerate(CN[sid]) if len(c) > 1}) for sid in G})
    allv = np.array([v for sid in SC for row in SC[sid] for _, v in row])
    for q in (0.80, 0.90, 0.95, 0.98, 0.99):
        th = float(np.quantile(allv, q))
        gl = {}
        for sid in G:
            cuts = {}
            for i, row in enumerate(SC[sid]):
                vs = [v for _, v in row]
                c = [k for j, (k, v) in enumerate(row) if v > th and (j == 0 or v >= vs[j - 1]) and (j == len(vs) - 1 or v >= vs[j + 1])]
                if c:
                    cuts[i] = c
            gl[sid] = L.split_at(G[sid], cuts)
        show(f"split at peaks > q{q:.2f} ({th:.2f})", gl)
    rv = np.array([v for sid in RS for v in RS[sid].values()])
    for q in (0.02, 0.05, 0.10, 0.20):
        th = float(np.quantile(rv, q))
        gl = {sid: L.merge(G[sid], {i for i, v in RS[sid].items() if v < th and G[sid][i - 1][2] == G[sid][i][2]}) for sid in G}
        show(f"merge same-pitch restarts < q{q:.2f} ({th:.2f})", gl)


for out in sys.argv[1:] or ["lead"]:
    for cfg in CFGS:
        run(out, cfg)
