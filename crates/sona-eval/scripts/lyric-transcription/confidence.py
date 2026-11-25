"""Would highlighting a transcript's doubtful words save the user a read-through? (lyrics-first 05)
Each hypothesis mora takes the probability of the timestamped word holding its first character; the
table gives, per probability band, the share of the transcript's moras in that band and the share of
those moras that are wrong (substituted or inserted). Deleted moras have no word and are not counted.
Numbers only.

    python confidence.py TRANSCRIPT_DIR REF_DIR"""
import glob
import json
import os
import sys
import unicodedata

import moras as M
import score

BANDS = (0.0, 0.3, 0.5, 0.7, 0.9, 1.01)
tr_dir, ref_dir = sys.argv[1], sys.argv[2]
cnt = [[0, 0] for _ in BANDS[:-1]]
for tp in sorted(glob.glob(os.path.join(tr_dir, "*.json"))):
    tr = json.load(open(tp))
    ref = [m[0] for m in json.load(open(os.path.join(ref_dir, tr["id"] + ".json")))["moras"]]
    hyp, prob = [], []
    for seg in tr["segments"]:
        text, owner = "", []
        for w in seg["words"]:
            piece = unicodedata.normalize("NFKC", w["word"])
            text += piece
            owner += [w["p"]] * len(piece)
        toks = M.read_tokens(text)
        ms, tags = M.tagged_moras([(k, t) for t, (_, k, _) in enumerate(toks)])
        for m, t in zip(ms, tags):
            hyp.append(m)
            prob.append(owner[toks[t][0]] if toks[t][0] < len(owner) else None)
    _, _, _, _, ops = score.align(ref, hyp)
    j = 0
    for op in ops:
        if op == "D":
            continue
        p = prob[j]
        if p is not None:
            b = next(k for k in range(len(BANDS) - 1) if BANDS[k] <= p < BANDS[k + 1])
            cnt[b][0] += op in "SI"
            cnt[b][1] += 1
        j += 1
total = sum(n for _, n in cnt)
print("| word probability | share of moras | wrong |")
print("|---|---|---|")
for k, (bad, n) in enumerate(cnt):
    print(f"| {BANDS[k]:.1f}–{min(BANDS[k + 1], 1.0):.1f} | {n / max(1, total):.0%} | {bad / max(1, n):.0%} |")
