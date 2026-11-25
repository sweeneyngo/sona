"""Evidence at the output's legato boundaries (one note ends where the next starts): where each audio source
puts the edge, against where the UST does. A legato boundary one cell off costs up to 12 (the earlier note's
length, the later one's shift and length), so it is the unit a fix moves.

For each boundary of the output whose later note is a pair with a legato UST start, `true` is the UST start
minus the output's (cells). Each source's offer is the nearest edge it has within ±2 cells (0 = agrees with
the output): GAME on the vocals, sona's pYIN grid notes, the pitch track's crossing of the two notes'
midpoint (different pitches only) and the vocals' energy minimum. Prints each offer's confusion and the
net of "move the boundary where the offer says" (right moves − wrong moves, among boundaries with truth).

usage: S18_DUMP=DIR python3 -I boundaries.py lead|stage
"""
import bisect, collections, math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import lib12

OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"


def nearest(cells, k, reach=2):
    best = None
    for d in range(0, reach + 1):
        for s in ((0,) if d == 0 else (-d, d)):
            if k + s in cells:
                return s
    return best


def boundary_rows(song, out=OUT):
    sp, an, ph = song["spacing"], song["ust_anchor"], song["onset_phase"]
    oc = lambda t: round((t - ph) / sp)  # onset-grid cell of a time
    notes = [tuple(n) for n in song["outputs"][out]]
    A = L.analyse(song, notes)
    sd = song["stage_detail"]
    lead = lib12.snap_notes([tuple(n) for n in sd["game_lead"]], sp, ph)
    voc = lib12.snap_notes([tuple(n) for n in sd["game_vocals"]], sp, ph)
    sona = [tuple(n) for n in song["sona_onset"]]
    edges = {k: {oc(n[0]) for n in v} | {oc(n[1]) for n in v} for k, v in (("lead", lead), ("vocals", voc), ("sona", sona))}
    ft = [x[0] for x in song["f0"]]
    fv = [x[1] for x in song["f0"]]
    en = L.Energy(song)
    raw_resid = None
    if out == "lead":
        snapped = lib12.snap_notes([tuple(n) for n in sd["game_lead"]], sp, ph, keep=True)
        raw_resid = {}
        for o in snapped:
            x = (sd["game_lead"][o[3]][0] - ph) / sp
            raw_resid[oc(o[0])] = x - oc(o[0])
    truth = {}
    for p in L.pairs(A, song):
        if p["start_ctx"] == "legato":
            truth[p["oi"]] = (p["r"][0] - p["o"][0], p)
    rows = []
    outs = A["outs"]
    for i in range(1, len(outs)):
        a, b = outs[i - 1], outs[i]
        if a[1] != b[0]:
            continue
        T = an + b[0] * sp
        k = oc(T)
        row = dict(song=song["id"], i=i, k=k, true=truth.get(i, (None,))[0], pa=a[2], pb=b[2],
                   la=a[1] - a[0], lb=b[1] - b[0])
        for name, e in edges.items():
            row[name] = nearest(e, k)
        # pitch crossing: first frame after (T - 2 cells) where the track is nearer b's pitch than a's
        row["f0"] = None
        if a[2] != b[2]:
            lo, hi = bisect.bisect_left(ft, T - 2.5 * sp), bisect.bisect_left(ft, T + 2.5 * sp)
            mid = (a[2] + b[2]) / 2
            up = b[2] > a[2]
            seq = [(ft[j], 69 + 12 * math.log2(fv[j] / 440)) for j in range(lo, hi) if fv[j] > 0]
            for j in range(1, len(seq)):
                x0, x1 = seq[j - 1][1], seq[j][1]
                if (x0 < mid <= x1) if up else (x0 > mid >= x1):
                    c = oc(seq[j][0]) - k
                    if abs(c) <= 2 and (row["f0"] is None or abs(c) < abs(row["f0"])):
                        row["f0"] = c
        # energy minimum over the cells around the boundary: the cell index whose start the dip sits at
        e = [en.mean(T + (c - 0.5) * sp, T + (c + 0.5) * sp) for c in range(-2, 3)]
        row["dip"] = min(range(5), key=lambda c: e[c]) - 2 if max(e) > 0 else None
        # voicing onset: the end of the last unvoiced run (>= 2 frames) within ±2 cells (a consonant before the
        # vowel); energy rise: the cell with the largest rise of log energy from the cell before it
        lo, hi = bisect.bisect_left(ft, T - 2.5 * sp), bisect.bisect_left(ft, T + 2.5 * sp)
        row["von"] = None
        run = 0
        for j in range(lo, hi):
            if fv[j] <= 0:
                run += 1
            else:
                if run >= 2:
                    c = oc(ft[j]) - k
                    if abs(c) <= 2 and (row["von"] is None or abs(c) < abs(row["von"])):
                        row["von"] = c
                run = 0
        le = [math.log(max(en.mean(T + (c - 0.5) * sp, T + (c + 0.5) * sp), 1e-7)) for c in range(-3, 3)]
        rises = [le[c + 1] - le[c] for c in range(5)]
        row["rise"] = max(range(5), key=lambda c: rises[c]) - 2
        row["resid"] = raw_resid.get(k) if raw_resid is not None else None
        rows.append(row)
    return rows, A


def main():
    songs = L.loads()
    rows = []
    for s in songs:
        rows.extend(boundary_rows(s)[0])
    known = [r for r in rows if r["true"] is not None]
    print(f"== {OUT}: legato boundaries {len(rows)}, with UST truth {len(known)}, of those off: "
          f"{sum(1 for r in known if r['true'])} (true Δ: {dict(collections.Counter(r['true'] for r in known))}) ==")
    if OUT == "lead":
        for lo, hi in ((0, 0.1), (0.1, 0.2), (0.2, 0.3), (0.3, 0.4), (0.4, 0.51)):
            g = [r for r in known if r["resid"] is not None and lo <= abs(r["resid"]) < hi]
            w = [r for r in g if r["true"]]
            across = [r for r in w if r["true"] * r["resid"] > 0]
            print(f"  raw residual |r| in [{lo}, {hi}): {len(g):5} boundaries, off {len(w):4} ({100 * len(w) / max(1, len(g)):4.1f}%), "
                  f"truth on the residual's side {len(across)}")
    for src in ("lead", "vocals", "sona", "f0", "dip", "von", "rise"):
        c = collections.Counter()
        for r in known:
            o = r[src]
            if o is None:
                c["none"] += 1
            elif o == 0:
                c["agree, right" if r["true"] == 0 else "agree, wrong"] += 1
            elif o == r["true"]:
                c["moves, right"] += 1
            elif r["true"] == 0:
                c["moves, breaks"] += 1
            else:
                c["moves, still wrong"] += 1
        net = c["moves, right"] - c["moves, breaks"]
        print(f"  {src:7} " + "  ".join(f"{k} {v}" for k, v in sorted(c.items())) + f"   net {net:+d}")
    # agreement of two offers
    print("  -- two offers agreeing on the same move (≠ 0) --")
    for a, b in (("vocals", "sona"), ("vocals", "f0"), ("sona", "f0"), ("sona", "dip"), ("vocals", "dip"), ("f0", "dip")):
        c = collections.Counter()
        for r in known:
            if r[a] is not None and r[a] != 0 and r[a] == r[b]:
                c["right" if r[a] == r["true"] else ("breaks" if r["true"] == 0 else "still wrong")] += 1
        print(f"  {a}+{b}: " + "  ".join(f"{k} {v}" for k, v in sorted(c.items())))
    if OUT == "lead":
        near_tie_table(known)


def near_tie_table(known):
    """Lead only: flip a near-tie boundary (raw residual |r| ≥ t) to the residual's side when an offer agrees."""
    print("  -- flip a near tie to the residual's side when an offer agrees --")
    for t in (0.25, 0.3, 0.35, 0.4, 0.45):
        for src in ("none", "vocals", "sona", "f0", "von", "rise", "vocals|sona", "vocals&sona"):
            fix = brk = other = 0
            for r in known:
                if r["resid"] is None or abs(r["resid"]) < t:
                    continue
                side = 1 if r["resid"] > 0 else -1
                if src == "none":
                    ok = True
                elif src == "vocals|sona":
                    ok = side in (r["vocals"], r["sona"])
                elif src == "vocals&sona":
                    ok = r["vocals"] == side and r["sona"] == side
                else:
                    ok = r[src] == side
                if not ok:
                    continue
                if r["true"] == side:
                    fix += 1
                elif r["true"] == 0:
                    brk += 1
                else:
                    other += 1
            print(f"  |r| >= {t:.2f} + {src:12}: fixes {fix:4} breaks {brk:4} other {other:3}  net {fix - brk:+d}")


if __name__ == "__main__":
    main()
