"""How does the UST divide a sona note that holds k=2 moras? First-piece length by note length, and by the
second mora's kind."""
import collections
import lib07 as L
import lyric_dp as D
from oracle1a_lib import counts_per_note

songs = L.load()
tab = collections.Counter()
bykind = collections.Counter()
for s in songs:
    g = L.to_grid(s, s["outputs"]["lead"])
    cn = counts_per_note(s, g)
    u = L.ust_notes(s)
    kind_at = {L.g_of(s, x["on"]): D.kind_of(x["lyric"]) for x in u}
    for (a, b, _), st in zip(g, cn):
        if len(st) == 2 and b - a >= 2:
            Lc = b - a
            first = st[1] - a
            tab[(min(Lc, 9), first)] += 1
            k = kind_at.get(st[1], "?")
            rel = "eq" if first == L.equal_cuts(a, b, 2)[0] - a else ("last1" if b - st[1] == 1 else "first1" if first == 1 else "other")
            bykind[(k, rel)] += 1
for Lc in range(2, 10):
    row = {f: tab[(Lc, f)] for f in range(-2, 12) if tab[(Lc, f)]}
    print(Lc, row)
print(sorted(bykind.items()))
