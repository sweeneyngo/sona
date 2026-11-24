"""Oracle per chopped subclass: merge exactly the truly chopped cuts of that subclass (pitch from the longer
piece, or the UST's pitch), re-run the metric. Ceilings only; they never port."""
import collections
import wob
L = wob.L

songs = wob.load()


def ust_pitch(s):
    refs = ec_refs = wob.ec.to_cells(s["reference"], s["spacing"], s["ust_anchor"])
    shift = wob.ec.rround((s["onset_phase"] - s["ust_anchor"]) / s["spacing"])

    def f(a, b, grp):
        # the UST note covering most of [a, b) (output grid -> UST grid by the phase shift)
        best, cov = grp[0][2], 0
        for r in ec_refs:
            c = max(0, min(r[1], b + shift) - max(r[0], a + shift))
            if c > cov:
                best, cov = r[2], c
        return best
    del refs
    return f


for out in ("lead", "stage"):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    G = {s["id"]: wob.grid(s, out) for s in songs}
    C = {s["id"]: wob.chopped_cuts(s, G[s["id"]]) for s in songs}
    n = collections.Counter(k for c in C.values() for k in c.values())
    tot = sum(n.values())
    assert tot == L.pooled(base)["counts"]["chopped"], (tot, L.pooled(base)["counts"]["chopped"])
    print(f"== {out}: {tot} chopped cuts: " + " ".join(f"{k} {n[k]} ({100 * n[k] / tot:.1f}%)" for k in wob.KINDS))
    print(f"  baseline {L.cpm(base):.1f} cost/min, clean {L.clean(base):.1f}%")
    sets = [(k,) for k in wob.KINDS if n[k]] + [("step", "head"), ("step", "head", "other", "octave"), wob.KINDS]
    for kinds in sets:
        for pm in ("longer", "ust"):
            rows = []
            for s in songs:
                g = G[s["id"]]
                joins = {i for i, k in C[s["id"]].items() if k in kinds and k != "rest"}
                ref = ust_pitch(s) if pm == "ust" else None
                rows.append(L.score(s, L.from_grid(s, wob.merge(g, joins, ref_pitch=ref))))
            L.report(f"{out} merge {'+'.join(kinds)} [{pm}]", songs, base, rows)
