"""Lyrics-first 14: the lead stem's loudness in 20 ms frames (dB relative to the song's 95th percentile), for
the `--end energy` rule in score.py: a line ends where the lead goes quiet after its last mora's onset.

Writes $L14_WORK/rms/<id>.json: {"hop_s": 0.02, "db": [...]}.
Usage: L14_WORK=… SONA_STEMS=… python -I lead_rms.py [--stem lead]
"""
import json, os, sys

import numpy as np
import soundfile as sf

WORK = os.environ["L14_WORK"]
STEMS = os.environ["SONA_STEMS"]
HOP = 0.02


def main():
    stem = sys.argv[sys.argv.index("--stem") + 1] if "--stem" in sys.argv else "lead"
    os.makedirs(os.path.join(WORK, "rms"), exist_ok=True)
    for f in sorted(os.listdir(os.path.join(WORK, "text"))):
        sid = f[:-5]
        x, sr = sf.read(os.path.join(STEMS, sid, stem + ".wav"), dtype="float32", always_2d=True)
        x = x.mean(axis=1)
        hop = int(round(HOP * sr))
        n = len(x) // hop
        frames = x[: n * hop].reshape(n, hop)
        rms = np.sqrt((frames ** 2).mean(axis=1) + 1e-12)
        db = 20 * np.log10(rms)
        db -= np.percentile(db, 95)
        json.dump({"hop_s": HOP, "db": [round(float(v), 1) for v in db]}, open(os.path.join(WORK, "rms", sid + ".json"), "w"))
        print(sid, n)


if __name__ == "__main__":
    main()
