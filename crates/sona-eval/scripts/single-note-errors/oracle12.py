"""Oracles for single-note errors: fix one cause perfectly (the output note's start, end or pitch set to the
UST's, in pair components only), re-run the real metric, report cost/min and clean measures against the
baseline with a paired song bootstrap.

usage: S12_DUMP=DIR python3 -I oracle12.py
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L


def is_rest(g):
    return g is not None and g > 0


def legato(g):
    return g == 0


# Each fix: (name, predicate on a pair, what to set). Predicates see the pair dict from lib12.pairs.
FIXES = [
    ("end late into a UST rest (release)", lambda p: p["de"] > 0 and is_rest(p["r_next_gap"]), "end"),
    ("end early before a UST rest", lambda p: p["de"] < 0 and is_rest(p["r_next_gap"]), "end"),
    ("end late, UST legato", lambda p: p["de"] > 0 and legato(p["r_next_gap"]), "end"),
    ("end early, UST legato", lambda p: p["de"] < 0 and legato(p["r_next_gap"]), "end"),
    ("every end", lambda p: p["de"] != 0, "end"),
    ("start late after a UST rest", lambda p: p["ds"] > 0 and is_rest(p["r_prev_gap"]), "start"),
    ("start early after a UST rest", lambda p: p["ds"] < 0 and is_rest(p["r_prev_gap"]), "start"),
    ("start late, UST legato", lambda p: p["ds"] > 0 and legato(p["r_prev_gap"]), "start"),
    ("start early, UST legato", lambda p: p["ds"] < 0 and legato(p["r_prev_gap"]), "start"),
    ("every start", lambda p: p["ds"] != 0, "start"),
    ("every start and end", lambda p: p["ds"] != 0 or p["de"] != 0, "both"),
    ("pitch: octave (±12)", lambda p: abs(p["dp"]) == 12, "pitch"),
    ("pitch: ±1 semitone", lambda p: abs(p["dp"]) == 1, "pitch"),
    ("pitch: ±2 semitones", lambda p: abs(p["dp"]) == 2, "pitch"),
    ("pitch: other", lambda p: abs(p["dp"]) not in (0, 1, 2, 12), "pitch"),
    ("every pitch", lambda p: p["dp"] != 0, "pitch"),
    ("every pair error", lambda p: p["cost"] > 0, "all"),
]


def fixed(song, A, pred, what):
    sp, an = song["spacing"], song["ust_anchor"]
    outs = [list(o) for o in A["outs"]]
    for p in L.pairs(A):
        if not pred(p):
            continue
        o, r = outs[p["oi"]], p["r"]
        if what in ("end", "both", "all"):
            o[1] = r[1]
        if what in ("start", "both", "all"):
            o[0] = r[0]
        if what in ("pitch", "all"):
            o[2] = r[2]
    return L.analyse(song, [(an + o[0] * sp, an + o[1] * sp, o[2]) for o in outs])


def main():
    songs = L.load()
    for OUT in ("lead", "stage"):
        base = [L.analyse(s, s["outputs"][OUT]) for s in songs]
        pb = L.pool(base)
        print(f"\n== {OUT}: baseline {L.per_min(pb):.1f} cost/min, clean {L.clean(pb):.1f}% ==")
        # identity check: re-running from cells gives the same counts
        ident = [fixed(s, A, lambda p: False, "end") for s, A in zip(songs, base)]
        assert all(a["counts"] == b["counts"] for a, b in zip(ident, base)), "cell round trip changed the cost"
        print(f"  {'fixed perfectly':40} {'cost/min':>8} {'Δ [95%]':>24} {'share':>6} {'clean':>6}")
        for name, pred, what in FIXES:
            new = [fixed(s, A, pred, what) for s, A in zip(songs, base)]
            d, lo, hi = L.boot_delta(base, new)
            pn = L.pool(new)
            print(f"  {name:40} {L.per_min(pn):8.1f} {d:+7.1f} [{lo:+6.1f}, {hi:+6.1f}] {-100 * d / L.per_min(pb):5.1f}% {L.clean(pn):5.1f}%")


if __name__ == "__main__":
    main()
