"""Nested leave-one-song-out: pick (beta, tail, kind-aware) on 8 songs, apply to the 9th; pooled out-of-fold."""
import itertools
import lib07 as L
from sonaphrase_sweep import fit, songs

grid = list(itertools.product([0.0, 0.5, 1.0], [None, 1, 2, 3], [True, False]))
for out in ("lead", "stage"):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    rows = {}
    for beta, tail, kind in grid:
        p = dict(beta=beta, zero=2.0, skip=3.0, kmax=8, kind_aware=kind, flat=1.0, tail=tail)
        rows[(beta, tail, kind)] = [L.score(s, fit(s, out, p)) for s in songs]
    oof = []
    for k, s in enumerate(songs):
        others = [i for i in range(len(songs)) if i != k]
        best = min(grid, key=lambda c: L.cpm([rows[c][i] for i in others]))
        oof.append(rows[best][k])
        print(f"  {out} held out {s['id']:28} chose beta={best[0]} tail={best[1]} kind={best[2]}")
    L.report(f"{out} nested LOSO choice", songs, base, oof)
