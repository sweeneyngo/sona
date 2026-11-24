"""Run one beat/tempo tracker on the dev songs' full mixes and write its beats, downbeats and tempo as JSON.

Usage (each candidate in its own venv, see reports/beat-trackers.md):
    VENV/bin/python -I run.py CANDIDATE OUT_DIR [SONG ...]

Only `split == "dev"` songs are ever opened; a holdout id on the command line is refused.
Output: OUT_DIR/CANDIDATE/SONG.json with bpm, beats, downbeats (seconds), cpu_s, wall_s, load and extras.
"""
import json
import os
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
MANIFEST = REPO / "crates/sona-eval/corpus/ust-manifest.json"
AUDIO = REPO / "crates/sona-eval/data/pifuyuu-audio"


def dev_songs():
    songs = json.loads(MANIFEST.read_text())["songs"]
    return [s["id"] for s in songs if s.get("split") == "dev" and s.get("bpm_from_ust")]


def median_ibi_bpm(beats):
    import numpy as np

    b = np.asarray(beats, dtype=float)
    return float(60.0 / np.median(np.diff(b))) if len(b) > 2 else None


def beat_this(path, variant):
    import soundfile as sf
    import torch
    from beat_this.inference import Audio2Beats

    torch.manual_seed(0)
    model = {"beat_this": "final0", "beat_this_small": "small0"}[variant]
    a2b = Audio2Beats(checkpoint_path=model, device="cpu", dbn=False)
    y, sr = sf.read(path, dtype="float32", always_2d=True)
    beats, downbeats = a2b(y.mean(axis=1), sr)
    return {"beats": list(map(float, beats)), "downbeats": list(map(float, downbeats)), "bpm": median_ibi_bpm(beats)}


def madmom_run(path, variant):
    from madmom.features.beats import DBNBeatTrackingProcessor, RNNBeatProcessor
    from madmom.features.downbeats import DBNDownBeatTrackingProcessor, RNNDownBeatProcessor
    from madmom.features.tempo import TempoEstimationProcessor

    bpb = [4] if variant == "madmom_4" else [3, 4]
    act_b = RNNBeatProcessor()(path)
    beats = DBNBeatTrackingProcessor(fps=100)(act_b)
    tempi = TempoEstimationProcessor(fps=100)(act_b)
    act_d = RNNDownBeatProcessor()(path)
    bd = DBNDownBeatTrackingProcessor(beats_per_bar=bpb, fps=100)(act_d)
    downbeats = [float(t) for t, k in bd if int(k) == 1]
    return {
        "beats": list(map(float, beats)),
        "downbeats": downbeats,
        "downbeat_tracker_beats": [float(t) for t, _ in bd],
        "bpm": median_ibi_bpm(beats),
        "tempo_estimator": [[float(t), float(s)] for t, s in tempi[:3]],
    }


def essentia_run(path, variant):
    import essentia.standard as es

    y = es.MonoLoader(filename=str(path), sampleRate=44100)()
    if variant == "essentia_percival":
        return {"bpm": float(es.PercivalBpmEstimator()(y)), "beats": [], "downbeats": None}
    if variant == "essentia_tempocnn":
        y11 = es.MonoLoader(filename=str(path), sampleRate=11025)()
        model = Path.home() / ".cache/sona/beat-trackers/models/deeptemp-k16-3.pb"
        g, local, prob = es.TempoCNN(graphFilename=str(model))(y11)
        return {"bpm": float(g), "beats": [], "downbeats": None, "local_bpm": list(map(float, local))}
    method = "degara" if variant == "essentia_degara" else "multifeature"
    bpm, beats, conf, estimates, intervals = es.RhythmExtractor2013(method=method)(y)
    return {
        "bpm": float(bpm),
        "beats": list(map(float, beats)),
        "downbeats": None,
        "confidence": float(conf),
        "ibi_bpm": median_ibi_bpm(beats),
    }


def librosa_run(path, variant):
    import librosa
    import numpy as np

    y, sr = librosa.load(path, sr=22050, mono=True)
    tempo, frames = librosa.beat.beat_track(y=y, sr=sr)
    beats = librosa.frames_to_time(frames, sr=sr)
    return {"bpm": float(np.atleast_1d(tempo)[0]), "beats": list(map(float, beats)), "downbeats": None}


RUNNERS = {
    "beat_this": beat_this,
    "beat_this_small": beat_this,
    "madmom": madmom_run,
    "madmom_4": madmom_run,
    "essentia_multifeature": essentia_run,
    "essentia_degara": essentia_run,
    "essentia_percival": essentia_run,
    "essentia_tempocnn": essentia_run,
    "librosa": librosa_run,
}


def main():
    cand, out = sys.argv[1], Path(sys.argv[2])
    allowed = dev_songs()
    songs = sys.argv[3:] or allowed
    for s in songs:
        if s not in allowed:
            sys.exit(f"{s}: not a dev song; refusing to open it")
    (out / cand).mkdir(parents=True, exist_ok=True)
    for s in songs:
        dst = out / cand / f"{s}.json"
        if dst.exists():
            continue
        load = os.getloadavg()[0]
        c0, w0 = time.process_time(), time.perf_counter()
        r = RUNNERS[cand](str(AUDIO / f"{s}.wav"), cand)
        r.update(cpu_s=time.process_time() - c0, wall_s=time.perf_counter() - w0, load=load, song=s, candidate=cand)
        dst.write_text(json.dumps(r))
        print(f"{cand} {s}: bpm {r['bpm']} beats {len(r['beats'])} cpu {r['cpu_s']:.1f}s wall {r['wall_s']:.1f}s", flush=True)


if __name__ == "__main__":
    main()
