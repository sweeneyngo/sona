"""07 step 1b: align a mora stream to sona's notes by DP and split notes that receive several moras.

Mora kinds: 'c' (consonant onset), 'v' (bare vowel), 'n' (ん), 'x' (extension ー / small っ etc.).
A note receives a contiguous block of moras; the first mora of a block costs nothing, each further mora
(attached, i.e. hidden in sona's note) costs ATTACH[kind]. A note receiving no mora costs ZERO (a merge
candidate when it continues the previous note's pitch). A skipped mora costs SKIP. A duration term keeps
cells per mora near the phrase's median.
"""
import math
import lib07 as L
import taxo

ATTACH = dict(c=-math.log(0.15), v=-math.log(0.55), n=-math.log(0.55), x=-math.log(0.55))


def kind_of(lyric):
    k = taxo.mora(lyric)[1]
    return {"cv": "c", "vowel": "v", "n": "n", "ext": "x"}.get(k, "c")


def align(lens, kinds, p, gaps=None, ev=None):
    """lens: cells per note; kinds: mora kinds; gaps: rest cells before each note (a rest can hold moras
    sona missed, at GAP per mora). Returns per-note mora count (list) or None."""
    if gaps and p.get("gap") is not None:
        return _align_gaps(lens, kinds, p, gaps, ev)
    N, M = len(lens), len(kinds)
    INF = float("inf")
    tot = sum(lens)
    cpm = tot / max(1, M)  # cells per mora over the phrase
    dp = [[INF] * (M + 1) for _ in range(N + 1)]
    bk = [[None] * (M + 1) for _ in range(N + 1)]
    dp[0][0] = 0.0
    for j in range(1, M + 1):  # skip leading moras
        dp[0][j] = dp[0][j - 1] + p["skip"]
        bk[0][j] = ("skip",)
    for i in range(1, N + 1):
        Li = lens[i - 1]
        for j in range(0, M + 1):
            best, arg = dp[i - 1][j] + p["zero"], ("k", 0)
            if j > 0 and dp[i][j - 1] + p["skip"] < best:
                best, arg = dp[i][j - 1] + p["skip"], ("skip",)
            acc = 0.0
            for k in range(1, min(Li, p["kmax"], j) + 1):
                # block = moras j-k .. j-1 (0-based); attach cost for all but the first
                if k > 1:
                    acc += ATTACH[kinds[j - k + 1]] if p.get("kind_aware", True) else p["flat"]
                dur = p["beta"] * abs(math.log((Li / k) / cpm))
                c = dp[i - 1][j - k] + acc + dur
                if c < best:
                    best, arg = c, ("k", k)
            dp[i][j], bk[i][j] = best, arg
    if dp[N][M] == INF:
        return None
    out = [0] * N
    i, j = N, M
    while i > 0 or j > 0:
        a = bk[i][j]
        if a[0] == "skip":
            j -= 1
        else:
            out[i - 1] = a[1]
            j -= a[1]
            i -= 1
    return out


def _align_gaps(lens, kinds, p, gaps, ev=None):
    """Items: the rest before note i (cells gaps[i], may be 0), then note i, then a trailing rest. Rest
    items take 0..cells moras at p['gap'] each; notes as in align(). ev(note position, k) is detector
    evidence for k moras in that note (subtracted, times p['gamma'])."""
    items = []
    for n_, (L_, G_) in enumerate(zip(lens, gaps)):
        if G_ > 0:
            items.append(("g", G_, None))
        items.append(("n", L_, n_))
    if len(gaps) > len(lens) and gaps[-1] > 0:
        items.append(("g", gaps[-1], None))
    gamma = p.get("gamma", 0.0)
    N, M = len(items), len(kinds)
    INF = float("inf")
    cpm = sum(lens) / max(1, M)
    dp = [[INF] * (M + 1) for _ in range(N + 1)]
    bk = [[None] * (M + 1) for _ in range(N + 1)]
    dp[0][0] = 0.0
    for j in range(1, M + 1):
        dp[0][j] = dp[0][j - 1] + p["skip"]
        bk[0][j] = ("skip",)
    for i in range(1, N + 1):
        typ, Li, npos = items[i - 1]
        for j in range(M + 1):
            if typ == "g":
                best, arg = dp[i - 1][j], ("k", 0)
                for k in range(1, min(Li, p["kmax"], j) + 1):
                    c = dp[i - 1][j - k] + p["gap"] * k
                    if c < best:
                        best, arg = c, ("k", k)
            else:
                best, arg = dp[i - 1][j] + p["zero"], ("k", 0)
                acc = 0.0
                for k in range(1, min(Li, p["kmax"], j) + 1):
                    if k > 1:
                        acc += ATTACH[kinds[j - k + 1]] if p.get("kind_aware", True) else p["flat"]
                    c = dp[i - 1][j - k] + acc + p["beta"] * abs(math.log((Li / k) / cpm))
                    if gamma and k > 1 and ev is not None:
                        c -= gamma * ev(npos, k)
                    if c < best:
                        best, arg = c, ("k", k)
            if j > 0 and dp[i][j - 1] + p["skip"] < best:
                best, arg = dp[i][j - 1] + p["skip"], ("skip",)
            dp[i][j], bk[i][j] = best, arg
    if dp[N][M] == INF:
        return None
    ks = [0] * N
    i, j = N, M
    while i > 0 or j > 0:
        a = bk[i][j]
        if a[0] == "skip":
            j -= 1
        else:
            ks[i - 1] = a[1]
            j -= a[1]
            i -= 1
    return [k for (typ, _, _), k in zip(items, ks) if typ == "n"]


def apply_counts(g, idx, ks, merge_zero=True, place=None):
    """Split notes idx[i] into ks[i] pieces; merge zero-mora notes into a touching same-pitch previous note."""
    cuts, joins = {}, set()
    for i, k in zip(idx, ks):
        if k > 1:
            cuts[i] = place(i, k) if place else L.equal_cuts(g[i][0], g[i][1], k)
        elif k == 0 and merge_zero and i > 0 and g[i - 1][1] == g[i][0] and g[i - 1][2] == g[i][2]:
            joins.add(i)
    gs = L.split_at(g, cuts)
    # split_at keeps note order; map join indices through the splits
    if joins:
        newidx, pos = {}, 0
        for i, (a, b, m) in enumerate(g):
            newidx[i] = pos
            pos += len([c for c in cuts.get(i, []) if a < c < b]) + 1
        gs = L.merge(gs, {newidx[i] for i in joins})
    return gs
