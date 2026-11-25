"""Taxonomy and per-slice oracles of the timing errors (length, shift, moved) of one output.

A pair is one UST note matched to one output note. Its timing classes: length (cell counts differ), shift
(start one cell off), moved (two or more); 4 each under the revised weights. So a note whose start alone is
off pays 8 (shift + length), a whole note moved pays 4, an end alone 4.

usage: S18_DUMP=DIR python3 -I taxo18.py lead|stage
"""
import collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import lib12
import ec

OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"
songs = L.loads()
base = [L.analyse(s, s["outputs"][OUT]) for s in songs]
P = L.pool(base)
W = ec.W
total = ec.cost(P["counts"])
print(f"== {OUT}: {L.per_min(P):.1f} cost/min, clean {L.clean(P):.1f}%, missing {L.missing(P)}; timing units "
      f"{sum(W[k] * P['counts'][k] for k in ('length', 'shift', 'moved'))} of {total} "
      f"({100 * sum(W[k] * P['counts'][k] for k in ('length', 'shift', 'moved')) / total:.1f}%) ==")

rows = []
for s, A in zip(songs, base):
    for p in L.pairs(A, s):
        p["song"] = s["id"]
        rows.append(p)
timing = [p for p in rows if p["timing"]]


def tcost(ps):
    return sum(4 * p["timing"] for p in ps)


def share(ps):
    return 100 * tcost(ps) / total


def size(d):
    return "0" if d == 0 else (("+" if d > 0 else "-") + (str(abs(d)) if abs(d) < 3 else "3+"))


print(f"\npairs {len(rows)}, with a timing error {len(timing)} ({100 * len(timing) / len(rows):.1f}%)")
print("\n-- pattern --")
pat = collections.defaultdict(list)
for p in timing:
    if p["ds"] == 0:
        k = "end only (length)"
    elif p["de"] == 0:
        k = "start only (shift/moved + length)"
    elif p["de"] == p["ds"]:
        k = "whole note moved (shift/moved)"
    else:
        k = "start and end, different (shift/moved + length)"
    pat[k].append(p)
for k, ps in sorted(pat.items(), key=lambda kv: -tcost(kv[1])):
    print(f"  {k:48} {len(ps):5} pairs {tcost(ps):6} units {share(ps):5.1f}% of all cost")

print("\n-- start errors: direction/size x UST context before the note --")
tab = collections.defaultdict(list)
for p in timing:
    if p["ds"]:
        tab[(size(p["ds"]), p["start_ctx"])].append(p)
for k, ps in sorted(tab.items(), key=lambda kv: -len(kv[1])):
    print(f"  start {k[0]:3} after {k[1]:7} {len(ps):5}   same-length {sum(1 for p in ps if p['de'] == p['ds']):4}")
print("\n-- end errors: direction/size x UST context after the note --")
tab = collections.defaultdict(list)
for p in timing:
    if p["de"] != p["ds"]:
        tab[(size(p["de"] - p["ds"] if p["ds"] else p["de"]), p["end_ctx"], p["ds"] != 0)].append(p)
for k, ps in sorted(tab.items(), key=lambda kv: -len(kv[1])):
    print(f"  end {k[0]:3} (length Δ) before {k[1]:7} start also off {str(k[2]):5} {len(ps):5}")

print("\n-- by UST note length (cells) --")
B = [(1, 1), (2, 2), (3, 4), (5, 8), (9, 999)]
for lo, hi in B:
    ps = [p for p in rows if lo <= p["rlen"] <= hi]
    tp = [p for p in ps if p["timing"]]
    print(f"  {lo}-{hi:<3} pairs {len(ps):5} timing-wrong {100 * len(tp) / max(1, len(ps)):5.1f}%  units {tcost(tp):6} {share(tp):5.1f}%")

print("\n-- per song: timing cost/min, start errors, end errors --")
for s, A in zip(songs, base):
    ps = [p for p in timing if p["song"] == s["id"]]
    print(f"  {s['id']:28} {tcost(ps) / (A['span_s'] / 60):6.1f}  starts {sum(1 for p in ps if p['ds']):4} ends(length) "
          f"{sum(1 for p in ps if p['counts']['length']):4}")


# Which sources put an edge exactly where the UST does (scored cells; GAME notes snapped as the stage does).
def edges(song):
    sp, an, ph = song["spacing"], song["ust_anchor"], song["onset_phase"]
    cell = lambda t: ec.rround((t - an) / sp)
    sd = song["stage_detail"]
    src = {
        "lead": lib12.snap_notes([tuple(n) for n in sd["game_lead"]], sp, ph),
        "vocals": lib12.snap_notes([tuple(n) for n in sd["game_vocals"]], sp, ph),
        "sona": [tuple(n) for n in song["sona_onset"]],
    }
    return {k: ({cell(n[0]) for n in v}, {cell(n[1]) for n in v}) for k, v in src.items()}


print("\n-- does a source have the UST's edge? (wrong starts / wrong ends of pairs) --")
E = {s["id"]: edges(s) for s in songs}
for which, idx, cond in (("start", 0, lambda p: p["ds"] != 0), ("end", 1, lambda p: p["counts"]["length"] and p["de"] != 0)):
    ps = [p for p in timing if cond(p)]
    has = collections.Counter()
    near = collections.Counter()
    for p in ps:
        e = E[p["song"]]
        hit = {k: p["r"][idx] in e[k][0 if idx == 0 else 1] for k in e}
        # an end may also be found as the next note's start (legato)
        if idx == 1:
            hit = {k: hit[k] or p["r"][1] in e[k][0] for k in e}
        for k, v in hit.items():
            has[k] += v
        has["any"] += any(hit.values())
        has["vocals or sona"] += hit["vocals"] or hit["sona"]
    print(f"  {which:5} wrong {len(ps):4}: " + "  ".join(f"{k} {100 * v / max(1, len(ps)):4.1f}%" for k, v in has.items()))


print("\n== oracles: the output note of each pair in the slice takes the UST's start and/or end; real metric re-run ==")
print(f"  {'slice':52} {'c/min':>6} {'Δ [95%]':>22}  {'clean (Δ [95%])':>27}  missing (Δ)")
S = lambda p: p["r"][0]
Ee = lambda p: p["r"][1]
SL = [
    ("every start and end", lambda p: (S(p) if p["ds"] else None, Ee(p) if p["de"] else None)),
    ("every start (end kept)", lambda p: (S(p) if p["ds"] else None, None)),
    ("every start, length kept (note moved)", lambda p: (S(p), p["o"][1] - p["ds"]) if p["ds"] else (None, None)),
    ("every end (start kept)", lambda p: (None, Ee(p) if p["de"] else None)),
    ("every length (end set from start + UST length)", lambda p: (None, p["o"][0] + p["rlen"]) if p["counts"]["length"] else (None, None)),
    ("one-cell errors only (|Δ| = 1 at that edge)", lambda p: (S(p) if abs(p["ds"]) == 1 else None, Ee(p) if abs(p["de"]) == 1 else None)),
    ("errors of 2+ cells only", lambda p: (S(p) if abs(p["ds"]) >= 2 else None, Ee(p) if abs(p["de"]) >= 2 else None)),
    ("start-only errors (de = 0)", lambda p: (S(p), None) if p["ds"] and not p["de"] else (None, None)),
    ("starts late", lambda p: (S(p) if p["ds"] > 0 else None, None)),
    ("starts early", lambda p: (S(p) if p["ds"] < 0 else None, None)),
    ("starts after legato", lambda p: (S(p) if p["ds"] and p["start_ctx"] == "legato" else None, None)),
    ("starts after a short rest", lambda p: (S(p) if p["ds"] and p["start_ctx"] == "rest" else None, None)),
    ("starts at a phrase start", lambda p: (S(p) if p["ds"] and p["start_ctx"] == "phrase" else None, None)),
    ("ends late", lambda p: (None, Ee(p) if p["de"] > 0 else None)),
    ("ends early", lambda p: (None, Ee(p) if p["de"] < 0 else None)),
    ("ends before legato", lambda p: (None, Ee(p) if p["de"] and p["end_ctx"] == "legato" else None)),
    ("ends before a short rest", lambda p: (None, Ee(p) if p["de"] and p["end_ctx"] == "rest" else None)),
    ("ends at a phrase end", lambda p: (None, Ee(p) if p["de"] and p["end_ctx"] == "phrase" else None)),
    ("ends late before a rest or phrase end (release)", lambda p: (None, Ee(p) if p["de"] > 0 and p["end_ctx"] != "legato" else None)),
    ("ends early before a rest or phrase end", lambda p: (None, Ee(p) if p["de"] < 0 and p["end_ctx"] != "legato" else None)),
    ("ends late, legato", lambda p: (None, Ee(p) if p["de"] > 0 and p["end_ctx"] == "legato" else None)),
    ("ends early, legato", lambda p: (None, Ee(p) if p["de"] < 0 and p["end_ctx"] == "legato" else None)),
    ("notes of 1-2 UST cells", lambda p: (S(p) if p["ds"] else None, Ee(p) if p["de"] else None) if p["rlen"] <= 2 else (None, None)),
    ("notes of 3+ UST cells", lambda p: (S(p) if p["ds"] else None, Ee(p) if p["de"] else None) if p["rlen"] >= 3 else (None, None)),
]


def src_fix(names):
    def mk(song):
        e = E[song["id"]]
        def fix(p):
            s_ok = any(p["r"][0] in e[k][0] for k in names)
            e_ok = any(p["r"][1] in e[k][1] or p["r"][1] in e[k][0] for k in names)
            return (S(p) if p["ds"] and s_ok else None, Ee(p) if p["de"] and e_ok else None)
        return fix
    return mk


for name, fix in SL:
    new = [L.with_fixes(s, A, fix) for s, A in zip(songs, base)]
    print(L.fmt_row(name, base, new))
for names in (("lead",), ("vocals",), ("sona",), ("lead", "vocals"), ("lead", "vocals", "sona")):
    new = [L.with_fixes(s, A, src_fix(names)(s)) for s, A in zip(songs, base)]
    print(L.fmt_row("edge fixed where " + "/".join(names) + " has it", base, new))
