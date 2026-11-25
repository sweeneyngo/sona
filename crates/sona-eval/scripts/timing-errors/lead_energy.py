"""The lead stem's RMS envelope, framed as `OnsetDetector::energy_envelope` frames the vocals (1024-sample
window, 256 hop, time at the window's centre), for each song in the dump. The vocals stem carries the backing
voices, so a lead singer's release can hide under a harmony there.

usage: S18_DUMP=DIR python3 -I lead_energy.py STEMS_DIR OUT_DIR   (OUT_DIR is audio-derived: keep it out of the repo)
"""
import json, os, sys, wave
import numpy as np

dump, stems, out = os.environ["S18_DUMP"], sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
for f in sorted(os.listdir(dump)):
    if not f.endswith(".json"):
        continue
    sid = f[:-5]
    w = wave.open(os.path.join(stems, sid, "lead.wav"))
    assert w.getsampwidth() == 2
    x = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float64) / 32768.0
    if w.getnchannels() > 1:
        x = x.reshape(-1, w.getnchannels()).mean(axis=1)
    sr = w.getframerate()
    n = (len(x) - 1024) // 256 + 1
    c = np.concatenate([[0.0], np.cumsum(x * x)])
    starts = np.arange(n) * 256
    rms = np.sqrt((c[starts + 1024] - c[starts]) / 1024)
    t = (starts + 512) / sr
    json.dump([[float(a), float(b)] for a, b in zip(t, rms)], open(os.path.join(out, f), "w"))
    print(sid, n)
