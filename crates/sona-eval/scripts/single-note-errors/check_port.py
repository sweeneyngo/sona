"""The Python snap port reproduces the dumped GAME lead output, and ec.py the dumped Rust counts.

usage: S12_DUMP=DIR python3 -I check_port.py
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
import ec

songs = L.load()
ok = True
for s in songs:
    sn = L.snap_notes(s["stage_detail"]["game_lead"], s["spacing"], s["onset_phase"])
    lead = [tuple(x) for x in s["outputs"]["lead"]]
    same = len(sn) == len(lead) and all(abs(a[0] - b[0]) < 1e-6 and abs(a[1] - b[1]) < 1e-6 and a[2] == b[2] for a, b in zip(sn, lead))
    ok &= same
    for out in ("lead", "stage", "pyin"):
        A = L.analyse(s, s["outputs"][out])
        got = [A["counts"][k] for k in ec.CLASSES]
        if got != s["edit_cost"][out]:
            ok = False
            print("MISMATCH", s["id"], out, got, s["edit_cost"][out])
    print(f"{s['id']:28} snap port identical: {same} ({len(sn)} notes)")
for o in ("pyin", "lead", "stage"):
    p = L.pool([L.analyse(s, s["outputs"][o]) for s in songs])
    print(f"{o:6} {L.per_min(p):6.1f} cost/min  clean {L.clean(p):5.1f}%  " + " ".join(f"{k} {p['counts'].get(k, 0)}" for k in ec.CLASSES))
print("OK" if ok else "MISMATCH")
