"""Lyrics-first 01: do the lyric text's line breaks fall on sona's rests, and on the UST's?

A break between UST notes k-1 and k spans the UST cells [end of k-1, start of k] (zero-width when they
touch). It falls on a rest of an output when the output has a gap between two of its notes (on the UST's
cells) that comes within one cell of that span. Also: how many of the output's rests are breaks, and how
many rests a line holds.
Usage: L01_DUMP=… L01_WORK=… python3 breaks.py
"""
import collections
import lib01 as L
from lib01 import ec


def cells_of(s, notes):
    return ec.to_cells([tuple(n[:3]) for n in notes], s["spacing"], s["ust_anchor"])


def gaps(cells, lo, hi, min_cells=1):
    return [(a[1], b[0]) for a, b in zip(cells, cells[1:]) if b[0] - a[1] >= min_cells and a[1] >= lo and b[0] <= hi]


def ust_bounds(s):
    sp, an = s["spacing"], s["ust_anchor"]
    return [(ec.rround((a - an) / sp), ec.rround((b - an) / sp)) for a, b, _, _ in s["ust_l"]]


def main():
    songs = L.load()
    for min_rest in (1, 2, 4):
        tot = collections.Counter()
        rows = []
        for s in songs:
            ub = ust_bounds(s)
            lo, hi = ub[0][0], ub[-1][1]
            line_breaks = s["lines"]["line_starts"]
            ust_rest_breaks = [k for k in range(1, len(ub)) if ub[k][0] - ub[k - 1][1] >= min_rest]
            row = collections.Counter(lines=len(line_breaks) + 1, ust_rests=len(ust_rest_breaks))
            spans = {k: (ub[k - 1][1], ub[k][0]) for k in range(1, len(ub))}
            row["line_on_ust_rest"] = sum(1 for k in line_breaks if k in set(ust_rest_breaks))
            row["ust_rest_on_line"] = sum(1 for k in ust_rest_breaks if k in set(line_breaks))
            for out in ("lead", "stage"):
                g = gaps(cells_of(s, s["outputs"][out]), lo, hi, min_rest)
                near = lambda span: any(a <= span[1] + 1 and b >= span[0] - 1 for a, b in g)
                row[f"line_on_{out}_rest"] = sum(1 for k in line_breaks if near(spans[k]))
                row[f"ust_rest_on_{out}_rest"] = sum(1 for k in ust_rest_breaks if near(spans[k]))
                row[f"{out}_rests"] = len(g)
                used = sum(1 for a, b in g if any(a <= spans[k][1] + 1 and b >= spans[k][0] - 1 for k in line_breaks))
                row[f"{out}_rest_is_line"] = used
            rows.append((s["id"], row))
            tot.update(row)
        print(f"\nrests of at least {min_rest} cell(s)")
        print(f"  {'song':28} lines  line@UST-rest  UST-rests  @line  " + "  ".join(f"line@{o}-rest {o}-rests  is-line" for o in ("lead", "stage")))
        for sid, r in rows + [("POOLED", tot)]:
            pct = lambda a, b: f"{a:4}/{b:<4} {100 * a / max(b, 1):5.1f}%"
            print(f"  {sid:28} {r['lines']:5}  {pct(r['line_on_ust_rest'], r['lines'] - 1)}  {pct(r['ust_rest_on_line'], r['ust_rests'])}  "
                  + "  ".join(f"{pct(r[f'line_on_{o}_rest'], r['lines'] - 1)}  {pct(r[f'{o}_rest_is_line'], r[f'{o}_rests'])}" for o in ("lead", "stage")))


if __name__ == "__main__":
    main()
