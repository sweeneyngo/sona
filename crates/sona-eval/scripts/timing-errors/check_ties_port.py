"""The Rust port (`sona-eval-cellstage --edit-cost --settle-near-ties`) reproduces the prototype: compares the
per-song class counts of its two "near ties settled" reports with cands.c_tie_flip_notes (0.35, GAME on the
vocals, |lean| <= 0.5, one voice kept).

usage: S18_DUMP=DIR python3 -I check_ties_port.py RUST_OUTPUT
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import ec
import cands as C

text = open(sys.argv[1]).read()
sections = {"lead": "GAME lead + octave rule, near ties settled (opt-in)", "stage": "learned cell stage, near ties settled (opt-in)"}
songs = L.loads()
ok = True
for out, title in sections.items():
    block = text[text.index(title):]
    block = block[: block.index("pooled cost by class")]
    rust = {}
    for line in block.splitlines():
        parts = line.split()
        if len(parts) >= 13 and all(x.isdigit() for x in parts[-8:]) and parts[0] != "POOLED":
            rust[parts[0]] = [int(x) for x in parts[-9:]]
    for s in songs:
        A = L.analyse(s, C.c_tie_flip_notes(s, s["outputs"][out], 0.35, ("vocals",), True))
        py = [A["counts"][k] for k in ec.CLASSES]
        same = rust.get(s["id"]) == py
        ok &= same
        print(f"{out:5} {s['id']:28} {'same' if same else 'DIFFERENT'}  {py}" + ("" if same else f"  rust {rust.get(s['id'])}"))
print("OK: the port reproduces the prototype on every song" if ok else "MISMATCH")
