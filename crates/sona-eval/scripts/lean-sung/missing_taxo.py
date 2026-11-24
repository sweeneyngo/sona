"""Where the missing notes come from (utau-edit-cost 06). usage: missing_taxo.py DUMP"""
import sys, os, collections, math
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *


def cover(notes, sp, an, s, e, keep=lambda n: True):
    """Share of cells s..e (UST grid at anchor an) covered by notes (raw seconds), and the pitch of the
    note covering most of it."""
    best, bestp, tot = 0.0, None, 0.0
    a, b = an + s * sp, an + e * sp
    for n in notes:
        if not keep(n):
            continue
        ov = max(0.0, min(b, n[1]) - max(a, n[0]))
        if ov > 0:
            tot += ov
            if ov > best:
                best, bestp = ov, n[2]
    return tot / (b - a), bestp


songs = load(sys.argv[1])
for out in ("lead", "stage"):
    C = collections.Counter()
    per_song = collections.Counter()
    for s in songs:
        sd = s["stage_detail"]
        sp, an, ua = s["spacing"], s["onset_phase"], s["ust_anchor"]
        notes = [tuple(x) for x in s["outputs"][out]]
        a = score(s, notes)
        refs, outs = a["refs"], a["outs"]
        tune = song_tune(sd["game_lead"])
        f0 = s["f0"]
        k0 = sd["k0"]
        for comp in a["comps"]:
            if comp["counts"]["missing"] != 1:
                continue
            r = refs[comp["refs"][0]]
            per_song[s["id"]] += 1
            C["n"] += 1
            L = r[1] - r[0]
            C["len1" if L == 1 else "len2" if L == 2 else "len3+"] += 1
            # neighbours in the output
            left = [o for o in outs if o[1] == r[0]]
            right = [o for o in outs if o[0] == r[1]]
            gapl = min([r[0] - o[1] for o in outs if o[1] <= r[0]] or [99])
            gapr = min([o[0] - r[1] for o in outs if o[0] >= r[1]] or [99])
            C["touches_out_note"] += bool(left or right)
            C["same_pitch_touch"] += bool([o for o in left + right if o[2] == r[2]])
            C["isolated(gap>=2 both)"] += gapl >= 2 and gapr >= 2
            # in a rest of the output? (no output note overlapping at all)
            ov = any(o[0] < r[1] and o[1] > r[0] for o in outs)
            C["overlapped_by_out_minor"] += ov
            # raw GAME lead: unvoiced note covering it?
            cu, pu = cover(sd["game_lead"], sp, ua, r[0], r[1], keep=lambda n: not n[3])
            cv, pv = cover(sd["game_lead"], sp, ua, r[0], r[1], keep=lambda n: n[3])
            if cu >= 0.5:
                C["lead_unvoiced_covers"] += 1
                C["lead_unvoiced_right_pitch"] += round(pu + tune) == r[2]
            if cv >= 0.5:
                C["lead_voiced_covers(lost in snap)"] += 1
            cvv, pvv = cover(sd["game_vocals"], sp, ua, r[0], r[1], keep=lambda n: n[3])
            if cvv >= 0.5:
                C["vocals_voiced_covers"] += 1
            cs, ps = cover(s["sona_onset"], sp, ua, r[0], r[1])
            if cs >= 0.5:
                C["sona_covers"] += 1
                C["sona_right_pitch"] += ps == r[2]
            # f0 voicing over it
            t0, t1 = ua + r[0] * sp, ua + r[1] * sp
            fr = [f for f in f0 if t0 <= f[0] < t1]
            vf = sum(1 for f in fr if f[1] > 0) / max(1, len(fr))
            C["f0_voiced>=0.5"] += vf >= 0.5
            # stage cells (onset phase grid): cells whose midpoint lies in the UST note
            cells = [c for c in range(len(sd["classes"])) if t0 <= an + (k0 + c + 0.5) * sp < t1]
            if cells:
                anyc = any(any(sd["cands"][c][j] >= 0 for j in range(1, 6)) for c in cells)
                C["stage_has_candidate"] += anyc
                prest = sum(sd["probs"][c][0] for c in cells) / len(cells)
                C["stage_p_rest<0.9"] += prest < 0.9
                C["stage_p_rest<0.7"] += prest < 0.7
                C["stage_p_rest<0.5"] += prest < 0.5
                right = any(sd["cands"][c][j] == r[2] for c in cells for j in range(1, 6))
                C["stage_right_pitch_candidate"] += right
    print(f"== {out}: {C['n']} missing UST notes")
    for k, v in C.items():
        if k != "n":
            print(f"  {k:36} {v:5} {100 * v / max(1, C['n']):5.1f}%")
    print("  per song:", dict(per_song.most_common()))
