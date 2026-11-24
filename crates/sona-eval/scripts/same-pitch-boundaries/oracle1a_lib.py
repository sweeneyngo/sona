import bisect
import lib07 as L


def true_starts(s):
    """UST note (onset, midpoint) on the output grid, monophonic order."""
    u = L.ust_notes(s)
    return [(L.g_of(s, x["on"]), (L.g_of(s, x["on"]) + L.g_of(s, x["off"])) / 2, x["midi"]) for x in u]


def counts_per_note(s, g):
    """Number of UST notes whose midpoint lies inside each output note, and their onsets."""
    ts = true_starts(s)
    mids = [m for _, m, _ in ts]
    order = sorted(range(len(ts)), key=lambda i: mids[i])
    ms = [mids[i] for i in order]
    res = []
    for a, b, _ in g:
        lo, hi = bisect.bisect_left(ms, a), bisect.bisect_left(ms, b)
        res.append(sorted(ts[order[j]][0] for j in range(lo, hi)))
    return res


