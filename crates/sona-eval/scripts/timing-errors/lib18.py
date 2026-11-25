"""Shared helpers for utau-edit-cost 18 (off-time starts and ends), on `sona-eval-cellstage --dump` output.

The dump directory comes from $S18_DUMP. Nothing here writes into the repo, and no candidate reads the
UST: only the oracles and the scoring do.
"""
import bisect, collections, math, os, random, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "edit-cost-taxonomy"))
sys.path.insert(0, os.path.join(HERE, "..", "single-note-errors"))
os.environ.setdefault("S12_DUMP", os.environ.get("S18_DUMP", ""))
import ec  # noqa: E402
import lib12  # noqa: E402

OUTS = ("lead", "stage")
load = lib12.load
pool, per_min, clean, boot_delta, boot = lib12.pool, lib12.per_min, lib12.clean, lib12.boot_delta, lib12.boot


def loads():
    return lib12.load(os.environ["S18_DUMP"])


def analyse(song, notes):
    return ec.analyse(song["reference"], notes, song["spacing"], song["ust_anchor"], song["division"])


def missing(p):
    return p["counts"].get("missing", 0)


def beat_cells(song):
    return song["division"] // 4


def cells_to_times(song, outs):
    sp, an = song["spacing"], song["ust_anchor"]
    return [(an + o[0] * sp, an + o[1] * sp, o[2]) for o in outs]


def gap_kind(g, beat):
    """A UST neighbour gap in cells: legato, a short rest (under a beat), or a phrase edge (a beat or more,
    or no neighbour)."""
    if g is None or g >= beat:
        return "phrase"
    return "legato" if g == 0 else "rest"


def pairs(A, song):
    beat = beat_cells(song)
    rows = []
    for p in lib12.pairs(A):
        p["start_ctx"] = gap_kind(p["r_prev_gap"], beat)
        p["end_ctx"] = gap_kind(p["r_next_gap"], beat)
        p["rlen"] = p["r"][1] - p["r"][0]
        p["timing"] = p["counts"]["length"] + p["counts"]["shift"] + p["counts"]["moved"]
        rows.append(p)
    return rows


def with_fixes(song, A, fix):
    """Re-score after `fix(pair) -> (new_start or None, new_end or None)` on every pair component; cells."""
    outs = [list(o) for o in A["outs"]]
    for p in pairs(A, song):
        s, e = fix(p)
        o = outs[p["oi"]]
        if s is not None:
            o[0] = s
        if e is not None:
            o[1] = e
    return analyse(song, cells_to_times(song, outs))


class Energy:
    """The vocals' RMS envelope (the dump's `rms`, ~5.8 ms frames) with cell means."""

    _lead = {}

    def __init__(self, song, src=None):
        src = src or os.environ.get("S18_ENERGY", "vocals")
        if src == "lead":
            if song["id"] not in Energy._lead:
                import json
                Energy._lead[song["id"]] = json.load(open(os.path.join(os.environ["S18_LEAD_RMS"], song["id"] + ".json")))
            rms = Energy._lead[song["id"]]
        else:
            rms = song["rms"]
        self.t = [x[0] for x in rms]
        self.v = [x[1] for x in rms]
        self.sp = song["spacing"]

    def mean(self, a, b):
        k, l = bisect.bisect_left(self.t, a), bisect.bisect_left(self.t, b)
        return sum(self.v[k:l]) / (l - k) if l > k else 0.0

    def cells(self, on, n):
        return [self.mean(on + c * self.sp, on + (c + 1) * self.sp) for c in range(n)]


def per_min_missing4(p):
    """Cost/min with a missing note weighted 4 (as an off-time one) instead of 2: the selection rule's check
    that a candidate does not win by dropping notes."""
    return (ec.cost(p["counts"]) + 2 * p["counts"].get("missing", 0)) / (p["span_s"] / 60)


def class_delta(base, new):
    pb, pn = pool(base), pool(new)
    return " ".join(f"{k} {pn['counts'].get(k, 0) - pb['counts'].get(k, 0):+d}" for k in ec.CLASSES
                    if pn["counts"].get(k, 0) != pb["counts"].get(k, 0))


def fmt_row(name, base, new, extra=""):
    if os.environ.get("S18_CLASSES"):
        extra += "   [" + class_delta(base, new) + "]"
    d, lo, hi = boot_delta(base, new)
    pn, pb = pool(new), pool(base)
    dc, clo, chi = boot_delta(base, new, stat=clean)
    return (f"  {name:52} {per_min(pn):6.1f} {d:+6.1f} [{lo:+6.1f}, {hi:+6.1f}]  clean {clean(pn):5.1f}% "
            f"({dc:+4.1f} [{clo:+4.1f}, {chi:+4.1f}])  missing {missing(pn):4d} ({missing(pn) - missing(pb):+d}){extra}")
