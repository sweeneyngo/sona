"""Share of the lead UST's moras whose note onset falls inside the voice-activity spans the recognisers
are fed, for a few VAD thresholds (lyrics-first 05). A mora outside every span can only be deleted.

    python vad_cover.py REF_DIR STEMS_DIR THRESHOLD..."""
import glob
import json
import os
import sys

from faster_whisper import decode_audio
from faster_whisper.vad import VadOptions, get_speech_timestamps

ref_dir, stems, ths = sys.argv[1], sys.argv[2], [float(t) for t in sys.argv[3:]]
tot = {t: [0, 0] for t in ths}
for rp in sorted(glob.glob(os.path.join(ref_dir, "*.json"))):
    ref = json.load(open(rp))
    audio = decode_audio(os.path.join(stems, ref["id"], "lead.wav"), sampling_rate=16000)
    row = [ref["id"]]
    for t in ths:
        spans = get_speech_timestamps(audio, VadOptions(threshold=t))
        spans = [(s["start"] / 16000, s["end"] / 16000) for s in spans]
        inside = sum(any(a - 0.2 <= m[1] <= b + 0.2 for a, b in spans) for m in ref["moras"])
        tot[t][0] += inside
        tot[t][1] += len(ref["moras"])
        row.append(f"{t}: {inside / len(ref['moras']):.0%} ({sum(b - a for a, b in spans):.0f}s)")
    print(*row, flush=True)
print("pooled", *(f"{t}: {a / b:.1%}" for t, (a, b) in tot.items()))
