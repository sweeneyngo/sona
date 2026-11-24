"""Energy profiles (vocals RMS per cell over the note's max) of GAME lead notes that run into a UST rest, and
of notes that end on time before one, to see where the release shows.

usage: S12_DUMP=DIR python3 -I release_diag.py
"""
import collections, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
from release_energy import cell_energy

songs = L.load()
prof = collections.defaultdict(list)
for s in songs:
    sp, an = s["spacing"], s["ust_anchor"]
    rt = [x[0] for x in s["rms"]]
    rv = [x[1] for x in s["rms"]]
    A = L.analyse(s, s["outputs"]["lead"])
    for p in L.pairs(A):
        r, o = p["r"], p["o"]
        if p["ds"] != 0 or not (p["r_next_gap"] and p["r_next_gap"] > 0):
            continue
        e = [cell_energy(rt, rv, an + c * sp, an + (c + 1) * sp) for c in range(o[0], o[1] + 2)]
        mx = max(e[: o[1] - o[0]]) or 1
        rel = [x / mx for x in e]
        key = ("late" if p["de"] > 0 else "on time" if p["de"] == 0 else "early", p["de"], p["r_next_gap"])
        # cells counted back from the UST end: ... -2, -1 | 0, 1, (output end) ...
        prof[key].append({c - r[1]: rel[c - o[0]] for c in range(o[0], o[1] + 2)})
for key in sorted(prof, key=lambda k: -len(prof[k]))[:8]:
    rows = prof[key]
    cols = range(-3, 4)
    med = []
    for c in cols:
        v = [r[c] for r in rows if c in r]
        med.append(f"{st.median(v):.2f}" if len(v) > 5 else "  - ")
    print(f"{str(key):28} n {len(rows):3d}  cells rel. to UST end {list(cols)}: {' '.join(med)}")
