"""1b, user-segmented: phrases are sona's own runs of notes between rests of >= R cells; each gets the kana of
the UST notes sung inside its span (a user pasting that phrase's lyric onto its notes). Per-phrase DP."""
import bisect, itertools
import lib07 as L
import lyric_dp as D

songs = L.load()


def sona_phrases(g, R):
    ph = []
    for i, (a, b, _) in enumerate(g):
        if ph and a - g[ph[-1][-1]][1] < R:
            ph[-1].append(i)
        else:
            ph.append([i])
    return ph


def phrase_inputs(s, g, R):
    u = L.ust_notes(s)
    mids = [((L.g_of(s, x["on"]) + L.g_of(s, x["off"])) / 2, D.kind_of(x["lyric"])) for x in u]
    mids.sort()
    keys = [m for m, _ in mids]
    out = []
    for idx in sona_phrases(g, R):
        a, b = g[idx[0]][0], g[idx[-1]][1]
        kinds = [k for _, k in mids[bisect.bisect_left(keys, a):bisect.bisect_left(keys, b)]]
        gaps = [0]
        for x, y in zip(idx, idx[1:]):
            gaps.append(g[y][0] - g[x][1])
        gaps.append(0)
        out.append((idx, kinds, gaps))
    return out


def run(out, p, label):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    gl = []
    for s in songs:
        g = L.to_grid(s, s["outputs"][out])
        allidx, allks = [], []
        for idx, kinds, gaps in phrase_inputs(s, g, p["R"]):
            if not kinds:
                continue
            ks = D.align([g[i][1] - g[i][0] for i in idx], kinds, p, gaps)
            if ks is None:
                continue
            allidx += idx
            allks += ks
        gl.append(D.apply_counts(g, allidx, allks, merge_zero=p["merge"], place=lambda i, k, g=g: L.tail_cuts(g[i][0], g[i][1], k, 2)))
    notes = [L.from_grid(s, gg) for s, gg in zip(songs, gl)]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    return L.report(f"{out} {label}", songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


if __name__ == "__main__":
    for out in ("lead", "stage"):
        for R, gap, skip, merge in itertools.product([1, 2, 4], [0.0, 0.5], [3.0], [True, False]):
            p = dict(beta=0.5, zero=2.0, skip=skip, kmax=8, kind_aware=True, flat=1.0, merge=merge, gap=gap, R=R)
            run(out, p, f"sona phrases R={R} gap={gap} skip={skip} merge={int(merge)}")
