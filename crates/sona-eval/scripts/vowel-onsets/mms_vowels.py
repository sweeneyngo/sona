"""Lyrics-first 17: the vowel's own onset from 14's romanised MMS alignment.

14 aligned each mora's Hepburn letters (`ka` → k, a) and kept the mora's first frame. This re-runs the same
Viterbi on the cached emissions and keeps, per mora, the first frame of its last letter: the vowel (or `n`).
Writes $L17_WORK/onsets/mms-vowel/<id>.json ({"on": […]}) and mms-first (the first letter, 14's onset, as a
check). Lyric-derived: the scratchpad only.

Usage: L14_WORK=… L17_WORK=… SONA_STEMS=… python -I mms_vowels.py
"""
import json, os, sys

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "line-alignment"))
import ctc_align as C  # noqa: E402

W14, W17 = os.environ["L14_WORK"], os.environ["L17_WORK"]


def main():
    vocab = json.load(open(os.path.join(C.local_path(C.MODELS["mms"][0]), "vocab.json")))
    if isinstance(next(iter(vocab.values())), dict):
        vocab = next(iter(vocab.values()))
    filler = vocab.get("|")
    for name in ("mms-vowel", "mms-first"):
        os.makedirs(os.path.join(W17, "onsets", name), exist_ok=True)
    for f in sorted(os.listdir(os.path.join(W14, "text"))):
        sid = f[:-5]
        text = json.load(open(os.path.join(W14, "text", f)))
        epath = os.path.join(W14, "emis", "mms-lead", sid + ".npy")
        lp = np.load(epath).astype(np.float32)
        blank = json.load(open(epath + ".json"))["blank"]
        lp[lp.max(axis=1) < -1e3, blank] = 0.0
        pieces = set(text["pieces"])
        tokens, owner, gap_filler = [], [], []
        for m in text["moras"]:
            ts = C.mora_tokens("mms", m["kana"], vocab) or [vocab.get("<unk>", vocab.get("[UNK]"))]
            for q, tk in enumerate(ts):
                tokens.append(tk)
                owner.append(m["i"])
                gap_filler.append(q == 0 and m["i"] in pieces)
        gap_filler.append(True)
        first, _last = C.viterbi(lp, tokens, owner, gap_filler, blank, filler)
        n = len(text["moras"])
        vowel, head = [None] * n, [None] * n
        for k, mi in enumerate(owner):
            vowel[mi] = float(first[k] * C.FRAME_S)
            if head[mi] is None:
                head[mi] = float(first[k] * C.FRAME_S)
        json.dump({"on": vowel}, open(os.path.join(W17, "onsets", "mms-vowel", f), "w"))
        json.dump({"on": head}, open(os.path.join(W17, "onsets", "mms-first", f), "w"))
        print(sid, n, flush=True)


if __name__ == "__main__":
    main()
