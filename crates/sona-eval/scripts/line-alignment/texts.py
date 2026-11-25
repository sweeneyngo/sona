"""Lyrics-first 14: the text a user pastes, per dev song, from 03's dump and 01's line files.

Each line is the lead UST's moras between 01's line starts; a space splits it where 01 found one. Writes
$L14_WORK/text/<id>.json: {"moras": [{"i": UST note index, "kana": mora, "on": s, "off": s}],
"lines": [first index …], "pieces": [first index …]} (lyric-derived: stays out of the repo).

Usage: L01_DUMP=… L01_WORK=… L14_WORK=… python3 texts.py
"""
import json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "lyric-placement"))
import lib01 as L  # noqa: E402

WORK = os.environ["L14_WORK"]


def main():
    os.makedirs(os.path.join(WORK, "text"), exist_ok=True)
    for s in L.load():
        if s["lines"] is None:
            continue
        n = len(s["ust_l"])
        clean = lambda xs: sorted(set([0] + [x for x in xs if 0 < x < n]))
        moras = [{"i": i, "kana": L.norm(l), "on": a, "off": b} for i, (a, b, _, l) in enumerate(s["ust_l"])]
        out = {"id": s["id"], "spacing": s["spacing"], "moras": moras, "lines": clean(s["lines"]["line_starts"]), "pieces": clean(s["lines"]["space_starts"])}
        json.dump(out, open(os.path.join(WORK, "text", s["id"] + ".json"), "w"), ensure_ascii=False)
        print(s["id"], n, "moras", len(out["lines"]), "lines", len(out["pieces"]), "pieces")


if __name__ == "__main__":
    main()
