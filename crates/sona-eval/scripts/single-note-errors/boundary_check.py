"""Legato boundaries between two GAME lead notes of different pitch: does the vocals' pitch track place the
boundary on the UST's cell more often than GAME does? The track's boundary is the first frame after which
the track stays closer to the new pitch than the old one for MINRUN frames, searched within ±1.5 cells of
GAME's boundary.

usage: S12_DUMP=DIR python3 -I boundary_check.py
"""
import bisect, collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L

MINRUN = 4
songs = L.load()
tally = collections.Counter()
for s in songs:
    sp, an = s["spacing"], s["ust_anchor"]
    tune = L.song_tune(s["stage_detail"]["game_lead"])
    A = L.analyse(s, s["outputs"]["lead"])
    ft = [x[0] for x in s["f0"]]
    ref_starts = {r[0] for r in A["refs"]}
    outs = A["outs"]
    for a, b in zip(outs, outs[1:]):
        if a[1] != b[0] or a[2] == b[2]:
            continue
        # the UST boundary nearest GAME's (within 2 cells) between notes of these two pitches
        cand = [r for r in A["refs"] if abs(r[0] - b[0]) <= 2 and r[2] == b[2]]
        if not cand:
            continue
        truth = min(cand, key=lambda r: abs(r[0] - b[0]))[0]
        t0, t1 = an + (b[0] - 1.5) * sp, an + (b[0] + 1.5) * sp
        i, j = bisect.bisect_left(ft, t0), bisect.bisect_left(ft, t1)
        close_new = []
        for q in range(i, j):
            hz = s["f0"][q][1]
            if hz <= 0:
                close_new.append(None)
                continue
            m = L.midi_of(hz) - tune
            close_new.append(abs(m - b[2]) < abs(m - a[2]))
        cross = None
        for q in range(len(close_new) - MINRUN + 1):
            if all(close_new[q + d] for d in range(MINRUN)):
                cross = s["f0"][i + q][0]
                break
        if cross is None:
            tally["track: no crossing"] += 1
            continue
        k = round((cross - an) / sp)
        g_ok, t_ok = b[0] == truth, k == truth
        tally[("GAME right" if g_ok else "GAME wrong") + ", " + ("track right" if t_ok else "track wrong")] += 1
for k, v in sorted(tally.items()):
    print(f"  {k}: {v}")
