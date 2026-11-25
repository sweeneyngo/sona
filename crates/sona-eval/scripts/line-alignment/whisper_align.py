"""Lyrics-first 14: Whisper-family alignment of each dev song's pasted moras (stable-ts `align`: the known text
is forced through the decoder and word timestamps come from cross-attention), on a stem.

The text is the song's lines, one per line, in hiragana. stable-ts returns timed tokens; each mora takes the
token holding its first character (a token's span is shared out evenly by character when it holds several
moras). The score is the token's probability, as a log.

Writes $L14_WORK/align/<name>/<id>.json like ctc_align.py. Lyric-derived: the scratchpad only.

Usage: L14_WORK=… SONA_STEMS=… python -I whisper_align.py --model PATH_OR_NAME --name NAME [--stem lead] [ids…]
"""
import argparse, json, math, os, time

import numpy as np

WORK = os.environ["L14_WORK"]
STEMS = os.environ["SONA_STEMS"]


def load_audio(path):
    import soundfile as sf, soxr
    x, sr = sf.read(path, dtype="float32", always_2d=True)
    x = x.mean(axis=1)
    return soxr.resample(x, sr, 16000).astype(np.float32) if sr != 16000 else x


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--name", required=True)
    ap.add_argument("--stem", default="lead")
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument("ids", nargs="*")
    a = ap.parse_args()
    import stable_whisper
    model = stable_whisper.load_faster_whisper(a.model, device="cpu", compute_type="int8", cpu_threads=a.threads)
    ids = a.ids or sorted(f[:-5] for f in os.listdir(os.path.join(WORK, "text")))
    os.makedirs(os.path.join(WORK, "align", a.name), exist_ok=True)
    for sid in ids:
        t0 = time.time()
        text = json.load(open(os.path.join(WORK, "text", sid + ".json")))
        moras = text["moras"]
        firsts = text["lines"] + [len(moras)]
        lines = ["".join(m["kana"] for m in moras[x:y]) for x, y in zip(firsts, firsts[1:])]
        owner = [m["i"] for m in moras for _ in m["kana"]]
        audio = load_audio(os.path.join(STEMS, sid, a.stem + ".wav"))
        res = model.align(audio, "\n".join(lines), language="ja", original_split=True)
        words = res.all_words()
        at = [None] * len(moras)
        pos = 0
        for w in words:
            chars = [c for c in w.word if not c.isspace()]
            if not chars:
                continue
            n = len(chars)
            lp = math.log(max(getattr(w, "probability", 1.0) or 1e-6, 1e-6))
            for q in range(n):
                if pos >= len(owner):
                    break
                mi = owner[pos]
                if at[mi] is None:
                    st = w.start + (w.end - w.start) * q / n
                    at[mi] = [st, w.start + (w.end - w.start) * (q + 1) / n, lp]
                else:
                    at[mi][1] = w.start + (w.end - w.start) * (q + 1) / n
                pos += 1
        last = 0.0
        for k in range(len(at)):
            if at[k] is None:
                at[k] = [last, last, -10.0]
            last = at[k][1]
        json.dump({"model": a.model, "stem": a.stem, "moras": at, "chars_matched": pos, "chars": len(owner)}, open(os.path.join(WORK, "align", a.name, sid + ".json"), "w"))
        print(f"{sid:28} {time.time() - t0:6.1f}s chars {pos}/{len(owner)}", flush=True)


if __name__ == "__main__":
    main()
