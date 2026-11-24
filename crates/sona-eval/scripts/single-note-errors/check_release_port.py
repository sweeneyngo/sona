"""The Rust port (`sona-eval-cellstage --edit-cost --trim-releases`) reproduces the prototype: compares the
per-song class counts of its two "releases trimmed" reports with release_energy.trim_dip (0.3, 0.8, 2).

usage: S12_DUMP=DIR python3 -I check_release_port.py RUST_OUTPUT
"""
import os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L
import ec
from release_energy import trim_dip

text = open(sys.argv[1]).read()
sections = {"lead": "GAME lead + octave rule, releases trimmed", "stage": "learned cell stage, releases trimmed"}
songs = L.load()
ok = True
for out, title in sections.items():
    block = text[text.index(title):]
    block = block[: block.index("pooled cost by class")]
    rust = {}
    for line in block.splitlines():
        parts = line.split()
        if len(parts) >= 13 and all(x.isdigit() for x in parts[-8:]) and not parts[0] == "POOLED":
            rust[parts[0]] = [int(x) for x in parts[-8:]]
    for s in songs:
        A = L.analyse(s, trim_dip(s, [tuple(n) for n in s["outputs"][out]], s["onset_phase"], theta=0.3, theta_tail=0.8, minrun=2))
        py = [A["counts"][k] for k in ec.CLASSES]
        same = rust.get(s["id"]) == py
        ok &= same
        print(f"{out:5} {s['id']:28} {'same' if same else 'DIFFERENT'}  {py}" + ("" if same else f"  rust {rust.get(s['id'])}"))
print("OK: the port reproduces the prototype on every song" if ok else "MISMATCH")
