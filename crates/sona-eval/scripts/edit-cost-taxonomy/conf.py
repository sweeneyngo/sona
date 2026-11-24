"""Note confidence for the learned cell stage (utau-edit-cost 13): which per-note signal finds the edit cost.

usage: python3 -I conf.py DEV_DUMP [--model] [--per-song]   every signal's curve, LOSO models, the decision table
       python3 -I conf.py DUMP --fit-on DEV_DUMP          the winner fitted on dev, scored on another split
       python3 -I conf.py DUMP --ported-check             a dump from the ported Rust equals the prototype

Each shipped (stage) note gets features computed from what `cell_stage_notes` has, and 03's attributed cost
(an equal share of its component's cost). Each song flags its lowest-scored x% of notes; the pooled share of
attributable cost caught is the metric (selection rule in issues/13-note-confidence.md).
"""
import json, math, os, random, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ec
from taxo import comp_label

FRACS = (5, 10, 20, 30, 40, 50)
# The ported score's weights, as in crates/sona-core/src/cellstage.rs (CONFIDENCE_WEIGHTS): the winner fitted
# on all dev songs (`--fit-on` prints the same numbers).
PORTED = (2.273766350383677, -4.442041607198855, 0.581798590291169)


def ported_confidence(top_probs, cells):
    """`note_confidence` exactly as Rust computes it: f32 probabilities, an in-order f64 sum, an f32 result."""
    acc = 0.0
    for v in top_probs:
        acc += float(np.float32(v))
    mean = acc / len(top_probs)
    b0, b_prob, b_len = PORTED
    return float(np.float32(1.0 / (1.0 + math.exp(b0 + b_prob * mean + b_len * math.log(cells)))))
RATE_WINDOW_S = 2.0


def load(d):
    return [json.load(open(os.path.join(d, f))) for f in sorted(os.listdir(d)) if f.endswith(".json")]


def rround(x):
    return ec.rround(x)


def semitones(hz):
    return 69 + 12 * math.log2(hz / 440.0)


def note_rows(song):
    """One row per stage note inside the UST span: features, attributed cost, label."""
    sp, an_ust, ph, div = song["spacing"], song["ust_anchor"], song["onset_phase"], song["division"]
    det = song["stage_detail"]
    k0, probs, classes, cands, sources = det["k0"], det["probs"], det["classes"], det["cands"], det["sources"]
    stage = song["outputs"]["stage"]
    A = ec.analyse(song["reference"], stage, sp, an_ust, div)
    comp_of = {}
    for c in A["comps"]:
        for o in c["outs"]:
            comp_of[o] = c
    shipped_conf = {(rround((on - an_ust) / sp), p): cf for on, off, p, cf, _ in det["confidence"]}

    def cell(t):
        return rround((t - ph) / sp) - k0

    sona_starts = sorted(cell(n[0]) for n in song["sona_onset"])
    sona_on_s = sorted(n[0] for n in song["sona_onset"])
    lead_starts = sorted(cell(n[0]) for n in det["game_lead"] if n[3])
    voc_starts = sorted(cell(n[0]) for n in det["game_vocals"] if n[3])
    voc_on_s = sorted(n[0] for n in det["game_vocals"] if n[3])
    f0 = song["f0"]
    f0_t = np.array([f[0] for f in f0])
    f0_hz = np.array([f[1] for f in f0])
    rms_t = np.array([r[0] for r in song["rms"]])
    rms_v = np.array([r[1] for r in song["rms"]])
    import bisect

    def count_in(xs, lo, hi):  # lo < x < hi
        return bisect.bisect_left(xs, hi) - bisect.bisect_right(xs, lo)

    def count_in_s(xs, lo, hi):  # lo <= x < hi
        return bisect.bisect_left(xs, hi) - bisect.bisect_left(xs, lo)

    rows = []
    for oi, o in enumerate(A["outs"]):
        on, off, pitch = stage[o[3]]
        c0, c1 = cell(on), cell(off)
        cs = range(c0, c1)
        pmax = [max(probs[c]) for c in cs]
        pch = [probs[c][classes[c]] for c in cs]
        n_cells = c1 - c0
        mid = 0.5 * (on + off)
        lo_w, hi_w = mid - RATE_WINDOW_S, mid + RATE_WINDOW_S
        rate_sona = count_in_s(sona_on_s, lo_w, hi_w) / (2 * RATE_WINDOW_S)
        rate_voc = count_in_s(voc_on_s, lo_w, hi_w) / (2 * RATE_WINDOW_S)
        restarts = 0
        for c in range(c0 + 1, c1):
            if any(cands[c][j] >= 0 and cands[c - 1][j] >= 0 and sources[c][j] != sources[c - 1][j] for j in (1, 2, 3)):
                restarts += 1
        i0, i1 = np.searchsorted(f0_t, on), np.searchsorted(f0_t, off)
        hz = f0_hz[i0:i1]
        voiced = hz[hz > 0]
        if len(voiced) >= 3:
            st = 69 + 12 * np.log2(voiced / 440.0)
            f0_dev = float(np.sqrt(np.mean((st - pitch) ** 2)))
            f0_std = float(np.std(st))
        else:
            f0_dev = f0_std = 12.0
        vfrac = len(voiced) / max(1, len(hz))
        j0, j1 = np.searchsorted(rms_t, on + 0.02), np.searchsorted(rms_t, off - 0.02)
        seg = rms_v[j0:j1]
        if len(seg) >= 5:
            peak = float(seg.max())
            # deepest valley between two higher points: min over interior of min(left max, right max) / value
            lmax = np.maximum.accumulate(seg)
            rmax = np.maximum.accumulate(seg[::-1])[::-1]
            depth = np.minimum(lmax, rmax) / np.maximum(seg, 1e-9)
            dip_db = float(20 * np.log10(max(1.0, depth.max())))
        else:
            dip_db = 0.0
        comp = comp_of[oi]
        if len(comp["refs"]) <= 1 and len(comp["outs"]) == 1:
            own = comp["cost"]
        else:
            # the note's own joins, plus an equal share of each chopped UST note it is a piece of
            own = 4 * (len(A["adj_o"][oi]) - 1) + sum(4 * (len(A["adj_r"][ri]) - 1) / len(A["adj_r"][ri]) for ri in A["adj_o"][oi])
        rows.append(dict(
            song=song["id"], cost=comp["cost"] / len(comp["outs"]), own_cost=own, label=comp_label(comp),
            conf=shipped_conf.get((o[0], o[2]), 1.0), ported=ported_confidence(pmax, n_cells),
            pmean=sum(pmax) / len(pmax), pmin=min(pmax), pch=sum(pch) / len(pch), pchmin=min(pch),
            cells=n_cells, beats=n_cells * 4 / song["division"], secs=off - on,
            rate_sona=rate_sona, rate_voc=rate_voc,
            moras_sona=(off - on) * rate_sona, moras_voc=(off - on) * rate_voc,
            in_sona=count_in(sona_starts, c0, c1), in_lead=count_in(lead_starts, c0, c1), in_voc=count_in(voc_starts, c0, c1),
            restarts=restarts, f0_dev=f0_dev, f0_std=f0_std, vfrac=vfrac, dip_db=dip_db,
            lead_share=sum(1 for c in cs if classes[c] == 1) / n_cells,
            onset_lead=int(c0 in set(lead_starts)),
        ))
    return rows


# doubt(row): higher = flag first. Directions fixed by meaning, not fitted.
SIGNALS = {
    "shipped confidence": lambda r: -r["conf"],
    "stage mean class prob": lambda r: -r["pmean"],
    "ported score (all-dev fit, f32)": lambda r: -r["ported"],
    "stage min class prob": lambda r: -r["pmin"],
    "chosen-class prob, mean": lambda r: -r["pch"],
    "length (cells)": lambda r: r["cells"],
    "length (s)": lambda r: r["secs"],
    "notes/s nearby (sona)": lambda r: r["rate_sona"],
    "notes/s nearby (GAME vocals)": lambda r: r["rate_voc"],
    "expected moras (s x sona rate)": lambda r: r["moras_sona"],
    "expected moras (s x vocals rate)": lambda r: r["moras_voc"],
    "sona starts inside": lambda r: r["in_sona"],
    "GAME lead starts inside": lambda r: r["in_lead"],
    "GAME vocals starts inside": lambda r: r["in_voc"],
    "source restarts inside": lambda r: r["restarts"],
    "pitch track off the note (st)": lambda r: r["f0_dev"],
    "pitch track spread (st)": lambda r: r["f0_std"],
    "unvoiced share": lambda r: 1 - r["vfrac"],
    "energy dip inside (dB)": lambda r: r["dip_db"],
    "not GAME lead's pitch": lambda r: 1 - r["lead_share"],
    "onset not a GAME lead onset": lambda r: 1 - r["onset_lead"],
}


def caught(rows_by_song, doubt, frac, label=None, seed=7):
    """Per song: (cost caught, attributable cost) when the song flags its top-`frac`% doubtful notes."""
    out = []
    for sid, rows in rows_by_song.items():
        rng = random.Random(f"{seed}:{sid}")
        keyed = sorted(((doubt(r), rng.random(), i) for i, r in enumerate(rows)), reverse=True)
        k = len(rows) * frac // 100
        sel = [rows[i] for _, _, i in keyed[:k]]
        f = (lambda r: label in r["label"]) if label else (lambda r: True)
        out.append((sum(r["cost"] for r in sel if f(r)), sum(r["cost"] for r in rows if f(r))))
    return out


def pooled(pairs):
    return sum(a for a, _ in pairs) / max(1e-9, sum(b for _, b in pairs))


def boot_diff(pa, pb, iters=10000, seed=1):
    """Song-bootstrap 95% interval of pooled(pa) - pooled(pb), paired by song."""
    rng = random.Random(seed)
    n = len(pa)
    v = []
    for _ in range(iters):
        idx = [rng.randrange(n) for _ in range(n)]
        v.append(pooled([pa[i] for i in idx]) - pooled([pb[i] for i in idx]))
    v.sort()
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def boot_one(pa, iters=10000, seed=1):
    rng = random.Random(seed)
    n = len(pa)
    v = sorted(pooled([pa[rng.randrange(n)] for _ in range(n)]) for _ in range(iters))
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def random_pairs(rows_by_song, frac):
    """Random flags' expected catch: the flagged share of notes times the song's cost."""
    return [((len(rows) * frac // 100) / len(rows) * sum(r["cost"] for r in rows), sum(r["cost"] for r in rows)) for rows in rows_by_song.values()]


# ---- the tiny logistic model ----

def fit_logistic(X, y, sw=None, l2=1e-2, iters=100):
    X1 = np.hstack([X, np.ones((len(X), 1))])
    sw = np.ones(len(X)) if sw is None else sw
    w = np.zeros(X1.shape[1])
    for _ in range(iters):
        p = 1 / (1 + np.exp(-X1 @ w))
        g = X1.T @ (sw * (p - y)) + l2 * np.r_[w[:-1], 0]
        H = (X1.T * (sw * p * (1 - p))) @ X1 + l2 * np.diag(np.r_[np.ones(len(w) - 1), 0])
        step = np.linalg.solve(H, g)
        w -= step
        if np.abs(step).max() < 1e-10:
            break
    return w


def features(r, names):
    return [FEATS[n](r) for n in names]


FEATS = {
    "pmean": lambda r: r["pmean"],
    "pmin": lambda r: r["pmin"],
    "log_cells": lambda r: math.log(r["cells"]),
    "log_beats": lambda r: math.log(r["beats"]),
    "log_moras_sona": lambda r: math.log(max(r["moras_sona"], 0.05)),
    "rate_sona": lambda r: r["rate_sona"],
    "rate_voc": lambda r: r["rate_voc"],
    "in_sona": lambda r: r["in_sona"],
    "restarts": lambda r: r["restarts"],
    "f0_dev": lambda r: min(r["f0_dev"], 3.0),
    "dip_db": lambda r: min(r["dip_db"], 30.0),
    "lead_share": lambda r: r["lead_share"],
}


TARGETS = {
    "any error": (lambda r: r["cost"] > 0, lambda r: 1.0),
    "joined or chopped": (lambda r: "joined" in r["label"] or "chopped" in r["label"], lambda r: 1.0),
    "cost-weighted": (lambda r: r["cost"] > 0, lambda r: max(r["cost"], 1.0)),
}


def loso_model(rows_by_song, names, target="any error"):
    """Leave-one-song-out: every song's notes scored by a model fitted on the other songs. Returns
    {song: [p(error) per row]} and the all-song fit."""
    preds = {}
    for held in rows_by_song:
        train = [r for s, rs in rows_by_song.items() if s != held for r in rs]
        X = np.array([features(r, names) for r in train])
        mu, sd = X.mean(0), X.std(0) + 1e-9
        y, wt = TARGETS[target]
        w = fit_logistic((X - mu) / sd, np.array([float(y(r)) for r in train]), np.array([wt(r) for r in train]))
        Xh = (np.array([features(r, names) for r in rows_by_song[held]]) - mu) / sd
        preds[held] = list(1 / (1 + np.exp(-(Xh @ w[:-1] + w[-1]))))
    allr = [r for rs in rows_by_song.values() for r in rs]
    X = np.array([features(r, names) for r in allr])
    mu, sd = X.mean(0), X.std(0) + 1e-9
    y, wt = TARGETS[target]
    w = fit_logistic((X - mu) / sd, np.array([float(y(r)) for r in allr]), np.array([wt(r) for r in allr]))
    return preds, (mu, sd, w)


def curve_line(name, rows_by_song, doubt):
    cells = []
    for f in FRACS:
        cells.append(f"{100 * pooled(caught(rows_by_song, doubt, f)):5.1f}")
    j = 100 * pooled(caught(rows_by_song, doubt, 30, "joined"))
    c = 100 * pooled(caught(rows_by_song, doubt, 30, "chopped"))
    return f"  {name:34} " + " ".join(cells) + f"   | 30%: joined {j:5.1f} chopped {c:5.1f}"


def rank_blend(rows_by_song, keys):
    """Per song, the mean of each signal's percentile rank (no fit): {id(row): doubt}."""
    out = {}
    for rows in rows_by_song.values():
        acc = [0.0] * len(rows)
        for k in keys:
            f = SIGNALS[k]
            order = sorted(range(len(rows)), key=lambda i: f(rows[i]))
            for rank, i in enumerate(order):
                acc[i] += rank / len(rows)
        for r, a in zip(rows, acc):
            out[id(r)] = a
    return out


def decision(by_song, candidates):
    base = SIGNALS["stage mean class prob"]
    pb, pr = caught(by_song, base, 30), random_pairs(by_song, 30)
    print(f"  30% flagged, song bootstrap 95% (10 000, paired): catch | minus class prob | minus random")
    for name, f in candidates.items():
        pa = caught(by_song, f, 30)
        lo, hi = boot_one(pa)
        dlo, dhi = boot_diff(pa, pb)
        rlo, rhi = boot_diff(pa, pr)
        a = pooled(pa)
        win = dlo > 0 and rlo > 0
        print(f"  {name:40} {100 * a:5.1f} [{100 * lo:.1f}, {100 * hi:.1f}] | {100 * (a - pooled(pb)):+5.1f} [{100 * dlo:+.1f}, {100 * dhi:+.1f}] | {100 * (a - pooled(pr)):+5.1f} [{100 * rlo:+.1f}, {100 * rhi:+.1f}]{'  WINS' if win else ''}")


WINNER = (["pmean", "log_cells"], "joined or chopped")


def model_doubt(fit, names):
    mu, sd, w = fit
    return lambda r: float(((np.array(features(r, names)) - mu) / sd) @ w[:-1] + w[-1])


def apply_from(dev_dir, d):
    """Fit the winner on every dev song, score the songs in `d` (diagnosis of a spent split)."""
    dev = {s["id"]: note_rows(s) for s in load(dev_dir)}
    names, target = WINNER
    _, fit = loso_model(dev, names, target)
    mu, sd, w = fit
    raw = w[:-1] / sd
    print(f"winner fitted on all {len(dev)} dev songs: logit = {w[-1] - (raw * mu).sum():+.4f} " + " ".join(f"{c:+.4f}*{n}" for c, n in zip(raw, names)))
    by_song = {s["id"]: note_rows(s) for s in load(d)}
    print(f"{len(by_song)} songs, {sum(len(v) for v in by_song.values())} stage notes")
    print(f"  {'signal (per-song flags)':34} " + " ".join(f"{f:>4}%" for f in FRACS))
    print(f"  {'random':34} " + " ".join(f"{100 * pooled(random_pairs(by_song, f)):5.1f}" for f in FRACS))
    for name in ("shipped confidence", "stage mean class prob", "length (cells)", "expected moras (s x vocals rate)"):
        print(curve_line(name, by_song, SIGNALS[name]))
    doubt = model_doubt(fit, names)
    print(curve_line("winner (fitted on dev)", by_song, doubt))
    decision(by_song, {"length (cells)": SIGNALS["length (cells)"], "winner (fitted on dev)": doubt})


def robustness(by_song, doubt):
    """The winner under the note's own cost (not the component's equal share) and under one global ranking."""
    own = {s: [dict(r, cost=r["own_cost"]) for r in rs] for s, rs in by_song.items()}
    keymap = {}
    for s, rs in by_song.items():
        for r, o in zip(rs, own[s]):
            keymap[id(o)] = doubt(r)
    print("  robustness: each note's own cost (its joins + a share of each UST note it chops)")
    for name in ("stage mean class prob", "length (cells)"):
        print(curve_line(name, own, SIGNALS[name]))
    print(curve_line("winner", own, lambda r: keymap[id(r)]))
    decision(own, {"winner": lambda r: keymap[id(r)]})
    allr = [r for v in by_song.values() for r in v]
    tot = sum(r["cost"] for r in allr)
    rng = random.Random(7)
    print("  robustness: one ranking over all songs (a fixed threshold), 30% flagged")
    for name, f in (("stage mean class prob", SIGNALS["stage mean class prob"]), ("length (cells)", SIGNALS["length (cells)"]), ("winner", doubt)):
        order = sorted(allr, key=lambda r: (-f(r), rng.random()))
        print(f"    {name:30} {100 * sum(r['cost'] for r in order[: len(order) * 30 // 100]) / tot:5.1f}")


def ported_check(d):
    """A `--dump` written by the ported Rust: its shipped confidence must equal the prototype's score on every
    note, and so give the same curve."""
    by_song = {s["id"]: note_rows(s) for s in load(d)}
    rows = [r for v in by_song.values() for r in v]
    off = [r for r in rows if float(np.float32(r["conf"])) != r["ported"]]
    print(f"{len(rows)} notes; shipped confidence != prototype's ported score on {len(off)}")
    for r in off[:5]:
        print("  ", r["song"], r["conf"], r["ported"])
    for name in ("shipped confidence", "ported score (all-dev fit, f32)"):
        print(curve_line(name, by_song, SIGNALS[name]))
    return 1 if off else 0


def main():
    if "--ported-check" in sys.argv:
        return ported_check(sys.argv[1])
    if "--fit-on" in sys.argv:
        return apply_from(sys.argv[sys.argv.index("--fit-on") + 1], sys.argv[1])
    d = sys.argv[1]
    songs = load(d)
    by_song = {s["id"]: note_rows(s) for s in songs}
    n = sum(len(v) for v in by_song.values())
    print(f"{len(songs)} songs, {n} stage notes in the UST span; attributable cost {sum(r['cost'] for v in by_song.values() for r in v):.0f}")
    print(f"  {'signal (per-song flags)':34} " + " ".join(f"{f:>4}%" for f in FRACS))
    print(f"  {'random':34} " + " ".join(f"{100 * pooled(random_pairs(by_song, f)):5.1f}" for f in FRACS))
    for name, f in SIGNALS.items():
        print(curve_line(name, by_song, f))
    # 03's global ranking, as a reproduction check
    allr = [r for v in by_song.values() for r in v]
    rng = random.Random(7)
    for name in ("shipped confidence", "stage mean class prob"):
        f = SIGNALS[name]
        order = sorted(allr, key=lambda r: (-f(r), rng.random()))
        tot = sum(r["cost"] for r in allr)
        print(f"  global (03) {name:22} 30%: {100 * sum(r['cost'] for r in order[: len(order) * 30 // 100]) / tot:5.1f}")
    cands = {k: SIGNALS[k] for k in ("stage min class prob", "length (cells)", "length (s)", "expected moras (s x sona rate)", "expected moras (s x vocals rate)", "energy dip inside (dB)", "source restarts inside")}
    for keys in (("length (cells)", "stage mean class prob"), ("length (s)", "stage mean class prob"), ("expected moras (s x vocals rate)", "stage mean class prob")):
        rb = rank_blend(by_song, keys)
        name = "rank blend: " + " + ".join(keys)
        print(curve_line(name, by_song, lambda r, rb=rb: rb[id(r)]))
        cands[name] = lambda r, rb=rb: rb[id(r)]
    if "--model" in sys.argv:
        for target in TARGETS:
            print(f"  -- LOSO logistic, target {target}")
            for names in MODEL_SETS:
                preds, (mu, sd, w) = loso_model(by_song, names, target)
                lookup = {id(r): p for s, rs in by_song.items() for r, p in zip(rs, preds[s])}
                print(curve_line("+".join(names), by_song, lambda r, lookup=lookup: lookup[id(r)]))
                if names == ["pmean", "log_cells"]:
                    cands[f"LOSO pmean+log_cells ({target})"] = lambda r, lookup=lookup: lookup[id(r)]
    decision(by_song, cands)
    if "--model" in sys.argv:
        names, target = WINNER
        preds, _ = loso_model(by_song, names, target)
        lookup = {id(r): p for s, rs in by_song.items() for r, p in zip(rs, preds[s])}
        robustness(by_song, lambda r: lookup[id(r)])
        if "--per-song" in sys.argv:
            print("  per song, 30% flagged: class prob / winner (LOSO), share of the song's cost")
            for sid, rows in by_song.items():
                one = {sid: rows}
                print(f"    {sid:28} {100 * pooled(caught(one, SIGNALS['stage mean class prob'], 30)):5.1f} {100 * pooled(caught(one, lambda r: lookup[id(r)], 30)):5.1f}")


MODEL_SETS = [
    ["pmean", "log_cells"],
    ["pmin", "log_cells"],
    ["pmean", "log_moras_sona"],
    ["pmean", "log_cells", "rate_sona"],
    ["pmean", "log_cells", "dip_db"],
    ["pmean", "log_cells", "restarts"],
    ["pmean", "log_cells", "rate_sona", "dip_db"],
]

if __name__ == "__main__":
    sys.exit(main())
