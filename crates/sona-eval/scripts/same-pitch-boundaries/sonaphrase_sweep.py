"""Robustness of the user-segmented phrase fit (R=1): which parts of the DP matter; per-song deltas."""
import itertools, sys
import lib07 as L
import lyric_dp as D
from oracle1b_sonaphrase import phrase_inputs, songs


def fit(s, out, p):
    g = L.to_grid(s, s["outputs"][out])
    allidx, allks = [], []
    for idx, kinds, _ in phrase_inputs(s, g, 1):
        if not kinds:
            continue
        ks = D.align([g[i][1] - g[i][0] for i in idx], kinds, p)
        allidx += idx
        allks += ks
    T = p["tail"]
    place = (lambda i, k: L.tail_cuts(g[i][0], g[i][1], k, T)) if T else None
    return L.from_grid(s, D.apply_counts(g, allidx, allks, merge_zero=True, place=place))


def run(out, p, label, per_song=False):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    notes = [fit(s, out, p) for s in songs]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    L.report(f"{out} {label}", songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")
    if per_song:
        for s, b, r in zip(songs, base, rows):
            print(f"     {s['id']:28} {L.cpm([b]):6.1f} -> {L.cpm([r]):6.1f}  ({L.cpm([r]) - L.cpm([b]):+6.1f})  clean {L.clean([b]):5.1f}% -> {L.clean([r]):5.1f}%")
    return notes


if __name__ == "__main__":
    P0 = dict(beta=0.5, zero=2.0, skip=3.0, kmax=8, kind_aware=True, flat=1.0, tail=2)
    for out in ("lead", "stage"):
        run(out, P0, "chosen", per_song=True)
        for key, vals in (("beta", [0.0, 1.0]), ("kind_aware", [False]), ("zero", [1.0, 4.0]), ("skip", [1.5, 6.0]), ("tail", [None, 1, 3]), ("kmax", [2, 4])):
            for v in vals:
                p = dict(P0, **{key: v})
                run(out, p, f"{key}={v}")
