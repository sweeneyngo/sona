"""No-lyrics split with the tail prior: one candidate cut per note (T cells before its end), kept when the
detector's score there passes a threshold. Also merge oracles for same-pitch restarts."""
import sys
import numpy as np
import lib07 as L
import det

songs = L.load()
CFGS = [dict(feat="mfcc", w=6, alpha=0.0, lead=0.0), dict(feat="mfcc", w=6, alpha=1.0, lead=0.25), dict(feat="logmel-cos", w=6, alpha=1.0, lead=0.25)]
out = sys.argv[1] if len(sys.argv) > 1 else "lead"
base = [L.score(s, s["outputs"][out]) for s in songs]
G = {s["id"]: L.to_grid(s, s["outputs"][out]) for s in songs}
ON = {s["id"]: set(det.ust_onsets(s)) for s in songs}


def show(name, gl):
    notes = [L.from_grid(s, gl[s["id"]]) for s in songs]
    rows = [L.score(s, n) for s, n in zip(songs, notes)]
    eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
    L.report(name, songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


print(f"== {out}: baseline {L.cpm(base):.1f}")
# merge oracles: same-pitch touching restarts with no UST onset at the boundary
for tol in (0, 1):
    gl = {sid: L.merge(g, {i for i in range(1, len(g)) if g[i - 1][1] == g[i][0] and g[i - 1][2] == g[i][2]
                           and not any(g[i][0] + d in ON[sid] for d in range(-tol, tol + 1))}) for sid, g in G.items()}
    show(f"merge oracle: restart without UST onset +-{tol}", gl)
for T in (2,):
    for minlen in (3, 4, 6):
        cand = {sid: [(i, g[i][1] - T) for i in range(len(g)) if g[i][1] - g[i][0] >= minlen] for sid, g in G.items()}
        npos = sum(1 for sid in cand for i, c in cand[sid] if c in ON[sid])
        ntot = sum(len(v) for v in cand.values())
        # oracle: split exactly the candidates that have a UST onset
        gl = {sid: L.split_at(G[sid], {i: [c] for i, c in cand[sid] if c in ON[sid]}) for sid in G}
        show(f"tail{T} L>={minlen} oracle (base rate {100 * npos / ntot:.0f}% of {ntot})", gl)
        for cfg in CFGS:
            sc = {}
            for s in songs:
                t, n, _, _ = det.novelty(s["id"], cfg)
                import bisect
                tol = 0.5 * s["spacing"]
                for i, c in cand[s["id"]]:
                    tk = L.t_of(s, c) - cfg.get("lead", 0) * s["spacing"]
                    a, b = bisect.bisect_left(t, tk - tol), bisect.bisect_right(t, tk + tol)
                    sc[(s["id"], i)] = float(n[a:b].max())
            pos = [sc[(sid, i)] for sid in cand for i, c in cand[sid] if c in ON[sid]]
            neg = [sc[(sid, i)] for sid in cand for i, c in cand[sid] if c not in ON[sid]]
            allv = np.array(pos + neg)
            print(f"   {cfg} AUC {det.auc(pos, neg):.3f}")
            for q in (0.5, 0.7, 0.85, 0.95):
                th = float(np.quantile(allv, q))
                gl = {sid: L.split_at(G[sid], {i: [c] for i, c in cand[sid] if sc[(sid, i)] > th}) for sid in G}
                show(f"   tail{T} L>={minlen} {cfg['feat']} a={cfg['alpha']} > q{q}", gl)
