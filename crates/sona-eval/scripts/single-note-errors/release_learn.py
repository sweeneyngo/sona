"""Release trims chosen by a small logistic model on energy and voicing, leave-one-song-out.

Candidates: every output note of at least 2 cells whose energy falls under THETA_C × the note's maximum at
some cell k ≥ 1 (the earliest such k); the candidate cut ends the note at k. Label: the cut lands exactly on
the UST's end of a note that ran into a UST rest. A logistic model on the candidate's features is trained on
8 songs, its probability threshold is chosen on the same 8 songs by the real metric, and the 9th song is cut
where p > threshold. Also: the perfect-label ceiling (cut every positive candidate, nothing else).

usage: S12_DUMP=DIR python3 -I release_learn.py [lead|stage]
"""
import bisect, os, sys
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
from release_energy import cell_energy

THETA_C = 0.5
FEATS = ["tail_min", "tail_mean", "cut_cell", "before_cut", "after_note", "run_cells", "run_s", "note_cells",
         "tail_voiced", "before_voiced", "out_legato", "next_same_pitch", "next_step"]


def candidates(song, notes):
    sp = song["spacing"]
    rt = [x[0] for x in song["rms"]]
    rv = [x[1] for x in song["rms"]]
    ft = [x[0] for x in song["f0"]]

    def voiced(a, b):
        i, j = bisect.bisect_left(ft, a), bisect.bisect_left(ft, b)
        return sum(1 for q in range(i, j) if song["f0"][q][1] > 0) / max(1, j - i)

    out = []
    for i, (on, off, p) in enumerate(notes):
        n = round((off - on) / sp)
        if n < 2:
            continue
        e = [cell_energy(rt, rv, on + c * sp, on + (c + 1) * sp) for c in range(n + 1)]
        mx = max(e[:n])
        if mx <= 0:
            continue
        rel = [x / mx for x in e]
        k = next((c for c in range(1, n) if rel[c] < THETA_C), None)
        if k is None:
            continue
        nxt = notes[i + 1] if i + 1 < len(notes) else None
        legato = nxt is not None and abs(nxt[0] - off) < 1e-6
        f = dict(tail_min=min(rel[k:n]), tail_mean=sum(rel[k:n]) / (n - k), cut_cell=rel[k], before_cut=rel[k - 1],
                 after_note=rel[n], run_cells=n - k, run_s=(n - k) * sp, note_cells=n,
                 tail_voiced=voiced(on + k * sp, off), before_voiced=voiced(on + (k - 1) * sp, on + k * sp),
                 out_legato=float(legato), next_same_pitch=float(legato and nxt[2] == p),
                 next_step=float(abs(nxt[2] - p)) if legato else 0.0)
        out.append((i, k, f))
    return out


def label(song, notes, cands):
    A = L.analyse(song, notes)
    pr = {p["o"][3]: p for p in L.pairs(A)}
    sp = song["spacing"]
    lab = []
    for i, k, _ in cands:
        p = pr.get(i)
        n = round((notes[i][1] - notes[i][0]) / sp)
        lab.append(int(p is not None and p["de"] > 0 and bool(p["r_next_gap"]) and p["r_next_gap"] > 0 and n - k == p["de"]))
    return lab


def apply(song, notes, cands, chosen):
    sp = song["spacing"]
    out = [list(n) for n in notes]
    for (i, k, _), c in zip(cands, chosen):
        if c:
            out[i][1] = out[i][0] + k * sp
    return [tuple(n) for n in out]


def fit(X, y, l2=1.0, iters=3000, lr=0.1):
    mu, sd = X.mean(0), X.std(0) + 1e-9
    Z = (X - mu) / sd
    w = np.zeros(Z.shape[1])
    b = 0.0
    for _ in range(iters):
        p = 1 / (1 + np.exp(-(Z @ w + b)))
        g = p - y
        w -= lr * (Z.T @ g / len(y) + l2 * w / len(y))
        b -= lr * g.mean()
    return lambda Xn: 1 / (1 + np.exp(-(((Xn - mu) / sd) @ w + b))), (mu, sd, w, b)


def main():
    out_name = sys.argv[1] if len(sys.argv) > 1 else "lead"
    songs = L.load()
    notes = [[tuple(n) for n in s["outputs"][out_name]] for s in songs]
    base = [L.analyse(s, ns) for s, ns in zip(songs, notes)]
    C = [candidates(s, ns) for s, ns in zip(songs, notes)]
    Y = [label(s, ns, c) for s, ns, c in zip(songs, notes, C)]
    X = [np.array([[f[k] for k in FEATS] for _, _, f in c]) for c in C]
    print(f"{out_name}: candidates {sum(len(c) for c in C)}, positives {sum(sum(y) for y in Y)}")
    ceil = [L.analyse(s, apply(s, ns, c, y)) for s, ns, c, y in zip(songs, notes, C, Y)]
    d, lo, hi = L.boot_delta(base, ceil)
    print(f"perfect labels (cut exactly the positives): {d:+.1f} [{lo:+.1f}, {hi:+.1f}]  clean "
          f"{L.clean(L.pool(base)):.1f} → {L.clean(L.pool(ceil)):.1f}")
    ths = [0.3, 0.4, 0.5, 0.6, 0.7, 0.8]
    held = []
    for k in range(len(songs)):
        tr = [i for i in range(len(songs)) if i != k]
        pred, _ = fit(np.vstack([X[i] for i in tr]), np.concatenate([Y[i] for i in tr]).astype(float))
        # threshold chosen on the training songs by the real metric
        best_t, best_c = None, None
        for t in ths:
            rows = [L.analyse(songs[i], apply(songs[i], notes[i], C[i], pred(X[i]) > t)) for i in tr]
            c = L.per_min(L.pool(rows))
            if best_c is None or c < best_c:
                best_t, best_c = t, c
        p_held = pred(X[k]) if len(X[k]) else np.array([])
        held.append(L.analyse(songs[k], apply(songs[k], notes[k], C[k], p_held > best_t)))
        b = L.per_min(L.pool([base[k]]))
        print(f"  fold {songs[k]['id']:28} threshold {best_t}  cuts {int((p_held > best_t).sum()):3d} "
              f"(positives {sum(Y[k])})  {b:6.1f} → {L.per_min(L.pool([held[-1]])):6.1f}", flush=True)
    d, lo, hi = L.boot_delta(base, held)
    print(f"leave-one-song-out: {d:+.1f} [{lo:+.1f}, {hi:+.1f}]  clean {L.clean(L.pool(base)):.1f} → {L.clean(L.pool(held)):.1f}")
    pred, (mu, sd, w, b) = fit(np.vstack(X), np.concatenate(Y).astype(float))
    print("weights on all songs (standardised):", ", ".join(f"{n} {v:+.2f}" for n, v in sorted(zip(FEATS, w), key=lambda t: -abs(t[1]))))


if __name__ == "__main__":
    main()
