"""AUC of detector configs: hidden UST onsets among interior grid points of GAME lead notes, and true
boundaries among same-pitch touching restarts."""
import itertools, sys
import lib07 as L
import det

songs = L.load()
OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"
G = {s["id"]: L.to_grid(s, s["outputs"][OUT]) for s in songs}
ON = {s["id"]: set(det.ust_onsets(s)) for s in songs}
cfgs = []
for feat, w, alpha, lead in itertools.product(["mfcc", "logmel", "logmel-cos"], [6, 10, 16], [0.0, 0.5, 1.0], [0.0, 0.25]):
    cfgs.append(dict(feat=feat, w=w, alpha=alpha, lead=lead))
cfgs.append(dict(feat="mfcc", w=10, alpha=1000.0, lead=0.0))  # dip only (scaled)
for cfg in cfgs:
    pos, neg, rp, rn = [], [], [], []
    for s in songs:
        nov = det.novelty(s["id"], cfg)
        g = G[s["id"]]
        for row in det.grid_scores(s, g, cfg, nov):
            for k, v in row:
                (pos if k in ON[s["id"]] else neg).append(v)
        for i, v in det.restart_scores(s, g, cfg, nov).items():
            if g[i - 1][2] == g[i][2]:
                (rp if g[i][0] in ON[s["id"]] else rn).append(v)
    print(f"{cfg}  split AUC {det.auc(pos, neg):.3f} (pos {len(pos)}, neg {len(neg)})  restart AUC {det.auc(rp, rn):.3f} (true {len(rp)}, false {len(rn)})", flush=True)
