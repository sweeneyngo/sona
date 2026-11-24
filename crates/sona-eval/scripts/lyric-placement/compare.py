"""Paired song-bootstrap differences between oracle rows saved by `oracles.py --dump-rows FILE`.
Usage: python3 compare.py FILE lead "b/r" "a/r" [more pairs…]   (prints B − A for edit cost and wrong moras/min)
"""
import json, sys
import lib01 as L
from lib01 import ec

rows = json.load(open(sys.argv[1]))
out = sys.argv[2]
pairs = sys.argv[3:]


def ecrow(r):
    return dict(counts=dict(zip(ec.CLASSES, r[1])), span_s=r[2]["span_s"])


def ec_pool(rs):
    c = {k: sum(r["counts"][k] for r in rs) for k in ec.CLASSES}
    return dict(counts=c, span_s=sum(r["span_s"] for r in rs))


def ec_cpm(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


for b, a in zip(pairs[::2], pairs[1::2]):
    ra, rb = rows[f"{out}|{a}"], rows[f"{out}|{b}"]
    d, lo, hi = L.paired([ecrow(r) for r in ra], [ecrow(r) for r in rb], ec_cpm, pool_fn=ec_pool)
    w, wlo, whi = L.paired([r[2] for r in ra], [r[2] for r in rb], L.wrong_per_min)
    per = " ".join(f"{x[0][:8]}:{ec_cpm(ec_pool([ecrow(y)])) - ec_cpm(ec_pool([ecrow(x)])):+.1f}" for x, y in zip(ra, rb))
    print(f"{out:5} {b:6} − {a:6}  edit cost {d:+6.1f} [{lo:+6.1f}, {hi:+6.1f}]  wrong/min {w:+6.1f} [{wlo:+6.1f}, {whi:+6.1f}]   {per}")
