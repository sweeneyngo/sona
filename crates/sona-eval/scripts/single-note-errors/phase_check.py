"""Per song: where GAME lead's raw onsets sit against the onset phase (circular mean of their fractional cell)
and against the UST, and the cost of snapping GAME lead on its own phase (as `snap_to_grid` does) or on a
blend, instead of the vocals' onset phase.

usage: S12_DUMP=DIR python3 -I phase_check.py
"""
import math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L


def circ(xs):
    x = sum(math.cos(2 * math.pi * v) for v in xs)
    y = sum(math.sin(2 * math.pi * v) for v in xs)
    return math.atan2(y, x) / (2 * math.pi)


songs = L.load()
base = [L.analyse(s, s["outputs"]["lead"]) for s in songs]
rows = {"own": [], "half": []}
for s, b in zip(songs, base):
    sp, ph, an = s["spacing"], s["onset_phase"], s["ust_anchor"]
    on = [g[0] for g in s["stage_detail"]["game_lead"] if g[3]]
    game_vs_onset = circ([(t - ph) / sp for t in on])
    ust_vs_onset = circ([(an - ph) / sp])
    own = ph + game_vs_onset * sp
    half = ph + 0.5 * game_vs_onset * sp
    a_own = L.analyse(s, L.snap_notes(s["stage_detail"]["game_lead"], sp, own))
    a_half = L.analyse(s, L.snap_notes(s["stage_detail"]["game_lead"], sp, half))
    rows["own"].append(a_own)
    rows["half"].append(a_half)
    m = b["span_s"] / 60
    print(f"  {s['id']:28} GAME onsets vs onset phase {game_vs_onset:+.2f} cell, UST vs onset phase {ust_vs_onset:+.2f}; "
          f"cost/min {L.per_min(L.pool([b])):6.1f} → own phase {L.per_min(L.pool([a_own])):6.1f}, half {L.per_min(L.pool([a_half])):6.1f}")
for k, v in rows.items():
    d, lo, hi = L.boot_delta(base, v)
    print(f"{k}: {d:+.1f} [{lo:+.1f}, {hi:+.1f}]  clean {L.clean(L.pool(v)) - L.clean(L.pool(base)):+.1f}")
