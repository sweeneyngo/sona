"""How much of a transcript's error comes from the reading step (lyrics-first 05): of the hypothesis moras
that align to a UST mora (match or substitution), the substitution rate of those read from a token with
kanji against those the recogniser wrote in kana. Numbers only.

    python reading_errors.py TRANSCRIPT_DIR REF_DIR"""
import glob
import json
import os
import sys
import unicodedata

import moras as M
import score


def kanji(text):
    return any("一" <= c <= "鿿" or c == "々" for c in text)


tr_dir, ref_dir = sys.argv[1], sys.argv[2]
tot = {True: [0, 0], False: [0, 0]}
for tp in sorted(glob.glob(os.path.join(tr_dir, "*.json"))):
    tr = json.load(open(tp))
    ref = [m[0] for m in json.load(open(os.path.join(ref_dir, tr["id"] + ".json")))["moras"]]
    hyp, src = [], []
    for seg in tr["segments"]:
        text = unicodedata.normalize("NFKC", "".join(w["word"] for w in seg["words"]) or seg["text"])
        toks = M.read_tokens(text)
        ms, tags = M.tagged_moras([(k, t) for t, (_, k, _) in enumerate(toks)])
        for m, t in zip(ms, tags):
            start = toks[t][0]
            end = toks[t + 1][0] if t + 1 < len(toks) else len(text)
            hyp.append(m)
            src.append(kanji(text[start:end]))
    _, _, _, _, ops = score.align(ref, hyp)
    j = 0
    for op in ops:
        if op in "=S":
            tot[src[j]][0] += op == "S"
            tot[src[j]][1] += 1
        if op != "D":
            j += 1
for k, name in ((True, "read from kanji"), (False, "written in kana")):
    s, n = tot[k]
    print(f"{name}: {n} aligned moras, {s / max(1, n):.1%} substituted")
