"""Prototype (utau-edit-cost 06): draw a note where the stage believes the cell is sung (P(rest) < tau) but
no source proposes a pitch, so its masked choice falls to rest. usage: fill.py DUMP"""
import sys, os, bisect, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import sweep

songs = load(sys.argv[1])


def fill(s, tau, rule="neighbour", min_cells=1, out_name="stage", with_cands=False, touch="separate"):
    sd = s["stage_detail"]
    sp, an, k0 = s["spacing"], s["onset_phase"], sd["k0"]
    notes = sweep.stage0(s) if out_name == "stage" else [tuple(x) for x in s["outputs"]["lead"]]
    occupied = set()
    for n in notes:
        occupied.update(range(round((n[0] - an) / sp) - k0, round((n[1] - an) / sp) - k0))
    want = [c for c in range(len(sd["classes"]))
            if c not in occupied and sd["probs"][c][0] < tau and (with_cands or all(p < 0 for p in sd["cands"][c][1:]))]
    runs = []
    for c in want:
        if runs and runs[-1][1] == c:
            runs[-1][1] = c + 1
        else:
            runs.append([c, c + 1])
    tune = song_tune(sd["game_lead"])
    gl = sd["game_lead"]
    glo = [n[0] for n in gl]
    f0 = s["f0"]
    ft = [f[0] for f in f0]
    by_end = {round((n[1] - an) / sp) - k0: n for n in notes}
    by_start = {round((n[0] - an) / sp) - k0: n for n in notes}
    add, ext = [], []
    for a, b in runs:
        if b - a < min_cells:
            continue
        t0, t1 = an + (k0 + a) * sp, an + (k0 + b) * sp
        pitch = None
        if rule in ("f0", "f0+neighbour"):
            fr = [69 + 12 * math.log2(f[1] / 440) for f in f0[bisect.bisect_left(ft, t0):bisect.bisect_left(ft, t1)] if f[1] > 0]
            if len(fr) >= 3:
                fr.sort()
                pitch = round(fr[len(fr) // 2])
        if pitch is None and rule == "game":
            i = bisect.bisect_right(glo, (t0 + t1) / 2) - 1
            if i >= 0 and gl[i][1] > (t0 + t1) / 2:
                pitch = round(gl[i][2] + tune)
        if pitch is None:
            if a in by_end:
                pitch = by_end[a][2]
            elif b in by_start:
                pitch = by_start[b][2]
            elif notes:
                pitch = min(notes, key=lambda n: min(abs(n[1] - t0), abs(n[0] - t1)))[2]
            else:
                continue
        touching = a in by_end or b in by_start
        if touching and touch == "skip":
            continue
        if touching and touch == "extend":
            ext.append((a, b))
            continue
        add.append((t0, t1, pitch))
    if ext:
        notes = [list(n) for n in notes]
        for a, b in ext:
            t0, t1 = an + (k0 + a) * sp, an + (k0 + b) * sp
            for n in notes:
                if abs(n[1] - t0) < 1e-6:
                    n[1] = t1
                    break
            else:
                for n in notes:
                    if abs(n[0] - t1) < 1e-6:
                        n[0] = t0
                        break
        notes = [tuple(n) for n in notes]
    return sorted(notes + add)


def table(title, out_name, variants):
    base_fn = sweep.stage0 if out_name == "stage" else sweep.lead0
    outs0 = [base_fn(s) for s in songs]
    base = [row_of(s, o) for s, o in zip(songs, outs0)]
    bc, bcl = pooled(base)
    bok = [cell_acc(s, o) for s, o in zip(songs, outs0)]
    print(f"\n== {title}: base {bc:.1f} clean {bcl:.1f}% cells {100 * sum(o for o, _ in bok) / sum(n for _, n in bok):.2f}%")
    print(f"  {'variant':40} {'cost/min':>8} {'delta':>7} {'95%':>16} {'clean':>6} {'cells':>7} {'miss':>5} {'extra':>5} {'join':>5} {'chop':>5} {'pitch':>5} {'len':>5} {'shift':>5}")
    for name, fn in variants:
        outs = [fn(s) for s in songs]
        rows = [row_of(s, o) for s, o in zip(songs, outs)]
        c, cl = pooled(rows)
        lo, hi = boot_delta(base, rows)
        ok = [cell_acc(s, o) for s, o in zip(songs, outs)]
        cnt = lambda k: sum(r[4][k] for r in rows)
        print(f"  {name:40} {c:8.1f} {c - bc:+7.1f} [{lo:+6.1f}, {hi:+6.1f}] {cl:5.1f}% {100 * sum(o for o, _ in ok) / sum(n for _, n in ok):6.2f}% {cnt('missing'):5} {cnt('extra'):5} {cnt('joined'):5} {cnt('chopped'):5} {cnt('pitch'):5} {cnt('length'):5} {cnt('shift'):5}")
    return base


if __name__ == "__main__" and os.environ.get("TOUCH"):
    taus = (0.05, 0.1, 0.2, 0.3, 0.5)
    for out_name in ("stage", "lead"):
        for touch in ("skip", "extend"):
            table(f"{out_name}: fill uncovered sung cells, neighbour pitch, runs touching a note: {touch}", out_name,
                  [(f"P(rest) < {t}", (lambda t: lambda s: fill(s, t, "neighbour", 1, out_name, touch=touch))(t)) for t in taus])
elif __name__ == "__main__":
    taus = (0.1, 0.2, 0.3, 0.5, 0.7, 0.9)
    for out_name in ("stage", "lead"):
        for rule in ("neighbour", "f0+neighbour", "game"):
            table(f"{out_name}: fill uncovered sung cells, pitch = {rule}", out_name,
                  [(f"P(rest) < {t}", (lambda t: lambda s: fill(s, t, rule, 1, out_name))(t)) for t in taus])
    table("stage: fill uncovered sung cells, neighbour, runs >= 2 cells", "stage",
          [(f"P(rest) < {t}", (lambda t: lambda s: fill(s, t, "neighbour", 2))(t)) for t in taus])
