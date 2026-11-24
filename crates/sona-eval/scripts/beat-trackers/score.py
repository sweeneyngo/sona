"""Score the trackers' JSON (run.py) against the UST BPM and the manifest's music frame (ticket 15's rule).

Usage:
    python -I score.py OUT_DIR SONA_TEMPO.txt SONA_BAR_ONE.txt [CANDIDATE ...]

SONA_TEMPO.txt is `sona-eval-tempo`'s output, SONA_BAR_ONE.txt `sona-eval-frame --estimate-bar-one`'s; both give
sona's rows of the same table. Dev songs only (the manifest's `split == "dev"`).
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
MANIFEST = REPO / "crates/sona-eval/corpus/ust-manifest.json"
CORROBORATED = {"innocent-addict", "kamaitachi-ni-yoru", "telescope-nevermore", "azemichi-ni-ochita-ginga", "my-axiom"}
BEATS_PER_BAR = 4
ITERS = 10_000
MASK = (1 << 64) - 1


def song_bootstrap(scores):
    """`sona_eval::frame::song_bootstrap`, bit for bit: xorshift64 from the same seed, 95% of the summed resample."""
    state = 0x9E3779B97F4A7C15
    n = len(scores)
    draws = []
    for _ in range(ITERS):
        total = 0.0
        for _ in range(n):
            state ^= (state << 13) & MASK
            state ^= state >> 7
            state ^= (state << 17) & MASK
            total += scores[state % n]
        draws.append(total)
    draws.sort()
    q = lambda p: draws[round((len(draws) - 1) * p)]
    return q(0.025), q(0.975)


def multiple(est, truth):
    if est is None:
        return "none"
    near = lambda a: abs(a - truth) / truth <= 0.02
    return "1x" if near(est) else "2x" if near(est / 2) else "1/2x" if near(est * 2) else "other"


def fitted_bpm(beats):
    """Diagnosis only: 60 / the mean of the intervals within 10% of the median (the median of a 50 fps tracker's
    intervals is quantised to 20 ms, i.e. about 2% at 200 BPM)."""
    if len(beats) < 8:
        return None
    d = [b - a for a, b in zip(beats, beats[1:])]
    med = sorted(d)[len(d) // 2]
    near = [x for x in d if abs(x - med) <= 0.1 * med]
    return 60.0 * len(near) / sum(near)


def songs():
    out = []
    for s in json.loads(MANIFEST.read_text())["songs"]:
        if s.get("split") != "dev" or not s.get("bpm_from_ust"):
            continue
        div = s.get("grid_division") or 16
        bpm = s["bpm_from_ust"]
        beat = 60.0 / bpm
        cell = beat * 4 / div
        frame = None
        if s.get("alignment_status", "").startswith("aligned") and s.get("music_bar_shift_cells") is not None:
            off = s["alignment_offset_s"]
            frame = {
                "beat_line": off + s["music_beat_shift_cells"] * cell,
                "bar_line": off + s["music_bar_shift_cells"] * cell,
                "ust_tick0": off,
                "beat_shift": s["music_beat_shift_cells"],
            }
        out.append({"id": s["id"], "bpm": bpm, "beat": beat, "cell": cell, "cpb": div // 4, "frame": frame})
    return out


def fold_beat(beats, song):
    """The cell of the beat (after the UST's beat lines) most of the tracker's beats fall on, its vote share,
    and the median residual in ms of the beats on that cell against the music's beat."""
    if not beats:
        return None
    f, cell, cpb = song["frame"], song["cell"], song["cpb"]
    k = [round((t - f["ust_tick0"]) / cell) for t in beats]
    votes = Counter(x % cpb for x in k)
    best, n = votes.most_common(1)[0]
    res = sorted(((t - f["beat_line"] + song["beat"] / 2) % song["beat"] - song["beat"] / 2) * 1e3 for t, x in zip(beats, k) if x % cpb == best)
    return {"cell": best, "share": n / len(beats), "residual_ms": res[len(res) // 2] if best == f["beat_shift"] else None}


def fold_bar(downbeats, song):
    """Whole beats (mod 4) the tracker's downbeats sit after the music's bar line, by vote, and the vote share."""
    if not downbeats:
        return None
    f = song["frame"]
    votes = Counter(round((t - f["bar_line"]) / song["beat"]) % BEATS_PER_BAR for t in downbeats)
    best, n = votes.most_common(1)[0]
    return {"beats": best, "share": n / len(downbeats)}


def sona_rows(tempo_txt, bar_txt):
    bpm = {}
    for line in Path(tempo_txt).read_text().splitlines():
        m = re.match(r"(\S+)\s+dev\s+UST\s+([\d.]+)\s+est\s+([\d.]+|NaN)", line)
        if m:
            bpm[m[1]] = None if m[3] == "NaN" else float(m[3])
    bar = {}
    for line in Path(bar_txt).read_text().splitlines():
        parts = line.split()
        if len(parts) == 13 and re.match(r"^\d+[+-]", parts[5]):
            whole = lambda c: int(re.match(r"^(\d+)", c)[1])
            bar[parts[0]] = {"today": whole(parts[5]), "harmonic": whole(parts[10]), "est_beat": float(parts[4])}
    return bpm, bar


def main():
    out = Path(sys.argv[1])
    sona_bpm, sona_bar = sona_rows(sys.argv[2], sys.argv[3])
    cands = sys.argv[4:] or sorted(p.name for p in out.iterdir() if p.is_dir())
    ss = songs()
    framed = [s for s in ss if s["frame"]]
    summary = {}
    today = [1.0 if sona_bar[s["id"]]["today"] == 0 else 0.0 for s in framed]
    harm = [1.0 if sona_bar[s["id"]]["harmonic"] == 0 else 0.0 for s in framed]
    print(f"sona today: bar-1 hits {int(sum(today))}/{len(framed)} {song_bootstrap(today)}; harmonic (ADR-0031) {int(sum(harm))}/{len(framed)}")
    sona_mult = {s["id"]: multiple(sona_bpm.get(s["id"]), s["bpm"]) for s in ss}
    summary["sona"] = {"mult": sona_mult}
    for c in cands:
        print(f"\n## {c}")
        rows = {}
        for s in ss:
            p = out / c / f"{s['id']}.json"
            if not p.exists():
                continue
            rows[s["id"]] = json.loads(p.read_text())
        if not rows:
            continue
        mult = {s["id"]: multiple(rows[s["id"]]["bpm"], s["bpm"]) for s in ss if s["id"] in rows}
        beat_hits, bar_hits, bar_hits_c, cpu, wall, res_ms = [], [], [], [], [], []
        print(f"{'song':28} {'UST':>5} {'bpm':>7} {'fit':>7} {'mult':>5} | {'beat cell':>9} {'ref':>3} {'share':>5} {'res ms':>6} | {'bar':>3} {'share':>5} {'today':>5} {'harm':>4} | {'cpu s':>6} {'wall s':>6}")
        for s in ss:
            r = rows.get(s["id"])
            if r is None:
                continue
            bpm = r["bpm"]
            fit = fitted_bpm(r["beats"])
            line = f"{s['id']:28} {s['bpm']:5.0f} {bpm if bpm is not None else float('nan'):7.2f} {fit if fit else float('nan'):7.2f} {mult[s['id']]:>5} | "
            if s["frame"]:
                fb = fold_beat(r["beats"], s)
                fd = fold_bar(r["downbeats"], s) if r.get("downbeats") is not None else None
                if fb:
                    hit = fb["cell"] == s["frame"]["beat_shift"]
                    beat_hits.append(1.0 if hit else 0.0)
                    if fb["residual_ms"] is not None:
                        res_ms.append(fb["residual_ms"])
                    line += f"{fb['cell']:>9} {s['frame']['beat_shift']:>3} {fb['share']:5.2f} {fb['residual_ms'] if fb['residual_ms'] is not None else float('nan'):6.1f} | "
                else:
                    line += f"{'-':>9} {s['frame']['beat_shift']:>3} {'-':>5} {'-':>6} | "
                sb = sona_bar[s["id"]]
                if fd:
                    bar_hits.append(1.0 if fd["beats"] == 0 else 0.0)
                    if s["id"] in CORROBORATED:
                        bar_hits_c.append(1.0 if fd["beats"] == 0 else 0.0)
                    line += f"{fd['beats']:>3} {fd['share']:5.2f} {sb['today']:>5} {sb['harmonic']:>4} | "
                else:
                    line += f"{'-':>3} {'-':>5} {sb['today']:>5} {sb['harmonic']:>4} | "
            else:
                line += f"{'(no music frame)':>36} | "
            cpu.append(r["cpu_s"])
            wall.append(r["wall_s"])
            print(line + f"{r['cpu_s']:6.1f} {r['wall_s']:6.1f}")
        cnt = Counter(mult.values())
        offered = sum(1 for m in mult.values() if m in ("1x", "2x"))
        sona_offered_missed = [k for k, m in mult.items() if sona_mult[k] in ("1x", "2x") and m not in ("1x", "2x")]
        print(f"BPM ({len(mult)} songs): " + ", ".join(f"{k} {v}" for k, v in sorted(cnt.items())) + f"; truth offered {offered}; offered by sona but not here: {sona_offered_missed}")
        if beat_hits:
            print(f"beat hits {int(sum(beat_hits))}/{len(beat_hits)}; median |residual| {sorted(map(abs, res_ms))[len(res_ms)//2] if res_ms else float('nan'):.1f} ms")
        if bar_hits:
            d = [b - t for b, t in zip(bar_hits, today)]
            dh = [b - h for b, h in zip(bar_hits, harm)]
            print(f"bar-1 hits {int(sum(bar_hits))}/{len(bar_hits)} {song_bootstrap(bar_hits)}; corroborated {int(sum(bar_hits_c))}/{len(bar_hits_c)}; vs today {sum(d):+.0f} {song_bootstrap(d)}; vs harmonic {sum(dh):+.0f}")
        cpu_sorted, wall_sorted = sorted(cpu), sorted(wall)
        print(f"cpu s/song median {cpu_sorted[len(cpu)//2]:.1f} (max {cpu_sorted[-1]:.1f}); wall median {wall_sorted[len(wall)//2]:.1f} (max {wall_sorted[-1]:.1f})")


if __name__ == "__main__":
    main()
