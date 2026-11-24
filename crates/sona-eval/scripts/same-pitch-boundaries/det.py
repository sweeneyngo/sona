"""07 step 2: no-lyrics same-pitch boundary detector from the lead stem's spectrum (dev only)."""
import os, bisect
import numpy as np
import lib07 as L

FEAT = os.path.join(L.WORK, "feat")
_cache = {}


def feats(sid, stem="lead"):
    k = (sid, stem)
    if k not in _cache:
        z = np.load(os.path.join(FEAT, f"{sid}.{stem}.npz"))
        _cache[k] = {n: z[n] for n in z.files}
    return _cache[k]


def box_mean(x, w):
    """Mean of x[:, t-w:t] (left) and x[:, t:t+w] (right) for every t (edges padded by replication)."""
    T = x.shape[1]
    p = np.concatenate([np.repeat(x[:, :1], w, 1), x, np.repeat(x[:, -1:], w, 1)], 1)
    c = np.concatenate([np.zeros((x.shape[0], 1)), np.cumsum(p, 1)], 1)
    idx = np.arange(T) + w
    left = (c[:, idx] - c[:, idx - w]) / w
    right = (c[:, idx + w] - c[:, idx]) / w
    return left, right


def novelty(sid, cfg):
    f = feats(sid, cfg.get("stem", "lead"))
    w = cfg["w"]
    if cfg["feat"] == "mfcc":
        x = f["mfcc"][1:13].astype(np.float64)
    else:
        x = f["logmel"].astype(np.float64)
    lr = np.log(f["rms"].astype(np.float64) + 1e-6)
    voiced = lr > (np.percentile(lr, 95) - 6)  # within ~26 dB of the loud frames
    mu = x[:, voiced].mean(1, keepdims=True)
    sd = x[:, voiced].std(1, keepdims=True) + 1e-9
    x = (x - mu) / sd
    l, r = box_mean(x, w)
    if cfg["feat"] == "logmel-cos":
        l = l - l.mean(0, keepdims=True)
        r = r - r.mean(0, keepdims=True)
        d = 1 - (l * r).sum(0) / (np.linalg.norm(l, axis=0) * np.linalg.norm(r, axis=0) + 1e-9)
    else:
        d = np.sqrt(((l - r) ** 2).mean(0))
    wd = cfg.get("wd", w)
    T = len(lr)
    p = np.concatenate([np.full(wd, lr[0]), lr, np.full(wd, lr[-1])])
    from numpy.lib.stride_tricks import sliding_window_view
    win = sliding_window_view(p, wd)
    lmax = win[np.arange(T)]  # p[t : t+wd] = lr[t-wd : t]
    rmax = win[np.arange(T) + wd + 1]  # lr[t+1 : t+1+wd]
    dip = np.minimum(lmax.max(1), rmax.max(1)) - lr
    dip = np.clip(dip, 0, None)
    return f["t"].astype(np.float64), d + cfg.get("alpha", 0.0) * dip, d, dip


def grid_scores(s, g, cfg, nov=None):
    """For every output note, the detector's score at each interior grid point: the max novelty within
    +-`tol` cells of the grid time. Returns list (per note) of [(grid, score)]."""
    t, n, _, _ = nov if nov is not None else novelty(s["id"], cfg)
    sp = s["spacing"]
    tol = cfg.get("tol", 0.5) * sp
    lead = cfg.get("lead", 0.0) * sp  # look this far before the grid time (consonant before the beat)
    out = []
    for a, b, _ in g:
        row = []
        for k in range(a + 1, b):
            tk = L.t_of(s, k) - lead
            i0, i1 = bisect.bisect_left(t, tk - tol), bisect.bisect_right(t, tk + tol)
            row.append((k, float(n[i0:i1].max()) if i1 > i0 else 0.0))
        out.append(row)
    return out


def restart_scores(s, g, cfg, nov=None):
    """Score at each boundary between two touching output notes."""
    t, n, _, _ = nov if nov is not None else novelty(s["id"], cfg)
    sp = s["spacing"]
    tol = cfg.get("tol", 0.5) * sp
    lead = cfg.get("lead", 0.0) * sp
    out = {}
    for i in range(1, len(g)):
        if g[i - 1][1] == g[i][0]:
            tk = L.t_of(s, g[i][0]) - lead
            i0, i1 = bisect.bisect_left(t, tk - tol), bisect.bisect_right(t, tk + tol)
            out[i] = float(n[i0:i1].max()) if i1 > i0 else 0.0
    return out


def ust_onsets(s):
    return sorted({L.g_of(s, x["on"]) for x in L.ust_notes(s)})


def auc(pos, neg):
    if not pos or not neg:
        return float("nan")
    allv = sorted([(v, 1) for v in pos] + [(v, 0) for v in neg])
    # rank-sum with ties averaged
    ranks = {}
    i = 0
    while i < len(allv):
        j = i
        while j < len(allv) and allv[j][0] == allv[i][0]:
            j += 1
        for k in range(i, j):
            ranks[k] = (i + j + 1) / 2
        i = j
    rp = sum(ranks[k] for k, (v, y) in enumerate(allv) if y)
    return (rp - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))
