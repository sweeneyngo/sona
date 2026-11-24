"""Lyrics-first 02 diagnosis: where the Viterbi puts each line's first note against where the UST says the line
starts (the first output note whose midpoint is at or after the line's first UST onset), in notes.
Usage: L01_DUMP=… L01_WORK=… python3 diag02.py OUT KIND PB SKIP|none GAMMA [ids…]
"""
import bisect, sys
import lib01 as L
import oracles as O
import lines02 as X

out, kind, pb, skip, gamma = sys.argv[1], sys.argv[2], float(sys.argv[3]), (None if sys.argv[4] == "none" else float(sys.argv[4])), float(sys.argv[5])
for s in L.load([a for a in sys.argv[6:] if not a.startswith("--")] or None):
    cells = O.out_cells(s, out)
    first_on, last_off = s["ust_l"][0][0], s["ust_l"][-1][1]
    body = [c for c in cells if first_on <= O.t_of(s, (c[0] + c[1]) / 2) < last_off]
    starts = s["lines"]["line_starts" if kind == "line" else "space_starts"]
    kinds, groups = X.line_kinds(s, starts)
    ranges = X.place_lines(body, kinds, pb, skip=skip, gamma=gamma, span="--span" in sys.argv)
    mids = [O.t_of(s, (c[0] + c[1]) / 2) for c in body]
    errs = []
    for (a, e), g in zip(ranges, groups):
        true_first = bisect.bisect_left(mids, s["ust_l"][g[0]][0])
        errs.append(a - true_first)
    exact = sum(1 for x in errs if x == 0)
    near = sum(1 for x in errs if abs(x) <= 1)
    print(f"{s['id']:28} lines {len(errs):3} exact {100 * exact / len(errs):5.1f}% within 1 {100 * near / len(errs):5.1f}%  errs {' '.join(str(x) for x in errs[:40])}")
