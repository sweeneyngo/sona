"""Per-song drivers of joined cost: mora rate, same-pitch legato boundaries, hidden-note duration.

usage: python3 -I extra.py lead|stage DUMP_DIR...
"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ec
import taxo


def spearman(x, y):
    def rank(v):
        o = sorted(range(len(v)), key=lambda i: v[i])
        r = [0] * len(v)
        for k, i in enumerate(o):
            r[i] = k
        return r
    rx, ry = rank(x), rank(y)
    n = len(x)
    mx, my = sum(rx) / n, sum(ry) / n
    num = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    den = (sum((a - mx) ** 2 for a in rx) * sum((b - my) ** 2 for b in ry)) ** 0.5
    return num / den


for d in sys.argv[2:]:
    out = sys.argv[1]
    songs = taxo.load(d)
    rows = []
    ms = []
    for s in songs:
        a = ec.analyse(s["reference"], s["outputs"][out], s["spacing"], s["ust_anchor"], s["division"])
        mins = a["span_s"] / 60
        refs = a["refs"]
        same = sum(1 for x, y in zip(refs, refs[1:]) if y[0] == x[1] and y[2] == x[2])
        J = taxo.joined_boundaries(s, a)
        ms.extend(r["len_b"] * s["spacing"] * 1000 for r in J)
        rows.append((s["id"], len(refs) / a["span_s"], same / mins, 4 * a["counts"]["joined"] / mins, 4 * a["counts"]["chopped"] / mins, ec.cost(a["counts"]) / mins))
    print(f"\n{d} [{out}]")
    print(f"  {'song':28} {'notes/s':>7} {'same-legato/min':>15} {'joined cost/min':>15} {'chopped cost/min':>16} {'cost/min':>8}")
    for r in sorted(rows, key=lambda r: -r[3]):
        print(f"  {r[0]:28} {r[1]:7.2f} {r[2]:15.1f} {r[3]:15.1f} {r[4]:16.1f} {r[5]:8.1f}")
    js = [r[3] for r in rows]
    print(f"  Spearman(joined/min, notes/s) {spearman([r[1] for r in rows], js):+.2f}; (joined/min, same-pitch legato/min) {spearman([r[2] for r in rows], js):+.2f}")
    ms.sort()
    if ms:
        q = lambda p: ms[int(p * (len(ms) - 1))]
        print(f"  hidden-note duration ms: median {q(0.5):.0f}, quartiles {q(0.25):.0f}-{q(0.75):.0f}, share <= 125 ms {100 * sum(1 for m in ms if m <= 125) / len(ms):.0f}%")
