"""Compare the Rust --fit-phrase-lyrics per-song class counts with the prototype's (proto_counts.json)."""
import json, os, re, sys
HERE = os.environ.get("S07_WORK", ".")
proto = json.load(open(os.path.join(HERE, "proto_counts.json")))
text = open(os.path.join(HERE, "rust-fit.out")).read()
sections = re.split(r"\nedit cost, ", text)
want = {"GAME lead + octave rule, each phrase's lyric fitted": "lead", "learned cell stage, each phrase's lyric fitted": "stage"}
ok = bad = 0
for sec in sections:
    out = next((v for k, v in want.items() if sec.startswith(k)), None)
    if not out:
        continue
    for line in sec.splitlines():
        m = re.match(r"\s+(\S+)\s+[\d.]+\s+\d+\s+[\d.]+\s+[\d.]+%\s+\d+/\d+\s+((?:\d+\s*)+)$", line)
        if m and not m.group(1).startswith("POOLED"):
            counts = [int(x) for x in m.group(2).split()]
            p = proto[f"{out}/{m.group(1)}"]
            if counts == p:
                ok += 1
            else:
                bad += 1
                print("DIFF", out, m.group(1), "rust", counts, "proto", p)
        if "pooled:" in line:
            print(out, line.strip())
print(f"{ok} song x output rows equal, {bad} differ")
