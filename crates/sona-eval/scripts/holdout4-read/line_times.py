"""utau-edit-cost 17: 14's `--lyric-line-times` files (lyrics CTC, energy ends) without scoring anything.

`line-alignment/score.py --end energy` writes these files and also scores the lines and the landing against the
UST. Inside the holdout4 read the times are an input, not a result, so this writes them alone, with score.py's own
`spans` (the same rule, the same constants):
  $L14_WORK/times/<aligner>-energy/<id>.json = {"lines": [[first, start_s, end_s, score]], "pieces": […]}
from $L14_WORK/text/<id>.json (line and piece starts), align/<aligner>/<id>.json and rms/<id>.json.

Usage: L14_WORK=… python3 -I line_times.py [--aligner lyrics-lead] ID …
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import json, os  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "line-alignment"))
import score as SC  # noqa: E402


def main():
    args = sys.argv[1:]
    aligner = "lyrics-lead"
    if "--aligner" in args:
        i = args.index("--aligner")
        aligner = args[i + 1]
        del args[i:i + 2]
    work = os.environ["L14_WORK"]
    out = os.path.join(work, "times", aligner + "-energy")
    os.makedirs(out, exist_ok=True)
    for sid in args:
        text = json.load(open(os.path.join(work, "text", sid + ".json")))
        al = json.load(open(os.path.join(work, "align", aligner, sid + ".json")))
        rms = json.load(open(os.path.join(work, "rms", sid + ".json")))
        lines = SC.spans(text, al, text["lines"], "energy", rms)
        pieces = SC.spans(text, al, text["pieces"], "energy", rms)
        json.dump({"lines": lines, "pieces": pieces}, open(os.path.join(out, sid + ".json"), "w"))
        print(sid, len(lines), "lines", len(pieces), "pieces")


if __name__ == "__main__":
    main()
