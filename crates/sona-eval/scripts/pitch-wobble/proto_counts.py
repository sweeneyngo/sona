"""Per-song edit-cost class counts of the chosen merge rule, alone and followed by 07's fit, for checking the
Rust port (`sona_core::unheard_pieces`, `sona-eval-cellstage --merge-unheard-pieces`) exactly.
Writes $S11_WORK/proto_counts.json and checks it against a Rust run's output when given its path."""
import json, os, re, sys
import wob
import rules as R
import fit07
L = wob.L

CHOSEN = dict(ls=1, smax=11, novoc=True, xf=None, nof0=True, pos="any", ties=True)


def merged(s, out):
    g = wob.grid(s, out)
    return wob.merge(g, R.joins_for(g, R.prepare(s, g), CHOSEN))


if __name__ == "__main__":
    songs = wob.load()
    res = {}
    for out in ("lead", "stage"):
        for s in songs:
            m = merged(s, out)
            res[f"{out}/{s['id']}"] = [L.score(s, L.from_grid(s, m))["counts"][k] for k in wob.ec.CLASSES]
            res[f"{out}+fit/{s['id']}"] = [L.score(s, L.from_grid(s, fit07.fit(s, m)))["counts"][k] for k in wob.ec.CLASSES]
    path = os.path.join(os.environ.get("S11_WORK", "."), "proto_counts.json")
    json.dump(res, open(path, "w"), indent=1)
    print(f"wrote {path}")
    if len(sys.argv) > 1:
        want = {
            "GAME lead + octave rule, unheard pieces merged": "lead",
            "learned cell stage, unheard pieces merged": "stage",
            "GAME lead + octave rule, unheard pieces merged, each phrase's lyric fitted": "lead+fit",
            "learned cell stage, unheard pieces merged, each phrase's lyric fitted": "stage+fit",
        }
        ok = bad = 0
        for sec in re.split(r"\nedit cost, ", open(sys.argv[1]).read()):
            title = sec.split(" (weights")[0].replace(" (opt-in)", "")
            key = want.get(title)
            if not key:
                continue
            for line in sec.splitlines():
                m = re.match(r"\s+(\S+)\s+[\d.]+\s+\d+\s+[\d.]+\s+[\d.]+%\s+\d+/\d+\s+((?:\d+\s*)+)$", line)
                if m and not m.group(1).startswith("POOLED"):
                    counts = [int(x) for x in m.group(2).split()]
                    if counts == res[f"{key}/{m.group(1)}"]:
                        ok += 1
                    else:
                        bad += 1
                        print("DIFF", key, m.group(1), "rust", counts, "proto", res[f"{key}/{m.group(1)}"])
                if "pooled:" in line:
                    print(key, line.strip())
        print(f"{ok} song x output rows equal, {bad} differ")
