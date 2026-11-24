"""Snapping: does moving GAME lead's rounding point (a global onset/offset bias, in cells or in ms) lower the
cost? Re-snaps the raw GAME notes with lib12.snap_notes (the shipped rule at bias 0).

usage: S12_DUMP=DIR python3 -I bias_sweep.py
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L

songs = L.load()
base = [L.analyse(s, s["outputs"]["lead"]) for s in songs]
print("bias in cells (positive: a time rounds down more often, i.e. GAME is treated as late)")
for ob in (-0.2, -0.1, 0.0, 0.1, 0.2, 0.3):
    for fb in (-0.2, -0.1, 0.0, 0.1, 0.2, 0.3):
        if ob == 0 and fb == 0:
            continue
        new = [L.analyse(s, L.snap_notes(s["stage_detail"]["game_lead"], s["spacing"], s["onset_phase"], on_bias=ob, off_bias=fb)) for s in songs]
        d, lo, hi = L.boot_delta(base, new, iters=2000)
        print(f"  on {ob:+.1f} off {fb:+.1f}: {d:+6.1f} [{lo:+5.1f}, {hi:+5.1f}]  clean {L.clean(L.pool(new)) - L.clean(L.pool(base)):+.1f}", flush=True)
print("\nshift in ms (raw GAME times moved earlier by MS before snapping)")
for ms in (-20, -10, 10, 20, 30):
    new = []
    for s in songs:
        g = [[a - ms / 1000, b - ms / 1000, p, v] for a, b, p, v in s["stage_detail"]["game_lead"]]
        new.append(L.analyse(s, L.snap_notes(g, s["spacing"], s["onset_phase"])))
    d, lo, hi = L.boot_delta(base, new, iters=2000)
    print(f"  {ms:+3d} ms: {d:+6.1f} [{lo:+5.1f}, {hi:+5.1f}]  clean {L.clean(L.pool(new)) - L.clean(L.pool(base)):+.1f}", flush=True)
