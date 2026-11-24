"""Prototype: end a note where the vocals' energy falls away (a release before a breath).

Each output note's cells get the vocals' mean RMS over the cell, relative to the note's median RMS over all
its cells. Walking back from the note's last cell, every cell under THETA is dropped; the run must be at
least MINRUN cells long and the note keeps at least one cell. Optionally only notes another note follows
directly (`legato`), or only runs that reach the next note (`gap`).

usage: S12_DUMP=DIR python3 -I release_energy.py [sweep|loso|song|diag]
"""
import bisect, itertools, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L


def cell_energy(rt, rv, a, b):
    k, l = bisect.bisect_left(rt, a), bisect.bisect_left(rt, b)
    return sum(rv[k:l]) / (l - k) if l > k else 0.0


def trim(song, notes, anchor, theta=0.4, minrun=2, legato_only=False, maxrun=99, ref="median"):
    sp = song["spacing"]
    rt = [x[0] for x in song["rms"]]
    rv = [x[1] for x in song["rms"]]
    out = []
    for i, (on, off, p) in enumerate(notes):
        n = round((off - on) / sp)
        nxt = notes[i + 1][0] if i + 1 < len(notes) else None
        if n < 2 or (legato_only and (nxt is None or abs(nxt - off) > 1e-6)):
            out.append((on, off, p))
            continue
        e = [cell_energy(rt, rv, on + c * sp, on + (c + 1) * sp) for c in range(n)]
        level = max(e) if ref == "max" else st.median(e)
        if level <= 0:
            out.append((on, off, p))
            continue
        k = n
        while k > 1 and e[k - 1] < theta * level:
            k -= 1
        run = n - k
        if minrun <= run <= maxrun:
            out.append((on, on + k * sp, p))
        else:
            out.append((on, off, p))
    return out


def trim_dip(song, notes, anchor, theta=0.3, theta_tail=0.6, minrun=2, ref="max"):
    """v2: the note ends at the earliest cell k ≥ 1 that is under THETA, provided every later cell is under
    THETA_TAIL (the next mora's consonant may lift the last cell) and at least MINRUN cells go."""
    sp = song["spacing"]
    rt = [x[0] for x in song["rms"]]
    rv = [x[1] for x in song["rms"]]
    out = []
    for on, off, p in notes:
        n = round((off - on) / sp)
        if n < 2:
            out.append((on, off, p))
            continue
        e = [cell_energy(rt, rv, on + c * sp, on + (c + 1) * sp) for c in range(n)]
        level = max(e) if ref == "max" else st.median(e)
        if level <= 0:
            out.append((on, off, p))
            continue
        # suffix maximum: the largest energy from cell c to the end
        suf = e[:]
        for c in range(n - 2, -1, -1):
            suf[c] = max(e[c], suf[c + 1])
        cut = None
        for k in range(1, n - minrun + 1):
            if e[k] < theta * level and (k + 1 >= n or suf[k + 1] < theta_tail * level):
                cut = k
                break
        out.append((on, on + cut * sp, p) if cut is not None else (on, off, p))
    return out


def run(songs, out, fn=None, **kw):
    fn = fn or trim
    return [L.analyse(s, fn(s, [tuple(n) for n in s["outputs"][out]], s["onset_phase"], **kw)) for s in songs]


GRID2 = list(itertools.product([0.2, 0.3, 0.4], [0.4, 0.6, 0.8, 1.01], [1, 2]))


GRID = list(itertools.product([0.2, 0.3, 0.4, 0.5, 0.6], [1, 2], [False], ["median", "max"]))


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "sweep"
    songs = L.load()
    base = {o: [L.analyse(s, s["outputs"][o]) for s in songs] for o in ("lead", "stage")}
    if mode == "sweep":
        print(f"{'theta':>5} {'run':>3} {'leg':>3} {'ref':>6}   lead Δ [95%]            clean Δ    stage Δ [95%]           clean Δ")
        for th, mr, leg, ref in GRID:
            line = f"{th:5} {mr:3} {int(leg):3} {ref:>6}  "
            for o in ("lead", "stage"):
                new = run(songs, o, theta=th, minrun=mr, legato_only=leg, ref=ref)
                d, lo, hi = L.boot_delta(base[o], new, iters=2000)
                dc = L.clean(L.pool(new)) - L.clean(L.pool(base[o]))
                line += f" {d:+6.1f} [{lo:+5.1f}, {hi:+5.1f}] {dc:+5.1f}  "
            print(line, flush=True)
    elif mode == "sweep2":
        print(f"{'theta':>5} {'tail':>5} {'run':>3}   lead Δ [95%]            clean Δ    stage Δ [95%]           clean Δ")
        for th, tt, mr in GRID2:
            if tt < th:
                continue
            line = f"{th:5} {tt:5} {mr:3}  "
            for o in ("lead", "stage"):
                new = run(songs, o, fn=trim_dip, theta=th, theta_tail=tt, minrun=mr)
                d, lo, hi = L.boot_delta(base[o], new, iters=2000)
                dc = L.clean(L.pool(new)) - L.clean(L.pool(base[o]))
                line += f" {d:+6.1f} [{lo:+5.1f}, {hi:+5.1f}] {dc:+5.1f}  "
            print(line, flush=True)
    elif mode == "loso2":
        grid = [g for g in itertools.product([0.2, 0.3, 0.4], [0.4, 0.6, 0.8, 1.01], [1, 2, 3]) if g[1] >= g[0]]
        cache = {g: run(songs, "lead", fn=trim_dip, theta=g[0], theta_tail=g[1], minrun=g[2]) for g in grid}
        held = []
        for k in range(len(songs)):
            rest = [i for i in range(len(songs)) if i != k]
            best = min(grid, key=lambda g: L.per_min(L.pool([cache[g][i] for i in rest])))
            held.append(cache[best][k])
            print(f"  fold {songs[k]['id']:28} chose theta {best[0]} tail {best[1]} run {best[2]}  "
                  f"{L.per_min(L.pool([base['lead'][k]])):6.1f} → {L.per_min(L.pool([held[-1]])):6.1f}")
        d, lo, hi = L.boot_delta(base["lead"], held)
        print(f"lead, dip rule chosen leave-one-song-out: {d:+.1f} [{lo:+.1f}, {hi:+.1f}]; clean "
              f"{L.clean(L.pool(base['lead'])):.1f} → {L.clean(L.pool(held)):.1f}")
    elif mode == "song":
        th, tt, mr = (float(x) for x in sys.argv[2:5])
        for o in ("lead", "stage"):
            new = run(songs, o, fn=trim_dip, theta=th, theta_tail=tt, minrun=int(mr))
            d, lo, hi = L.boot_delta(base[o], new)
            print(f"{o}: {L.per_min(L.pool(base[o])):.1f} → {L.per_min(L.pool(new)):.1f}  Δ {d:+.1f} [{lo:+.1f}, {hi:+.1f}]  "
                  f"clean {L.clean(L.pool(base[o])):.1f} → {L.clean(L.pool(new)):.1f} "
                  f"(Δ [95%] {' '.join(f'{x:+.1f}' for x in L.boot_delta(base[o], new, stat=L.clean))})")
            for s, b, nw in zip(songs, base[o], new):
                print(f"   {s['id']:28} {L.per_min(L.pool([b])):6.1f} → {L.per_min(L.pool([nw])):6.1f}  "
                      + " ".join(f"{k} {nw['counts'][k] - b['counts'][k]:+d}" for k in L.ec.CLASSES if nw['counts'][k] != b['counts'][k]))
    elif mode == "loso":
        cache = {g: run(songs, "lead", theta=g[0], minrun=g[1], legato_only=g[2], ref=g[3]) for g in GRID}
        held = []
        for k in range(len(songs)):
            rest = [i for i in range(len(songs)) if i != k]
            best = min(GRID, key=lambda g: L.per_min(L.pool([cache[g][i] for i in rest])))
            held.append(cache[best][k])
            print(f"  fold {songs[k]['id']:28} chose theta {best[0]} run {best[1]} legato {best[2]} ref {best[3]}")
        d, lo, hi = L.boot_delta(base["lead"], held)
        print(f"lead, design chosen leave-one-song-out: {d:+.1f} [{lo:+.1f}, {hi:+.1f}]; clean "
              f"{L.clean(L.pool(base['lead'])):.1f} → {L.clean(L.pool(held)):.1f}")


if __name__ == "__main__":
    main()
