"""07 step 1a: ceilings given the UST's true mora counts (dev only)."""
import lib07 as L
from oracle1a_lib import counts_per_note

songs = L.load()


def run(out):
    base = [L.score(s, s["outputs"][out]) for s in songs]
    print(f"\n== {out}: baseline {L.cpm(base):.1f} cost/min, clean {L.clean(base):.1f}% ==")
    eqb = [L.phrase_equal_share(s, s["outputs"][out]) for s in songs]
    print(f"  phrases with note count == mora count: {100 * sum(e for e, _ in eqb) / sum(n for _, n in eqb):.1f}% of {sum(n for _, n in eqb)}")
    variants = {}
    for s in songs:
        g = L.to_grid(s, s["outputs"][out])
        cn = counts_per_note(s, g)
        v = variants.setdefault
        # (i) split at the true onsets of the UST notes inside each note (count and placement perfect)
        cuts = {i: [c for c in st[1:]] for i, st in enumerate(cn) if len(st) > 1}
        v("split: true count, true place", []).append(L.split_at(g, cuts))
        # (ii) true count, equal division
        cuts = {i: L.equal_cuts(g[i][0], g[i][1], len(st)) for i, st in enumerate(cn) if len(st) > 1}
        gs = L.split_at(g, cuts)
        v("split: true count, equal place", []).append(gs)
        for T in (1, 2, 3):
            cuts = {i: L.tail_cuts(g[i][0], g[i][1], len(st), T) for i, st in enumerate(cn) if len(st) > 1}
            v(f"split: true count, tail {T} place", []).append(L.split_at(g, cuts))
        for T in (1, 2):
            cuts = {i: L.tail_cuts(g[i][0], g[i][1], len(st), T * s["division"] // 16) for i, st in enumerate(cn) if len(st) > 1}
            v(f"split: true count, tail {T}/16 note place", []).append(L.split_at(g, cuts))
        # (iii) + merge a note holding no UST midpoint into a touching same-pitch neighbour before it
        joins = {i for i, st in enumerate(cn) if not st and i > 0 and g[i - 1][1] == g[i][0] and g[i - 1][2] == g[i][2]}
        v("merge only: true count (0-mora same-pitch)", []).append(L.merge(g, joins))
        gm = L.merge(g, joins)
        cn2 = counts_per_note(s, gm)
        cuts = {i: L.equal_cuts(gm[i][0], gm[i][1], len(st)) for i, st in enumerate(cn2) if len(st) > 1}
        v("split+merge: true count, equal place", []).append(L.split_at(gm, cuts))
        # (iv) only the phrase's count known: deficit (moras - notes) per UST phrase, given to the longest notes
        ph = [(a, b, n) for a, b, n in L.phrases(s)]
        # phrase boundaries in output-grid units: UST cells are within 0.06 cell of the output grid
        shift = L.ec.rround((s["ust_anchor"] - s["onset_phase"]) / s["spacing"])
        cuts = {}
        for a, b, n in ph:
            a, b = a + shift, b + shift
            idx = [i for i, (x, y, _) in enumerate(g) if a <= (x + y) / 2 < b]
            extra = n - len(idx)
            k = {i: 1 for i in idx}
            while extra > 0 and idx:
                # give one more mora to the note with the most cells per mora
                i = max(idx, key=lambda i: (g[i][1] - g[i][0]) / k[i])
                if (g[i][1] - g[i][0]) <= k[i]:
                    break
                k[i] += 1
                extra -= 1
            for i in idx:
                if k[i] > 1:
                    cuts[i] = L.equal_cuts(g[i][0], g[i][1], k[i])
        v("split: phrase count, longest notes, equal", []).append(L.split_at(g, cuts))
    for name, gl in variants.items():
        notes = [L.from_grid(s, gg) for s, gg in zip(songs, gl)]
        rows = [L.score(s, n) for s, n in zip(songs, notes)]
        eq = [L.phrase_equal_share(s, n) for s, n in zip(songs, notes)]
        L.report(name, songs, base, rows, f"phr= {100 * sum(e for e, _ in eq) / sum(n for _, n in eq):.1f}%")


for out in ("lead", "stage"):
    run(out)
