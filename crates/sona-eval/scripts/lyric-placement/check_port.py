"""The Rust port reproduces the prototype: per-song edit-cost class counts and lyric placement counts from
`sona-eval-cellstage --edit-cost --lyric-placement --lyric-lines DIR` against `oracles.py --dump-rows FILE`.
Usage: python3 check_port.py RUST_OUTPUT ROWS_JSON
"""
import json, re, sys

TITLES = {
    "pasted in order, no fit (today)": "d",
    "fitted per run of touching notes (07 as ported)": "a'",
    "fitted per UST phrase": "a",
    "fitted per UST phrase, rests hold moras": "a/r",
    "fitted as one phrase": "c",
    "fitted as one phrase, rests hold moras": "c/r",
    "fitted per lyric line": "b",
    "fitted per lyric line, rests hold moras": "b/r",
    "fitted per lyric line and space": "b+",
    "fitted per lyric line and space, rests hold moras": "b+/r",
    "fitted per lyric line cut at UST phrases, rests hold moras": "b|a/r",
}
OUTS = {"GAME lead + octave rule": "lead", "learned cell stage": "stage"}

text = open(sys.argv[1]).read().splitlines()
rows = json.load(open(sys.argv[2]))
rust = {}
section = None
for line in text:
    m = re.match(r"^(edit cost|lyric placement), (.*?), lyric (.*?)( \(weights.*)?$", line)
    if m:
        section = (m.group(1), OUTS[m.group(2)], TITLES[m.group(3)])
        continue
    if line and not line.startswith(" "):
        section = None
    if section and line.startswith("  ") and not line.strip().startswith(("song", "pooled", "POOLED")):
        f = line.split()
        if section[0] == "edit cost":
            rust.setdefault((section[1], section[2]), {}).setdefault(f[0], {})["ec"] = [int(x) for x in f[-8:]]
        else:
            placed, notes = map(int, f[5].split("/"))
            landed, phrases = map(int, f[7].split("/"))
            rust.setdefault((section[1], section[2]), {}).setdefault(f[0], {})["pl"] = [placed, notes, landed, phrases]
bad = checked = 0
for (out, v), songs in sorted(rust.items()):
    py = {r[0]: r for r in rows.get(f"{out}|{v}", [])}
    if not py:
        print(f"{out} {v}: not in the prototype rows")
        continue
    for sid, got in songs.items():
        r = py[sid]
        want = dict(ec=r[1], pl=[r[2]["placed"], r[2]["notes"], r[2]["landed"], r[2]["phrases"]])
        checked += 1
        if got != want:
            bad += 1
            print(f"MISMATCH {out} {v} {sid}: rust {got} python {want}")
print(f"{checked} song × output × landing rows compared, {bad} differ")
sys.exit(1 if bad or not checked else 0)
