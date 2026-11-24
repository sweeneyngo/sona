"""Where does the per-phrase kana DP put extra moras, against the true per-note counts?"""
import collections
import lib07 as L
import lyric_dp as D
from oracle1a_lib import counts_per_note
from oracle1b_phrase import phrase_items, songs

p = dict(beta=0.5, zero=2.0, skip=3.0, kmax=8, kind_aware=True, flat=1.0)
conf = collections.Counter()
deficit = collections.Counter()
for s in songs:
    g = L.to_grid(s, s["outputs"]["lead"])
    cn = counts_per_note(s, g)
    for idx, kinds in phrase_items(s, g):
        if not idx:
            continue
        ks = D.align([g[i][1] - g[i][0] for i in idx], kinds, p)
        true = [len(cn[i]) for i in idx]
        deficit[(min(len(kinds) - len(idx), 5), min(sum(max(0, t - 1) for t in true), 5))] += 1
        for i, k, t in zip(idx, ks, true):
            conf[(min(t, 3), min(k, 3))] += 1
print("rows true count, cols assigned (0..3+)")
for t in range(4):
    print(t, [conf[(t, k)] for k in range(4)])
print("phrase deficit (moras - sona notes) vs true extra moras in sona notes (count of phrases):")
for d in range(-2, 6):
    print(d, [deficit[(d, e)] for e in range(6)])
