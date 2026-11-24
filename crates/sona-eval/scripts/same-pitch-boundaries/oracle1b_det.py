"""Per-phrase kana DP (perfect text and phrases) with detector evidence for the extra moras; also detector
placement within +-1 cell of the tail cut."""
import itertools
import numpy as np
import lib07 as L
import lyric_dp as D
import det
from oracle1b_phrase import phrase_items, songs

CFG = dict(feat="mfcc", w=6, alpha=0.0, lead=0.0)


def run(out, p, label, nudge=False):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    gl = []
    for s in songs:
        g = L.to_grid(s, s["outputs"][out])
        sc = [dict(row) for row in det.grid_scores(s, g, CFG)]
        allv = np.array([v for r in sc for v in r.values()]) if any(sc) else np.zeros(1)
        mu = float(np.median(allv))
        T = p["tail"]
        allidx, allks = [], []
        for idx, kinds, gaps in phrase_items(s, g):
            if not idx:
                continue

            def ev(npos, k, idx=idx):
                i = idx[npos]
                cuts = L.tail_cuts(g[i][0], g[i][1], k, T)
                return sum(sc[i].get(c, mu) - mu for c in cuts)

            ks = D.align([g[i][1] - g[i][0] for i in idx], kinds, p, gaps, ev)
            if ks is None:
                continue
            allidx += idx
            allks += ks

        def place(i, k):
            cuts = L.tail_cuts(g[i][0], g[i][1], k, T)
            if not nudge:
                return cuts
            out_ = []
            lo = g[i][0]
            for j, c in enumerate(cuts):
                hi = cuts[j + 1] if j + 1 < len(cuts) else g[i][1]
                cand = [x for x in (c - 1, c, c + 1) if lo < x < hi]
                best = max(cand, key=lambda x: (sc[i].get(x, mu) - (0.3 if x != c else 0.0)))
                out_.append(best)
                lo = best
            return out_

        gl.append(D.apply_counts(g, allidx, allks, merge_zero=True, place=place))
    notes = [L.from_grid(s, gg) for s, gg in zip(songs, gl)]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    return L.report(f"{out} {label}", songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


for out in ("lead", "stage"):
    for gamma, nudge in itertools.product([0.0, 1.0, 3.0, 6.0], [False, True]):
        p = dict(beta=0.5, zero=2.0, skip=3.0, kmax=8, kind_aware=True, flat=1.0, merge=True, gap=0.0, tail=2, gamma=gamma)
        run(out, p, f"tail2 gap=0 skip=3 gamma={gamma} nudge={int(nudge)}", nudge)
