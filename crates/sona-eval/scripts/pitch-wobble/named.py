"""The ticket's literal rules and the fold-majority rule, each alone and combined with 07's per-phrase lyric fit
(merge first, then fit), with the cuts each merges by truth and per-song deltas."""
import sys
import wob
import rules as R
import fit07
L = wob.L

NAMED = {
    "ticket head: 1-cell head after a rest, any step": dict(ls=1, smax=11, novoc=False, xf=None, nof0=True, pos="head"),
    "ticket head, a 1-cell body counts too": dict(ls=1, smax=11, novoc=False, xf=None, nof0=True, pos="head", ties=True),
    "ticket step: <=2 cells, 1-2 st, pitch track < half the step": dict(ls=2, smax=2, novoc=False, xf=0.5, nof0=False, pos="any"),
    "ticket step, 1 cell only": dict(ls=1, smax=2, novoc=False, xf=0.5, nof0=False, pos="any"),
    "fold majority: 1-cell piece, vocals run has no cut there": dict(ls=1, smax=11, novoc=True, xf=None, nof0=True, pos="any"),
    "chosen (every LOSO fold): ... and a 1-cell head of a 1-cell note": dict(ls=1, smax=11, novoc=True, xf=None, nof0=True, pos="any", ties=True),
}

if __name__ == "__main__":
    songs = wob.load()
    for out in ("lead", "stage"):
        G = {s["id"]: wob.grid(s, out) for s in songs}
        P = {s["id"]: R.prepare(s, G[s["id"]]) for s in songs}
        base = [L.score(s, L.from_grid(s, G[s["id"]])) for s in songs]
        fitted = [L.score(s, L.from_grid(s, fit07.fit(s, G[s["id"]]))) for s in songs]
        print(f"== {out}: baseline {L.cpm(base):.1f}; 07 fit alone {L.cpm(fitted):.1f}")
        for name, rule in NAMED.items():
            if len(sys.argv) > 1 and not name.startswith(sys.argv[1]):
                continue
            rows, t = R.score_rule(songs, G, P, rule)
            L.report(name, songs, base, rows, f"merged ch/bd/ot {t['chopped']}/{t['boundary']}/{t['other']}")
            both = [L.score(s, L.from_grid(s, fit07.fit(s, wob.merge(G[s["id"]], R.joins_for(G[s["id"]], P[s["id"]], rule))))) for s in songs]
            L.report("  + 07 fit, against the baseline", songs, base, both)
            L.report("  + 07 fit, against 07 fit alone", songs, fitted, both)
            print("    per song Δ (alone): " + " ".join(f"{s['id'][:10]} {L.cpm([r]) - L.cpm([b]):+.1f}" for s, b, r in zip(songs, base, rows)))
