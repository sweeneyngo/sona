"""Candidate timing fixes from audio evidence, scored with the real metric (dev, paired song bootstrap). No
candidate reads the UST: each sees GAME's raw notes on the lead and the vocals, sona's grid notes, the pitch
track and the vocals' energy, and the grid (spacing, onset phase).

usage: S18_DUMP=DIR python3 -I cands.py [lead|stage] [NAME ...]
"""
import bisect, math, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib18 as L
import lib12
from release_energy import trim_dip  # noqa: E402  (single-note-errors, on the path via lib18)


def raw(song, key="game_lead"):
    return [tuple(n) for n in song["stage_detail"][key]]


def snap(song, game):
    return lib12.snap_notes(game, song["spacing"], song["onset_phase"])


# --- candidates on GAME's raw lead notes (then snapped as shipped) ---

def c_bias(song, q=None):
    """Move every raw time by GAME's own phase against the vocals' onset phase (circular mean of the raw
    onsets' cell fractions), so GAME's latency rounds away: 12's 'own phase', grid kept."""
    sp, ph = song["spacing"], song["onset_phase"]
    g = raw(song)
    x = y = 0.0
    for on, off, m, v in g:
        if v:
            f = (on - ph) / sp
            x += math.cos(2 * math.pi * f)
            y += math.sin(2 * math.pi * f)
    b = math.atan2(y, x) / (2 * math.pi) * sp
    if q is not None:
        b *= q
    return snap(song, [(on - b, off - b, m, v) for on, off, m, v in g])


def c_average(song, reach=0.6, w=0.5):
    """Each raw lead edge averaged with the nearest raw edge of GAME on the vocals within `reach` cells
    (weight `w` on the vocals' edge): two noisy listens of one boundary."""
    sp = song["spacing"]
    g = raw(song)
    ve = sorted({t for on, off, m, v in raw(song, "game_vocals") if v for t in (on, off)})

    def avg(t):
        i = bisect.bisect_left(ve, t)
        best = None
        for j in (i - 1, i):
            if 0 <= j < len(ve) and abs(ve[j] - t) <= reach * sp and (best is None or abs(ve[j] - t) < abs(best - t)):
                best = ve[j]
        return t if best is None else (1 - w) * t + w * best

    return snap(song, [(avg(on), avg(off), m, v) for on, off, m, v in g])


def c_tie_flip(song, t=0.4, srcs=("vocals", "sona")):
    """A snapped lead edge whose raw time sat within (0.5 − t) of the rounding point moves one cell to the
    raw time's side when GAME on the vocals or sona has an edge exactly there."""
    sp, ph = song["spacing"], song["onset_phase"]
    g = raw(song)
    oc = lambda x: round((x - ph) / sp)
    edges = set()
    if "vocals" in srcs:
        for n in snap(song, raw(song, "game_vocals")):
            edges |= {oc(n[0]), oc(n[1])}
    if "sona" in srcs:
        for n in song["sona_onset"]:
            edges |= {oc(n[0]), oc(n[1])}

    def move(x):
        f = (x - ph) / sp
        k = round(f)
        r = f - k
        if abs(r) >= t:
            side = 1 if r > 0 else -1
            if k + side in edges:
                return ph + (k + side) * sp
        return x

    return snap(song, [(move(on), move(off), m, v) for on, off, m, v in g])


def c_tie_flip_notes(song, notes, t=0.35, srcs=("vocals",), clean_ties=True):
    """c_tie_flip on finished notes (GAME lead's or the stage's): an edge that sits on one of GAME lead's
    snapped edges whose raw time leans at least `t` cell to one side moves one cell that way when a second
    source has an edge there. Every edge on that cell moves, so a legato boundary moves as one.

    `clean_ties`: True is what `sona_core::near_ties` ports (leans over half a cell, from the one-voice trim,
    ignored; crossing moves dropped; a cell whose move would empty a note stays put for every note on it).
    "cross" keeps those leans (same numbers on dev). False is the first version: it only undoes an emptied
    note's own move, so neighbours can overlap and the scorer's overlap trim eats one-cell pieces (an
    artefact, kept to reproduce the report's −6.8)."""
    sp, ph = song["spacing"], song["onset_phase"]
    oc = lambda x: round((x - ph) / sp)
    g = raw(song)
    if clean_ties in (True, "resid"):
        # Only real rounding residuals (|r| <= 0.5: the snapped edge is the raw time's own rounding, not a
        # monophony trim), and a note's start wins over the earlier note's end on a shared cell.
        ends, starts = {}, {}
        for o in lib12.snap_notes(g, sp, ph, keep=True):
            for d, edge, rawt in ((ends, o[1], g[o[3]][1]), (starts, o[0], g[o[3]][0])):
                k = oc(edge)
                r = (rawt - ph) / sp - k
                if abs(r) <= 0.5:
                    d[k] = r
        resid = {**ends, **starts}
    else:
        resid = {}
        for o in lib12.snap_notes(g, sp, ph, keep=True):
            for edge, rawt in ((o[0], g[o[3]][0]), (o[1], g[o[3]][1])):
                k = oc(edge)
                resid[k] = (rawt - ph) / sp - k
    edges = set()
    if "vocals" in srcs:
        for n in snap(song, raw(song, "game_vocals")):
            edges |= {oc(n[0]), oc(n[1])}
    if "sona" in srcs:
        for n in song["sona_onset"]:
            edges |= {oc(n[0]), oc(n[1])}
    move = {}
    for k, r in resid.items():
        if abs(r) >= t:
            side = 1 if r > 0 else -1
            if k + side in edges:
                move[k] = k + side
    if clean_ties in (True, "cross"):
        # a move onto a cell whose own edge moves too would cross it: neither of such a pair moves
        move = {k: v for k, v in move.items() if v not in move}
    spans = [(oc(n[0]), oc(n[1]), n[2]) for n in notes]
    if clean_ties in (True, "cross"):
        # a cell whose move would leave a note without a cell stays put for every note on it (the port)
        while True:
            emptied = {k for a, b, _ in spans if move.get(b, b) <= move.get(a, a) for k in (a, b) if k in move}
            if not emptied:
                break
            for k in emptied:
                del move[k]
    out = []
    for a, b, p in spans:
        a2, b2 = move.get(a, a), move.get(b, b)
        if b2 <= a2:
            a2, b2 = a, b
        out.append((ph + a2 * sp, ph + b2 * sp, p))
    return out


def c_trim(song, notes, theta=0.3, tail=0.8, minrun=2):
    return trim_dip(song, [tuple(n) for n in notes], song["onset_phase"], theta=theta, theta_tail=tail, minrun=minrun)


def c_trim_voiced(song, notes, theta=0.3, tail=0.8, minrun=2, voiced_max=0.5, tail_all=None):
    """12's dip rule, but a tail the pitch track still hears (voiced share of its frames above
    `voiced_max`) is kept: a quiet mora (ん, a vowel continuation) is sung, a release is not. With
    `tail_all`, every cut cell must also be under that share of the loudest."""
    sp = song["spacing"]
    en = L.Energy(song)
    ft = [x[0] for x in song["f0"]]
    fv = [x[1] for x in song["f0"]]
    out = []
    for on, off, p in [tuple(n) for n in notes]:
        n = round((off - on) / sp)
        if n < 2:
            out.append((on, off, p))
            continue
        e = en.cells(on, n)
        top = max(e)
        if top <= 0:
            out.append((on, off, p))
            continue
        suf = e[:]
        for c in range(n - 2, -1, -1):
            suf[c] = max(e[c], suf[c + 1])
        cut = None
        for k in range(1, n - minrun + 1):
            if e[k] < theta * top and (k + 1 >= n or suf[k + 1] < tail * top) and (tail_all is None or suf[k] < tail_all * top):
                cut = k
                break
        if cut is not None and voiced_max is not None:
            a, b = bisect.bisect_left(ft, on + cut * sp), bisect.bisect_left(ft, off)
            share = sum(1 for j in range(a, b) if fv[j] > 0) / max(1, b - a)
            if share > voiced_max:
                cut = None
        out.append((on, on + cut * sp, p) if cut is not None else (on, off, p))
    return out


CANDS = {
    **{f"trim voiced<= {vm} tail {tl} quiet {q}": (lambda vm, tl, q: lambda s, o: c_trim_voiced(s, s["outputs"][o], theta=q, tail=tl, voiced_max=vm))(vm, tl, q)
       for vm in (None, 0.25, 0.5) for tl in (0.5, 0.8) for q in (0.2, 0.3)},
    **{f"trim all-quiet {ta} quiet {q}": (lambda ta, q: lambda s, o: c_trim_voiced(s, s["outputs"][o], theta=q, tail=1.0, voiced_max=None, tail_all=ta))(ta, q)
       for ta in (0.2, 0.3, 0.4) for q in (0.2, 0.3)},
    "bias (GAME's own phase)": lambda s, o: c_bias(s) if o == "lead" else None,
    "bias x0.5": lambda s, o: c_bias(s, 0.5) if o == "lead" else None,
    "average with vocals, w 0.5": lambda s, o: c_average(s) if o == "lead" else None,
    "average with vocals, w 0.33": lambda s, o: c_average(s, w=1 / 3) if o == "lead" else None,
    "average with vocals, reach 1.0": lambda s, o: c_average(s, reach=1.0) if o == "lead" else None,
    "tie flip t 0.40 vocals|sona": lambda s, o: c_tie_flip(s) if o == "lead" else None,
    "tie flip t 0.35 vocals|sona": lambda s, o: c_tie_flip(s, 0.35) if o == "lead" else None,
    "tie flip t 0.40 vocals": lambda s, o: c_tie_flip(s, 0.4, ("vocals",)) if o == "lead" else None,
    "tie flip t 0.30 vocals": lambda s, o: c_tie_flip(s, 0.3, ("vocals",)) if o == "lead" else None,
    "trim (12's rule)": lambda s, o: c_trim(s, s["outputs"][o]),
}


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "lead"
    names = sys.argv[2:] or list(CANDS)
    songs = L.loads()
    base = [L.analyse(s, s["outputs"][out]) for s in songs]
    pb = L.pool(base)
    print(f"== {out}: baseline {L.per_min(pb):.1f} cost/min, clean {L.clean(pb):.1f}%, missing {L.missing(pb)} ==")
    for name in names:
        res = [CANDS[name](s, out) for s in songs]
        if any(r is None for r in res):
            continue
        new = [L.analyse(s, r) for s, r in zip(songs, res)]
        print(L.fmt_row(name, base, new))


if __name__ == "__main__":
    main()
