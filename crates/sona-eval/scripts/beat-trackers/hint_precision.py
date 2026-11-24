"""Diagnosis (not in ticket 15's pre-registered rule): would a tracker's tempo make the start screen's whole-BPM hint
exact? The hint shows round(est) and round(est / 2) (ADR-0028). For each tracker, keep sona's octave and take the
tracker's tempo (60 / mean of the intervals within 10% of the median): est = tracker × 2^round(log2(sona / tracker)).

Usage: python -I hint_precision.py OUT_DIR SONA_TEMPO.txt SONA_BAR_ONE.txt [CANDIDATE ...]
"""
import importlib.util
import json
import math
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("score", Path(__file__).with_name("score.py"))
sc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sc)


def offered(est, truth):
    return truth in (round(est), round(est / 2))


def main():
    out = Path(sys.argv[1])
    sona, _ = sc.sona_rows(sys.argv[2], sys.argv[3])
    cands = sys.argv[4:] or sorted(p.name for p in out.iterdir() if p.is_dir())
    ss = sc.songs()
    print(f"sona alone: whole-BPM hint exact on {sum(offered(sona[s['id']], s['bpm']) for s in ss)}/{len(ss)}")
    for c in cands:
        hits, n, misses = 0, 0, []
        for s in ss:
            p = out / c / f"{s['id']}.json"
            if not p.exists():
                continue
            r = json.loads(p.read_text())
            t = sc.fitted_bpm(r["beats"]) if r["beats"] else r["bpm"]
            est = t * 2 ** round(math.log2(sona[s["id"]] / t))
            n += 1
            if offered(est, s["bpm"]):
                hits += 1
            else:
                misses.append(f"{s['id']} {est:.2f} (UST {s['bpm']:.0f})")
        print(f"sona octave x {c}: {hits}/{n}; misses: {', '.join(misses) or 'none'}")


if __name__ == "__main__":
    main()
