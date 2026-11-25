"""Shared helpers for utau-edit-cost 06 (lean toward sung): load dumps, port snap_notes and the stage's
class choice + cells_to_notes, so a rest bias can be swept offline."""
import json, math, os, sys, random

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "edit-cost-taxonomy"))
import ec  # noqa: E402


def load(d):
    return [json.load(open(os.path.join(d, f))) for f in sorted(os.listdir(d)) if f.endswith(".json")]


def pyround(x):
    # round half to even, like f64::round_ties_even
    return float(round(x))


def song_tune(notes):
    x = y = 0.0
    anyv = False
    for on, off, midi, voiced in (n[:4] for n in notes):
        if voiced:
            frac = midi - round(midi)
            x += math.cos(2 * math.pi * frac)
            y += math.sin(2 * math.pi * frac)
            anyv = True
    return -math.atan2(y, x) / (2 * math.pi) if anyv else 0.0


def snap_notes(notes, spacing, anchor, keep=None):
    """sona_core::cellstage::snap_notes; `keep(n)` overrides the voiced filter."""
    tune = song_tune(notes)
    out = []
    for n in notes:
        if not (keep(n) if keep else n[3]):
            continue
        on = anchor + pyround((n[0] - anchor) / spacing) * spacing
        off = anchor + pyround((n[1] - anchor) / spacing) * spacing
        if off <= on + 1e-9:
            off = on + spacing
        out.append([on, off, int(pyround(n[2] + tune))])
    out.sort(key=lambda a: (a[0], a[1], a[2]))
    for i in range(1, len(out)):
        if out[i][0] < out[i - 1][1] - 1e-9:
            if out[i][0] > out[i - 1][0] + 1e-9:
                out[i - 1][1] = out[i][0]
            else:
                out[i][0] = out[i - 1][1]
                out[i][1] = max(out[i][1], out[i][0] + spacing)
    return [tuple(o) for o in out]


def choose(probs, cands, bias=0.0):
    """Masked argmax over log-probabilities with `bias` subtracted from rest's (class 0)."""
    out = []
    for row, cp in zip(probs, cands):
        best, bv = 0, (math.log(row[0]) if row[0] > 0 else -1e30) - bias
        for j in range(1, 6):
            if cp[j] >= 0:
                v = math.log(row[j]) if row[j] > 0 else -1e30
                if v > bv:
                    best, bv = j, v
        out.append(best)
    return out


def cells_to_notes(classes, cands, sources, k0, spacing, anchor):
    out = []
    prev = None
    for c, cl in enumerate(classes):
        pitch = -1 if cl == 0 else cands[c][cl]
        start = anchor + (k0 + c) * spacing
        cont = prev is not None and pitch >= 0 and pitch == prev[1] and not (prev[0] == cl and sources[c][cl] != sources[c - 1][cl])
        if pitch < 0:
            prev = None
            continue
        if cont:
            out[-1][1] = start + spacing
        else:
            out.append([start, start + spacing, pitch])
        prev = (cl, pitch)
    return [tuple(o) for o in out]


def score(song, notes):
    return ec.analyse(song["reference"], notes, song["spacing"], song["ust_anchor"], song["division"])


def pooled(rows):
    """rows: list of (cost, minutes, clean, measures)."""
    c = sum(r[0] for r in rows)
    m = sum(r[1] for r in rows)
    return c / m, 100.0 * sum(r[2] for r in rows) / max(1, sum(r[3] for r in rows))


def row_of(song, notes):
    a = score(song, notes)
    return (ec.cost(a["counts"]), a["span_s"] / 60.0, a["clean"], a["measures"], a["counts"])


def boot_delta(base, new, n=10000, seed=7):
    """Song bootstrap of the pooled cost/min difference new - base; rows are row_of tuples per song."""
    rng = random.Random(seed)
    k = len(base)
    ds = []
    for _ in range(n):
        idx = [rng.randrange(k) for _ in range(k)]
        m = sum(base[i][1] for i in idx)
        ds.append((sum(new[i][0] for i in idx) - sum(base[i][0] for i in idx)) / m)
    ds.sort()
    return ds[int(0.025 * n)], ds[int(0.975 * n) - 1]


def cell_acc(song, notes):
    """Cells right on the UST's span, the Rust `cells` on the UST anchor."""
    ref = sorted(tuple(r) for r in song["reference"])
    est = sorted(tuple(r) for r in notes)
    sp, anchor = song["spacing"], song["ust_anchor"]
    import bisect
    ro = [r[0] for r in ref]
    eo = [e[0] for e in est]

    def at(ns, os_, t):
        i = bisect.bisect_right(os_, t)
        if i > 0 and ns[i - 1][0] <= t < ns[i - 1][1]:
            return ns[i - 1][2]
        return -1
    first, last = ref[0][0], ref[-1][1]
    k = round((first - anchor) / sp)
    ok = n = 0
    while anchor + (k + 0.5) * sp < last:
        t = anchor + (k + 0.5) * sp
        ok += at(ref, ro, t) == at(est, eo, t)
        n += 1
        k += 1
    return ok, n
