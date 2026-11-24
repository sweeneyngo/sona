"""Lyrics-first 01: edit cost and lyric placement on dev for (a) UST phrases, (a') 07 as ported, (b) the
lyric text's lines, (b+) lines and spaces, (c) the whole song, (d) today's paste in order; plus line breaks
against sona's rests. Usage: L01_DUMP=… L01_WORK=… python3 oracles.py [--per-song] [--dump-rows FILE]
"""
import bisect, json, sys
import lib01 as L
from lib01 import ec

OUTS = ("lead", "stage")


def g_of(s, t):
    return ec.rround((t - s["onset_phase"]) / s["spacing"])


def t_of(s, g):
    return s["onset_phase"] + g * s["spacing"]


def out_cells(s, out):
    return [(g_of(s, a), g_of(s, b), int(round(m))) for a, b, m in s["outputs"][out]]


def to_secs(s, cells):
    return [(t_of(s, a), t_of(s, b), p, l) for a, b, p, l in cells]


def ust_phrase_groups(s):
    """UST phrases as lists of indices into s['ust_l']: runs of touching notes on the UST's cells."""
    sp, an = s["spacing"], s["ust_anchor"]
    groups, prev_end = [], None
    for i, (a, b, _, _) in enumerate(s["ust_l"]):
        ca, cb = ec.rround((a - an) / sp), ec.rround((b - an) / sp)
        if groups and ca <= prev_end:
            groups[-1].append(i)
        else:
            groups.append([i])
        prev_end = cb
    return groups


def groups_from_starts(n, starts):
    b = [0] + sorted(set(x for x in starts if 0 < x < n)) + [n]
    return [list(range(x, y)) for x, y in zip(b, b[1:]) if y > x]


def fit_spans(s, cells, groups, rests=False):
    """Each group of UST notes is a phrase: its span runs from its first note's start to its last note's end;
    the output notes whose midpoint lies inside get its moras, fitted. Other notes carry none."""
    u = s["ust_l"]
    spans = [(u[g[0]][0], u[g[-1]][1], [u[i][3] for i in g]) for g in groups]
    starts = [a for a, _, _ in spans]
    member = [[] for _ in spans]
    loose = []
    for c in cells:
        mid = t_of(s, (c[0] + c[1]) / 2)
        k = bisect.bisect_right(starts, mid) - 1
        if k >= 0 and mid < spans[k][1]:
            member[k].append(c)
        else:
            loose.append((c[0], c[1], c[2], None))
    out = loose
    for (a, b, lyr), notes in zip(spans, member):
        span = (g_of(s, a), g_of(s, b)) if rests else None
        out += L.fit_phrase_lyric(notes, lyr, span, REST_COST) if notes else []
    return sorted(out)


def sona_phrases(s, cells):
    """07 as ported (`fit_to_sung_phrases`): runs of touching output notes; the UST notes whose midpoint lies inside."""
    mids = sorted((g_of(s, a) + g_of(s, b), i) for i, (a, b, _, _) in enumerate(s["ust_l"]))
    keys = [m for m, _ in mids]
    out, i = [], 0
    while i < len(cells):
        j = i + 1
        while j < len(cells) and cells[j][0] <= cells[j - 1][1]:
            j += 1
        lo, hi = bisect.bisect_left(keys, 2 * cells[i][0]), bisect.bisect_left(keys, 2 * cells[j - 1][1])
        out += L.fit_phrase_lyric(cells[i:j], [s["ust_l"][k][3] for _, k in mids[lo:hi]])
        i = j
    return out


def paste_in_order(s, cells):
    """Today: the UST's moras one per note, in order, from the first output note that ends after the UST starts."""
    first_on = s["ust_l"][0][0]
    out, k = [], 0
    lyr = [x[3] for x in s["ust_l"]]
    for c in cells:
        if t_of(s, c[1]) > first_on + 0.5 * s["spacing"] and k < len(lyr):
            out.append((c[0], c[1], c[2], lyr[k]))
            k += 1
        else:
            out.append((c[0], c[1], c[2], None))
    return out


def variant(s, out, name):
    cells = out_cells(s, out)
    rests = name.endswith("/r")
    name = name.removesuffix("/r")
    n = len(s["ust_l"])
    if name == "none":
        return [(a, b, p, None) for a, b, p in cells]
    if name == "d":
        return paste_in_order(s, cells)
    if name == "a":
        return fit_spans(s, cells, ust_phrase_groups(s), rests)
    if name == "a'":
        return sona_phrases(s, cells)
    if name == "c":
        return fit_spans(s, cells, [list(range(n))], rests)
    lines = s["lines"]
    if name == "b":
        return fit_spans(s, cells, groups_from_starts(n, lines["line_starts"]), rests)
    if name == "b+":
        return fit_spans(s, cells, groups_from_starts(n, lines["space_starts"]), rests)
    if name == "b|a":
        ust_starts = [g[0] for g in ust_phrase_groups(s)]
        return fit_spans(s, cells, groups_from_starts(n, lines["line_starts"] + ust_starts), rests)
    raise ValueError(name)


def score(s, notes):
    secs = to_secs(s, notes)
    e = ec.analyse(s["reference"], [(a, b, p) for a, b, p, _ in secs], s["spacing"], s["ust_anchor"], s["division"])
    pl = L.placement(s["ust_l"], secs, s["spacing"], s["ust_anchor"])
    return e, pl


def ec_pool(rows):
    import collections
    c = collections.Counter()
    span = 0.0
    for r in rows:
        c.update(r["counts"])
        span += r["span_s"]
    return dict(counts=dict(c), span_s=span)


def ec_cpm(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


ALL_VARIANTS = ("none", "d", "a'", "a", "b", "b+", "b|a", "c", "a/r", "b/r", "b+/r", "b|a/r", "c/r")
REST_COST = float(sys.argv[sys.argv.index("--rest-cost") + 1]) if "--rest-cost" in sys.argv else 0.0
VARIANTS = tuple(sys.argv[sys.argv.index("--variants") + 1].split(",")) if "--variants" in sys.argv else ALL_VARIANTS


def main():
    songs = L.load()
    per_song = "--per-song" in sys.argv
    res = {}
    for out in OUTS:
        for v in VARIANTS:
            res[(out, v)] = [score(s, variant(s, out, v)) for s in songs]
    for s, (e, _) in zip(songs, res[("lead", "none")]):
        assert [e["counts"][k] for k in ec.CLASSES] == s["edit_cost"]["lead"], s["id"]
    print(f"dev, {len(songs)} songs. edit cost cost/min [95%], Δ vs no fit [paired 95%]; wrong moras/min [95%], Δ vs (d); placed %; phrases landed %")
    for out in OUTS:
        base = [e for e, _ in res[(out, "none")]]
        dpl = [p for _, p in res[(out, "d")]]
        print(f"\n{out}")
        for v in VARIANTS:
            ecs = [e for e, _ in res[(out, v)]]
            pls = [p for _, p in res[(out, v)]]
            c = ec_cpm(ec_pool(ecs))
            d, dlo, dhi = L.paired(base, ecs, ec_cpm, pool_fn=ec_pool)
            pp = L.pool(pls)
            wlo, whi = L.boot(pls, L.wrong_per_min)
            wd, wdlo, wdhi = L.paired(dpl, pls, L.wrong_per_min)
            print(f"  {v:5} {c:6.1f}  Δ {d:+6.1f} [{dlo:+6.1f}, {dhi:+6.1f}]   wrong {L.wrong_per_min(pp):6.1f} [{wlo:5.1f}, {whi:5.1f}] "
                  f"Δ(d) {wd:+6.1f} [{wdlo:+6.1f}, {wdhi:+6.1f}]   placed {L.placed_pct(pp):5.1f}%  landed {L.landed_pct(pp):5.1f}%  ({pp['landed']}/{pp['phrases']})")
            if per_song:
                for s, e, p in zip(songs, ecs, pls):
                    print(f"      {s['id']:28} {ec_cpm(ec_pool([e])):6.1f} wrong {L.wrong_per_min(p):6.1f} placed {L.placed_pct(p):5.1f}% landed {L.landed_pct(p):5.1f}%")
    if "--dump-rows" in sys.argv:
        path = sys.argv[sys.argv.index("--dump-rows") + 1]
        json.dump({f"{o}|{v}": [[s["id"], [e["counts"][k] for k in ec.CLASSES], p] for s, (e, p) in zip(songs, rows)] for (o, v), rows in res.items()}, open(path, "w"))


if __name__ == "__main__":
    main()
