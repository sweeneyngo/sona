"""Paired song-bootstrap intervals for any two sections of a `sona-eval-cellstage --edit-cost` report (the
combinations the prototype does not build), from the per-song class counts it prints. Song spans come from
the dump, so the cost/min is the report's.

usage: S18_DUMP=DIR python3 -I rust_rows.py RUST_OUTPUT "BASE TITLE" "NEW TITLE" [...more NEW TITLE]
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import ec


def section(text, title):
    head = f"edit cost, {title} (weights"
    block = text[text.index(head):]
    block = block[: block.index("pooled cost by class")]
    rows = {}
    for line in block.splitlines():
        parts = line.split()
        if len(parts) >= 13 and all(x.isdigit() for x in parts[-9:]) and parts[0] != "POOLED":
            meas = parts[-10].split("/")
            rows[parts[0]] = (dict(zip(ec.CLASSES, map(int, parts[-9:]))), int(meas[0]), int(meas[1]))
    return rows


text = open(sys.argv[1]).read()
songs = L.loads()
span = {s["id"]: L.analyse(s, s["outputs"]["lead"])["span_s"] for s in songs}


def as_rows(sec):
    return [dict(counts=sec[s["id"]][0], clean=sec[s["id"]][1], measures=sec[s["id"]][2], span_s=span[s["id"]]) for s in songs]


base = as_rows(section(text, sys.argv[2]))
print(f"base: {sys.argv[2]}: {L.per_min(L.pool(base)):.1f} cost/min, clean {L.clean(L.pool(base)):.1f}%, missing {L.missing(L.pool(base))}")
for title in sys.argv[3:]:
    new = as_rows(section(text, title))
    _, _, hi4 = L.boot_delta(base, new, stat=L.per_min_missing4)
    print(L.fmt_row(title[:52], base, new, f"  missing@4 upper {hi4:+5.1f}"))
    print("    per song Δ cost/min: " + ", ".join(f"{s['id'].split('-')[0]} {L.per_min(n) - L.per_min(b):+.1f}" for s, b, n in zip(songs, base, new)))
