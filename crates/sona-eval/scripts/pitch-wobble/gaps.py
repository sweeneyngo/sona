"""How far apart the evidence puts the two pieces at a 1-2 semitone cut, as a fraction of the rounded step:
GAME lead's unrounded pitches, GAME's run on the full vocals, sona's pitch track. A drift split rounds a
small change into a whole step (gap well under 1); a real note change keeps the whole step."""
import wob
import cands as C
import wfeats as F

songs = wob.load()
for out in ("lead", "stage"):
    R = []
    for s in songs:
        g = wob.grid(s, out)
        lead = F.snapped(s, s["stage_detail"]["game_lead"], F.tune_of(s["stage_detail"]["game_lead"]))
        voc = F.snapped(s, s["stage_detail"]["game_vocals"], F.tune_of(s["stage_detail"]["game_vocals"]))
        rows = F.features(s, g, C.candidates(s, g))
        for r in rows:
            a, b = g[r["i"] - 1], g[r["i"]]
            step = b[2] - a[2]
            ra, rb = F.best_overlap(lead, a[0], a[1]), F.best_overlap(lead, b[0], b[1])
            r["rawgap"] = None if ra is None or rb is None or ra is rb else (rb[2] - ra[2]) / step
            va, vb = F.best_overlap(voc, a[0], a[1]), F.best_overlap(voc, b[0], b[1])
            r["vocgap"] = None if va is None or vb is None else (vb[2] - va[2]) / step
            r["f0gap"] = None if r["ma"] is None or r["mb"] is None else (r["mb"] - r["ma"]) / step
        R += rows
    pools = (("1-2 st, shorter piece 1 cell", [r for r in R if abs(r["iv"]) <= 2 and r["ls"] == 1]),
             ("1-2 st, shorter piece 2 cells", [r for r in R if abs(r["iv"]) <= 2 and r["ls"] == 2]),
             ("1-2 st, shorter piece 3+ cells", [r for r in R if abs(r["iv"]) <= 2 and r["ls"] >= 3]),
             ("1-2 st, all", [r for r in R if abs(r["iv"]) <= 2]))
    for name, P in pools:
        ch = [r for r in P if r["label"] == "chopped"]
        bd = [r for r in P if r["label"] == "boundary"]
        print(f"{out} {name}: chopped {len(ch)}, boundary {len(bd)}")
        for f in ("rawgap", "vocgap", "f0gap"):
            pc = [r[f] for r in ch if r[f] is not None]
            pb = [r[f] for r in bd if r[f] is not None]
            line = f"   {f:7} AUC {F.auc([-x for x in pc], [-x for x in pb]):.3f} (n {len(pc)}/{len(pb)})"
            for t in (0.25, 0.5, 0.75):
                line += f" | gap < {t}: chopped {sum(1 for x in pc if x < t)}, boundary {sum(1 for x in pb if x < t)}"
            print(line)
