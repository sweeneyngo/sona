"""Lyrics-first 02 prototype: place each pasted line on a run of sona's notes without the UST's timing, then
fit its moras with the rest-aware fit (01's b/r, with the lines placed by the UST, is the ceiling).

A Viterbi over lines × note boundaries: line k takes the notes s..e (possibly none); its cost is the fit's DP
cost of its moras on those notes (rests between them may hold moras), with the cells per mora fixed at the
song's; a line that starts where two notes touch costs `pb` more; notes before the first line or after the
last cost NOTE_WITHOUT_MORA each. Then each line is refitted with `fit_phrase_lyric(span=its notes)`.
Usage: L01_DUMP=… L01_WORK=… python3 lines02.py [--pb 0,1,3] [--breaks line|space] [--jobs N] [--dump-rows FILE]
"""
import json, math, sys
from multiprocessing import Pool
import lib01 as L
import oracles as O

INF = float("inf")


def line_kinds(s, starts):
    n = len(s["ust_l"])
    groups = O.groups_from_starts(n, starts)
    return [[L.kind(s["ust_l"][i][3]) for i in g] for g in groups], groups


def seg_costs(cells, kinds, s, max_len, cpm):
    """cost[e - s] for the line's moras on notes s..e-1 (e = s .. s + max_len); rests between notes may hold moras."""
    m = len(kinds)
    row = [j * L.MORA_WITHOUT_NOTE for j in range(m + 1)]
    out = [row[m]]
    for e in range(s, min(len(cells), s + max_len)):
        items = []
        if e > s and cells[e][0] > cells[e - 1][1]:
            items.append((False, cells[e][0] - cells[e - 1][1]))
        items.append((True, cells[e][1] - cells[e][0]))
        for is_note, ln in items:
            new = [INF] * (m + 1)
            for j in range(m + 1):
                best = row[j] + (L.NOTE_WITHOUT_MORA if is_note else 0.0)
                if j > 0 and new[j - 1] + L.MORA_WITHOUT_NOTE < best:
                    best = new[j - 1] + L.MORA_WITHOUT_NOTE
                att = 0.0
                for k in range(1, min(ln, L.MAX_MORAS_PER_NOTE, j) + 1):
                    if is_note:
                        if k > 1:
                            att += L.ATTACH[kinds[j - k + 1]]
                        c = row[j - k] + att + L.DURATION_WEIGHT * abs(math.log((ln / k) / cpm))
                    else:
                        c = row[j - k]
                    if c < best:
                        best = c
                new[j] = best
            row = new
        out.append(row[m])
    return out


def place_lines(cells, lines, pb, window=60):
    """Note ranges [(s, e)] per line by Viterbi."""
    N, K = len(cells), len(lines)
    M = sum(len(x) for x in lines)
    cpm = sum(b - a for a, b, _ in cells) / max(M, 1)
    cum = [0]
    for x in lines:
        cum.append(cum[-1] + len(x))
    touch = [False] + [cells[i][0] <= cells[i - 1][1] for i in range(1, N)]
    V = [[INF] * (N + 1) for _ in range(K + 1)]
    back = [[None] * (N + 1) for _ in range(K + 1)]
    for e in range(N + 1):
        V[0][e] = L.NOTE_WITHOUT_MORA * e
    for k in range(K):
        m = len(lines[k])
        max_len = 2 * m + 8
        exp = cum[k] * N / max(M, 1)
        lo, hi = max(0, int(exp - window)), min(N, int(exp + window))
        for s in range(lo, hi + 1):
            if V[k][s] == INF:
                continue
            base = V[k][s] + (pb if s < N and touch[s] and k > 0 else 0.0)
            for d, c in enumerate(seg_costs(cells, lines[k], s, max_len, cpm)):
                e = s + d
                if base + c < V[k + 1][e]:
                    V[k + 1][e], back[k + 1][e] = base + c, s
    end = min(range(N + 1), key=lambda e: V[K][e] + L.NOTE_WITHOUT_MORA * (N - e))
    ranges = [None] * K
    e = end
    for k in range(K, 0, -1):
        s = back[k][e]
        ranges[k - 1] = (s, e)
        e = s
    return ranges


def realistic(s, out, starts, pb):
    cells = O.out_cells(s, out)
    first_on, last_off = s["ust_l"][0][0], s["ust_l"][-1][1]
    inside = [i for i, c in enumerate(cells) if first_on <= O.t_of(s, (c[0] + c[1]) / 2) < last_off]
    lo, hi = (inside[0], inside[-1] + 1) if inside else (0, 0)
    body = cells[lo:hi]
    kinds, groups = line_kinds(s, starts)
    ranges = place_lines(body, kinds, pb)
    res = [(a, b, p, None) for a, b, p in cells[:lo] + cells[hi:]]
    used = set()
    for (a, e), g in zip(ranges, groups):
        notes = body[a:e]
        used.update(range(a, e))
        lyr = [s["ust_l"][i][3] for i in g]
        if notes:
            res += L.fit_phrase_lyric(notes, lyr, (notes[0][0], notes[-1][1]))
    res += [(c[0], c[1], c[2], None) for i, c in enumerate(body) if i not in used]
    return sorted(res, key=lambda x: (x[0], x[1]))


def job(args):
    sid, out, kind, pb = args
    s = [x for x in L.load([sid])][0]
    starts = s["lines"]["line_starts" if kind == "line" else "space_starts"]
    e, p = O.score(s, realistic(s, out, starts, pb))
    return (sid, out, kind, pb), ([e["counts"][k] for k in O.ec.CLASSES], p, e["span_s"])


def main():
    pbs = [float(x) for x in (sys.argv[sys.argv.index("--pb") + 1].split(",") if "--pb" in sys.argv else ["1"])]
    kinds = sys.argv[sys.argv.index("--breaks") + 1].split(",") if "--breaks" in sys.argv else ["line"]
    jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else 4
    songs = L.load()
    tasks = [(s["id"], out, kind, pb) for out in O.OUTS for kind in kinds for pb in pbs for s in songs]
    with Pool(jobs) as pool:
        res = dict(pool.map(job, tasks))
    rows = {}
    for (sid, out, kind, pb), (counts, p, _) in res.items():
        rows.setdefault(f"{out}|{kind}@{pb}", []).append([sid, counts, p])
    for key in rows:
        rows[key].sort()
    if "--dump-rows" in sys.argv:
        json.dump(rows, open(sys.argv[sys.argv.index("--dump-rows") + 1], "w"))
    for key, rs in sorted(rows.items()):
        ecs = [dict(counts=dict(zip(O.ec.CLASSES, c)), span_s=p["span_s"]) for _, c, p in rs]
        pp = L.pool([p for _, _, p in rs])
        print(f"{key:18} edit cost {O.ec_cpm(O.ec_pool(ecs)):6.1f}  wrong {L.wrong_per_min(pp):6.1f}  placed {L.placed_pct(pp):5.1f}%  landed {L.landed_pct(pp):5.1f}%")


if __name__ == "__main__":
    main()
