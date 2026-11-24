import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *

songs = load(sys.argv[1])
bl, bs = [], []
for s in songs:
    sd = s["stage_detail"]
    sp, an = s["spacing"], s["onset_phase"]
    lead = snap_notes(sd["game_lead"], sp, an)
    ok_l = [tuple(x) for x in s["outputs"]["lead"]] == [tuple(x) for x in lead]
    cl = choose(sd["probs"], sd["cands"])
    ok_c = cl == sd["classes"]
    st = cells_to_notes(cl, sd["cands"], sd["sources"], sd["k0"], sp, an)
    ok_s = [tuple(x) for x in s["outputs"]["stage"]] == st
    rl, rs = row_of(s, lead), row_of(s, st)
    same = lambda r, k: [r[4][c] for c in ec.CLASSES] == s["edit_cost"][k]
    print(s["id"], "lead notes", ok_l, "classes", ok_c, "stage notes", ok_s, "Rust counts", same(rl, "lead"), same(rs, "stage"))
    bl.append(rl)
    bs.append(rs)
print("lead", pooled(bl), "stage", pooled(bs))
