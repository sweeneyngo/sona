"""Transcription-time evidence for a cut between a short piece S and its touching neighbour N with a small
pitch step: where sona's pitch track and GAME's unrounded pitch put S, as a fraction of the step
(0 = at N's pitch, 1 = at S's own), and whether GAME's run on the full vocals cuts there too.
Prints the separation (AUC) of truly chopped cuts from true boundaries within each candidate pool."""
import math
import wob
import cands as C
L = wob.L


def tune_of(raw):
    x = y = 0.0
    for a, b, m, v in raw:
        if v:
            f = m - round(m)
            x += math.cos(2 * math.pi * f)
            y += math.sin(2 * math.pi * f)
    return -math.atan2(y, x) / (2 * math.pi) if (x or y) else 0.0


def snapped(s, raw, tune):
    sp, ph = s["spacing"], s["onset_phase"]
    out = []
    for a, b, m, v in raw:
        if not v:
            continue
        on, off = round((a - ph) / sp), round((b - ph) / sp)
        if off <= on:
            off = on + 1
        out.append((on, off, m + tune))
    return out


def best_overlap(spans, a, b):
    best, cov = None, 0
    for x in spans:
        c = min(x[1], b) - max(x[0], a)
        if c > cov:
            best, cov = x, c
    return best


def features(s, g, rows, tr=None):
    tr = tr or wob.Track(s)
    tune = tune_of(s["stage_detail"]["game_lead"])
    lead = snapped(s, s["stage_detail"]["game_lead"], tune)
    voc_tune = tune_of(s["stage_detail"]["game_vocals"])
    voc = snapped(s, s["stage_detail"]["game_vocals"], voc_tune)
    voc_cuts = {x[0] for x in voc} | {x[1] for x in voc}
    for r in rows:
        i = r["i"]
        a, b = g[i - 1], g[i]
        S, N = (a, b) if r["la"] <= r["lb"] else (b, a)
        r["short_first"] = S is a
        r["ls"], r["ln"] = S[1] - S[0], N[1] - N[0]
        step = S[2] - N[2]
        fs = [m + tune for m in tr.frames(L.t_of(s, S[0]), L.t_of(s, S[1]))]
        ms = wob.median(fs)
        r["x_f0"] = None if ms is None else (ms - N[2]) / step
        fn = [m + tune for m in tr.frames(L.t_of(s, N[0]), L.t_of(s, N[1]))]
        mn = wob.median(fn)
        r["xn_f0"] = None if mn is None else (mn - N[2]) / step
        rs = best_overlap(lead, S[0], S[1])
        r["x_raw"] = None if rs is None else (rs[2] - N[2]) / step
        r["voc_cut"] = b[0] in voc_cuts
        # pitch-track motion across S toward N: mean of the last third minus the first third, along the step
        if len(fs) >= 6:
            k = len(fs) // 3
            mv = (sum(fs[-k:]) - sum(fs[:k])) / k
            r["toward"] = (-mv if r["short_first"] else mv) * (1 if step > 0 else -1) * -1
        else:
            r["toward"] = None
    return rows


def auc(pos, neg):
    if not pos or not neg:
        return float("nan")
    w = 0.0
    for p in pos:
        for n in neg:
            w += 1.0 if p > n else 0.5 if p == n else 0.0
    return w / (len(pos) * len(neg))


if __name__ == "__main__":
    songs = wob.load()
    for out in ("lead", "stage"):
        R = []
        for s in songs:
            g = wob.grid(s, out)
            R += features(s, g, C.candidates(s, g))
        pools = {
            "|step| 1-2, shorter piece 1 cell": [r for r in R if abs(r["iv"]) <= 2 and r["ls"] == 1],
            "|step| 1-2, shorter piece 2 cells": [r for r in R if abs(r["iv"]) <= 2 and r["ls"] == 2],
            "|step| 1-2, shorter 1 cell, after a rest (head)": [r for r in R if abs(r["iv"]) <= 2 and r["ls"] == 1 and r["short_first"] and r["after_rest"]],
            "|step| 3+, first piece 1 cell": [r for r in R if abs(r["iv"]) > 2 and r["la"] == 1],
        }
        print(f"== {out}")
        for name, P in pools.items():
            ch = [r for r in P if r["label"] == "chopped"]
            bd = [r for r in P if r["label"] == "boundary"]
            print(f"  {name}: chopped {len(ch)}, boundary {len(bd)}")
            for f, sign in (("x_f0", -1), ("xn_f0", -1), ("x_raw", -1), ("toward", 1)):
                pc = [sign * r[f] for r in ch if r[f] is not None]
                pb = [sign * r[f] for r in bd if r[f] is not None]
                print(f"    {f:7} AUC {auc(pc, pb):.3f}  (missing: chopped {len(ch) - len(pc)}, boundary {len(bd) - len(pb)})")
            pc = [0 if r["voc_cut"] else 1 for r in ch]
            pb = [0 if r["voc_cut"] else 1 for r in bd]
            print(f"    no vocals cut AUC {auc(pc, pb):.3f}; vocals cut there: chopped {sum(1 - x for x in pc)}/{len(pc)}, boundary {sum(1 - x for x in pb)}/{len(pb)}")
            pc = [1 if r["short_first"] else 0 for r in ch]
            pb = [1 if r["short_first"] else 0 for r in bd]
            print(f"    short piece first: chopped {sum(pc)}/{len(pc)}, boundary {sum(pb)}/{len(pb)}")
