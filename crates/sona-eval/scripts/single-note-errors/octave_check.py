"""Octave errors on GAME lead: per song, and whether the octave check's odd/even harmonic ratio (already
used to raise notes) separates the notes still an octave low from right notes.

usage: S12_DUMP=DIR python3 -I octave_check.py
"""
import collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
from release_auc import auc

songs = L.load()
per = collections.Counter()
low, right = [], []
for s in songs:
    game = s["stage_detail"]["game_lead"]
    ratios = s["stage_detail"]["lead_ratios"]
    raw = L.snap_notes(game, s["spacing"], s["onset_phase"], keep=True)
    A = L.analyse(s, s["outputs"]["lead"])
    for p in L.pairs(A):
        r = ratios[raw[p["o"][3]][3]]
        if p["dp"] == -12:
            per[s["id"]] += 1
            low.append(r)
        elif p["dp"] == 0:
            right.append(r)
print("octave-low pairs per song:", dict(per))
print(f"harmonic ratio AUC (low ratio ⇒ octave low): {auc([-x for x in low], [-x for x in right]):.3f} "
      f"(n {len(low)} vs {len(right)})")
for t in (-0.5, 0.0, 0.5, 1.0):
    print(f"  ratio < {t:+.1f}: octave-low {sum(x < t for x in low)}, right {sum(x < t for x in right)}")
