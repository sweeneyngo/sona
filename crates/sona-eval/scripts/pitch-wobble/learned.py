"""The best a combination of the evidence can do: a logistic model over every short piece's cut (pitch track,
GAME's unrounded pitch, GAME's run on the full vocals, lengths, position), trained leave-one-song-out; the
merge threshold is chosen on the 8 training songs with the real metric and applied to the 9th.
Also prints the pool's own ceiling (merge exactly its truly chopped cuts)."""
import math, sys
import numpy as np
import wob
import cands as C
import wfeats as F
L = wob.L

LS_MAX = int(sys.argv[1]) if len(sys.argv) > 1 else 2
THRESHOLDS = [0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6]


def pool(s, g):
    lead = F.snapped(s, s["stage_detail"]["game_lead"], F.tune_of(s["stage_detail"]["game_lead"]))
    voc = F.snapped(s, s["stage_detail"]["game_vocals"], F.tune_of(s["stage_detail"]["game_vocals"]))
    rows = []
    for r in F.features(s, g, C.candidates(s, g)):
        if abs(r["iv"]) in (12, 24) or r["ls"] > LS_MAX or r["ls"] >= r["ln"]:
            continue
        a, b = g[r["i"] - 1], g[r["i"]]
        step = b[2] - a[2]
        ra, rb = F.best_overlap(lead, a[0], a[1]), F.best_overlap(lead, b[0], b[1])
        va, vb = F.best_overlap(voc, a[0], a[1]), F.best_overlap(voc, b[0], b[1])
        rawgap = 1.0 if ra is None or rb is None or ra is rb else (rb[2] - ra[2]) / step
        vocgap = None if va is None or vb is None else (vb[2] - va[2]) / step
        f0gap = None if r["ma"] is None or r["mb"] is None else (r["mb"] - r["ma"]) / step
        x = [
            1.0, float(r["ls"] == 1), math.log(r["ln"]), float(abs(r["iv"]) == 1), float(abs(r["iv"]) > 2),
            float(r["short_first"]), float(r["after_rest"]), float(r["before_rest"]), float(r["voc_cut"]),
            float(vocgap is None), min(2.0, max(-1.0, vocgap if vocgap is not None else 1.0)),
            float(f0gap is None), min(2.0, max(-1.0, f0gap if f0gap is not None else 1.0)),
            float(r["x_f0"] is None), min(2.0, max(-1.0, r["x_f0"] if r["x_f0"] is not None else 1.0)),
            min(2.0, max(-1.0, rawgap)),
        ]
        rows.append((r, np.array(x)))
    return rows


def fit(X, y, l2=1.0, iters=50):
    w = np.zeros(X.shape[1])
    for _ in range(iters):
        p = 1 / (1 + np.exp(-X @ w))
        grad = X.T @ (p - y) + l2 * np.r_[0, w[1:]]
        H = X.T @ (X * (p * (1 - p))[:, None]) + l2 * np.diag(np.r_[0, np.ones(len(w) - 1)])
        w -= np.linalg.solve(H, grad)
    return w


def joins(rows, prob, th):
    want = {}
    for (r, _), p in zip(rows, prob):
        if p < th:
            continue
        short = r["i"] - 1 if r["short_first"] else r["i"]
        if short not in want or p > want[short][0]:
            want[short] = (p, r["i"])
    return {i for _, i in want.values()}


if __name__ == "__main__":
    songs = wob.load()
    for out in ("lead", "stage"):
        G = {s["id"]: wob.grid(s, out) for s in songs}
        P = {s["id"]: pool(s, G[s["id"]]) for s in songs}
        base = [L.score(s, L.from_grid(s, G[s["id"]])) for s in songs]
        n_ch = sum(1 for s in songs for r, _ in P[s["id"]] if r["label"] == "chopped")
        n_bd = sum(1 for s in songs for r, _ in P[s["id"]] if r["label"] == "boundary")
        print(f"== {out}: pool (shorter piece <= {LS_MAX} cells, step not an octave): chopped {n_ch}, boundary {n_bd}")
        ceil = [L.score(s, L.from_grid(s, wob.merge(G[s["id"]], {r["i"] for r, _ in P[s["id"]] if r["label"] == "chopped"}))) for s in songs]
        L.report("pool ceiling: merge its truly chopped cuts", songs, base, ceil)
        oof, tal = [], {"chopped": 0, "boundary": 0, "other": 0}
        for k, s in enumerate(songs):
            train = [o for o in songs if o["id"] != s["id"]]
            X = np.array([x for o in train for _, x in P[o["id"]]])
            y = np.array([1.0 if r["label"] == "chopped" else 0.0 for o in train for r, _ in P[o["id"]]])
            w = fit(X, y)
            prob = {o["id"]: 1 / (1 + np.exp(-np.array([x for _, x in P[o["id"]]]) @ w)) if P[o["id"]] else [] for o in songs}
            best = None
            for th in THRESHOLDS:
                c = L.cpm([L.score(o, L.from_grid(o, wob.merge(G[o["id"]], joins(P[o["id"]], prob[o["id"]], th)))) for o in train])
                if best is None or c < best[0]:
                    best = (c, th)
            j = joins(P[s["id"]], prob[s["id"]], best[1])
            for r, _ in P[s["id"]]:
                if r["i"] in j:
                    tal[r["label"]] += 1
            oof.append(L.score(s, L.from_grid(s, wob.merge(G[s["id"]], j))))
            print(f"    held out {s['id']:28} threshold {best[1]}  merges {len(j)}", flush=True)
        L.report("learned, LOSO (out of fold)", songs, base, oof, f"merged ch/bd/ot {tal['chopped']}/{tal['boundary']}/{tal['other']}")
