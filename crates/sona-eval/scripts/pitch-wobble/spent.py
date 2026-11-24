"""Diagnosis only, on a spent split's `--dump` (logged in the holdout ledger as a re-analysis): the per-subclass
oracle and the two dev rules (a 1-cell piece where GAME's run on the vocals has no cut; the chosen one also
lets a 1-cell head join a 1-cell note), unchanged.

usage: S11_DUMP=<dump dir> python3 spent.py
"""
import collections
import wob
import rules as R
L = wob.L

MAJORITY = dict(ls=1, smax=11, novoc=True, xf=None, nof0=True, pos="any")
CHOSEN = dict(MAJORITY, ties=True)

if __name__ == "__main__":
    songs = wob.load()
    for out in ("lead", "stage"):
        G = {s["id"]: wob.grid(s, out) for s in songs}
        base = [L.score(s, L.from_grid(s, G[s["id"]])) for s in songs]
        Cc = {s["id"]: wob.chopped_cuts(s, G[s["id"]]) for s in songs}
        n = collections.Counter(k for c in Cc.values() for k in c.values())
        print(f"== {out}: {len(songs)} songs, baseline {L.cpm(base):.1f}; chopped cuts " + " ".join(f"{k} {n[k]}" for k in wob.KINDS))
        for kinds in (("step",), ("head",), ("step", "head")):
            rows = [L.score(s, L.from_grid(s, wob.merge(G[s["id"]], {i for i, k in Cc[s["id"]].items() if k in kinds}))) for s in songs]
            L.report(f"oracle: merge truly chopped {'+'.join(kinds)}", songs, base, rows)
        P = {s["id"]: R.prepare(s, G[s["id"]]) for s in songs}
        for name, rule in (("first grid's fold majority", MAJORITY), ("chosen rule (ported)", CHOSEN)):
            rows, t = R.score_rule(songs, G, P, rule)
            L.report(name, songs, base, rows, f"merged ch/bd/ot {t['chopped']}/{t['boundary']}/{t['other']}")
