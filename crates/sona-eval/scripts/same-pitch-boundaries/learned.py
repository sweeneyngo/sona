"""Learned same-pitch boundary classifier, leave-one-song-out on dev: out-of-fold probabilities at every
interior grid point of an output's notes (split candidates) and every touching same-pitch restart (merge
candidates). Then: thresholded splits, tail-candidate splits, restart merges."""
import bisect, sys
import numpy as np
import lib07 as L
import det

songs = L.load()
OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"


def frame_feats(s):
    f = det.feats(s["id"])
    x = f["mfcc"][1:13].astype(np.float64)
    lr = np.log(f["rms"].astype(np.float64) + 1e-6)
    voiced = lr > (np.percentile(lr, 95) - 6)
    x = (x - x[:, voiced].mean(1, keepdims=True)) / (x[:, voiced].std(1, keepdims=True) + 1e-9)
    t = f["t"].astype(np.float64)
    cols = []
    for w in (3, 6, 12):
        l, r = det.box_mean(x, w)
        cols.append(np.sqrt(((l - r) ** 2).mean(0)))
        if w == 6:
            vec = (r - l)
    for wd in (6, 12):
        _, _, _, dip = det.novelty(s["id"], dict(feat="mfcc", w=6, wd=wd, alpha=0.0))
        cols.append(dip)
    lrz = lr - np.percentile(lr, 95)
    cols.append(lrz)
    # f0 of the vocals stem from the dump (5.8 ms hop): cents relative to the local +-60 ms median
    f0 = np.array(s["f0"], dtype=np.float64)
    ft, fh = f0[:, 0], f0[:, 1]
    cents = np.where(fh > 0, 1200 * np.log2(np.maximum(fh, 1e-3) / 440.0), np.nan)
    idx = np.clip(np.searchsorted(ft, t), 0, len(ft) - 1)
    c_at = cents[idx]
    k = 10
    pad = np.concatenate([np.full(k, np.nan), cents, np.full(k, np.nan)])
    from numpy.lib.stride_tricks import sliding_window_view
    win = sliding_window_view(pad, 2 * k + 1)[idx]
    with np.errstate(all="ignore"):
        med = np.nanmedian(win, 1)
        vfrac = np.mean(~np.isnan(win), 1)
    f0dev = np.nan_to_num(c_at - med, nan=0.0)
    cols += [np.clip(f0dev, -300, 300) / 100, vfrac, np.isnan(c_at).astype(float)]
    F = np.vstack(cols + [vec])  # (feat, T)
    return t, F


def cand_rows(s, g, t, F):
    """Rows: (kind, note index, grid, features). kind 0 = interior split candidate, 1 = restart."""
    tol = 0.5 * s["spacing"]
    nd = 6  # distance/dip features: max over the window; the rest: at the grid time
    rows = []

    def feat_at(tk):
        a, b = bisect.bisect_left(t, tk - tol), bisect.bisect_right(t, tk + tol)
        i = min(bisect.bisect_left(t, tk), len(t) - 1)
        return np.concatenate([F[:nd, a:b].max(1) if b > a else F[:nd, i], F[nd:, i]])

    for i, (a, b, m) in enumerate(g):
        for k in range(a + 1, b):
            pos = [k - a, b - k, b - a, (k - a) / (b - a), 0.0]
            rows.append((0, i, k, np.concatenate([feat_at(L.t_of(s, k)), pos])))
        if i > 0 and g[i - 1][1] == a and g[i - 1][2] == m:
            pa = g[i - 1][1] - g[i - 1][0]
            pos = [pa, b - a, pa + b - a, pa / (pa + b - a), 1.0]
            rows.append((1, i, a, np.concatenate([feat_at(L.t_of(s, a)), pos])))
    return rows


def fit_logreg(X, y, lam=1.0, iters=30):
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Z = np.hstack([(X - mu) / sd, np.ones((len(X), 1))])
    w = np.zeros(Z.shape[1])
    for _ in range(iters):
        p = 1 / (1 + np.exp(-Z @ w))
        g = Z.T @ (p - y) + lam * np.r_[w[:-1], 0]
        H = (Z * (p * (1 - p))[:, None]).T @ Z + lam * np.diag(np.r_[np.ones(len(w) - 1), 0])
        w -= np.linalg.solve(H, g)
    return lambda Xn: 1 / (1 + np.exp(-np.hstack([(Xn - mu) / sd, np.ones((len(Xn), 1))]) @ w))


G = {s["id"]: L.to_grid(s, s["outputs"][OUT]) for s in songs}
ON = {s["id"]: set(det.ust_onsets(s)) for s in songs}
R = {}
for s in songs:
    t, F = frame_feats(s)
    R[s["id"]] = cand_rows(s, G[s["id"]], t, F)
P = {}
for kind in (0, 1):
    for s in songs:
        tr = [(r[3], r[2] in ON[o["id"]]) for o in songs if o["id"] != s["id"] for r in R[o["id"]] if r[0] == kind]
        X = np.array([a for a, _ in tr]); y = np.array([float(b) for _, b in tr])
        f = fit_logreg(X, y)
        te = [r for r in R[s["id"]] if r[0] == kind]
        if te:
            p = f(np.array([r[3] for r in te]))
            for r, pv in zip(te, p):
                P[(s["id"], kind, r[1], r[2])] = float(pv)
for kind, name in ((0, "split candidates"), (1, "restarts")):
    pos = [v for (sid, k, i, c), v in P.items() if k == kind and c in ON[sid]]
    neg = [v for (sid, k, i, c), v in P.items() if k == kind and c not in ON[sid]]
    print(f"{OUT} {name}: out-of-fold AUC {det.auc(pos, neg):.3f}  (pos {len(pos)}, neg {len(neg)})")

base = [L.score(s, s["outputs"][OUT]) for s in songs]


def show(name, gl):
    notes = [L.from_grid(s, gl[s["id"]]) for s in songs]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    L.report(name, songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


for th in (0.3, 0.4, 0.5, 0.6, 0.7):
    gl = {}
    for sid, g in G.items():
        cuts = {}
        for (s_, k, i, c), v in P.items():
            if s_ == sid and k == 0 and v > th:
                cuts.setdefault(i, []).append(c)
        gl[sid] = L.split_at(g, {i: sorted(v) for i, v in cuts.items()})
    show(f"learned splits p > {th}", gl)
for th in (0.3, 0.5, 0.7, 0.8, 0.9):
    gl = {sid: L.merge(g, {i for (s_, k, i, c), v in P.items() if s_ == sid and k == 1 and v < 1 - th}) for sid, g in G.items()}
    show(f"learned merges p(no boundary) > {th}", gl)
