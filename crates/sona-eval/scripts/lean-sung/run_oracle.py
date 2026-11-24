"""Per uncovered sung run (P(rest) < tau, no candidate), the best of skip / own note / extend the neighbour,
chosen greedily by the UST's edit cost: the ceiling of a perfect split-or-extend decision. usage: run_oracle.py DUMP"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import sweep
import fill

songs = fill.songs


def run_oracle(s, tau, out_name="stage"):
    sd = s["stage_detail"]
    sp, an, k0 = s["spacing"], s["onset_phase"], sd["k0"]
    notes = sweep.stage0(s) if out_name == "stage" else sweep.lead0(s)
    occupied = set()
    for n in notes:
        occupied.update(range(round((n[0] - an) / sp) - k0, round((n[1] - an) / sp) - k0))
    want = [c for c in range(len(sd["classes"])) if c not in occupied and sd["probs"][c][0] < tau and all(p < 0 for p in sd["cands"][c][1:])]
    runs = []
    for c in want:
        if runs and runs[-1][1] == c:
            runs[-1][1] = c + 1
        else:
            runs.append([c, c + 1])
    cur = [list(n) for n in notes]
    cost = row_of(s, [tuple(n) for n in cur])[0]
    picks = {"skip": 0, "own": 0, "extend": 0}
    for a, b in runs:
        t0, t1 = an + (k0 + a) * sp, an + (k0 + b) * sp
        prev = next((n for n in cur if abs(n[1] - t0) < 1e-6), None)
        nxt = next((n for n in cur if abs(n[0] - t1) < 1e-6), None)
        nb = prev or nxt
        pitch = nb[2] if nb else min(cur, key=lambda n: min(abs(n[1] - t0), abs(n[0] - t1)))[2]
        opts = {"own": [list(n) for n in cur] + [[t0, t1, pitch]]}
        if nb:
            ext = [list(n) for n in cur]
            for n in ext:
                if prev and abs(n[1] - t0) < 1e-6:
                    n[1] = t1
                    break
                if not prev and abs(n[0] - t1) < 1e-6:
                    n[0] = t0
                    break
            opts["extend"] = ext
        best, bc = "skip", cost
        for k, v in opts.items():
            c = row_of(s, sorted(tuple(n) for n in v))[0]
            if c < bc:
                best, bc = k, c
        picks[best] += 1
        if best != "skip":
            cur, cost = sorted(opts[best]), bc
    return [tuple(n) for n in cur], picks


for out_name in ("stage", "lead"):
    base = [row_of(s, sweep.stage0(s) if out_name == "stage" else sweep.lead0(s)) for s in songs]
    for tau in (0.2, 0.5, 0.9):
        res = [run_oracle(s, tau, out_name) for s in songs]
        rows = [row_of(s, r[0]) for s, r in zip(songs, res)]
        picks = {k: sum(r[1][k] for r in res) for k in ("skip", "own", "extend")}
        print(f"{out_name} oracle, P(rest) < {tau}: {pooled(base)[0]:.1f} -> {pooled(rows)[0]:.1f} ({pooled(rows)[0] - pooled(base)[0]:+.1f}), clean {pooled(rows)[1]:.1f}%  picks {picks}")
