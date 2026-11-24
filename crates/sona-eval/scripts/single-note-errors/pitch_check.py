"""Semitone errors: is GAME's raw (tuned) pitch near the rounding point when it is a semitone off, and does
the vocals' pitch track (median, or the mean of the note's middle half, which skips scoops and releases)
fall on the UST's side? Then the cost of re-rounding near-boundary notes by the track.

usage: S12_DUMP=DIR python3 -I pitch_check.py
"""
import bisect, collections, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L

songs = L.load()
hist = collections.defaultdict(collections.Counter)
data = []
for s in songs:
    sp, an = s["spacing"], s["ust_anchor"]
    game = s["stage_detail"]["game_lead"]
    tune = L.song_tune(game)
    raw = L.snap_notes(game, sp, s["onset_phase"], keep=True)
    A = L.analyse(s, s["outputs"]["lead"])
    ft = [x[0] for x in s["f0"]]
    for p in L.pairs(A):
        g = game[raw[p["o"][3]][3]]
        x = g[2] + tune
        dist = abs((x - round(x)))  # 0 = centre of a semitone, 0.5 = rounding point
        a, b = an + p["o"][0] * sp, an + p["o"][1] * sp
        mid_a, mid_b = a + (b - a) / 4, b - (b - a) / 4
        i, j = bisect.bisect_left(ft, mid_a), bisect.bisect_left(ft, mid_b)
        m = [L.midi_of(s["f0"][k][1]) - tune for k in range(i, j) if s["f0"][k][1] > 0]
        track = st.median(m) if m else None
        kind = "right" if p["dp"] == 0 else ("±1" if abs(p["dp"]) == 1 else ("octave" if abs(p["dp"]) == 12 else "other"))
        hist[kind][min(4, int(dist * 10))] += 1
        data.append(dict(song=s["id"], kind=kind, x=x, dist=dist, track=track, r=p["r"][2], o=p["o"][2]))
print("GAME's tuned pitch: distance from the semitone centre (0.0–0.5), in tenths")
for k, c in hist.items():
    tot = sum(c.values())
    print(f"  {k:7} n {tot:4d}  " + " ".join(f"{b / 10:.1f}-:{100 * c[b] / tot:4.1f}%" for b in range(5)))
print("\nrule: if GAME's pitch is within D of the rounding point and the track's middle-half median rounds to the")
print("neighbouring semitone on the same side, take the track's semitone. Fixes / breaks (pairs only):")
for D in (0.05, 0.1, 0.15, 0.2, 0.3, 0.5):
    fix = brk = other = 0
    for d in data:
        if d["track"] is None or d["dist"] < 0.5 - D:
            continue
        t = round(d["track"])
        if t == d["o"] or abs(t - d["o"]) != 1 or (t - d["o"]) * (d["x"] - d["o"]) <= 0:
            continue
        if d["kind"] == "right":
            brk += 1
        elif t == d["r"]:
            fix += 1
        else:
            other += 1
    print(f"  D {D:.2f}: fixes {fix}, breaks {brk}, changes another wrong note {other}")
