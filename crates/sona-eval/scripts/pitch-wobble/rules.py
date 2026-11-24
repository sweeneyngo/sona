"""Merge rules from transcription-time evidence, scored with the real metric (dev).

A rule looks at each short output note S (at most `ls` cells) that touches a neighbour N with a pitch step of
1..`smax` semitones (octaves never), and merges S into N (N's pitch) when the evidence says S is a wobble or a
scoop, not a note: optionally GAME's run on the full vocals has no cut there (`novoc`), and sona's pitch track
over S sits at most `xf` of the way from N's pitch to S's (`xf`; a piece with no voiced frame passes only if
`nof0` is set). `pos` restricts S to a head (first piece after a rest) or allows any position. When S could
join either neighbour, it joins the one with the smaller step (the later one on a tie).

Prints, per rule: cost/min with the paired song-bootstrap interval against the unmerged output, the cuts it
merges by truth (chopped / boundary / other) and the true notes it eats (boundary merges: joined, weight 4).
Then the leave-one-song-out choice: the rule with the lowest pooled cost on 8 songs, scored on the 9th.
"""
import itertools, json, os, sys
import wob
import cands as C
import wfeats as F
L = wob.L

GRID = dict(ls=[1, 2], smax=[1, 2, 11], novoc=[False, True], xf=[None, 0.75, 0.5, 0.25], nof0=[True, False], pos=["head", "any"], ties=[False, True])


def rules():
    for v in itertools.product(*GRID.values()):
        r = dict(zip(GRID.keys(), v))
        if r["xf"] is None and not r["nof0"]:
            continue
        yield r


def prepare(s, g):
    tr = wob.Track(s)
    rows = F.features(s, g, C.candidates(s, g, tr), tr)
    return {r["i"]: r for r in rows}


def joins_for(g, rows, rule):
    """Cut indices i (merge g[i] into g[i-1]) chosen by `rule`. Each short piece joins at most one neighbour."""
    want = {}  # short piece index -> (step, prefer-later, cut)
    for i, r in rows.items():
        if r["ls"] > rule["ls"] or abs(r["iv"]) > rule["smax"] or abs(r["iv"]) in (12, 24):
            continue
        if r["ls"] >= r["ln"] and not (rule.get("ties") and r["ls"] == r["ln"] and r["short_first"]):
            continue  # the short piece must be shorter than its neighbour (`ties`: or as long, and first)
        if rule["pos"] == "head" and not (r["short_first"] and r["after_rest"]):
            continue
        if rule["novoc"] and r["voc_cut"]:
            continue
        if rule["xf"] is not None:
            if r["x_f0"] is None:
                if not rule["nof0"]:
                    continue
            elif r["x_f0"] > rule["xf"]:
                continue
        short = i - 1 if r["short_first"] else i
        key = (abs(r["iv"]), 0 if r["short_first"] else 1)
        if short not in want or key < want[short][0]:
            want[short] = (key, i)
    # a piece both short and the absorbing neighbour of another: keep cuts as chosen; merge() chains them
    return {i for _, i in want.values()}


def score_rule(songs, G, P, rule):
    rows, tally = [], {"chopped": 0, "boundary": 0, "other": 0}
    for s in songs:
        g, p = G[s["id"]], P[s["id"]]
        j = joins_for(g, p, rule)
        for i in j:
            tally[p[i]["label"]] += 1
        rows.append(L.score(s, L.from_grid(s, wob.merge(g, j))))
    return rows, tally


def name(r):
    return f"ls<={r['ls']} step<={r['smax']} {'novoc' if r['novoc'] else 'any-voc'} xf<={r['xf']} {'nof0-ok' if r['nof0'] else 'f0-needed'} {r['pos']}{' ties' if r.get('ties') else ''}"


if __name__ == "__main__":
    songs = wob.load()
    outs = sys.argv[1:] or ["lead", "stage"]
    res = {}
    for out in outs:
        G = {s["id"]: wob.grid(s, out) for s in songs}
        P = {s["id"]: prepare(s, G[s["id"]]) for s in songs}
        base = [L.score(s, L.from_grid(s, G[s["id"]])) for s in songs]
        R = list(rules())
        scored = []
        for r in R:
            rows, t = score_rule(songs, G, P, r)
            scored.append((r, rows, t))
        scored.sort(key=lambda x: L.cpm(x[1]))
        print(f"== {out}: baseline {L.cpm(base):.1f}; {len(R)} rules, best 12 (in-sample)")
        for r, rows, t in scored[:12]:
            L.report(name(r), songs, base, rows, f"merged ch/bd/ot {t['chopped']}/{t['boundary']}/{t['other']}")
        # leave one song out: pick the rule on 8 songs, score the 9th
        oof, picks = [], []
        for k, s in enumerate(songs):
            best = min(scored, key=lambda x: L.cpm([row for m, row in enumerate(x[1]) if m != k]))
            oof.append(best[1][k])
            picks.append(name(best[0]))
        L.report("LOSO choice (out of fold)", songs, base, oof)
        for s, p in zip(songs, picks):
            print(f"    held out {s['id']:28} -> {p}")
        res[out] = dict(base=[L.cpm([b]) for b in base], oof=[L.cpm([o]) for o in oof], picks=picks)
    work = os.environ.get("S11_WORK")
    if work:
        json.dump(res, open(os.path.join(work, "rules-loso.json"), "w"), indent=1)
