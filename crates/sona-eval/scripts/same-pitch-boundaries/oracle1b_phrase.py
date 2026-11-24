"""1b ceiling: the UST's exact kana per UST phrase (perfect text, perfect phrase segmentation), aligned to
sona's notes in the phrase by the mora-kind DP; split by equal division."""
import itertools, sys
import lib07 as L
import lyric_dp as D

songs = L.load()


def phrase_items(s, g):
    shift = L.ec.rround((s["ust_anchor"] - s["onset_phase"]) / s["spacing"])
    u = L.ust_notes(s)
    refs = L.ec.to_cells(s["reference"], s["spacing"], s["ust_anchor"])
    # UST notes by their on-cell, to recover lyrics for refs
    lyr = {}
    for x in u:
        lyr.setdefault(L.ec.rround((x["on"] - s["ust_anchor"]) / s["spacing"]), x["lyric"])
    out = []
    for a, b, n in L.phrases(s):
        kinds = [D.kind_of(lyr.get(r[0], "?")) for r in refs if a <= r[0] < b]
        idx = [i for i, (x, y, _) in enumerate(g) if a + shift <= (x + y) / 2 < b + shift]
        gaps = []
        prev_end = a + shift
        for i in idx:
            gaps.append(max(0, g[i][0] - prev_end))
            prev_end = g[i][1]
        gaps.append(max(0, b + shift - prev_end))
        out.append((idx, kinds, gaps))
    return out


def run(out, p, label):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    gl = []
    for s in songs:
        g = L.to_grid(s, s["outputs"][out])
        allidx, allks = [], []
        for idx, kinds, gaps in phrase_items(s, g):
            if not idx:
                continue
            ks = D.align([g[i][1] - g[i][0] for i in idx], kinds, p, gaps)
            if ks is None:
                continue
            allidx += idx
            allks += ks
        T = p.get("tail")
        place = (lambda i, k, g=g: L.tail_cuts(g[i][0], g[i][1], k, T)) if T else None
        gl.append(D.apply_counts(g, allidx, allks, merge_zero=p.get("merge", True), place=place))
    notes = [L.from_grid(s, gg) for s, gg in zip(songs, gl)]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    return L.report(f"{out} {label}", songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


if __name__ == "__main__":
    for out in ("lead", "stage"):
      for beta, gap, skip in itertools.product([0.0, 0.5, 1.0], [None, 0.0, 0.5], [1.0, 3.0]):
        p = dict(beta=beta, zero=2.0, skip=skip, kmax=8, kind_aware=True, flat=1.0, merge=True, gap=gap, tail=2)
        run(out, p, f"tail2 b={beta} gap={gap} skip={skip}")
