"""What a release trim does, note by note: which trims land on the UST's end, which cut a right note, and
which overruns it misses (and their energy).

usage: S12_DUMP=DIR python3 -I release_outcome.py THETA TAIL MINRUN
"""
import collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
from release_energy import trim_dip, cell_energy

th, tt, mr = float(sys.argv[1]), float(sys.argv[2]), int(sys.argv[3])
songs = L.load()
tally = collections.Counter()
missed = []
broken = []
for s in songs:
    sp, an = s["spacing"], s["ust_anchor"]
    notes = [tuple(n) for n in s["outputs"]["lead"]]
    new = trim_dip(s, notes, s["onset_phase"], theta=th, theta_tail=tt, minrun=mr)
    A = L.analyse(s, notes)
    by_out = {}
    for c in A["comps"]:
        for oi in c["outs"]:
            by_out[A["outs"][oi][3]] = c
    pr = {p["o"][3]: p for p in L.pairs(A)}
    rt = [x[0] for x in s["rms"]]
    rv = [x[1] for x in s["rms"]]
    for i, (a, b) in enumerate(zip(notes, new)):
        p = pr.get(i)
        target = p is not None and p["de"] > 0 and p["r_next_gap"] and p["r_next_gap"] > 0
        cut = round((a[1] - b[1]) / sp)
        if cut == 0:
            if target:
                tally["target missed"] += 1
                o = p["o"]
                e = [cell_energy(rt, rv, an + c * sp, an + (c + 1) * sp) for c in range(o[0], o[1])]
                mx = max(e) or 1
                missed.append([round(x / mx, 2) for x in e][-(p["de"] + 2):] + ["|de", p["de"], "len", o[1] - o[0]])
            continue
        if p is None:
            c = by_out.get(i)
            kind = "extra" if c and not c["refs"] else ("chop/join" if c else "outside span")
            tally[f"cut a note in a {kind} component"] += 1
        elif target:
            tally["target: cut exactly to the UST end" if cut == p["de"] else ("target: cut too little" if cut < p["de"] else "target: cut too much")] += 1
        elif p["de"] == 0:
            tally["pair with the right end: cut (new error)"] += 1
            o = p["o"]
            e = [cell_energy(rt, rv, an + c * sp, an + (c + 1) * sp) for c in range(o[0], o[1] + 2)]
            mx = max(e[:-2]) or 1
            broken.append([round(x / mx, 2) for x in e] + ["|cut", cut, "UST gap", p["r_next_gap"], "out gap", p["o_next_gap"]])
        else:
            tally[f"pair with de {p['de']:+d}, UST gap {p['r_next_gap']}: cut {cut}"] += 1
for k, v in sorted(tally.items(), key=lambda kv: -kv[1])[:20]:
    print(f"  {v:4d}  {k}")
print("\nsome missed overruns (cell energy / note max, last cells; then de and length):")
for m in missed[:12]:
    print("  ", m)
print("\nsome right notes the trim cut (cell energy / note max incl. 2 cells after; cut; gaps):")
for m in broken[:25]:
    print("  ", m)
