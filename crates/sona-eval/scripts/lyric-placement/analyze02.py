"""Lyrics-first 02: the realistic line placement against 01's landings, with the break penalty chosen on all of
dev and leave-one-song-out (by GAME lead's or the stage's own pooled edit cost on the other songs).
Usage: python3 analyze02.py ROWS01_JSON ROWS02_JSON
"""
import json, sys
import lib01 as L
from lib01 import ec

r01 = json.load(open(sys.argv[1]))
r02 = json.load(open(sys.argv[2]))


def ecrow(r):
    return dict(counts=dict(zip(ec.CLASSES, r[1])), span_s=r[2]["span_s"])


def ec_pool(rs):
    return dict(counts={k: sum(r["counts"][k] for r in rs) for k in ec.CLASSES}, span_s=sum(r["span_s"] for r in rs))


def cpm(rows):
    return ec.cost(ec_pool([ecrow(r) for r in rows])["counts"]) / (sum(r[2]["span_s"] for r in rows) / 60)


def line(label, rows, base, today):
    d, lo, hi = L.paired([ecrow(r) for r in base], [ecrow(r) for r in rows], lambda p: ec.cost(p["counts"]) / (p["span_s"] / 60), pool_fn=ec_pool)
    w, wlo, whi = L.paired([r[2] for r in today], [r[2] for r in rows], L.wrong_per_min)
    pp = L.pool([r[2] for r in rows])
    blo, bhi = L.boot([r[2] for r in rows], L.wrong_per_min)
    print(f"  {label:34} {cpm(rows):6.1f}  Δ {d:+6.1f} [{lo:+6.1f}, {hi:+6.1f}]  wrong {L.wrong_per_min(pp):6.1f} [{blo:5.1f}, {bhi:5.1f}] "
          f"vs today {w:+6.1f} [{wlo:+6.1f}, {whi:+6.1f}]  placed {L.placed_pct(pp):5.1f}%  landed {L.landed_pct(pp):5.1f}%")


for out in ("lead", "stage"):
    print(out)
    base, today = r01[f"{out}|none"], r01[f"{out}|d"]
    ids = [r[0] for r in base]
    for v in ("d", "b/r", "b+/r", "a/r"):
        if f"{out}|{v}" in r01:
            line(f"01 {v}", r01[f"{out}|{v}"], base, today)
    for kind in ("line", "space"):
        keys = sorted(k for k in r02 if k.startswith(f"{out}|{kind}@"))
        for k in keys:
            line(k.split("|")[1], r02[k], base, today)
        loso = []
        for i, sid in enumerate(ids):
            best = min(keys, key=lambda k: cpm([r for r in r02[k] if r[0] != sid]))
            loso.append([r for r in r02[best] if r[0] == sid][0])
            print(f"    held out {sid:28} pb chosen {best.split('@')[1]}")
        line(f"{kind} pb leave-one-song-out", loso, base, today)
