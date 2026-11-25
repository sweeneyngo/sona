"""Lyrics-first 14: score aligned lines against the UST, write the `--lyric-line-times` files, and land the
lyric (01's b+/r and b/r with the aligner's times) on GAME lead and the stage, as the prototype of the Rust
landing.

Line metrics: a line's start error is the aligned start minus the UST onset of its first mora, in ms and in
notes (the UST note whose onset is nearest the aligned start, against the line's first note). The doubt flag
is the line's mean per-mora score (lower = more doubtful), its pace (seconds per mora far from the song's
median), or the more doubtful of the two by rank.

Usage: L01_DUMP=… L01_WORK=… L14_WORK=… python3 score.py NAME [NAME …] [--end own|next|energy|ust] [--per-song]
       [--dump-rows FILE]
Writes $L14_WORK/times/<NAME>[-<end>]/<id>.json: {"lines": [[first, start_s, end_s, score]], "pieces": […]}.
"""
import bisect, json, math, os, random, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "lyric-placement"))
import lib01 as L  # noqa: E402
import oracles as O  # noqa: E402

WORK = os.environ["L14_WORK"]


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


QUIET_DB, QUIET_S, MAX_HOLD_S = -20.0, 0.1, 4.0


def quiet_after(rms, t, limit):
    """The first time after `t` where the lead stays QUIET_DB below its loud level for QUIET_S, before `limit`."""
    db, h = rms["db"], rms["hop_s"]
    need = max(1, round(QUIET_S / h))
    i, run = int(t / h), 0
    while i < len(db) and i * h < limit:
        run = run + 1 if db[i] < QUIET_DB else 0
        if run >= need:
            return (i - need + 1) * h
        i += 1
    return min(limit, len(db) * h)


def spans(text, al, firsts, end_rule, rms=None):
    """[(first, start_s, end_s, score)] per piece starting at each index of `firsts`. A piece ends where the
    aligner's last token ends (`own`: CTC ends are early, the peak of the last mora); `next`: extended to the next
    piece's start, at most 2 s; `energy`: where the lead goes quiet after the last mora's onset (QUIET_DB for
    QUIET_S), at most MAX_HOLD_S later and never past the next piece's start."""
    n = len(text["moras"])
    b = firsts + [n]
    out = []
    for k, (x, y) in enumerate(zip(b, b[1:])):
        start = al["moras"][x][0]
        end = al["moras"][y - 1][1]
        nxt = al["moras"][y][0] if y < n else float("inf")
        if end_rule == "next" and y < n:
            end = max(end, min(nxt, end + 2.0))
        if end_rule == "ust":
            # Diagnostic oracle: the aligner's start with the UST's own end, to split start error from end error.
            end = text["moras"][y - 1]["off"]
        if end_rule == "energy":
            last_on = al["moras"][y - 1][0]
            end = max(end, quiet_after(rms, last_on + QUIET_S, min(nxt, last_on + MAX_HOLD_S)))
        score = sum(m[2] for m in al["moras"][x:y]) / (y - x)
        out.append([x, start, end, score])
    return out


def line_errors(text, lines):
    ons = [m["on"] for m in text["moras"]]
    rows = []
    n = len(ons)
    b = [x[0] for x in lines] + [n]
    for (first, start, end, score), nxt in zip(lines, b[1:]):
        j = bisect.bisect_left(ons, start)
        near = min((c for c in (j - 1, j) if 0 <= c < len(ons)), key=lambda c: abs(ons[c] - start))
        pace = math.log(max(end - start, 0.02) / (nxt - first))
        rows.append(dict(ms=1000 * (start - ons[first]), notes=near - first, score=score, first=first, pace=pace))
    mid = statistics.median(r["pace"] for r in rows)
    for r in rows:
        # Doubt from pace: a line squeezed or stretched far from the song's usual seconds per mora.
        r["pace_doubt"] = -abs(r["pace"] - mid)
    return rows


def mora_errors(text, al):
    ons = [m["on"] for m in text["moras"]]
    out = []
    for i, (start, _, _) in enumerate(al["moras"]):
        j = bisect.bisect_left(ons, start)
        near = min((c for c in (j - 1, j) if 0 <= c < len(ons)), key=lambda c: abs(ons[c] - start))
        out.append((1000 * (start - ons[i]), near - i))
    return out


def auc(scores_bad, scores_good):
    """P(a bad line scores lower than a good one)."""
    if not scores_bad or not scores_good:
        return float("nan")
    wins = sum((b < g) + 0.5 * (b == g) for b in scores_bad for g in scores_good)
    return wins / (len(scores_bad) * len(scores_good))


def fit_timed(s, cells, groups, rests=True):
    """01's fit_spans with each group's span from the aligner: groups = [(first, last+1, start_s, end_s)]."""
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
        span = (O.g_of(s, t0), O.g_of(s, t1)) if rests else None
        out += L.fit_phrase_lyric(notes, lyr, span, 0.0) if notes else []
    return sorted(out)


def main():
    names = [a for a in sys.argv[1:] if not a.startswith("--") and a not in (arg("--end"), arg("--dump-rows"))]
    end_rule = arg("--end", "own")
    songs = {s["id"]: s for s in L.load()}
    ids = sorted(f[:-5] for f in os.listdir(os.path.join(WORK, "text")))
    allrows = {}
    base = {}
    for out in ("lead", "stage"):
        for v in ("none", "d", "b/r", "b+/r"):
            sc = [O.score(songs[sid], O.variant(songs[sid], out, v)) for sid in ids]
            base[(out, v)] = ([e for e, _ in sc], [p for _, p in sc])
        for v in ("none", "d", "b/r", "b+/r"):
            pp = L.pool(base[(out, v)][1])
            print(f"{out:5} {v:5} cost/min {O.ec_cpm(O.ec_pool(base[(out, v)][0])):6.1f}  wrong/min {L.wrong_per_min(pp):6.1f}  placed {L.placed_pct(pp):5.1f}%")
    for name in names:
        tag = name if end_rule == "own" else f"{name}-{end_rule}"
        os.makedirs(os.path.join(WORK, "times", tag), exist_ok=True)
        lrows, mrows, per = [], [], []
        land = {("lead", "b+/t"): [], ("lead", "b/t"): [], ("stage", "b+/t"): [], ("stage", "b/t"): []}
        for sid in ids:
            p = os.path.join(WORK, "align", name, sid + ".json")
            if not os.path.exists(p):
                print(f"{name}: no alignment for {sid}")
                continue
            text = json.load(open(os.path.join(WORK, "text", sid + ".json")))
            al = json.load(open(p))
            rms = json.load(open(os.path.join(WORK, "rms", sid + ".json"))) if end_rule == "energy" else None
            lines = spans(text, al, text["lines"], end_rule, rms)
            pieces = spans(text, al, text["pieces"], end_rule, rms)
            json.dump({"lines": lines, "pieces": pieces}, open(os.path.join(WORK, "times", tag, sid + ".json"), "w"))
            le = line_errors(text, lines)
            for r in le:
                r["song"] = sid
            lrows += le
            mrows += mora_errors(text, al)
            per.append((sid, le))
            s = songs[sid]
            n = len(s["ust_l"])
            for out in ("lead", "stage"):
                cells = O.out_cells(s, out)
                for v, ps in (("b+/t", pieces), ("b/t", lines)):
                    b = [x[0] for x in ps] + [n]
                    groups = [(x[0], y, x[1], x[2]) for x, y in zip(ps, b[1:])]
                    land[(out, v)].append((sid, O.score(s, fit_timed(s, cells, groups))))
        allrows[tag] = land
        within = lambda rs: 100 * sum(abs(r["notes"]) <= 1 for r in rs) / len(rs)
        exact = lambda rs: 100 * sum(r["notes"] == 0 for r in rs) / len(rs)
        med = statistics.median(abs(r["ms"]) for r in lrows)
        print(f"\n== {tag}: {len(lrows)} lines; start error |ms| median {med:.0f}, p90 {sorted(abs(r['ms']) for r in lrows)[int(0.9 * len(lrows))]:.0f}; "
              f"exact note {exact(lrows):.1f}%, within one note {within(lrows):.1f}%, within 3 {100 * sum(abs(r['notes']) <= 3 for r in lrows) / len(lrows):.1f}%")
        rng = random.Random(1)
        bs = sorted(within([r for sid, le in [per[rng.randrange(len(per))] for _ in per] for r in le]) for _ in range(2000))
        print(f"   within one note, song bootstrap 95%: [{bs[50]:.1f}, {bs[1949]:.1f}]")
        mm = statistics.median(abs(e[0]) for e in mrows)
        print(f"   moras: |onset error| median {mm:.0f} ms, within one note {100 * sum(abs(e[1]) <= 1 for e in mrows) / len(mrows):.1f}%, exact {100 * sum(e[1] == 0 for e in mrows) / len(mrows):.1f}%")
        rank = lambda k: {id(r): i / len(lrows) for i, r in enumerate(sorted(lrows, key=lambda r: r[k]))}
        rs, rp = rank("score"), rank("pace_doubt")
        for r in lrows:
            r["both"] = min(rs[id(r)], rp[id(r)])
        for k in ("score", "pace_doubt", "both"):
            bad = [r[k] for r in lrows if abs(r["notes"]) > 1]
            good = [r[k] for r in lrows if abs(r["notes"]) <= 1]
            cut = sorted(r[k] for r in lrows)[int(0.2 * len(lrows))]
            caught = sum(r[k] < cut for r in lrows if abs(r["notes"]) > 1)
            print(f"   doubt by {k:10}: {len(bad)} lines off by more than one note; the most doubtful 20% of lines catch {caught} ({100 * caught / max(len(bad), 1):.0f}%); AUC {auc(bad, good):.2f}")
        if "--per-song" in sys.argv:
            for sid, le in per:
                print(f"   {sid:28} lines {len(le):3}  within one {within(le):5.1f}%  exact {exact(le):5.1f}%  median |ms| {statistics.median(abs(r['ms']) for r in le):6.0f}")
        for (out, v), rows in land.items():
            ecs = [e for _, (e, _) in rows]
            pls = [p for _, (_, p) in rows]
            pp = L.pool(pls)
            print(f"   {out:5} {v:5} cost/min {O.ec_cpm(O.ec_pool(ecs)):6.1f}  wrong/min {L.wrong_per_min(pp):6.1f}  placed {L.placed_pct(pp):5.1f}%  landed {L.landed_pct(pp):5.1f}%")
            if "--per-song" in sys.argv:
                for (sid, (e, p)) in rows:
                    print(f"       {sid:28} {O.ec_cpm(O.ec_pool([e])):6.1f} wrong {L.wrong_per_min(p):6.1f} placed {L.placed_pct(p):5.1f}%")
        json.dump(lrows, open(os.path.join(WORK, "times", tag, "_line_errors.json"), "w"))
        for out in ("lead", "stage"):
            for v in ("b+/t", "b/t"):
                ecs = [e for _, (e, _) in land[(out, v)]]
                pls = [p for _, (_, p) in land[(out, v)]]
                have = [ids.index(sid) for sid, _ in land[(out, v)]]
                sub = lambda k: ([base[(out, k)][0][i] for i in have], [base[(out, k)][1][i] for i in have])
                d, dlo, dhi = L.paired(sub("none")[0], ecs, O.ec_cpm, pool_fn=O.ec_pool)
                w, wlo, whi = L.paired(sub("d")[1], pls, L.wrong_per_min)
                ceil = "b+/r" if v == "b+/t" else "b/r"
                cd, cdlo, cdhi = L.paired(sub(ceil)[0], ecs, O.ec_cpm, pool_fn=O.ec_pool)
                cw, cwlo, cwhi = L.paired(sub(ceil)[1], pls, L.wrong_per_min)
                print(f"   {out:5} {v:5} vs today: cost {d:+6.1f} [{dlo:+6.1f}, {dhi:+6.1f}]  wrong/min {w:+6.1f} [{wlo:+6.1f}, {whi:+6.1f}]   vs ceiling {ceil}: cost {cd:+6.1f} [{cdlo:+6.1f}, {cdhi:+6.1f}]  wrong/min {cw:+6.1f} [{cwlo:+6.1f}, {cwhi:+6.1f}]")
    if arg("--dump-rows"):
        rows = {}
        for tag, land in allrows.items():
            for (out, v), rs in land.items():
                rows[f"{out}|{tag}|{v}"] = [[sid, [e["counts"][k] for k in L.ec.CLASSES], p] for sid, (e, p) in rs]
        json.dump(rows, open(arg("--dump-rows"), "w"))


if __name__ == "__main__":
    main()
