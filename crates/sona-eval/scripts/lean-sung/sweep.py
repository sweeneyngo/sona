"""Prototype sweep for utau-edit-cost 06. usage: sweep.py DUMP"""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *

songs = load(sys.argv[1])


def stage_bias(s, b):
    sd = s["stage_detail"]
    cl = choose(sd["probs"], sd["cands"], b)
    return cells_to_notes(cl, sd["cands"], sd["sources"], sd["k0"], s["spacing"], s["onset_phase"])


def lead_unvoiced(s, min_s):
    sd = s["stage_detail"]
    return snap_notes(sd["game_lead"], s["spacing"], s["onset_phase"], keep=lambda n: n[3] or (n[1] - n[0] >= min_s))


def fill_from(notes, extra, min_cells, sp, an):
    """Add each note of `extra` that lies wholly in a rest of `notes`, is >= min_cells long, and does not touch
    a note of the same pitch (that would join)."""
    notes = sorted(notes)
    out = list(notes)
    for e in extra:
        cells = round((e[1] - e[0]) / sp)
        if cells < min_cells:
            continue
        if any(n[0] < e[1] - 1e-9 and n[1] > e[0] + 1e-9 for n in notes):
            continue
        if any((abs(n[1] - e[0]) < 1e-6 or abs(n[0] - e[1]) < 1e-6) and n[2] == e[2] for n in notes):
            continue
        out.append(tuple(e))
    return sorted(out)


def table(title, base_fn, variants):
    base = [row_of(s, base_fn(s)) for s in songs]
    bc, bcl = pooled(base)
    bok = [cell_acc(s, base_fn(s)) for s in songs]
    print(f"\n== {title}: base {bc:.1f} clean {bcl:.1f}% cells {100 * sum(o for o, _ in bok) / sum(n for _, n in bok):.2f}%")
    print(f"  {'variant':34} {'cost/min':>8} {'delta':>7} {'95%':>16} {'clean':>6} {'cells':>7} {'miss':>5} {'extra':>5} {'join':>5} {'chop':>5} {'pitch':>5} {'len':>5}")
    for name, fn in variants:
        outs = [fn(s) for s in songs]
        rows = [row_of(s, o) for s, o in zip(songs, outs)]
        c, cl = pooled(rows)
        lo, hi = boot_delta(base, rows)
        ok = [cell_acc(s, o) for s, o in zip(songs, outs)]
        cnt = lambda k: sum(r[4][k] for r in rows)
        print(f"  {name:34} {c:8.1f} {c - bc:+7.1f} [{lo:+6.1f}, {hi:+6.1f}] {cl:5.1f}% {100 * sum(o for o, _ in ok) / sum(n for _, n in ok):6.2f}% {cnt('missing'):5} {cnt('extra'):5} {cnt('joined'):5} {cnt('chopped'):5} {cnt('pitch'):5} {cnt('length'):5}")


stage0 = lambda s: stage_bias(s, 0.0)
lead0 = lambda s: [tuple(x) for x in s["outputs"]["lead"]]
if __name__ == "__main__" and not os.environ.get("NOTE_LEVEL"):
    table("stage, rest bias (log-prob subtracted from rest)", stage0,
          [(f"bias {b}", (lambda b: lambda s: stage_bias(s, b))(b)) for b in (0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 10.0)])
    table("GAME lead, unvoiced GAME notes kept if >= x s", lead0,
          [(f"unvoiced >= {x}s", (lambda x: lambda s: lead_unvoiced(s, x))(x)) for x in (0.0, 0.1, 0.2, 0.3, 0.5, 1.0)])
    table("GAME lead, rests filled with sona notes >= k cells", lead0,
          [(f"sona fill >= {k}", (lambda k: lambda s: fill_from(lead0(s), s["sona_onset"], k, s["spacing"], s["onset_phase"]))(k)) for k in (1, 2, 3, 4, 6)])
    table("stage, rests filled with sona notes >= k cells", stage0,
          [(f"sona fill >= {k}", (lambda k: lambda s: fill_from(stage0(s), s["sona_onset"], k, s["spacing"], s["onset_phase"]))(k)) for k in (1, 2, 3, 4, 6)])


def new_notes_only(base, biased, min_cells, sp):
    """`base` plus each note of `biased` lying wholly in a rest of `base` and >= min_cells long."""
    out = list(base)
    for e in biased:
        if round((e[1] - e[0]) / sp) < min_cells:
            continue
        if any(n[0] < e[1] - 1e-9 and n[1] > e[0] + 1e-9 for n in base):
            continue
        out.append(tuple(e))
    return sorted(out)


if __name__ == "__main__" and os.environ.get("NOTE_LEVEL"):
    for k in (1, 2, 3):
        table(f"stage + new notes (wholly in its rests, >= {k} cells) from the biased stage", stage0,
              [(f"bias {b}", (lambda b: lambda s: new_notes_only(stage0(s), stage_bias(s, b), k, s["spacing"]))(b)) for b in (0.5, 1.0, 2.0, 3.0, 4.0, 6.0)])
    table("GAME lead + new notes (wholly in its rests) from the biased stage", lead0,
          [(f"bias {b}", (lambda b: lambda s: new_notes_only(lead0(s), stage_bias(s, b), 1, s["spacing"]))(b)) for b in (0.0, 0.5, 1.0, 2.0, 3.0, 4.0)])
