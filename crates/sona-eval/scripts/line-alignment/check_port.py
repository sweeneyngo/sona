"""Lyrics-first 14: the Rust landing (`sona-eval-cellstage --lyric-line-times DIR`, `fit_lines`) against the
prototype (`score.py --dump-rows FILE`), per song: edit-cost class counts and lyric placement counts.

  python3 check_port.py RUST_OUTPUT ROWS_JSON TAG     # aligned rows against score.py's rows for TAG
  python3 check_port.py RUST_OUTPUT --ceiling         # UST-timed lines (times/ust) against 01's b/r, b+/r
"""
import json, re, sys

TITLES = {
    "fitted per lyric line, rests hold moras": "b/r",
    "fitted per lyric line and space, rests hold moras": "b+/r",
    "fitted per aligned lyric line, rests hold moras": "b/t",
    "fitted per aligned lyric line and space, rests hold moras": "b+/t",
}
OUTS = {"GAME lead + octave rule": "lead", "learned cell stage": "stage"}


def parse(path):
    rust, section = {}, None
    for line in open(path).read().splitlines():
        m = re.match(r"^(edit cost|lyric placement), (.*?), lyric (.*?)( \(weights.*)?$", line)
        if m:
            section = (m.group(1), OUTS.get(m.group(2)), TITLES.get(m.group(3)))
            continue
        if line and not line.startswith(" "):
            section = None
        if section and section[2] and line.startswith("  ") and not line.strip().startswith(("song", "pooled", "POOLED")):
            f = line.split()
            row = rust.setdefault((section[1], section[2]), {}).setdefault(f[0], {})
            if section[0] == "edit cost":
                row["ec"] = [int(x) for x in f[-8:]]
            else:
                placed, notes = map(int, f[5].split("/"))
                landed, phrases = map(int, f[7].split("/"))
                row["pl"] = [placed, notes, landed, phrases]
    return rust


def main():
    rust = parse(sys.argv[1])
    bad = checked = 0
    if sys.argv[2] == "--ceiling":
        pairs = [((o, t), (o, r)) for o in ("lead", "stage") for t, r in (("b/t", "b/r"), ("b+/t", "b+/r"))]
        for got_k, want_k in pairs:
            for sid, got in rust.get(got_k, {}).items():
                checked += 1
                if got != rust[want_k][sid]:
                    bad += 1
                    print(f"MISMATCH {got_k} {sid}: timed {got} groups {rust[want_k][sid]}")
    else:
        rows, tag = json.load(open(sys.argv[2])), sys.argv[3]
        for (out, v), songs in sorted(rust.items()):
            if v not in ("b/t", "b+/t"):
                continue
            py = {r[0]: r for r in rows[f"{out}|{tag}|{v}"]}
            for sid, got in songs.items():
                r = py[sid]
                want = dict(ec=r[1], pl=[r[2]["placed"], r[2]["notes"], r[2]["landed"], r[2]["phrases"]])
                checked += 1
                if got != want:
                    bad += 1
                    print(f"MISMATCH {out} {v} {sid}: rust {got} python {want}")
    print(f"{checked} song × output × landing rows compared, {bad} differ")
    sys.exit(1 if bad or not checked else 0)


if __name__ == "__main__":
    main()
