"""Lyrics-first 16: split and time the fitted notes at per-mora onsets, on a `sona-eval-cellstage --dump` of dev.

The landing is 14's (lines and spaces placed by the aligner, `fit_lines` = 01's fit_spans with rests), with the
fit's splits and starts taken from per-mora onsets instead of the tail rule. Onsets come from the UST (oracle), the
UST jittered by the aligner's measured error (oracle), or the aligner (candidate).

Usage: L01_DUMP=… L01_WORK=… L14_WORK=… python3 split.py [--variants a,b,…] [--per-song] [--dump-rows FILE]
Everything it reads is lyric-derived and stays outside the repo; it prints pooled numbers and per-song rates.
"""
import bisect, collections, json, os, random, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "lyric-placement"))
import lib01 as L  # noqa: E402
import oracles as O  # noqa: E402

ec = L.ec
WORK = os.environ["L14_WORK"]
ALIGNER = os.environ.get("L16_ALIGNER", "lyrics-lead")
TIMES = os.environ.get("L16_TIMES", "lyrics-lead-energy")


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


# ---- the fit, with onsets ----

def onset_cuts(note, first, k, on):
    """Cuts at the onset cells of moras first+1 … first+k-1, if they all fall strictly inside the note and
    increase; None otherwise."""
    cs = [on[first + j] for j in range(1, k)]
    if any(c is None for c in cs):
        return None
    if all(note[0] < c < note[1] for c in cs) and all(a < b for a, b in zip(cs, cs[1:])):
        return cs
    return None


def fit_phrase(notes, lyrics, span, on, cfg):
    """lib01.fit_phrase_lyric with rests (span), then cfg's onset rules. `on[i]`: onset cell (int) of the
    phrase's mora i, or None. Returns [(start, end, pitch, mora index within phrase or None)]."""
    if not lyrics:
        return [(a, b, p, None) for a, b, p in notes]
    ends = [span[0]] + [b for _, b, _ in notes]
    rests = [max(0, a - e) for (a, _, _), e in zip(notes, ends)] + [max(0, span[1] - max(ends))]
    blocks = L.mora_blocks([b - a for a, b, _ in notes], [L.kind(l) for l in lyrics], rests, 0.0)
    out = []
    for i, (note, (first, k)) in enumerate(zip(notes, blocks)):
        continues = i > 0 and notes[i - 1][1] == note[0] and notes[i - 1][2] == note[2]
        if out and k == 0 and continues and out[-1][1] == note[0]:
            out[-1] = (out[-1][0], note[1], out[-1][2], out[-1][3])
        elif k > 1:
            cs = onset_cuts(note, first, k, on) if cfg.get("split") else None
            if cs is None:
                cs = [c for c in L.cuts(note, k) if note[0] < c < note[1]]
            start, pieces = note[0], []
            for c in cs + [note[1]]:
                if c > start:
                    pieces.append((start, c))
                    start = c
            for q, (x, y) in enumerate(pieces):
                out.append((x, y, note[2], first + q))
        else:
            out.append((note[0], note[1], note[2], first if k == 1 else None))
    if cfg.get("start") is not None:
        out = move_starts(out, on, cfg["start"], cfg.get("legato", True))
    return out


def move_starts(out, on, tol, legato):
    """Each note carrying a mora whose onset cell is within `tol` cells of its start starts there, if it stays
    at least a cell long. With `legato`, a note touching the one before keeps touching it (the previous note's
    end follows, if that note stays at least a cell long); a start that moves earlier into the previous note
    always cuts it."""
    out = [list(x) for x in out]
    for i, x in enumerate(out):
        m = x[3]
        if m is None or on[m] is None:
            continue
        c = on[m]
        if c == x[0] or abs(c - x[0]) > tol or c >= x[1]:
            continue
        prev = out[i - 1] if i > 0 else None
        if prev is not None and (prev[1] > c or (legato and prev[1] == x[0])):
            if c <= prev[0]:
                continue
            prev[1] = c
        x[0] = c
    return [tuple(x) for x in out]


def fit_timed(s, cells, groups, on_cells, cfg):
    """14's fit_timed (score.py) with per-mora onsets: groups = [(first, last+1, start_s, end_s)]."""
    u = s["ust_l"]
    sp = sorted(groups, key=lambda g: g[2])
    starts = [g[2] for g in sp]
    member = [[] for _ in sp]
    loose = []
    for c in cells:
        mid = O.t_of(s, (c[0] + c[1]) / 2)
        k = bisect.bisect_right(starts, mid) - 1
        if k >= 0 and mid < sp[k][3]:
            member[k].append(c)
        else:
            loose.append((c[0], c[1], c[2], None))
    out = loose
    for (a, b, t0, t1), notes in zip(sp, member):
        lyr = [u[i][3] for i in range(a, b)]
        span = (O.g_of(s, t0), O.g_of(s, t1))
        if notes:
            fitted = fit_phrase(notes, lyr, span, on_cells[a:b], cfg)
            out += [(x, y, p, None if m is None else lyr[m]) for x, y, p, m in fitted]
    return sorted(out)


# ---- data ----

def song_data(s):
    sid = s["id"]
    text = json.load(open(os.path.join(WORK, "text", sid + ".json")))
    al = json.load(open(os.path.join(WORK, "align", ALIGNER, sid + ".json")))
    times = json.load(open(os.path.join(WORK, "times", TIMES, sid + ".json")))
    return text, al, times


def onsets(text, al, source, rng=None, errs=None, bias_s=0.0):
    """Onset seconds per mora: 'ust', 'jitter' (UST + an error drawn from the aligner's pooled errors), 'aligned'."""
    if source == "ust":
        return [m["on"] for m in text["moras"]]
    if source == "jitter":
        return [m["on"] + errs[rng.randrange(len(errs))] for m in text["moras"]]
    return [x[0] + bias_s for x in al["moras"]]


def groups_of(s, text, times, spans):
    n = len(s["ust_l"])
    if spans == "ust":
        # 01's b+/r: pieces placed by the UST's own times.
        firsts = text["pieces"]
        b = firsts + [n]
        return [(x, y, s["ust_l"][x][0], s["ust_l"][y - 1][1]) for x, y in zip(firsts, b[1:])]
    ps = times["pieces"]
    b = [x[0] for x in ps] + [n]
    return [(x[0], y, x[1], x[2]) for x, y in zip(ps, b[1:])]


# ---- scoring ----

def score(s, notes):
    secs = O.to_secs(s, notes)
    e = ec.analyse(s["reference"], [(a, b, p) for a, b, p, _ in secs], s["spacing"], s["ust_anchor"], s["division"])
    pl = L.placement(s["ust_l"], secs, s["spacing"], s["ust_anchor"])
    return dict(counts=e["counts"], span_s=e["span_s"], measures=e["measures"], clean=e["clean"]), pl


def pool_e(rows):
    c = collections.Counter()
    span = meas = clean = 0
    for r in rows:
        c.update(r["counts"])
        span += r["span_s"]
        meas += r["measures"]
        clean += r["clean"]
    return dict(counts=dict(c), span_s=span, measures=meas, clean=clean)


def cpm(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


def clean_pct(p):
    return 100 * p["clean"] / p["measures"]


def boot(rows, stat, pool_fn, iters=10000, seed=1):
    rng = random.Random(seed)
    v = sorted(stat(pool_fn([rows[rng.randrange(len(rows))] for _ in rows])) for _ in range(iters))
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


# name -> (onset source, spans, cfg). cfg: split (cut at onsets), start (tolerance in cells for moving a start to
# its mora's onset), legato (the previous note's end follows when the two touch).
VARIANTS = {
    "none": None,
    "tail": ("aligned", "aligned", {}),
    "ust:split": ("ust", "aligned", {"split": True}),
    "ust:start1": ("ust", "aligned", {"start": 1}),
    "ust:split+start1": ("ust", "aligned", {"split": True, "start": 1}),
    "ust:split+start2": ("ust", "aligned", {"split": True, "start": 2}),
    "ust:split+start9": ("ust", "aligned", {"split": True, "start": 9}),
    "ustspan:tail": ("ust", "ust", {}),
    "ustspan:split+start1": ("ust", "ust", {"split": True, "start": 1}),
    "jit:split": ("jitter", "aligned", {"split": True}),
    "jit:split+start1": ("jitter", "aligned", {"split": True, "start": 1}),
    "al:split": ("aligned", "aligned", {"split": True}),
    "al:start1": ("aligned", "aligned", {"start": 1}),
    "al:split+start1": ("aligned", "aligned", {"split": True, "start": 1}),
    "al:split+start2": ("aligned", "aligned", {"split": True, "start": 2}),
}


def parse_variant(name):
    """A name from VARIANTS, or `<src>:<rules>` with rules from split, start<N>, nolegato joined by '+'."""
    if name in VARIANTS:
        return VARIANTS[name]
    src, rules = name.split(":")
    spans = "aligned"
    if src.endswith("span"):
        src, spans = src[:-4], "ust"
    src = {"al": "aligned", "jit": "jitter"}.get(src, src)
    cfg = {}
    for r in rules.split("+"):
        if r == "split":
            cfg["split"] = True
        elif r.startswith("start"):
            cfg["start"] = int(r[5:])
        elif r == "nolegato":
            cfg["legato"] = False
        elif r != "tail":
            raise ValueError(r)
    return src, spans, cfg


def run(songs, data, names, bias_s=0.0, jitter_seed=7):
    errs = []
    for s in songs:
        text, al, _ = data[s["id"]]
        errs += [x[0] - m["on"] for m, x in zip(text["moras"], al["moras"])]
    res = {}
    for name in names:
        v = parse_variant(name)
        for out in ("lead", "stage"):
            rows = []
            rng = random.Random(jitter_seed)
            for s in songs:
                cells = O.out_cells(s, out)
                if v is None:
                    notes = [(a, b, p, None) for a, b, p in cells]
                else:
                    src, spans, cfg = v
                    text, al, times = data[s["id"]]
                    on_s = onsets(text, al, src, rng, errs, bias_s if src == "aligned" else 0.0)
                    on_c = [O.g_of(s, t) for t in on_s]
                    notes = fit_timed(s, cells, groups_of(s, text, times, spans), on_c, cfg)
                rows.append(score(s, notes))
            res[(out, name)] = rows
    return res


def main():
    names = arg("--variants", ",".join(VARIANTS)).split(",")
    bias_s = float(arg("--bias-ms", "0")) / 1000
    songs = [s for s in L.load() if os.path.exists(os.path.join(WORK, "text", s["id"] + ".json"))]
    data = {s["id"]: song_data(s) for s in songs}
    res = run(songs, data, names, bias_s, int(arg("--jitter-seed", "7")))
    ids = [s["id"] for s in songs]
    for out in ("lead", "stage"):
        print(f"\n{out}: cost/min [95%] · Δ vs none [paired 95%] · Δ vs tail [paired 95%] · clean % · wrong/min · placed % · missing")
        base = [e for e, _ in res[(out, "none")]] if (out, "none") in res else None
        tail = [e for e, _ in res[(out, "tail")]] if (out, "tail") in res else None
        for name in names:
            es = [e for e, _ in res[(out, name)]]
            ps = [p for _, p in res[(out, name)]]
            pe, pp = pool_e(es), L.pool(ps)
            lo, hi = boot(es, cpm, pool_e)
            line = f"  {name:24} {cpm(pe):6.1f} [{lo:5.1f}, {hi:5.1f}]"
            for ref in (base, tail):
                if ref is None:
                    continue
                d, dlo, dhi = L.paired(ref, es, cpm, pool_fn=pool_e)
                line += f"  Δ {d:+6.1f} [{dlo:+6.1f}, {dhi:+6.1f}]"
            line += f"  clean {clean_pct(pe):5.1f}%  wrong {L.wrong_per_min(pp):6.1f}  placed {L.placed_pct(pp):5.1f}%  missing {pe['counts'].get('missing', 0):4d}"
            c = pe["counts"]
            line += "  [" + " ".join(f"{k[:3]} {c.get(k, 0)}" for k in ec.CLASSES) + "]"
            print(line)
            if "--per-song" in sys.argv:
                for sid, e, p in zip(ids, es, ps):
                    pe1 = pool_e([e])
                    print(f"      {sid:28} {cpm(pe1):6.1f} clean {clean_pct(pe1):5.1f}% wrong {L.wrong_per_min(p):6.1f} placed {L.placed_pct(p):5.1f}%")
    if arg("--dump-rows"):
        json.dump({f"{o}|{v}": [[sid, [e["counts"].get(k, 0) for k in ec.CLASSES], e["measures"], e["clean"], p] for sid, (e, p) in zip(ids, rows)] for (o, v), rows in res.items()}, open(arg("--dump-rows"), "w"))


if __name__ == "__main__":
    main()
