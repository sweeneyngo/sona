"""Python port of sona_eval::correction::edit_cost, with the components exposed for diagnosis."""
import math

CLASSES = ["extra", "pitch", "length", "shift", "moved", "missing", "chopped", "joined"]
W = dict(extra=1, pitch=1, length=1, shift=1, moved=1, missing=2, chopped=4, joined=4)


def rround(x):
    return math.floor(x + 0.5) if x >= 0 else -math.floor(-x + 0.5)


def to_cells(notes, spacing, anchor, keep_index=False):
    v = []
    for i, n in enumerate(notes):
        s, e, p = rround((n[0] - anchor) / spacing), rround((n[1] - anchor) / spacing), rround(n[2])
        if e > s:
            v.append([s, e, p, i])
    v.sort(key=lambda c: (c[0], c[1]))
    for i in range(1, len(v)):
        if v[i - 1][1] > v[i][0]:
            v[i - 1][1] = v[i][0]
    return [c for c in v if c[1] > c[0]]


def shared(a, b):
    return max(0, min(a[1], b[1]) - max(a[0], b[0]))


def analyse(ref_notes, out_notes, spacing, anchor, cpm):
    """Returns dict: counts, measures, clean, span_s, refs, outs (cells), comps (list of dicts)."""
    refs = to_cells(ref_notes, spacing, anchor)
    if not refs:
        return None
    s0, s1 = refs[0][0], refs[-1][1]
    outs = [o for o in to_cells(out_notes, spacing, anchor) if o[1] > s0 and o[0] < s1]
    n, m = len(refs), len(outs)
    edges = []
    j0 = 0
    for ri, r in enumerate(refs):
        # both sorted and monophonic: only nearby outs can share cells
        while j0 < m and outs[j0][1] <= r[0]:
            j0 += 1
        oi = j0
        while oi < m and outs[oi][0] < r[1]:
            o = outs[oi]
            s = shared(r, o)
            if s > 0 and (2 * s > r[1] - r[0] or 2 * s > o[1] - o[0]):
                edges.append((ri, oi))
            oi += 1
    parent = list(range(n + m))

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    deg_r, deg_o = [0] * n, [0] * m
    adj_r = [[] for _ in range(n)]
    adj_o = [[] for _ in range(m)]
    for ri, oi in edges:
        deg_r[ri] += 1
        deg_o[oi] += 1
        adj_r[ri].append(oi)
        adj_o[oi].append(ri)
        a, b = find(ri), find(n + oi)
        parent[a] = b
    members = {}
    for ri in range(n):
        members.setdefault(find(ri), ([], []))[0].append(ri)
    for oi in range(m):
        members.setdefault(find(n + oi), ([], []))[1].append(oi)
    counts = dict.fromkeys(CLASSES, 0)
    comps = []
    bad_r, bad_o = [True] * n, [True] * m
    for key in sorted(members):
        rs, os_ = members[key]
        c = dict(refs=rs, outs=os_, counts=dict.fromkeys(CLASSES, 0))
        if len(rs) == 1 and not os_:
            c["counts"]["missing"] = 1
        elif not rs and len(os_) == 1:
            c["counts"]["extra"] = 1
        elif len(rs) == 1 and len(os_) == 1:
            r, o = refs[rs[0]], outs[os_[0]]
            sh = abs(o[0] - r[0])
            c["counts"]["pitch"] = int(r[2] != o[2])
            c["counts"]["length"] = int(r[1] - r[0] != o[1] - o[0])
            c["counts"]["shift"] = int(sh == 1)
            c["counts"]["moved"] = int(sh > 1)
            clean = sum(c["counts"].values()) == 0
            bad_r[rs[0]] = not clean
            bad_o[os_[0]] = not clean
        else:
            c["counts"]["chopped"] = sum(deg_r[ri] - 1 for ri in rs)
            c["counts"]["joined"] = sum(deg_o[oi] - 1 for oi in os_)
        for k, v in c["counts"].items():
            counts[k] += v
        c["cost"] = sum(W[k] * v for k, v in c["counts"].items())
        comps.append(c)
    measures = {}
    for cell, bad in list(zip(refs, bad_r)) + list(zip(outs, bad_o)):
        for k in range(cell[0] // cpm, (cell[1] - 1) // cpm + 1):
            measures[k] = measures.get(k, False) or bad
    return dict(counts=counts, measures=len(measures), clean=sum(1 for b in measures.values() if not b),
                span_s=(s1 - s0) * spacing, refs=refs, outs=outs, comps=comps, adj_r=adj_r, adj_o=adj_o,
                bad_r=bad_r, bad_o=bad_o)


def cost(counts):
    return sum(W[k] * v for k, v in counts.items())
