"""07's ported per-phrase lyric fit (`sona_core::lyric_fit`, `--fit-phrase-lyrics`) applied to any grid notes,
so a merge rule can be measured combined with it. Same settings as 07's `sonaphrase_sweep.P0`."""
import bisect
import wob
import lyric_dp as D
L = wob.L

P0 = dict(beta=0.5, zero=2.0, skip=3.0, kmax=8, kind_aware=True, flat=1.0, tail=2)


def phrase_inputs(s, g):
    u = L.ust_notes(s)
    mids = sorted(((L.g_of(s, x["on"]) + L.g_of(s, x["off"])) / 2, D.kind_of(x["lyric"])) for x in u)
    keys = [m for m, _ in mids]
    ph = []
    for i, (a, b, _) in enumerate(g):
        if ph and a - g[ph[-1][-1]][1] < 1:
            ph[-1].append(i)
        else:
            ph.append([i])
    for idx in ph:
        a, b = g[idx[0]][0], g[idx[-1]][1]
        yield idx, [k for _, k in mids[bisect.bisect_left(keys, a):bisect.bisect_left(keys, b)]]


def fit(s, g, p=P0):
    allidx, allks = [], []
    for idx, kinds in phrase_inputs(s, g):
        if not kinds:
            continue
        allidx += idx
        allks += D.align([g[i][1] - g[i][0] for i in idx], kinds, p)
    return D.apply_counts(g, allidx, allks, merge_zero=True, place=lambda i, k: L.tail_cuts(g[i][0], g[i][1], k, p["tail"]))


if __name__ == "__main__":
    songs = wob.load()
    for out in ("lead", "stage"):
        base = [L.score(s, s["outputs"][out]) for s in songs]
        rows = [L.score(s, L.from_grid(s, fit(s, wob.grid(s, out)))) for s in songs]
        L.report(f"{out} 07 fit alone (expect lead -13.9, stage -17.8)", songs, base, rows)
