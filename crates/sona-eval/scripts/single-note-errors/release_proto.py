"""Prototype: end a note where the vocals stop being voiced (a release before a breath).

For each output note, find the last voiced frame of the vocals' pitch track inside it (a voiced run of at
least MINRUN frames counts; shorter blips are ignored). If the note's unvoiced tail is at least U cells,
the note ends at that frame rounded to the grid (at least one cell long). Optionally only notes the next
output note follows directly ("legato" in the output).

usage: S12_DUMP=DIR python3 -I release_proto.py [sweep|loso|song]
"""
import bisect, itertools, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L


def voiced_runs(song, minrun):
    """Voiced frames as (start time, end time) runs of at least `minrun` frames."""
    runs, cur = [], None
    f0 = song["f0"]
    for i, (t, hz) in enumerate(f0):
        if hz > 0:
            if cur is None:
                cur = [t, t, 0]
            cur[1] = t
            cur[2] += 1
        elif cur is not None:
            if cur[2] >= minrun:
                runs.append((cur[0], cur[1]))
            cur = None
    if cur is not None and cur[2] >= minrun:
        runs.append((cur[0], cur[1]))
    return runs


def trim(song, notes, anchor, u=1.0, minrun=3, legato_only=False, pad=0.0):
    sp = song["spacing"]
    runs = voiced_runs(song, minrun)
    ends = [r[1] for r in runs]
    out = []
    for i, (on, off, p) in enumerate(notes):
        nxt = notes[i + 1][0] if i + 1 < len(notes) else None
        if legato_only and (nxt is None or abs(nxt - off) > 1e-6):
            out.append((on, off, p))
            continue
        # last voiced run that starts before the note's end
        j = bisect.bisect_left([r[0] for r in runs], off) - 1
        last = None
        while j >= 0:
            a, b = runs[j]
            if b < on:
                break
            last = min(b, off)
            break
        if last is None:
            out.append((on, off, p))
            continue
        tail = (off - last) / sp
        if tail >= u:
            k = round((last + pad * sp - anchor) / sp)
            new_off = max(anchor + k * sp, on + sp)
            out.append((on, min(off, new_off), p))
        else:
            out.append((on, off, p))
    return out


def run(songs, out, **kw):
    return [L.analyse(s, trim(s, [tuple(n) for n in s["outputs"][out]], s["onset_phase"], **kw)) for s in songs]


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "sweep"
    songs = L.load()
    base = {o: [L.analyse(s, s["outputs"][o]) for s in songs] for o in ("lead", "stage")}
    grid = list(itertools.product([0.5, 1.0, 1.5, 2.0], [1, 3, 6], [False, True], [0.0, 0.25, 0.5]))
    if mode == "sweep":
        print(f"{'u':>4} {'run':>3} {'leg':>3} {'pad':>4}   lead Δ [95%]            clean Δ    stage Δ [95%]           clean Δ")
        for u, mr, leg, pad in grid:
            line = f"{u:4} {mr:3} {int(leg):3} {pad:4}  "
            for o in ("lead", "stage"):
                new = run(songs, o, u=u, minrun=mr, legato_only=leg, pad=pad)
                d, lo, hi = L.boot_delta(base[o], new, iters=2000)
                dc = L.clean(L.pool(new)) - L.clean(L.pool(base[o]))
                line += f" {d:+6.1f} [{lo:+5.1f}, {hi:+5.1f}] {dc:+5.1f}  "
            print(line, flush=True)
    elif mode == "loso":
        # choose (u, minrun, legato, pad) on 8 songs by GAME lead cost, score the 9th
        cache = {}
        for g in grid:
            cache[g] = run(songs, "lead", u=g[0], minrun=g[1], legato_only=g[2], pad=g[3])
        held = []
        for k in range(len(songs)):
            rest = [i for i in range(len(songs)) if i != k]
            best = min(grid, key=lambda g: L.per_min(L.pool([cache[g][i] for i in rest])))
            held.append(cache[best][k])
            print(f"  fold {songs[k]['id']:28} chose u {best[0]} run {best[1]} legato {best[2]} pad {best[3]}")
        d, lo, hi = L.boot_delta(base["lead"], held)
        print(f"lead, design chosen leave-one-song-out: {d:+.1f} [{lo:+.1f}, {hi:+.1f}]; clean "
              f"{L.clean(L.pool(base['lead'])):.1f} → {L.clean(L.pool(held)):.1f}")


if __name__ == "__main__":
    main()
