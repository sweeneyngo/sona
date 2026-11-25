"""utau-edit-cost 17: the lyric arm's audio-only inputs for a split, computed before its read.

The lyric arm (lyrics-first 14's line fit + 17's S1) forced-aligns a mora sequence to the lead stem with three
CTC models. The expensive part, each model's per-frame emissions, depends on the audio alone; the alignment
through the moras is a cheap Viterbi on top (`line-alignment/ctc_align.py` reuses cached emissions). So this
computes, per song of the split, from `lead.wav` only (no UST, no lyric, no sona output, no score):
  WORK/emis/<model>-lead/<id>.npy (+ .json: the blank token), for the models `lyrics`, `hira` and `jg` (S1's
  three; `lyrics` also places 14's lines), exactly as `ctc_align.py` caches them;
  WORK/rms/<id>.json, the lead's loudness for 14's `energy` line ends, exactly as `lead_rms.py` writes it.

Usage: L14_WORK=<work dir> SONA_STEMS=<stems> <line-align venv python> -I audio_inputs.py --split S [--threads 4]
       [--models lyrics,hira,jg] [ids…]
The work dir holds audio-derived data of a held-out split: keep it local (e.g. ~/.cache/sona/holdout4/l14).
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import json, os, time  # noqa: E402

import numpy as np  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "line-alignment"))
sys.path.insert(0, HERE)
import ctc_align as C  # noqa: E402
import labels as LB  # noqa: E402

HOP = 0.02


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def rms(sid):
    """lead_rms.py's frames, for one song."""
    import soundfile as sf
    x, sr = sf.read(os.path.join(C.STEMS, sid, "lead.wav"), dtype="float32", always_2d=True)
    x = x.mean(axis=1)
    hop = int(round(HOP * sr))
    n = len(x) // hop
    frames = x[: n * hop].reshape(n, hop)
    db = 20 * np.log10(np.sqrt((frames ** 2).mean(axis=1) + 1e-12))
    db -= np.percentile(db, 95)
    return {"hop_s": HOP, "db": [round(float(v), 1) for v in db]}


def main():
    split, threads = arg("--split"), int(arg("--threads", "4"))
    models = arg("--models", "lyrics,hira,jg").split(",")
    flags = {"--split", "--threads", "--models"}
    ids = [a for i, a in enumerate(sys.argv[1:], 1) if a not in flags and sys.argv[i - 1] not in flags] or [s["id"] for s in LB.songs_of(split)]
    os.makedirs(os.path.join(C.WORK, "rms"), exist_ok=True)
    for sid in ids:
        p = os.path.join(C.WORK, "rms", sid + ".json")
        if not os.path.exists(p):
            json.dump(rms(sid), open(p, "w"))
    print(f"rms: {len(ids)} songs", flush=True)
    for m in models:
        d = os.path.join(C.WORK, "emis", f"{m}-lead")
        os.makedirs(d, exist_ok=True)
        for sid in ids:
            epath = os.path.join(d, sid + ".npy")
            if os.path.exists(epath + ".json"):
                continue
            t = time.time()
            lp, blank = C.emissions(m, C.load_audio(os.path.join(C.STEMS, sid, "lead.wav")), threads)
            np.save(epath, lp.astype(np.float16))
            json.dump({"blank": blank}, open(epath + ".json", "w"))
            print(f"{m} {sid}: {time.time() - t:.0f} s", flush=True)


if __name__ == "__main__":
    main()
