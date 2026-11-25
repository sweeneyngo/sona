"""Per split and per song: the final CV's Rust rows of two recipes (rustcv19.py's inputs), pooled per split, and
how many songs get better or worse.

usage: splits19.py CELLS_DIR BASE_REPORTS NEW_REPORTS   (comma-separated, in the order dev,holdout,holdout2,holdout3)"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib19 as L
import rustcv19 as R  # noqa: F401  (rows_of)

cells = sys.argv[1]
names = ["dev", "holdout", "holdout2", "holdout3"]
base_paths, new_paths = sys.argv[2].split(","), sys.argv[3].split(",")
deltas = []
for split, bp, np_ in zip(names, base_paths, new_paths):
    b, n = R.rows_of(bp), R.rows_of(np_)
    for s in b:
        d = json.load(open(f"{cells}/{s}.onset.json"))
        span = L.ec.analyse(d["reference"], [], d["spacing"], d["ust_anchor"], d["division"])["span_s"]
        b[s]["span_s"] = n[s]["span_s"] = span
        deltas.append((L.per_min(L.pool([n[s]])) - L.per_min(L.pool([b[s]])), s))
    pb, pn = L.pool(list(b.values())), L.pool(list(n.values()))
    print(f"{split:9} {len(b):2} songs  {L.per_min(pb):6.1f} -> {L.per_min(pn):6.1f} ({L.per_min(pn) - L.per_min(pb):+6.1f})  "
          f"clean {L.clean(pb):4.1f}% -> {L.clean(pn):4.1f}%  missing {pb['counts']['missing']} -> {pn['counts']['missing']}")
deltas.sort()
print(f"songs better {sum(d < 0 for d, _ in deltas)}, worse {sum(d > 0 for d, _ in deltas)}; "
      f"largest gains {', '.join(f'{d:+.1f}' for d, _ in deltas[:3])}; largest losses {', '.join(f'{d:+.1f}' for d, _ in deltas[-3:])}")
