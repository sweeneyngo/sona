"""Lyrics-first 17: build the derived onset sources from the raw ones, on dev.

Raw sources (one onset per mora, seconds): 14's CTC alignments (`ctc-<aligner>`), `mms-vowel` (mms_vowels.py)
and `sofa-vowel` (sofa_align.py). Derived:
- `deb-<raw>`: minus the median signed error of the mora's consonant class, fitted on the other 8 songs
  (leave one song out). This is the per-class consonant-to-vowel offset.
- `ens3` / `ens5`: the per-mora median of the debiased lyrics, hira and jg CTCs (+ mms, sofa-vowel).
- `<x>+<kind>snap`: moved to the nearest of sona's own onset candidates within 0.6 cell (GAME's raw note
  starts, sona's onset segmenter, pitch-track voicing onsets, the output notes' starts).
- `<x>+prior`: rounded to the cell that maximises a Gaussian around the estimate (sigma 0.5 cell) times the
  lattice prior: the share of sona's own output note starts at that position in the beat (no lyric, no UST).
- `m3` / `m5+adaptmora`: the median of 3 (5) members with the lattice prior, sigma per mora = 2 × the
  members' median distance from their median, at least 0.2 cell.
- `…+abstK`: no onset where the members span more than K cells.

Only the per-class offsets read the UST, and they are leave-one-song-out. The sigma 0.5, the scale 2 and the
thresholds were looked at on dev (in sample). Writes $L17_WORK/onsets/<name>/<id>.json.

Usage: L01_DUMP=… L01_WORK=… L14_WORK=… L17_WORK=… python3 sources.py
"""
import bisect, collections, json, math, os, statistics, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import precision as P  # noqa: E402

L, O = P.L, P.O
W14, W17 = os.environ["L14_WORK"], os.environ["L17_WORK"]
SONGS = [s for s in L.load() if os.path.exists(os.path.join(W14, "text", s["id"] + ".json"))]
TEXT = {s["id"]: json.load(open(os.path.join(W14, "text", s["id"] + ".json"))) for s in SONGS}
CTC = ("lyrics-lead", "hira-lead", "jg-lead", "mms-lead", "whisper-large-v3-lead")
M3 = ["deb-ctc-lyrics-lead", "deb-ctc-hira-lead", "deb-ctc-jg-lead"]
M5 = M3 + ["deb-ctc-mms-lead", "deb-sofa-vowel"]


def src(name, sid):
    return P.load_source(name, sid)


def write(name, sid, on):
    d = os.path.join(W17, "onsets", name)
    os.makedirs(d, exist_ok=True)
    json.dump({"on": on}, open(os.path.join(d, sid + ".json"), "w"))


def cls(m):
    return P.mora_class(m["kana"])


def raw_ctc():
    for s in SONGS:
        write("ust", s["id"], [m["on"] for m in TEXT[s["id"]]["moras"]])
        for a in CTC:
            al = json.load(open(os.path.join(W14, "align", a, s["id"] + ".json")))
            write("ctc-" + a, s["id"], [x[0] for x in al["moras"]])


def debias(name):
    errs = {s["id"]: collections.defaultdict(list) for s in SONGS}
    for s in SONGS:
        for m, t in zip(TEXT[s["id"]]["moras"], src(name, s["id"])):
            if t is not None:
                errs[s["id"]][cls(m)].append(t - m["on"])
    for s in SONGS:
        pool = collections.defaultdict(list)
        for o in SONGS:
            if o["id"] != s["id"]:
                for k, v in errs[o["id"]].items():
                    pool[k] += v
        b = {k: statistics.median(v) for k, v in pool.items()}
        write("deb-" + name, s["id"], [None if t is None else t - b.get(cls(m), 0.0) for m, t in zip(TEXT[s["id"]]["moras"], src(name, s["id"]))])


def median_of(members, sid):
    cols = [src(n, sid) for n in members]
    return cols, [statistics.median(v) if (v := [c[i] for c in cols if c[i] is not None]) else None for i in range(len(cols[0]))]


def ensemble(members, new):
    for s in SONGS:
        write(new, s["id"], median_of(members, s["id"])[1])


def lattice_prior(s):
    """Share of sona's own output note starts (GAME lead) at each cell position within the beat, smoothed."""
    cpb = s["division"] // 4
    c = collections.Counter(O.g_of(s, a) % cpb for a, _, _ in s["outputs"]["lead"])
    n = sum(c.values())
    return [(c[k] + 1) / (n + cpb) for k in range(cpb)]


def snap_cell(s, t, sigma, pr):
    x = (t - s["onset_phase"]) / s["spacing"]
    best = max(range(math.floor(x) - 1, math.floor(x) + 3), key=lambda c: -((x - c) / sigma) ** 2 / 2 + math.log(pr[c % len(pr)]))
    return O.t_of(s, best)


def prior(name, new, sigma=0.5):
    for s in SONGS:
        pr = lattice_prior(s)
        write(new, s["id"], [None if t is None else snap_cell(s, t, sigma, pr) for t in src(name, s["id"])])


def adaptive(members, new, scale=2.0, floor=0.2):
    for s in SONGS:
        cols, med = median_of(members, s["id"])
        pr = lattice_prior(s)
        on = []
        for i, t in enumerate(med):
            dv = [abs(c[i] - t) / s["spacing"] for c in cols]
            on.append(snap_cell(s, t, max(floor, scale * statistics.median(dv)), pr))
        write(new, s["id"], on)


def abstain(name, members, new, max_span):
    for s in SONGS:
        cols = [src(n, s["id"]) for n in members]
        out = []
        for i, t in enumerate(src(name, s["id"])):
            v = [c[i] for c in cols if c[i] is not None]
            out.append(t if t is not None and v and (max(v) - min(v)) / s["spacing"] <= max_span else None)
        write(new, s["id"], out)


def candidates(s, kind):
    if kind == "game":
        return sorted(x[0] for x in s["stage_detail"]["game_lead"])
    if kind == "sona":
        return sorted(x[0] for x in s["sona_onset"])
    if kind == "voice":
        f0 = s["f0"]
        return [f0[i][0] for i in range(1, len(f0)) if f0[i][1] > 0 and f0[i - 1][1] == 0]
    return sorted(x[0] for x in s["outputs"]["lead"])


def audio_snap(name, kind, win=0.6):
    for s in SONGS:
        bs = candidates(s, kind)
        on = []
        for t in src(name, s["id"]):
            i = bisect.bisect_left(bs, t)
            c = [bs[j] for j in (i - 1, i) if 0 <= j < len(bs) and abs(bs[j] - t) <= win * s["spacing"]]
            on.append(min(c, key=lambda b: abs(b - t)) if c else t)
        write(f"{name}+{kind}snap", s["id"], on)


def main():
    raw_ctc()
    for n in ["ctc-" + a for a in CTC[:4]] + ["mms-vowel", "sofa-vowel"]:
        debias(n)
    ensemble(M3, "ens3")
    ensemble(M5, "ens5")
    for k in ("game", "sona", "voice", "out"):
        audio_snap("ens3", k)
    prior("ens3", "ens3+prior")
    prior("ens5", "ens5+prior")
    adaptive(M3, "m3+adaptmora")
    adaptive(M5, "m5+adaptmora")
    for k in (1.5, 3.0):
        abstain("m3+adaptmora", M3, f"m3+adaptmora+abst{k}", k)
        abstain("m5+adaptmora", M5, f"m5+adaptmora+abst{k}", k)


if __name__ == "__main__":
    main()
