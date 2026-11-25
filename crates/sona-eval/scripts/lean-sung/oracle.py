"""Ceilings for leaning toward sung (utau-edit-cost 06): the best any selector could do with a given pool of
proposals, by greedily keeping each proposal that lowers the song's edit cost (reads the UST: an oracle).
usage: oracle.py DUMP"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *
import sweep

songs = load(sys.argv[1])


def greedy(s, base, proposals, merge):
    cur = list(base)
    c = row_of(s, cur)[0]
    kept = 0
    for p in proposals:
        cand = merge(cur, p)
        cc = row_of(s, cand)[0]
        if cc < c:
            cur, c, kept = cand, cc, kept + 1
    return cur, kept


def add_note(cur, p):
    return sorted([n for n in cur] + [tuple(p)])


def promote(s):
    """GAME lead with any subset of its unvoiced notes promoted (what any presence threshold selects)."""
    sd = s["stage_detail"]
    sp, an = s["spacing"], s["onset_phase"]
    unv = [i for i, n in enumerate(sd["game_lead"]) if not n[3]]
    chosen = set()
    base = snap_notes(sd["game_lead"], sp, an)
    c = row_of(s, base)[0]
    for i in unv:
        idx = chosen | {i}
        trial = snap_notes([n + [j] for j, n in enumerate(sd["game_lead"])], sp, an, keep=lambda n: n[3] or n[4] in idx)
        cc = row_of(s, trial)[0]
        if cc < c:
            chosen, c = idx, cc
    return snap_notes([n + [j] for j, n in enumerate(sd["game_lead"])], sp, an, keep=lambda n: n[3] or n[4] in chosen), len(chosen), len(unv)


def report(title, base_fn, fn):
    base = [row_of(s, base_fn(s)) for s in songs]
    outs = [fn(s) for s in songs]
    rows = [row_of(s, o[0]) for s, o in zip(songs, outs)]
    (bc, bcl), (c, cl) = pooled(base), pooled(rows)
    print(f"{title:70} {bc:6.1f} -> {c:6.1f} ({c - bc:+.1f}) clean {bcl:.1f} -> {cl:.1f}%  kept {sum(o[1] for o in outs)} of {sum(o[2] for o in outs)}"
          f"  missing {sum(r[4]['missing'] for r in base)} -> {sum(r[4]['missing'] for r in rows)}")


lead0 = lambda s: [tuple(x) for x in s["outputs"]["lead"]]
stage0 = lambda s: sweep.stage_bias(s, 0.0)
report("oracle: GAME lead, best subset of unvoiced GAME notes promoted", lead0, promote)
for b in (2.0, 6.0, 20.0):
    def f(s, b=b):
        base = stage0(s)
        props = [p for p in sweep.stage_bias(s, b) if not any(n[0] < p[1] - 1e-9 and n[1] > p[0] + 1e-9 for n in base)]
        cur, kept = greedy(s, base, props, add_note)
        return cur, kept, len(props)
    report(f"oracle: stage + best new notes in its rests from the stage at rest bias {b}", stage0, f)
    def g(s, b=b):
        base = lead0(s)
        props = [p for p in sweep.stage_bias(s, b) if not any(n[0] < p[1] - 1e-9 and n[1] > p[0] + 1e-9 for n in base)]
        cur, kept = greedy(s, base, props, add_note)
        return cur, kept, len(props)
    report(f"oracle: GAME lead + best new notes in its rests from the stage at rest bias {b}", lead0, g)
