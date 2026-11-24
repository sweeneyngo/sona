"""Shared helpers for utau-edit-cost 12 (single-note errors), on `sona-eval-cellstage --dump` output.

The dump directory comes from $S12_DUMP. Nothing here writes into the repo.
"""
import json, math, os, random, sys, collections

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "edit-cost-taxonomy"))
import ec  # noqa: E402

PAIR = ("pitch", "length", "shift", "moved")


def load(d=None):
    d = d or os.environ["S12_DUMP"]
    return [json.load(open(os.path.join(d, f))) for f in sorted(os.listdir(d)) if f.endswith(".json")]


def song_tune(game):
    x = y = 0.0
    any_ = False
    for on, off, midi, voiced in game:
        if voiced:
            frac = midi - round(midi)
            x += math.cos(2 * math.pi * frac)
            y += math.sin(2 * math.pi * frac)
            any_ = True
    return -math.atan2(y, x) / (2 * math.pi) if any_ else 0.0


def snap_notes(game, sp, an, on_bias=0.0, off_bias=0.0, keep=False):
    """Port of `sona_core::cellstage::snap_notes` (Python's round() is ties-to-even, like the Rust).
    `on_bias`/`off_bias` (cells) move the rounding point: a time rounds to floor(x - bias + 0.5); 0 is the
    shipped rule. `keep` adds the raw note's index as a 4th field."""
    tune = song_tune(game)
    out = []
    for i, (on_s, off_s, midi, voiced) in enumerate(game):
        if not voiced:
            continue
        xo, xf = (on_s - an) / sp, (off_s - an) / sp
        ko = round(xo) if on_bias == 0 else math.floor(xo - on_bias + 0.5)
        kf = round(xf) if off_bias == 0 else math.floor(xf - off_bias + 0.5)
        on = an + ko * sp
        off = an + kf * sp
        if off <= on + 1e-9:
            off = on + sp
        out.append([on, off, round(midi + tune), i])
    out.sort(key=lambda a: (a[0], a[1], a[2]))
    for i in range(1, len(out)):
        if out[i][0] < out[i - 1][1] - 1e-9:
            if out[i][0] > out[i - 1][0] + 1e-9:
                out[i - 1][1] = out[i][0]
            else:
                out[i][0] = out[i - 1][1]
                out[i][1] = max(out[i][1], out[i][0] + sp)
    return out if keep else [tuple(o[:3]) for o in out]


def analyse(song, notes):
    return ec.analyse(song["reference"], notes, song["spacing"], song["ust_anchor"], song["division"])


def pool(rows):
    c = collections.Counter()
    meas = clean = 0
    span = 0.0
    for r in rows:
        c.update(r["counts"])
        meas += r["measures"]
        clean += r["clean"]
        span += r["span_s"]
    return dict(counts=dict(c), measures=meas, clean=clean, span_s=span)


def per_min(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


def clean(p):
    return 100 * p["clean"] / p["measures"]


def boot_delta(base, new, stat=per_min, iters=10000, seed=1):
    """Paired song bootstrap of stat(pool(new)) − stat(pool(base)): (delta, lo, hi)."""
    rng = random.Random(seed)
    n = len(base)
    v = []
    for _ in range(iters):
        ix = [rng.randrange(n) for _ in range(n)]
        v.append(stat(pool([new[i] for i in ix])) - stat(pool([base[i] for i in ix])))
    v.sort()
    return stat(pool(new)) - stat(pool(base)), v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def boot(rows, stat=per_min, iters=10000, seed=1):
    rng = random.Random(seed)
    v = sorted(stat(pool([rows[rng.randrange(len(rows))] for _ in rows])) for _ in range(iters))
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def pairs(A):
    """Every pair component: r, o (cells, o[3] = index in the output list), ds, de, dp, counts and the gaps
    to the neighbours on each side (None at the ends)."""
    refs, outs = A["refs"], A["outs"]
    rows = []
    for c in A["comps"]:
        if len(c["refs"]) == 1 and len(c["outs"]) == 1:
            ri, oi = c["refs"][0], c["outs"][0]
            r, o = refs[ri], outs[oi]
            rows.append(dict(ri=ri, oi=oi, r=r, o=o, ds=o[0] - r[0], de=o[1] - r[1], dp=o[2] - r[2], counts=c["counts"],
                             cost=c["cost"],
                             r_next_gap=(refs[ri + 1][0] - r[1]) if ri + 1 < len(refs) else None,
                             r_prev_gap=(r[0] - refs[ri - 1][1]) if ri > 0 else None,
                             o_next_gap=(outs[oi + 1][0] - o[1]) if oi + 1 < len(outs) else None,
                             o_prev_gap=(o[0] - outs[oi - 1][1]) if oi > 0 else None))
    return rows


def midi_of(hz):
    return 69 + 12 * math.log2(hz / 440.0) if hz > 0 else None
