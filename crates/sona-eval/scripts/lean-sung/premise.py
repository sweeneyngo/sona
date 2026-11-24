"""Are the missing UST notes audible? Compare f0 voicing and energy over missing UST notes with matched ones,
and list their mora kinds. usage: premise.py DUMP [lead|stage]"""
import sys, os, collections, bisect, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "edit-cost-taxonomy"))
import taxo

songs = load(sys.argv[1])
out = sys.argv[2] if len(sys.argv) > 2 else "lead"
stats = {"missing": [], "matched": []}
kinds = {"missing": collections.Counter(), "matched": collections.Counter()}
for s in songs:
    sp, ua = s["spacing"], s["ust_anchor"]
    a = score(s, [tuple(x) for x in s["outputs"][out]])
    refs = a["refs"]
    f0 = s["f0"]
    ft = [f[0] for f in f0]
    rms = s["rms"]
    rt = [r[0] for r in rms]
    voiced_rms = sorted(r[1] for r in rms)
    med_rms = voiced_rms[len(voiced_rms) // 2]
    lyr = taxo.ust_lookup(s)
    for comp in a["comps"]:
        if len(comp["refs"]) != 1:
            continue
        r = refs[comp["refs"][0]]
        k = "missing" if comp["counts"]["missing"] == 1 else ("matched" if len(comp["outs"]) == 1 else None)
        if not k:
            continue
        t0, t1 = ua + r[0] * sp, ua + r[1] * sp
        fr = f0[bisect.bisect_left(ft, t0):bisect.bisect_left(ft, t1)]
        vf = sum(1 for f in fr if f[1] > 0) / max(1, len(fr))
        rr = rms[bisect.bisect_left(rt, t0):bisect.bisect_left(rt, t1)]
        e = sum(x[1] for x in rr) / max(1, len(rr))
        stats[k].append((vf, math.log10(e / med_rms + 1e-9)))
        kinds[k][taxo.mora(lyr.get((r[0], r[2]), ""))[1]] += 1
for k, v in stats.items():
    v.sort()
    vfs = sorted(x[0] for x in v)
    es = sorted(x[1] for x in v)
    q = lambda xs, p: xs[int(p * (len(xs) - 1))]
    print(f"{k:8} n={len(v):5}  f0 voiced share q25/50/75 {q(vfs, .25):.2f}/{q(vfs, .5):.2f}/{q(vfs, .75):.2f}"
          f"  log10 energy vs song median q25/50/75 {q(es, .25):+.2f}/{q(es, .5):+.2f}/{q(es, .75):+.2f}")
    tot = sum(kinds[k].values())
    print("          mora kinds:", {m: f"{100 * c / tot:.0f}%" for m, c in kinds[k].most_common()})
