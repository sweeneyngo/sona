"""Why the editor flags a learned-stage note (utau-edit-cost 14, ADR-0034): of each song's least confident 20%, as the
editor ranks them (round(0.2 n) notes, ties to the longer), which reason it gives and what cost each reason catches.

usage: python3 -I doubt_reasons.py DEV_DUMP [SHARE]

A note's reason is "long" when ADR-0033's length term b_len*ln(cells) is at least its uncertainty term
-b_prob*(1 - mean class prob), else "unsure" (`cellstage::note_doubt`, `web/lib/doubt.ts`).
"""
import math, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from conf import PORTED, load, note_rows, pooled, boot_one

_, B_PROB, B_LEN = PORTED


def reason(r):
    return "long" if B_LEN * math.log(r["cells"]) >= -B_PROB * (1 - r["pmean"]) else "unsure"


def main():
    songs = load(sys.argv[1])
    share = float(sys.argv[2]) if len(sys.argv) > 2 else 0.2
    by_song = {s["id"]: note_rows(s) for s in songs}
    kinds = ("joined", "chopped")
    tally = {k: {"notes": 0, "cost": 0.0, **{x: 0.0 for x in kinds}} for k in ("long", "unsure")}
    total = {"notes": 0, "cost": 0.0, **{x: 0.0 for x in kinds}}
    pairs = []
    for sid, rows in by_song.items():
        order = sorted(range(len(rows)), key=lambda i: (rows[i]["ported"], -rows[i]["cells"], i))
        flagged = order[: round(share * len(rows))]
        pairs.append((sum(rows[i]["cost"] for i in flagged), sum(r["cost"] for r in rows)))
        total["notes"] += len(rows)
        total["cost"] += sum(r["cost"] for r in rows)
        for x in kinds:
            total[x] += sum(r["cost"] for r in rows if x in r["label"])
        for i in flagged:
            r, t = rows[i], tally[reason(rows[i])]
            t["notes"] += 1
            t["cost"] += r["cost"]
            for x in kinds:
                t[x] += r["cost"] if x in r["label"] else 0.0
    lo, hi = boot_one(pairs)
    n_flag = sum(t["notes"] for t in tally.values())
    print(f"{len(songs)} songs, {total['notes']} notes; {share:.0%} flagged per song = {n_flag} notes, "
          f"catching {pooled(pairs):.1%} [{lo:.1%}, {hi:.1%}] of the cost")
    print("reason   notes  share   cost caught   of joined cost   of chopped cost")
    for k, t in tally.items():
        print(f"{k:7s} {t['notes']:6d} {t['notes'] / max(1, n_flag):6.1%}   {t['cost'] / total['cost']:10.1%}   "
              f"{t['joined'] / total['joined']:13.1%}   {t['chopped'] / total['chopped']:14.1%}")


if __name__ == "__main__":
    main()
