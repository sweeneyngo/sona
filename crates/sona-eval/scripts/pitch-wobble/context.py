"""Local shapes around 1-2 semitone cuts, by truth: a return (A B A, B one step away: vibrato?), a run that
keeps going (A B C, a scale or a glide), and the lengths involved. Diagnosis for choosing a rule's gate."""
import collections
import wob
import cands as C

songs = wob.load()
for out in ("lead", "stage"):
    tally = collections.Counter()
    for s in songs:
        g = wob.grid(s, out)
        lab, _ = wob.truth(s, g)
        for i in range(1, len(g)):
            a, b = g[i - 1], g[i]
            if a[1] != b[0] or not 0 < abs(b[2] - a[2]) <= 2:
                continue
            t = lab.get(i, "other")
            # does the piece after b come back to a's pitch, touching? (A B A) or before a? (B A B)
            ret_after = i + 1 < len(g) and g[i + 1][0] == b[1] and g[i + 1][2] == a[2]
            ret_before = i >= 2 and g[i - 2][1] == a[0] and g[i - 2][2] == b[2]
            ret = ret_after or ret_before
            short = min(a[1] - a[0], b[1] - b[0])
            tally[(t, "return" if ret else "no return", "short 1" if short == 1 else "short 2" if short == 2 else "short 3+")] += 1
    print(f"== {out}: touching 1-2 st cuts by (truth, A-B-A return, shorter piece)")
    for ret in ("return", "no return"):
        for sh in ("short 1", "short 2", "short 3+"):
            c, b = tally[("chopped", ret, sh)], tally[("boundary", ret, sh)]
            print(f"  {ret:9} {sh:8}: chopped {c:4d}  boundary {b:5d}  precision {100 * c / max(1, c + b):5.1f}%")
