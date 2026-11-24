"""Song-level DP with the UST's kana and line breaks only at rests of >= R cells (sona's phrase-start rule)."""
import itertools
import lib07 as L
from lyric_song import evaluate

songs = L.load()
for out in ("lead", "stage"):
    for R, pb, gap in itertools.product([2, 4], [3.0, 10.0], [0.0, 0.5]):
        p = dict(beta=0.5, gap=gap, cap=2, skip=3.0, zero=2.0, kmax=8, pb=pb, R=R, tail=2, merge=True)
        evaluate(songs, out, "ust+breaksR", p, f"R={R} pb={pb} gap={gap} cap=2")
