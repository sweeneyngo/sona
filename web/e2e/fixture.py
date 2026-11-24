"""The drivers' default material: a synthetic annotation and the stem it describes, written on first use.

Eight two-bar phrases at 125 BPM in C major: four short legato notes that glide into each other, then a held
note with vibrato and a falling tail before a rest. The f0 contour is the line the stem sings; the stem is a
three-harmonic tone that follows it. Nothing here comes from a recording."""
import array, hashlib, json, math, pathlib, wave

BPM = 125
SR = 22050
HOP = 0.01
BEAT = 60 / BPM
START = 2 * BEAT
PHRASE = [(0, 0.5, 2), (0.5, 0.5, 3), (1, 1, 4), (2, 0.5, 3), (2.5, 0.5, 2), (3, 3.5, 1)]
SHIFTS = [0, 2, 1, 3, 0, 2, 4, 1]
SCALE = [60, 62, 64, 65, 67, 69, 71, 72, 74, 76]
DURATION = START + len(SHIFTS) * 8 * BEAT + 2 * BEAT


def notes():
    out = []
    for k, shift in enumerate(SHIFTS):
        t0 = START + k * 8 * BEAT
        for beat, length, degree in PHRASE:
            onset = round(t0 + beat * BEAT, 6)
            out.append({"onset_s": onset, "offset_s": round(onset + length * BEAT, 6), "midi_pitch": SCALE[degree + shift]})
    return out


def cents_at(ns, t):
    for i, n in enumerate(ns):
        if not n["onset_s"] <= t < n["offset_s"]:
            continue
        on, off, c = n["onset_s"], n["offset_s"], n["midi_pitch"] * 100
        prev = ns[i - 1] if i and abs(ns[i - 1]["offset_s"] - on) < 1e-6 else None
        lead = (prev["midi_pitch"] * 100 - c) if prev else -80
        c += lead * math.exp(-(t - on) / (0.03 if prev else 0.04))
        if off - on >= 1 and t - on > 0.35:
            c += 35 * min(1, (t - on - 0.35) / 0.2) * math.sin(2 * math.pi * 5.5 * (t - on - 0.35))
        nxt = ns[i + 1] if i + 1 < len(ns) else None
        if (not nxt or nxt["onset_s"] - off > 1e-6) and off - t < 0.1:
            c -= 120 * ((t - (off - 0.1)) / 0.1) ** 2
        return c + 3 * math.sin(2 * math.pi * 1.3 * t)
    return None


def build(ann_path, wav_path):
    ns = notes()
    sub = {"pitch_stability": 0.93, "voicing": 0.95, "onset_clarity": 0.9, "offset_clarity": 0.9}
    contour, energy = [], []
    for i in range(int(DURATION / HOP)):
        t = round(i * HOP, 6)
        c = cents_at(ns, t)
        contour.append({"time_s": t, "frequency_hz": 0.0 if c is None else round(440 * 2 ** ((c - 6900) / 1200), 3), "voicing": 0.0 if c is None else 0.95})
        energy.append({"time_s": t, "rms": 0.001 if c is None else 0.12})
    ann = {"version": 1, "sample_rate": SR, "bpm": BPM, "duration_s": round(DURATION, 6), "producer": "pyin",
           "notes": [{**n, "confidence": 0.92, "sub_scores": sub} for n in ns], "f0_contour": contour, "energy_envelope": energy}
    ann_path.write_text(json.dumps(ann))
    pcm, phase, amp = array.array("h"), 0.0, 0.0
    for s in range(int(DURATION * SR)):
        f = contour[min(len(contour) - 1, int(s / SR / HOP))]["frequency_hz"]
        amp += ((0.25 if f else 0.0) - amp) * 0.005
        if f:
            phase += 2 * math.pi * f / SR
        pcm.append(int(32767 * amp * (math.sin(phase) + 0.5 * math.sin(2 * phase) + 0.25 * math.sin(3 * phase)) / 1.75))
    part = wav_path.with_suffix(".part")
    with wave.open(str(part), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(SR); w.writeframes(pcm.tobytes())
    part.replace(wav_path)


def synthetic(out_dir):
    """The fixture's annotation and stem paths under `out_dir`, built once per version of this file."""
    key = hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()[:12]
    d = pathlib.Path(out_dir) / f"fixture-{key}"
    ann, wav = d / "synthetic.annotation.json", d / "synthetic.stem.wav"
    if not (ann.exists() and wav.exists()):
        d.mkdir(parents=True, exist_ok=True)
        build(ann, wav)
    return ann, wav
