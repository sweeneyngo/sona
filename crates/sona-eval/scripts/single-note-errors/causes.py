"""The other candidate causes of single-note errors on GAME lead (dev):

- snapping: per-song raw GAME onset/offset bias against the UST, and how many start/end errors are within a
  quarter cell of the rounding point (a global rounding bias could move them);
- the snap rules: pair errors on notes whose snapped boundaries were changed by the minimum length or the
  monophonic trim (not by rounding);
- pitch: for each pitch error, whether the vocals' pitch track (median over the note) names the UST's pitch,
  GAME's, or neither; and how often it does on right notes;
- the UST author's rests: the length of the UST's short rests, per song.

usage: S12_DUMP=DIR python3 -I causes.py
"""
import bisect, collections, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L

songs = L.load()
print("per song: raw GAME onset/offset − UST boundary (cells), median over pairs; start/end error counts")
mods = collections.Counter()
pitch_ev = collections.Counter()
rest_len = collections.defaultdict(collections.Counter)
for s in songs:
    sp, an = s["spacing"], s["ust_anchor"]
    game = s["stage_detail"]["game_lead"]
    raw = L.snap_notes(game, sp, s["onset_phase"], keep=True)
    A = L.analyse(s, s["outputs"]["lead"])
    ps = L.pairs(A)
    ft = [x[0] for x in s["f0"]]
    ons, offs = [], []
    for p in ps:
        g = game[raw[p["o"][3]][3]]
        x_on, x_off = (g[0] - an) / sp, (g[1] - an) / sp
        ons.append(x_on - p["r"][0])
        offs.append(x_off - p["r"][1])
        # did a snap rule (not rounding) move this note's boundaries?
        ph = s["onset_phase"]
        k_on, k_off = round((g[0] - ph) / sp), round((g[1] - ph) / sp)
        o_on, o_off = round((raw[p["o"][3]][0] - ph) / sp), round((raw[p["o"][3]][1] - ph) / sp)
        moved = (o_on != k_on) or (o_off != k_off)
        if p["cost"] > 0 and (p["ds"] or p["de"]):
            mods["timing error, boundary set by a snap rule" if moved else "timing error, rounding only"] += 1
        elif p["cost"] == 0:
            mods["right, boundary set by a snap rule" if moved else "right, rounding only"] += 1
        # pitch evidence from the vocals' track
        i, j = bisect.bisect_left(ft, an + p["o"][0] * sp), bisect.bisect_left(ft, an + p["o"][1] * sp)
        m = [L.midi_of(s["f0"][k][1]) for k in range(i, j) if s["f0"][k][1] > 0]
        if m:
            med = round(st.median(m))
            if p["dp"]:
                kind = "octave" if abs(p["dp"]) == 12 else ("±1" if abs(p["dp"]) == 1 else "other")
                pitch_ev[(kind, "track = UST" if med == p["r"][2] else ("track = GAME" if med == p["o"][2] else "neither"))] += 1
            else:
                pitch_ev[("right", "track = UST" if med == p["r"][2] else "track differs")] += 1
        else:
            pitch_ev[("right" if not p["dp"] else "wrong", "no voiced frames")] += 1
    refs = A["refs"]
    for a, b in zip(refs, refs[1:]):
        g = b[0] - a[1]
        if 0 < g <= 8:
            rest_len[s["id"]][g] += 1
    nerr_s = sum(1 for p in ps if p["ds"])
    nerr_e = sum(1 for p in ps if p["de"])
    print(f"  {s['id']:28} div {s['division']:2d}  on {st.median(ons):+.2f} (mean {st.mean(ons):+.2f})  off {st.median(offs):+.2f}  "
          f"start errors {nerr_s:3d}  end errors {nerr_e:3d}  of {len(ps)} pairs")
print("\nsnap rules:", dict(mods))
print("\npitch: vocals' track median over the output note")
for k, v in sorted(pitch_ev.items()):
    print(f"  {k}: {v}")
print("\nthe UST's rests of 1–8 cells between notes, per song")
for sid, c in rest_len.items():
    tot = sum(c.values())
    print(f"  {sid:28} n {tot:3d}  " + " ".join(f"{g}:{c[g]}" for g in range(1, 9)))
