"""07 step 1b (realistic): align a whole song's mora stream (no note boundaries) to sona's notes and rests
by DP, then split notes that receive several moras (tail placement) and merge same-pitch notes that receive
none. Text sources: UST kana (perfect text) with or without line breaks at sung phrases; the wiki lyric text
read by fugashi (`texts/<id>.json`, made by mora_text.py)."""
import json, math, os, sys, itertools
import lib07 as L
import lyric_dp as D

HERE = L.WORK


def ust_stream(s, breaks, min_rest_cells=0):
    u = L.ust_notes(s)
    out = []
    prev_off = None
    for x in u:
        ls = breaks and (prev_off is None or x["on"] - prev_off > max(1e-3, (min_rest_cells - 0.5) * s["spacing"]))
        out.append((D.kind_of(x["lyric"]), ls))
        prev_off = x["off"]
    return out


def wiki_stream(s):
    p = os.path.join(HERE, "texts", f"{s['id']}.json")
    if not os.path.exists(p):
        return None
    return [(k, bool(ls)) for k, ls in json.load(open(p))]


def items_of(g, R):
    """Notes and rests in order: ('n', cells, phrase_start, note index) / ('g', cells)."""
    it = []
    prev = None
    for i, (a, b, _) in enumerate(g):
        if prev is not None and a > prev:
            it.append(("g", a - prev, False, None))
        it.append(("n", b - a, prev is None or a - prev >= R, i))
        prev = b
    return it


def align_song(items, moras, p):
    N, M = len(items), len(moras)
    INF = float("inf")
    cpm = sum(c for t, c, _, _ in items if t == "n") / max(1, M)
    kinds = [k for k, _ in moras]
    ls = [b for _, b in moras]
    pb = p["pb"] if any(ls) else 0.0  # a stream without line breaks says nothing about phrases
    dp = [[INF] * (M + 1) for _ in range(N + 1)]
    bk = [[None] * (M + 1) for _ in range(N + 1)]
    dp[0][0] = 0.0
    for j in range(1, M + 1):
        dp[0][j] = dp[0][j - 1] + p["skip"]
        bk[0][j] = -1
    for i in range(1, N + 1):
        typ, Li, pstart, _ = items[i - 1]
        row, prow = dp[i], dp[i - 1]
        brow = bk[i]
        for j in range(M + 1):
            if typ == "g":
                best, arg = prow[j], 0
                for k in range(1, min(Li, p["cap"], j) + 1):
                    c = prow[j - k] + p["gap"] * k
                    if c < best:
                        best, arg = c, k
            else:
                best, arg = prow[j] + p["zero"], 0
                att = 0.0  # attach cost of moras first+1 .. j-1 (block = moras first .. j-1)
                for k in range(1, min(Li, p["kmax"], j) + 1):
                    first = j - k
                    if k > 1:
                        att += D.ATTACH[kinds[first + 1]] + (pb if ls[first + 1] else 0.0)
                    c = prow[first]
                    if c == INF:
                        continue
                    c += att + p["beta"] * abs(math.log((Li / k) / cpm))
                    if ls[first] != pstart:
                        c += pb
                    if c < best:
                        best, arg = c, k
            if j > 0 and row[j - 1] + p["skip"] < best:
                best, arg = row[j - 1] + p["skip"], -1
            row[j], brow[j] = best, arg
    ks = [0] * N
    i, j = N, M
    while i > 0 or j > 0:
        a = bk[i][j]
        if a == -1:
            j -= 1
        else:
            ks[i - 1] = a
            j -= a
            i -= 1
    return ks


def run_song(s, out, stream, p):
    g = L.to_grid(s, s["outputs"][out])
    items = items_of(g, p["R"])
    ks = align_song(items, stream, p)
    idx = [it[3] for it in items if it[0] == "n"]
    kn = [k for it, k in zip(items, ks) if it[0] == "n"]
    T = p.get("tail", 2)
    return D.apply_counts(g, idx, kn, merge_zero=p.get("merge", True),
                          place=lambda i, k: L.tail_cuts(g[i][0], g[i][1], k, T))


def evaluate(songs, out, src, p, label):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    rows, eqs = [], []
    for s, b in zip(songs, base):
        stream = {"ust": lambda: ust_stream(s, False), "ust+breaks": lambda: ust_stream(s, True),
                  "ust+breaksR": lambda: ust_stream(s, True, p["R"]), "wiki": lambda: wiki_stream(s)}[src]()
        if stream is None:
            rows.append(b)
            notes = s["outputs"][out]
        else:
            notes = L.from_grid(s, run_song(s, out, stream, p))
            rows.append(L.score(s, notes))
        eqs.append(L.phrase_equal_share(s, notes))
    return L.report(f"{out} {src:10} {label}", songs, base, rows, f"phr= {100 * sum(e for e, _ in eqs) / sum(n for _, n in eqs):.1f}%")


if __name__ == "__main__":
    songs = L.load()
    srcs = sys.argv[1].split(",") if len(sys.argv) > 1 else ["ust", "ust+breaks", "wiki"]
    for out in ("lead", "stage"):
        for src in srcs:
            for beta, gap, pb, cap in itertools.product([0.5], [0.0, 0.5], [1.0, 3.0], [2, 4]):
                p = dict(beta=beta, gap=gap, cap=cap, skip=3.0, zero=2.0, kmax=8, pb=pb, R=4, tail=2, merge=True)
                evaluate(songs, out, src, p, f"b={beta} gap={gap} pb={pb} cap={cap}")
