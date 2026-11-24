"""Does the spectrum change at hidden same-pitch UST onsets? Event-triggered novelty and per-kind AUC."""
import bisect, collections
import numpy as np
import lib07 as L
import det
import taxo

songs = L.load()
cfg = dict(feat="mfcc", w=6, alpha=0.0)
OFFS = np.arange(-24, 25)  # frames of 5 ms
prof = collections.defaultdict(list)
pos_kind = collections.defaultdict(list)
neg = []
for s in songs:
    g = L.to_grid(s, s["outputs"]["lead"])
    t, n, d, dip = det.novelty(s["id"], cfg)
    u = L.ust_notes(s)
    on = {}
    prev = None
    for x in u:
        k = L.g_of(s, x["on"])
        vb, kb = taxo.mora(x["lyric"])
        va = taxo.mora(prev["lyric"])[0] if prev else None
        same = prev is not None and prev["midi"] == x["midi"] and L.g_of(s, prev["off"]) == k
        kind = ("same " if same else "diff ") + ("n" if kb == "n" else "vowel=" if kb == "vowel" and va == vb else "vowel!=" if kb == "vowel" else kb)
        on[k] = kind
        prev = x
    for a, b, _ in g:
        for k in range(a + 1, b):
            tk = L.t_of(s, k)
            i = bisect.bisect_left(t, tk)
            if i + OFFS[-1] >= len(t) or i + OFFS[0] < 0:
                continue
            seg = n[i + OFFS]
            v = seg[(OFFS >= -6) & (OFFS <= 6)].max()
            if k in on:
                prof[on[k]].append(seg)
                pos_kind[on[k]].append(v)
            else:
                prof["none"].append(seg)
                neg.append(v)
for k in sorted(prof, key=lambda k: -len(prof[k])):
    m = np.mean(prof[k], 0)
    pk = OFFS[np.argmax(m)] * 5
    print(f"{k:16} n={len(prof[k]):5d} AUC {det.auc(pos_kind.get(k, []), neg) if k != 'none' else float('nan'):.3f}  peak at {pk:+4d} ms  "
          + " ".join(f"{v:.2f}" for v in m[::4]))
