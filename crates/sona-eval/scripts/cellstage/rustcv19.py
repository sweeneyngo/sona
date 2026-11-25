"""The CV table from the Rust scorer: per-song stage rows of `sona-eval-cellstage --edit-cost --fold-models DIR`
(one report per split), pooled over the songs, with paired song-bootstrap Δ against a base run.

usage: rustcv19.py CELLS_DIR BASE_REPORTS NEW_REPORTS [NEW_REPORTS ...]
  *_REPORTS = comma-separated report paths of one recipe (one per split); "title=" prefix names the row.
  SECTION picks the report section (default: the stage on the onset phase, what ships)."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib19 as L

SECTION = os.environ.get("SECTION", "learned cell stage, onset phase (ships)")


def rows_of(paths):
    rows = {}
    for path in paths.split(","):
        text = open(path).read()
        block = text[text.index(f"edit cost, {SECTION} (weights"):]
        block = block[: block.index("pooled cost by class")]
        for line in block.splitlines():
            p = line.split()
            if len(p) >= 13 and all(x.isdigit() for x in p[-9:]) and p[0] != "POOLED":
                meas = p[-10].split("/")
                rows[p[0]] = dict(counts=dict(zip(L.ec.CLASSES, map(int, p[-9:]))), clean=int(meas[0]), measures=int(meas[1]))
    return rows


def main():
    cells = sys.argv[1]
    base = rows_of(sys.argv[2].partition("=")[2] if "=" in sys.argv[2] else sys.argv[2])
    ids = sorted(base)
    for s in ids:
        d = json.load(open(f"{cells}/{s}.onset.json"))
        A = L.ec.analyse(d["reference"], [], d["spacing"], d["ust_anchor"], d["division"])
        base[s]["span_s"] = A["span_s"]
    B = [base[s] for s in ids]
    pb = L.pool(B)
    print(f"base ({len(ids)} songs): {L.per_min(pb):.1f} cost/min, clean {L.clean(pb):.1f}%, missing {pb['counts']['missing']}")
    print("  units: " + " ".join(f"{k} {v * L.ec.W[k]}" for k, v in pb["counts"].items()))
    for arg in sys.argv[3:]:
        title, _, paths = arg.rpartition("=")
        new = rows_of(paths)
        assert sorted(new) == ids, f"{title}: songs differ from the base's"
        R = [dict(new[s], span_s=base[s]["span_s"]) for s in ids]
        print(L.fmt(title or paths[:44], B, R))
        pr = L.pool(R)
        print("    Δ units by class: " + " ".join(f"{k} {(pr['counts'][k] - pb['counts'][k]) * L.ec.W[k]:+d}" for k in L.ec.CLASSES))


if __name__ == "__main__":
    main()
