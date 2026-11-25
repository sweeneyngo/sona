"""Rust `decode_classes` = lib19.viterbi: a checkpoint's logits decoded in Python at SWITCH give the stage notes
that `sona-eval-cellstage --dump-cells` recorded for the same model exported with that switch cost.
usage: parity_decode19.py CELLS_DIR(of the Rust run) CHECKPOINT SWITCH"""
import glob, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import torch
import lib19 as L, model19 as M

cells, ckp, switch = sys.argv[1], sys.argv[2], float(sys.argv[3])
ck = torch.load(ckp, weights_only=False)
ck["F"] = ck.get("F", 1046)
ok = total = 0
for f in sorted(glob.glob(f"{cells}/*.onset.json")):
    sid = os.path.basename(f).split(".")[0]
    d = L.load_song(cells, sid, "onset")
    cl = L.viterbi(M.log_softmax_masked(M.logits(ck, d), d["cands"]), d, switch)
    notes = L.cells_to_notes(cl, d)
    same = len(notes) == len(d["stage"]) and all(abs(a[0] - b[0]) < 1e-9 and abs(a[1] - b[1]) < 1e-9 and a[2] == b[2] for a, b in zip(notes, d["stage"]))
    ok += same
    total += 1
    if not same:
        diff = sum(1 for a, b in zip(notes, d["stage"]) if tuple(a) != tuple(b))
        print(f"{sid}: {len(notes)} vs {len(d['stage'])} notes, {diff} differ")
print(f"decoded notes equal on {ok} of {total} songs")
