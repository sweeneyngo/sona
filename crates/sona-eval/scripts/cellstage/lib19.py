"""utau-edit-cost 19: train the learned cell stage on edit cost, on `sona-eval-cellstage --dump-cells` rows.

Rows come from the Rust feature builder (ADR-0026's port), so a model trained here reads exactly what sona
gives it: the prototype's 1046 features per cell, then GAME lead's 8 rounding-residual features
(`FeatureSet::lead_residuals`). The dump directory holds UST-derived data: keep it out of the repo.
"""
import json, os, random, sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
for sub in ("edit-cost-taxonomy",):
    for base in (os.path.join(HERE, ".."), os.environ.get("SONA_SCRIPTS", "")):
        p = os.path.join(base, sub)
        if os.path.isdir(p):
            sys.path.insert(0, p)
import ec  # noqa: E402

BASE_WIDTH = 1046
RESID_WIDTH = 8


def load_song(cells_dir, sid, phase):
    meta = json.load(open(f"{cells_dir}/{sid}.{phase}.json"))
    n, w = meta["n"], meta["width"]
    X = np.fromfile(f"{cells_dir}/{sid}.{phase}.f32", dtype="<f4").reshape(n, w)
    cands = np.array(meta["cands"], dtype=np.int64)
    src = np.array([[-1 if s is None else s for s in row] for row in meta["sources"]], dtype=np.int64)
    meta.update(X=X, cands=cands, src=src)
    label(meta)
    return meta


def label(d):
    """The prototype's labels: per cell, the UST pitch at the cell's centre (-1 rest), over the UST's span; y is
    the first candidate with that pitch (0 rest, -100 none). Also the UST note each cell's centre is in."""
    R = sorted(map(tuple, d["reference"]))
    sp, anc, k0, n = d["spacing"], d["anchor"], d["k0"], d["n"]
    starts = [r[0] for r in R]
    ref = np.full(n, -1, np.int64)
    note = np.full(n, -1, np.int64)
    for c in range(n):
        t = anc + (k0 + c + 0.5) * sp
        i = np.searchsorted(starts, t, side="right") - 1
        if i >= 0 and R[i][0] <= t < R[i][1]:
            ref[c] = R[i][2]
            note[c] = i
    kf = round((R[0][0] - anc) / sp)
    m = 0
    while anc + (kf + m + 0.5) * sp < R[-1][1]:
        m += 1
    lo, hi = max(0, kf - k0), min(n, kf - k0 + m)
    span = np.zeros(n, bool)
    span[lo:hi] = True
    y = np.full(n, -100, np.int64)
    for c in range(n):
        if ref[c] < 0:
            y[c] = 0
        else:
            hit = np.nonzero(d["cands"][c, 1:] == ref[c])[0]
            if hit.size:
                y[c] = hit[0] + 1
    y[~span] = -100
    d.update(ref=ref, note=note, span=span, y=y)


def good_sets(d):
    """Per cell, the classes whose choice gives the UST's note at that cell: the right pitch, and where the
    previous cell holds the same pitch, a source boundary exactly where the UST has a note boundary. Empty
    rows fall back to the right pitch alone."""
    n = d["n"]
    cands, src, ref, note = d["cands"], d["src"], d["ref"], d["note"]
    G = np.zeros((n, 6), bool)
    for c in range(n):
        if not d["span"][c]:
            continue
        if ref[c] < 0:
            G[c, 0] = True
            continue
        right = cands[c] == ref[c]
        right[0] = False
        g = right.copy()
        if c > 0 and ref[c - 1] == ref[c] and note[c - 1] >= 0:
            ust_split = note[c] != note[c - 1]
            for j in range(1, 6):
                if g[j] and cands[c - 1, j] == ref[c]:
                    g[j] = (src[c, j] != src[c - 1, j]) == ust_split
                elif g[j] and ust_split:
                    # a class change on the same pitch continues the note: no split
                    g[j] = False
        G[c] = g if g.any() else right
    return G


def cells_to_notes(classes, d):
    """`sona_core::cellstage::cells_to_notes`."""
    cands, src, k0, sp, anc = d["cands"], d["src"], d["k0"], d["spacing"], d["anchor"]
    out, prev = [], None
    for c, cl in enumerate(classes):
        pitch = -1 if cl == 0 else int(cands[c, cl])
        start = anc + (k0 + c) * sp
        cont = prev is not None and pitch >= 0 and pitch == prev[1] and not (prev[0] == cl and src[c, cl] != src[c - 1, cl])
        if pitch < 0:
            prev = None
            continue
        if cont:
            out[-1][1] = start + sp
        else:
            out.append([start, start + sp, pitch])
        prev = (cl, pitch)
    return [tuple(o) for o in out]


def choose(logits, cands):
    """`choose_class`: masked argmax, ties to the lower class."""
    m = cands >= 0
    m[:, 0] = True
    lg = np.where(m, logits, -np.inf)
    return lg.argmax(1)


def viterbi(logp, d, switch):
    """`sona_core::cellstage::decode_classes`: maximise the sum of log-probabilities minus `switch` for each
    cell where a new note starts or a note ends (pitch change, rest edge, or a source restart on the same
    class). switch = 0 is the argmax; ties go to the lower class."""
    cands, src = d["cands"], d["src"]
    n = len(cands)
    valid = cands >= 0
    valid[:, 0] = True
    lp = np.where(valid, logp, -np.inf)
    if switch == 0:
        return lp.argmax(1)
    score = lp[0].copy()
    back = np.zeros((n, 6), np.int64)
    for c in range(1, n):
        pa, pb = cands[c - 1], cands[c]
        same = (pa[:, None] == pb[None, :]) & (pa[:, None] >= 0)
        restart = (src[c - 1][:, None] == src[c][None, :])
        eye = np.eye(6, dtype=bool)
        cont = same & ~(eye & ~restart)
        both_rest = np.zeros((6, 6), bool)
        both_rest[0, 0] = True
        pen = np.where(cont | both_rest, 0.0, switch)
        tot = score[:, None] - pen
        back[c] = tot.argmax(0)
        score = tot.max(0) + lp[c]
    out = np.zeros(n, np.int64)
    out[-1] = score.argmax()
    for c in range(n - 1, 0, -1):
        out[c - 1] = back[c, out[c]]
    return out


def score(d, notes):
    return ec.analyse(d["reference"], notes, d["spacing"], d["ust_anchor"], d["division"])


def row(d, notes):
    A = score(d, notes)
    return dict(counts=A["counts"], measures=A["measures"], clean=A["clean"], span_s=A["span_s"])


def pool(rows):
    c = {k: 0 for k in ec.CLASSES}
    meas = clean = 0
    span = 0.0
    for r in rows:
        for k, v in r["counts"].items():
            c[k] += v
        meas += r["measures"]
        clean += r["clean"]
        span += r["span_s"]
    return dict(counts=c, measures=meas, clean=clean, span_s=span)


def per_min(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


def per_min_missing4(p):
    return (ec.cost(p["counts"]) + 2 * p["counts"].get("missing", 0)) / (p["span_s"] / 60)


def clean(p):
    return 100 * p["clean"] / p["measures"]


def boot_delta(base, new, stat=per_min, iters=10000, seed=1):
    rng = random.Random(seed)
    n = len(base)
    v = []
    for _ in range(iters):
        ix = [rng.randrange(n) for _ in range(n)]
        v.append(stat(pool([new[i] for i in ix])) - stat(pool([base[i] for i in ix])))
    v.sort()
    return stat(pool(new)) - stat(pool(base)), v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def fmt(name, base, new):
    d, lo, hi = boot_delta(base, new)
    dc, clo, chi = boot_delta(base, new, stat=clean)
    _, _, hi4 = boot_delta(base, new, stat=per_min_missing4)
    pb, pn = pool(base), pool(new)
    mb, mn = pb["counts"]["missing"], pn["counts"]["missing"]
    return (f"{name:44} {per_min(pn):6.1f} {d:+6.1f} [{lo:+6.1f}, {hi:+6.1f}]  clean {clean(pn):5.1f}% ({dc:+4.1f} [{clo:+4.1f}, {chi:+4.1f}])"
            f"  missing {mn:4d} ({mn - mb:+d}; @4 upper {hi4:+5.1f})")
