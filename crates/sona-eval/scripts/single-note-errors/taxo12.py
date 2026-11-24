"""Taxonomy of single-note (pair) errors: start, end and pitch, with the raw GAME timing behind each.

usage: S12_DUMP=DIR python3 -I taxo12.py [lead|stage]
"""
import collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
import ec

OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"
songs = L.load()

tot_cost = 0
pair_rows = []
for s in songs:
    A = L.analyse(s, s["outputs"][OUT])
    tot_cost += ec.cost(A["counts"])
    sp, an = s["spacing"], s["ust_anchor"]
    raw = None
    if OUT == "lead":
        raw = L.snap_notes(s["stage_detail"]["game_lead"], sp, s["onset_phase"], keep=True)
    for p in L.pairs(A):
        p["song"] = s["id"]
        if raw is not None:
            g = s["stage_detail"]["game_lead"][raw[p["o"][3]][3]]
            p["raw_on"] = (g[0] - an) / sp - p["r"][0]
            p["raw_off"] = (g[1] - an) / sp - p["r"][1]
            p["raw_midi"] = g[2]
        pair_rows.append(p)

err = [p for p in pair_rows if p["cost"] > 0]
pc = sum(p["cost"] for p in err)
print(f"{OUT}: total cost {tot_cost}, pairs {len(pair_rows)}, pairs with an error {len(err)}, pair cost {pc} ({100 * pc / tot_cost:.1f}%)")
cls = collections.Counter()
for p in err:
    for k in L.PAIR:
        cls[k] += p["counts"][k]
print("  by class:", dict(cls))


def sgn(x):
    return "0" if x == 0 else ("+1" if x == 1 else ("-1" if x == -1 else ("+2+" if x > 0 else "-2-")))


print("\n(start delta, end delta) of pairs with a timing error: count, cost (timing only), share of pair cost")
tim = collections.Counter()
timc = collections.Counter()
for p in err:
    if p["ds"] or p["de"]:
        k = (sgn(p["ds"]), sgn(p["de"]))
        tim[k] += 1
        timc[k] += p["counts"]["length"] + p["counts"]["shift"] + p["counts"]["moved"]
for k, v in tim.most_common():
    print(f"  ds {k[0]:>3} de {k[1]:>3}: {v:4d}  cost {timc[k]:4d}  {100 * timc[k] / pc:5.1f}%")

print("\nend-only errors (ds 0, de ≠ 0) by the UST's and the output's gap to the next note")
eo = collections.Counter()
for p in err:
    if p["ds"] == 0 and p["de"]:
        rg = "UST legato" if p["r_next_gap"] == 0 else ("UST rest" if p["r_next_gap"] is not None else "end")
        og = "out legato" if p["o_next_gap"] == 0 else ("out rest" if p["o_next_gap"] is not None else "end")
        eo[(sgn(p["de"]), rg, og)] += 1
for k, v in eo.most_common():
    print(f"  de {k[0]:>3}  {k[1]:10} {k[2]:10}: {v}")

print("\nstart errors (ds ≠ 0) by the UST's and the output's gap before the note")
so = collections.Counter()
for p in err:
    if p["ds"]:
        rg = "UST legato" if p["r_prev_gap"] == 0 else ("UST rest" if p["r_prev_gap"] is not None else "start")
        og = "out legato" if p["o_prev_gap"] == 0 else ("out rest" if p["o_prev_gap"] is not None else "start")
        so[(sgn(p["ds"]), sgn(p["de"]), rg, og)] += 1
for k, v in so.most_common(14):
    print(f"  ds {k[0]:>3} de {k[1]:>3} {k[2]:10} {k[3]:10}: {v}")

print("\npitch errors: interval (out − UST)")
pi = collections.Counter(p["dp"] for p in err if p["dp"])
for k, v in sorted(pi.items(), key=lambda kv: -kv[1])[:12]:
    print(f"  {k:+3d}: {v}")

if OUT == "lead":
    import statistics as st
    print("\nraw GAME onset/offset − UST boundary, in cells (all pairs; positive = GAME later)")
    for key in ("raw_on", "raw_off"):
        for label, sel in (("all pairs", pair_rows), ("start right", [p for p in pair_rows if p["ds"] == 0]),
                           ("end right", [p for p in pair_rows if p["de"] == 0])):
            v = [p[key] for p in sel]
            print(f"  {key} {label:12} n {len(v):4d} median {st.median(v):+.2f} mean {st.mean(v):+.2f}")
        hist = collections.Counter()
        for p in pair_rows:
            hist[round(p[key] * 4) / 4] += 1
        print("   hist (quarter cells):", " ".join(f"{k:+.2f}:{hist[k]}" for k in sorted(hist) if -2.5 <= k <= 2.5))
    for key, d in (("raw_on", "ds"), ("raw_off", "de")):
        print(f"\n  {key} for pairs with {d} = +1 / -1: how far past the rounding point (|x| − 0.5)")
        for want in (1, -1):
            v = sorted(abs(p[key]) - 0.5 for p in pair_rows if p[d] == want)
            if v:
                q = lambda f: v[int(f * (len(v) - 1))]
                print(f"   {d} {want:+d}: n {len(v)}; within 0.1 cell of the rounding point {sum(x < 0.1 for x in v)}, "
                      f"0.25 {sum(x < 0.25 for x in v)}; quartiles {q(0.25):.2f} {q(0.5):.2f} {q(0.75):.2f}")

print("\nper song: pair cost per minute, and its split")
for s in songs:
    A = L.analyse(s, s["outputs"][OUT])
    ps = [p for p in pair_rows if p["song"] == s["id"]]
    c = collections.Counter()
    for p in ps:
        for k in L.PAIR:
            c[k] += p["counts"][k]
    m = A["span_s"] / 60
    print(f"  {s['id']:28} pairs {len(ps):4d}  len {c['length'] / m:5.1f} shift {c['shift'] / m:5.1f} moved {c['moved'] / m:4.1f} "
          f"pitch {c['pitch'] / m:5.1f}  total/min {ec.cost(A['counts']) / m:6.1f}")
