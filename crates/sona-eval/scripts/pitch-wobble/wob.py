"""Ticket 11 harness: the chopped pieces of a held note (pitch wobble, scoop head), their oracles, and
merge rules from transcription-time evidence (piece lengths, pitch step, sona's pitch track).

Needs a `sona-eval-cellstage --dump` of dev in $S11_DUMP (it holds UST content: keep it out of the repo).
Reuses ticket 07's harness (`../same-pitch-boundaries/lib07.py`) for scoring and the paired bootstrap.
"""
import math, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.append(os.path.join(HERE, "..", "same-pitch-boundaries"))
os.environ.setdefault("S07_DUMP", os.environ.get("S11_DUMP", ""))
import lib07 as L  # noqa: E402
import ec  # noqa: E402

KINDS = ("same", "step", "head", "other", "octave", "rest")


def load():
    return L.load(os.environ["S07_DUMP"])


def grid(s, out):
    return L.to_grid(s, s["outputs"][out])


def truth(s, g):
    """For each touching pair (i-1, i) of output notes on the output grid: 'chopped' when both pieces have an
    edge to the same UST note (the cut lies inside one held note), 'boundary' when the UST has a note
    boundary between them and no shared note, else 'other'. Returns {i: label}."""
    notes = L.from_grid(s, g)
    a = L.score(s, notes)
    # a['outs'] carry the original index in [3]
    pos = {o[3]: k for k, o in enumerate(a["outs"])}
    lab = {}
    for i in range(1, len(g)):
        if g[i - 1][1] != g[i][0]:
            continue
        x, y = pos.get(i - 1), pos.get(i)
        if x is None or y is None:
            lab[i] = "other"
            continue
        rx, ry = set(a["adj_o"][x]), set(a["adj_o"][y])
        if rx & ry:
            lab[i] = "chopped"
        elif rx and ry:
            lab[i] = "boundary"
        else:
            lab[i] = "other"
    return lab, a


def kind_of_cut(g, i, chopped_run_start):
    """03's subclass of the cut between g[i-1] and g[i] (taxo.chop_kind): rest, octave, same, head, step, other.
    `chopped_run_start` is the index of the first piece under the same UST note."""
    a, b = g[i - 1], g[i]
    iv = b[2] - a[2]
    if a[1] < b[0]:
        return "rest"
    if abs(iv) in (12, 24):
        return "octave"
    if iv == 0:
        return "same"
    if a[1] - a[0] == 1 and i - 1 == chopped_run_start:
        return "head"
    if abs(iv) <= 2:
        return "step"
    return "other"


def chopped_cuts(s, g):
    """{i: kind} for every chopped cut (the cut between g[i-1] and g[i] lies inside one UST note), with
    03's subclass. Matches taxo.chopped_cuts' count."""
    lab, a = truth(s, g)
    pos = {o[3]: k for k, o in enumerate(a["outs"])}
    inv = {k: o[3] for k, o in enumerate(a["outs"])}
    cuts = {}
    for ri, os_ in enumerate(a["adj_r"]):
        if len(os_) < 2:
            continue
        idx = sorted(inv[o] for o in os_)
        for x, y in zip(idx, idx[1:]):
            cuts[y] = kind_of_cut(g, y, idx[0]) if y == x + 1 else "rest"
    return cuts


def merge(g, joins, pitch="longer", ref_pitch=None):
    """Merge g[i] into g[i-1] for every i in joins (chains allowed). The merged note's pitch: the longest
    piece's ('longer'; ties go to the later piece), or ref_pitch(start, end) when given."""
    out, groups = [], []
    for i, n in enumerate(g):
        if i in joins and groups:
            groups[-1].append(n)
        else:
            groups.append([n])
    for grp in groups:
        a, b = grp[0][0], grp[-1][1]
        if ref_pitch is not None and len(grp) > 1:
            p = ref_pitch(a, b, grp)
        else:
            best = max(range(len(grp)), key=lambda k: (grp[k][1] - grp[k][0], k))
            p = grp[best][2]
        out.append((a, b, p))
    return out


def hz_to_midi(h):
    return 69 + 12 * math.log2(h / 440.0)


class Track:
    """sona's pitch track (pYIN on the vocals) in MIDI, voiced frames only."""

    def __init__(self, s):
        import bisect
        self.t = [t for t, h in s["f0"]]
        self.m = [hz_to_midi(h) if h > 0 else None for t, h in s["f0"]]
        self._b = bisect

    def frames(self, t0, t1):
        lo = self._b.bisect_left(self.t, t0)
        hi = self._b.bisect_left(self.t, t1)
        return [m for m in self.m[lo:hi] if m is not None]


def median(v):
    v = sorted(v)
    n = len(v)
    if not n:
        return None
    return v[n // 2] if n % 2 else 0.5 * (v[n // 2 - 1] + v[n // 2])
