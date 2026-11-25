"""Could a new detector find the missing notes? (utau-edit-cost 06)
1. Oracle with perfect timing but no pitch source: each missing UST note drawn on its own cells with the
   pitch of the nearest output note (what a quiet-note detector could give at best).
2. Separability: AUC, over the cells the output leaves at rest, of "a missing UST note covers this cell"
   against everything else, for the vocals' energy, the pitch track's voicing and the stage's 1 - P(rest).
3. Negative rest bias (lean toward rest) and the per-song best bias, as diagnosis.
usage: detector.py DUMP"""
import sys, os, bisect, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import sweep

songs = load(sys.argv[1])


def auc(pos, neg):
    allv = sorted([(v, 1) for v in pos] + [(v, 0) for v in neg])
    r, rank_sum, i = 0, 0.0, 0
    while i < len(allv):
        j = i
        while j < len(allv) and allv[j][0] == allv[i][0]:
            j += 1
        avg = (i + j + 1) / 2
        rank_sum += avg * sum(1 for k in range(i, j) if allv[k][1])
        i = j
    n1, n0 = len(pos), len(neg)
    return (rank_sum - n1 * (n1 + 1) / 2) / max(1, n1 * n0)


for out in ("lead", "stage"):
    base, fixed = [], []
    feats = {"energy": ([], []), "f0 voiced": ([], []), "stage 1-P(rest)": ([], [])}
    for s in songs:
        sp, ua, an = s["spacing"], s["ust_anchor"], s["onset_phase"]
        notes = [tuple(x) for x in s["outputs"][out]]
        a = score(s, notes)
        refs = a["refs"]
        miss = [refs[c["refs"][0]] for c in a["comps"] if c["counts"]["missing"] == 1]
        add = []
        for r in miss:
            t0, t1 = ua + r[0] * sp, ua + r[1] * sp
            near = min(notes, key=lambda n: min(abs(n[1] - t0), abs(n[0] - t1)), default=None)
            add.append((t0, t1, near[2] if near else r[2]))
        base.append(row_of(s, notes))
        fixed.append(row_of(s, sorted(notes + add)))
        # cells at rest in the output (UST grid, inside the UST's span)
        sd = s["stage_detail"]
        f0, rms = s["f0"], s["rms"]
        ft, rt = [f[0] for f in f0], [x[0] for x in rms]
        cover = set()
        for r in miss:
            cover.update(range(r[0], r[1]))
        oc = set()
        for o in a["outs"]:
            oc.update(range(o[0], o[1]))
        for k in range(refs[0][0], refs[-1][1]):
            if k in oc:
                continue
            t0, t1 = ua + k * sp, ua + (k + 1) * sp
            fr = f0[bisect.bisect_left(ft, t0):bisect.bisect_left(ft, t1)]
            rr = rms[bisect.bisect_left(rt, t0):bisect.bisect_left(rt, t1)]
            c = int(round((t0 + t1) / 2 - an) / sp - 0.5) if False else math.floor(((t0 + t1) / 2 - an) / sp) - sd["k0"]
            pr = 1 - sd["probs"][c][0] if 0 <= c < len(sd["probs"]) else 0.0
            vals = {"energy": math.log(sum(x[1] for x in rr) / max(1, len(rr)) + 1e-6),
                    "f0 voiced": sum(1 for f in fr if f[1] > 0) / max(1, len(fr)), "stage 1-P(rest)": pr}
            for name, v in vals.items():
                feats[name][0 if k in cover else 1].append(v)
    (bc, bcl), (fc, fcl) = pooled(base), pooled(fixed)
    print(f"{out}: missing notes drawn on the UST's cells with the nearest note's pitch: {bc:.1f} -> {fc:.1f} ({fc - bc:+.1f}), clean {bcl:.1f} -> {fcl:.1f}%")
    for name, (p, n) in feats.items():
        print(f"   AUC {name:16} {auc(p, n):.3f}   ({len(p)} missing-note cells, {len(n)} other rest cells)")

base = [row_of(s, sweep.stage0(s)) for s in songs]
bc = pooled(base)[0]
grid = (-2.0, -1.0, -0.5, -0.25, 0.0, 0.25, 0.5, 1.0, 2.0)
per = {b: [row_of(s, sweep.stage_bias(s, b)) for s in songs] for b in grid}
for b in grid:
    if b < 0:
        c, cl = pooled(per[b])
        lo, hi = boot_delta(base, per[b])
        print(f"stage rest bias {b:+.2f}: {c:.1f} ({c - bc:+.1f} [{lo:+.1f}, {hi:+.1f}]) clean {cl:.1f}%")
best = [min(grid, key=lambda b: per[b][i][0]) for i in range(len(songs))]
rows = [per[b][i] for i, b in enumerate(best)]
print(f"stage, per-song best bias (oracle over {grid}): {pooled(rows)[0]:.1f} ({pooled(rows)[0] - bc:+.1f}); chosen {best}")
