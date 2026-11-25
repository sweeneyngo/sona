"""Sweep of the candidate families on dev, with the selection rule's checks and a leave-one-song-out choice.

Families (all audio-only, see cands.py): trim (12's release rule, retuned: quiet, tail, min cells; on the
vocals' or the lead stem's energy), tie flip (a near-tie edge moves to the side a second source has an
edge on), bias (GAME's latency against the onset phase), average (raw edges averaged with GAME on the
vocals). For each setting: cost/min Δ [95%], clean Δ, missing Δ and the upper bound with missing weighted 4.
Then per family: the setting chosen on 8 songs, scored on the 9th, pooled.

usage: S18_DUMP=DIR S18_LEAD_RMS=DIR python3 -I sweep18.py lead|stage [family ...]
"""
import itertools, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import cands as C


def trim_grid():
    for src in ("vocals", "lead"):
        for q, t, m in itertools.product((0.15, 0.2, 0.25, 0.3), (0.2, 0.3, 0.4, 0.5, 0.6, 0.8), (1, 2, 3)):
            if t < q:
                continue
            def f(s, o, q=q, t=t, m=m, src=src):
                os.environ["S18_ENERGY"] = src
                r = C.c_trim_voiced(s, s["outputs"][o], theta=q, tail=t, minrun=m, voiced_max=None)
                os.environ["S18_ENERGY"] = "vocals"
                return r
            yield f"trim {src} quiet {q} tail {t} min {m}", f


def flip_grid():
    for t, srcs in itertools.product((0.3, 0.35, 0.4, 0.45), (("vocals",), ("sona",), ("vocals", "sona"))):
        yield f"tie flip |r|>={t} {'/'.join(srcs)}", (lambda s, o, t=t, srcs=srcs: C.c_tie_flip(s, t, srcs) if o == "lead" else None)


def flipnotes_grid():
    # "cross": one voice kept (crossing moves dropped, a move that would empty a note stays put for every note
    # on its cell), the port; True: also only |lean| <= 0.5. (Without the one-voice guard the moves overlap
    # notes and the scorer's overlap trim eats one-cell pieces: an artefact, not a fix.)
    for clean, t, srcs in itertools.product(("cross", True), (0.25, 0.3, 0.35, 0.4, 0.45), (("vocals",), ("vocals", "sona"))):
        yield (f"tie flip on notes{' (|lean| <= 0.5)' if clean is True else ''} |r|>={t} {'/'.join(srcs)}",
               (lambda s, o, t=t, srcs=srcs, clean=clean: C.c_tie_flip_notes(s, s["outputs"][o], t, srcs, clean)))


def bias_grid():
    for q in (0.25, 0.5, 0.75, 1.0):
        yield f"bias x{q}", (lambda s, o, q=q: C.c_bias(s, q) if o == "lead" else None)


def avg_grid():
    for reach, w in itertools.product((0.5, 0.75, 1.0), (0.25, 0.5)):
        yield f"average reach {reach} w {w}", (lambda s, o, reach=reach, w=w: C.c_average(s, reach, w) if o == "lead" else None)


FAMILIES = {"trim": trim_grid, "flip": flip_grid, "flipnotes": flipnotes_grid, "bias": bias_grid, "average": avg_grid}


def verdict(base, new):
    d, lo, hi = L.boot_delta(base, new)
    dc = L.clean(L.pool(new)) - L.clean(L.pool(base))
    dm = L.missing(L.pool(new)) - L.missing(L.pool(base))
    _, _, hi4 = L.boot_delta(base, new, stat=L.per_min_missing4)
    ok = hi < 0 and dc >= 0 and (dm <= 0 or hi4 < 0)
    return ok, f"  missing@4 upper {hi4:+5.1f}  {'PASS' if ok else 'fail'}"


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "lead"
    fams = sys.argv[2:] or list(FAMILIES)
    songs = L.loads()
    base = [L.analyse(s, s["outputs"][out]) for s in songs]
    pb = L.pool(base)
    print(f"== {out}: baseline {L.per_min(pb):.1f} cost/min, clean {L.clean(pb):.1f}%, missing {L.missing(pb)} ==")
    for fam in fams:
        rows = {}
        for name, f in FAMILIES[fam]():
            res = [f(s, out) for s in songs]
            if any(r is None for r in res):
                continue
            new = [L.analyse(s, r) for s, r in zip(songs, res)]
            rows[name] = new
            ok, v = verdict(base, new)
            print(L.fmt_row(name, base, new, v))
        if not rows:
            continue
        # leave one song out: the setting with the lowest pooled cost on the other songs (the declared
        # choice), and, as a labelled sensitivity, with missing weighted 4 in that choice
        for label, stat in (("", L.per_min), (" (choice with missing at 4; post hoc)", L.per_min_missing4)):
            loso, picks = [], []
            for i in range(len(songs)):
                rest = lambda new: stat(L.pool([r for j, r in enumerate(new) if j != i]))
                pick = min(rows, key=lambda k: rest(rows[k]))
                picks.append(pick)
                loso.append(rows[pick][i])
            ok, v = verdict(base, loso)
            print(L.fmt_row(f"{fam}: chosen leave-one-song-out{label}", base, loso, v))
            print("    picks: " + "; ".join(sorted(set(picks), key=picks.index)) + f"  ({len(set(picks))} distinct)")


if __name__ == "__main__":
    main()
