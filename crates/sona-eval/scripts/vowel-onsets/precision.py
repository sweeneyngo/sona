"""Lyrics-first 17: cell precision of per-mora onset sources against the UST's note starts, on dev.

A source is a directory $L17_WORK/onsets/<name>/<id>.json holding {"on": [seconds or null, one per mora of
$L14_WORK/text/<id>.json]}. A mora's onset is right when it rounds (on sona's grid phase) to the cell its UST
note starts on. Reported per source: exact cell, within one cell, the median signed error in cells, by
consonant class and per song; and the start moves it would make on today's aligned fit (14's lines and spaces,
the tail rule): a move is a placed note whose start is 1 … tol cells from its mora's onset cell. A move fixes
the start when the onset cell is the UST's, breaks it when the fitted start was the UST's.

Usage: L01_DUMP=… L01_WORK=… L14_WORK=… L17_WORK=… python3 precision.py SOURCE… [--per-song] [--tol 1]
Everything it reads is lyric-derived and stays outside the repo; it prints pooled numbers and per-song rates.
"""
import bisect, collections, json, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "mora-split"))
import split as S  # noqa: E402

L, O = S.L, S.O
W17 = os.environ["L17_WORK"]

# Consonant class of a mora (its first kana), for the breakdown.
CLASSES = {
    "vowel": "あいうえおぁぃぅぇぉ",
    "N": "ん",
    "unvoiced stop": "かきくけこたちつてとぱぴぷぺぽ",
    "voiced stop": "がぎぐげごだぢづでどばびぶべぼ",
    "fricative": "さしすせそはひふへほざじずぜぞ",
    "nasal": "なにぬねのまみむめも",
    "approximant": "らりるれろやゆよわを",
}
CLASS_OF = {k: c for c, ks in CLASSES.items() for k in ks}


def mora_class(kana):
    return CLASS_OF.get(kana[0], "other") if kana else "other"


def fitted_starts(s, text, times):
    """Start cell of the note carrying each mora in today's aligned fit (None: not placed or not a note's
    first mora). Mirrors split.fit_timed with the tail rule."""
    u = s["ust_l"]
    groups = S.groups_of(s, text, times, "aligned")
    sp = sorted(groups, key=lambda g: g[2])
    starts = [g[2] for g in sp]
    member = [[] for _ in sp]
    for c in O.out_cells(s, "lead"):
        mid = O.t_of(s, (c[0] + c[1]) / 2)
        k = bisect.bisect_right(starts, mid) - 1
        if k >= 0 and mid < sp[k][3]:
            member[k].append(c)
    res = [None] * len(u)
    for (a, b, t0, t1), notes in zip(sp, member):
        if not notes:
            continue
        span = (O.g_of(s, t0), O.g_of(s, t1))
        lyr = [u[i][3] for i in range(a, b)]
        for x, _y, _p, m in S.fit_phrase(notes, lyr, span, [None] * (b - a), {}):
            if m is not None:
                res[a + m] = x
    return res


def load_source(name, sid):
    p = os.path.join(W17, "onsets", name, sid + ".json")
    return json.load(open(p))["on"]


def summarize(rows):
    n = len(rows)
    if not n:
        return "n 0"
    ex = sum(1 for r in rows if r["d"] == 0)
    w1 = sum(1 for r in rows if abs(r["d"]) <= 1)
    med = statistics.median(r["dc"] for r in rows)
    return f"exact {100 * ex / n:5.1f}%  ±1 {100 * w1 / n:5.1f}%  median {med:+.2f} cell  n {n}"


def moves(rows, tol):
    fix = brk = other = 0
    for r in rows:
        f = r["fit"]
        if f is None or not 1 <= abs(r["c"] - f) <= tol:
            continue
        if r["c"] == r["u"]:
            fix += 1
        elif f == r["u"]:
            brk += 1
        else:
            other += 1
    return fix, brk, other


def main():
    names = [a for a in sys.argv[1:] if not a.startswith("--") and not a.replace(".", "").isdigit()]
    tol = int(S.arg("--tol", "1"))
    songs = [s for s in L.load() if os.path.exists(os.path.join(S.WORK, "text", s["id"] + ".json"))]
    base = {}
    for s in songs:
        text, _al, times = S.song_data(s)
        base[s["id"]] = (text, fitted_starts(s, text, times))
    # The fit itself as a source: its start cells against the UST (placed moras only).
    fitrows = []
    for s in songs:
        text, fit = base[s["id"]]
        for m, f in zip(text["moras"], fit):
            if f is not None:
                u = O.g_of(s, m["on"])
                fitrows.append(dict(d=f - u, dc=f - u))
    print(f"today's aligned fit, placed moras' note starts: {summarize(fitrows)}")
    for name in names:
        allrows, per = [], {}
        for s in songs:
            text, fit = base[s["id"]]
            on = load_source(name, s["id"])
            rows = []
            for m, t, f in zip(text["moras"], on, fit):
                if t is None:
                    continue
                u = O.g_of(s, m["on"])
                c = O.g_of(s, t)
                rows.append(dict(d=c - u, dc=(t - m["on"]) / s["spacing"], c=c, u=u, fit=f, cls=mora_class(m["kana"])))
            per[s["id"]] = rows
            allrows += rows
        cover = len(allrows) / sum(len(base[s["id"]][0]["moras"]) for s in songs)
        fix, brk, oth = moves(allrows, tol)
        print(f"\n{name}: {summarize(allrows)}  coverage {100 * cover:.1f}%  moves ±{tol}: fix {fix} break {brk} other {oth} net {fix - brk:+d}")
        by = collections.defaultdict(list)
        for r in allrows:
            by[r["cls"]].append(r)
        for c in list(CLASSES) + ["other"]:
            if by[c]:
                f1, b1, _ = moves(by[c], tol)
                print(f"    {c:14} {summarize(by[c])}  fix {f1} break {b1}")
        if "--per-song" in sys.argv:
            for sid, rows in per.items():
                f1, b1, _ = moves(rows, tol)
                print(f"    {sid:28} {summarize(rows)}  fix {f1} break {b1}")


if __name__ == "__main__":
    main()
