"""Ticket 07 harness: score modified outputs with the Python port of the edit cost, paired song bootstrap,
note splitting/merging on the output's own (onset-phase) grid, phrase note-count == mora-count share."""
import json, os, sys, random, collections
TAXO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "edit-cost-taxonomy")
sys.path.insert(0, TAXO)
import ec, taxo  # noqa: E402

DEV = os.environ.get("S07_DUMP", "")  # a `sona-eval-cellstage --dump` directory (holds UST content: keep it out of the repo)
WORK = os.environ.get("S07_WORK", ".")  # features, lyric streams and outputs (lyric-derived: keep out of the repo)


def load(d=DEV):
    songs = taxo.load(d)
    for s in songs:
        for name in ("lead", "stage"):
            a = score(s, s["outputs"][name])
            assert [a["counts"][k] for k in ec.CLASSES] == s["edit_cost"][name], (s["id"], name)
    return songs


def score(s, notes):
    return ec.analyse(s["reference"], [tuple(n) for n in notes], s["spacing"], s["ust_anchor"], s["division"])


def pooled(rows):
    return taxo.pool(rows)


def cpm(rows):
    return taxo.per_min(taxo.pool(rows))


def clean(rows):
    p = taxo.pool(rows)
    return 100 * p["clean"] / p["measures"]


def paired(rows_a, rows_b, iters=10000, seed=1):
    """Pooled cost/min of b minus a, with a song-bootstrap 95% interval (same songs drawn for both)."""
    rng = random.Random(seed)
    n = len(rows_a)
    d = []
    for _ in range(iters):
        idx = [rng.randrange(n) for _ in range(n)]
        d.append(cpm([rows_b[i] for i in idx]) - cpm([rows_a[i] for i in idx]))
    d.sort()
    return cpm(rows_b) - cpm(rows_a), d[int(0.025 * (iters - 1) + 0.5)], d[int(0.975 * (iters - 1) + 0.5)]


def boot(rows, stat, iters=10000, seed=1):
    return taxo.boot(rows, stat, iters, seed)


def class_cost(rows):
    p = taxo.pool(rows)
    m = p["span_s"] / 60
    return {k: ec.W[k] * p["counts"].get(k, 0) / m for k in ec.CLASSES}


# ---- grid helpers: the output's own grid is onset_phase + k * spacing ----

def g_of(s, t):
    return ec.rround((t - s["onset_phase"]) / s["spacing"])


def t_of(s, g):
    return s["onset_phase"] + g * s["spacing"]


def to_grid(s, notes):
    """[(g_on, g_off, midi)] on the output grid; asserts the notes are on it."""
    out = []
    for n in notes:
        a, b = g_of(s, n[0]), g_of(s, n[1])
        assert abs(t_of(s, a) - n[0]) < 1e-3 and abs(t_of(s, b) - n[1]) < 1e-3, (s["id"], n)
        out.append((a, b, n[2]))
    return out


def from_grid(s, gnotes):
    return [(t_of(s, a), t_of(s, b), m) for a, b, m in gnotes]


def split_at(gnotes, cuts):
    """cuts: {note index: sorted grid cuts strictly inside the note}."""
    out = []
    for i, (a, b, m) in enumerate(gnotes):
        pts = [a] + [c for c in cuts.get(i, []) if a < c < b] + [b]
        out.extend((x, y, m) for x, y in zip(pts, pts[1:]) if y > x)
    return out


def merge(gnotes, joins):
    """joins: set of note indices i such that note i is merged into note i-1 (must be touching)."""
    out = []
    for i, n in enumerate(gnotes):
        if i in joins and out and out[-1][1] == n[0]:
            out[-1] = (out[-1][0], n[1], out[-1][2])
        else:
            out.append(n)
    return out


def tail_cuts(a, b, k, tail=2):
    """k pieces: each mora after the first takes `tail` cells at the end of the note; the first mora keeps
    the rest. Falls back to equal division when the note is shorter than (k - 1) * tail + 1 cells."""
    L_ = b - a
    if L_ < (k - 1) * tail + 1:
        return equal_cuts(a, b, k)
    return [b - (k - j) * tail for j in range(1, k)]


def equal_cuts(a, b, k):
    L = b - a
    k = max(1, min(k, L))
    return [a + ec.rround(L * j / k) for j in range(1, k)]


# ---- phrases and lyric counts ----

def ust_notes(s):
    """UST notes (on, off, midi, lyric) in seconds, sorted, monophonic by the metric's rounding."""
    u = [x for x in s["stage_detail"]["ust"] if x["kind"] == "Note"]
    u.sort(key=lambda x: (x["on"], x["off"]))
    return u


def phrases(s):
    """UST phrases on the reference cells: maximal runs of ref cells with no rest between them.
    Returns [(start cell, end cell, n refs)] in UST cell coordinates."""
    refs = ec.to_cells(s["reference"], s["spacing"], s["ust_anchor"])
    out = []
    for r in refs:
        if out and out[-1][1] == r[0]:
            out[-1] = [out[-1][0], r[1], out[-1][2] + 1]
        else:
            out.append([r[0], r[1], 1])
    return out


def phrase_equal_share(s, notes):
    """Share of UST phrases whose output-note count (by note midpoint) equals the phrase's note (mora) count."""
    ph = phrases(s)
    outs = ec.to_cells([tuple(n) for n in notes], s["spacing"], s["ust_anchor"])
    mids = sorted((o[0] + o[1]) / 2 for o in outs)
    import bisect
    eq = 0
    for a, b, n in ph:
        k = bisect.bisect_left(mids, b) - bisect.bisect_left(mids, a)
        eq += k == n
    return eq, len(ph)


def report(label, songs, base_rows, rows, extra=""):
    d, lo, hi = paired(base_rows, rows)
    cc = class_cost(rows)
    print(f"  {label:44} {cpm(rows):6.1f}  d {d:+6.1f} [{lo:+.1f}, {hi:+.1f}]  clean {clean(rows):5.1f}%  "
          f"joined {cc['joined']:5.1f} chopped {cc['chopped']:5.1f} pairs {cc['pitch'] + cc['length'] + cc['shift'] + cc['moved']:5.1f} "
          f"miss {cc['missing']:5.1f} extra {cc['extra']:4.1f} {extra}", flush=True)
    return d, lo, hi
