"""Which transcription-time evidence tells a release overrun cell (GAME lead holds the note through a rest the
UST leaves) from a cell the note really holds? Per-cell AUCs on dev.

usage: S12_DUMP=DIR python3 -I release_auc.py
"""
import bisect, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L


def auc(pos, neg):
    """P(score(pos) > score(neg)), ties half."""
    allv = sorted([(v, 1) for v in pos] + [(v, 0) for v in neg])
    rank_sum, i = 0.0, 0
    while i < len(allv):
        j = i
        while j < len(allv) and allv[j][0] == allv[i][0]:
            j += 1
        r = (i + j + 1) / 2
        rank_sum += r * sum(1 for k in range(i, j) if allv[k][1])
        i = j
    n1, n0 = len(pos), len(neg)
    return (rank_sum - n1 * (n1 + 1) / 2) / (n1 * n0)


def main():
    songs = L.load()
    feats = {"pos": [], "last": [], "inner": []}
    for s in songs:
        sp, an = s["spacing"], s["ust_anchor"]
        ft = [x[0] for x in s["f0"]]
        rt = [x[0] for x in s["rms"]]
        voc = [g for g in s["stage_detail"]["game_vocals"] if g[3]]
        vs = [g[0] for g in voc]
        A = L.analyse(s, s["outputs"]["lead"])
        sec = lambda c: an + c * sp

        def cell_feat(c, note_rms, pitch):
            a, b = sec(c), sec(c + 1)
            i, j = bisect.bisect_left(ft, a), bisect.bisect_left(ft, b)
            hz = [s["f0"][k][1] for k in range(i, j)]
            voiced = sum(h > 0 for h in hz) / max(1, len(hz))
            dev = [abs(L.midi_of(h) - pitch) for h in hz if h > 0]
            k, l = bisect.bisect_left(rt, a), bisect.bisect_left(rt, b)
            e = [s["rms"][q][1] for q in range(k, l)]
            er = (st.mean(e) / note_rms) if e and note_rms > 0 else 0.0
            m = (a + b) / 2
            q = bisect.bisect_right(vs, m) - 1
            vcov = 1.0 if q >= 0 and voc[q][0] <= m < voc[q][1] else 0.0
            return dict(voiced=voiced, energy=er, vocals_game=vcov, pitch_dev=(st.median(dev) if dev else 12.0))

        for p in L.pairs(A):
            r, o = p["r"], p["o"]
            k, l = bisect.bisect_left(rt, sec(o[0])), bisect.bisect_left(rt, sec(min(o[1], r[1])))
            e = [s["rms"][q][1] for q in range(k, l)]
            note_rms = st.median(e) if e else 0.0
            if p["ds"] == 0 and p["de"] > 0 and p["r_next_gap"] and p["r_next_gap"] > 0:
                for c in range(r[1], o[1]):
                    feats["pos"].append(cell_feat(c, note_rms, o[2]))
            if p["cost"] == 0:
                cells = list(range(o[0], o[1]))
                for c in cells[-1:]:
                    feats["last"].append(cell_feat(c, note_rms, o[2]))
                for c in cells[1:-1]:
                    feats["inner"].append(cell_feat(c, note_rms, o[2]))
    print(f"overrun cells {len(feats['pos'])}, last cells of right notes {len(feats['last'])}, inner cells {len(feats['inner'])}")
    for key, sign in (("voiced", -1), ("energy", -1), ("vocals_game", -1), ("pitch_dev", 1)):
        pos = [sign * f[key] for f in feats["pos"]]
        print(f"  {key:12} AUC overrun vs last cell {auc(pos, [sign * f[key] for f in feats['last']]):.3f}, "
              f"vs inner {auc(pos, [sign * f[key] for f in feats['inner']]):.3f}; "
              f"means {st.mean(f[key] for f in feats['pos']):.2f} / {st.mean(f[key] for f in feats['last']):.2f} / "
              f"{st.mean(f[key] for f in feats['inner']):.2f}")


if __name__ == "__main__":
    main()
