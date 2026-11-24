"""Beat This! (CPJKU, ISMIR 2024; code and weights MIT): the beats and downbeats of one audio file, for sona's
bar 1 (ADR-0036).

`scripts/setup-separators.sh beat-this` installs this file into the lead separator's venv as
`bin/sona-beat-this`. `sona serve` and `sona transcribe --bar-one-tracker beat-this` run it as a separate
process on the full mix, under a deadline.

Usage: sona-beat-this [--threads N] [--checkpoint final0] --out OUT.json AUDIO

OUT.json: {"beats": [s], "downbeats": [s], "bpm", "checkpoint", "checkpoint_path", "beat_this", "torch",
"threads", "cpu_s", "wall_s", "load"}. Times are seconds from the start of AUDIO; cpu_s and wall_s include
loading the model; load is the 1-minute load average when it started. The defaults are ticket 15's
measured configuration: the final0 checkpoint on the mono mix, no DBN, CPU, seed 0.
"""
import argparse
import json
import os
import sys
import time


def main():
    p = argparse.ArgumentParser(prog="sona-beat-this", description="Beat This! beats and downbeats as JSON")
    p.add_argument("--threads", type=int, default=1, help="CPU threads for torch (default 1)")
    p.add_argument("--checkpoint", default="final0", help="Beat This checkpoint name (default final0)")
    p.add_argument("--out", required=True, help="where to write the JSON")
    p.add_argument("audio")
    a = p.parse_args()
    if a.threads < 1:
        p.error("--threads must be at least 1")
    for var in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"):
        os.environ[var] = str(a.threads)
    load = os.getloadavg()[0]
    c0, w0 = time.process_time(), time.perf_counter()

    import importlib.metadata

    import numpy as np
    import soundfile as sf
    import torch

    torch.set_num_threads(a.threads)
    torch.manual_seed(0)
    from beat_this.inference import Audio2Beats

    ckpt = os.path.join(torch.hub.get_dir(), "checkpoints", f"beat_this-{a.checkpoint}.ckpt")
    # By its path once downloaded: given a bare name, Beat This first tries a file of that name in the working directory.
    a2b = Audio2Beats(checkpoint_path=ckpt if os.path.isfile(ckpt) else a.checkpoint, device="cpu", dbn=False)
    y, sr = sf.read(a.audio, dtype="float32", always_2d=True)
    beats, downbeats = a2b(y.mean(axis=1), sr)
    beats = [float(t) for t in beats]
    intervals = np.diff(np.asarray(beats))
    result = {
        "beats": beats,
        "downbeats": [float(t) for t in downbeats],
        "bpm": float(60.0 / np.median(intervals)) if len(intervals) > 1 else None,
        "checkpoint": a.checkpoint,
        "checkpoint_path": ckpt if os.path.isfile(ckpt) else None,
        "beat_this": importlib.metadata.version("beat-this"),
        "torch": torch.__version__,
        "threads": a.threads,
        "cpu_s": time.process_time() - c0,
        "wall_s": time.perf_counter() - w0,
        "load": load,
    }
    tmp = a.out + ".part"
    with open(tmp, "w") as f:
        json.dump(result, f)
    os.replace(tmp, a.out)
    print(f"{len(beats)} beats, {len(result['downbeats'])} downbeats in {result['wall_s']:.1f} s", file=sys.stderr)


if __name__ == "__main__":
    main()
