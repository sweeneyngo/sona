"""Check the Python path against Rust: the shipped checkpoint on the dumped Rust features gives the stage's notes
`sona-eval-cellstage` recorded, and lib19's scoring of them gives the Rust edit-cost counts.
usage: parity19.py CELLS_DIR CHECKPOINT RUST_REPORT [RUST_REPORT ...]"""
import sys, glob, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np, torch
import lib19 as L, model19 as M

cells, ckp = sys.argv[1], sys.argv[2]
rust = {}
for path in sys.argv[3:]:
    text = open(path).read()
    head = "edit cost, learned cell stage, onset phase (ships)"
    block = text[text.index(head):]
    block = block[: block.index("pooled cost by class")]
    for line in block.splitlines():
        p = line.split()
        if len(p) >= 13 and all(x.isdigit() for x in p[-9:]) and p[0] != "POOLED":
            rust[p[0]] = dict(zip(L.ec.CLASSES, map(int, p[-9:])))
ck = torch.load(ckp, weights_only=False)
ck["F"] = ck.get("F", 1046)
same_notes = same_counts = 0
maxdiff = 0.0
for f in sorted(glob.glob(f"{cells}/*.onset.json")):
    sid = os.path.basename(f).split(".")[0]
    d = L.load_song(cells, sid, "onset")
    lg = M.logits(ck, d)
    notes = L.cells_to_notes(L.choose(lg, d["cands"]), d)
    ok_n = len(notes) == len(d["stage"]) and all(abs(a[0] - b[0]) < 1e-9 and abs(a[1] - b[1]) < 1e-9 and a[2] == b[2] for a, b in zip(notes, d["stage"]))
    rs = L.row(d, [tuple(s) for s in d["stage"]])["counts"]
    ok_c = rs == rust.get(sid)
    same_notes += ok_n
    same_counts += ok_c
    print(f"{sid:30} notes {'=' if ok_n else 'DIFF'} ({len(notes)} vs {len(d['stage'])})  counts {'=' if ok_c else 'DIFF'}")
print(f"notes equal on {same_notes}, Rust counts reproduced on {same_counts} songs")
