"""Mora error rate of transcribed lyrics against the lead UST's moras (lyrics-first 05).

    python score.py refs DUMP_DIR REF_DIR        # UST notes (sona-eval dumps) -> REF_DIR/<id>.json
    python score.py text LYRIC_DIR REF_DIR OUT_DIR [--kakasi|--openjtalk]   # pasted lyric text
    python score.py asr TRANSCRIPT_DIR REF_DIR OUT_DIR [--kakasi|--openjtalk]
    python score.py table REF_DIR OUT_DIR...      # pooled MER, song-bootstrap 95% CI, per-song numbers
    python score.py paired REF_DIR A_DIR B_DIR    # MER of B minus A, paired song bootstrap
    python score.py selftest

The reference is every lead-UST note (all lead parts), sorted by onset, read by `moras.ust_lyric_moras`.
MER = (substitutions + insertions + deletions) / reference moras, from a Levenshtein alignment. The
`table` and `paired` modes print numbers only; everything else this reads or writes is lyric-derived
and must stay out of the repo."""
import glob
import json
import os
import random
import statistics
import sys
import unicodedata

import moras as M


def align(ref, hyp):
    """Levenshtein alignment: (S, I, D, pairs, ops); pairs are matched (ref_i, hyp_j) with equal moras,
    ops the edit string in order (= match, S, I, D)."""
    n, m = len(ref), len(hyp)
    d = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n + 1):
        d[i][0] = i
    for j in range(m + 1):
        d[0][j] = j
    for i in range(1, n + 1):
        ri, row, prev = ref[i - 1], d[i], d[i - 1]
        for j in range(1, m + 1):
            row[j] = min(prev[j - 1] + (ri != hyp[j - 1]), prev[j] + 1, row[j - 1] + 1)
    s = ins = dels = 0
    pairs, ops = [], []
    i, j = n, m
    while i or j:
        if i and j and d[i][j] == d[i - 1][j - 1] + (ref[i - 1] != hyp[j - 1]):
            if ref[i - 1] == hyp[j - 1]:
                pairs.append((i - 1, j - 1))
                ops.append("=")
            else:
                s += 1
                ops.append("S")
            i, j = i - 1, j - 1
        elif i and d[i][j] == d[i - 1][j] + 1:
            dels, i = dels + 1, i - 1
            ops.append("D")
        else:
            ins, j = ins + 1, j - 1
            ops.append("I")
    return s, ins, dels, pairs[::-1], "".join(ops[::-1])


def runs(ops):
    """Lengths of the maximal runs of edits (S/I/D) in an edit string."""
    return [len(r) for r in ops.replace("=", " ").split()]


def refs(dump_dir, ref_dir):
    os.makedirs(ref_dir, exist_ok=True)
    for p in sorted(glob.glob(os.path.join(dump_dir, "*.json"))):
        d = json.load(open(p))
        notes = sorted((n for n in d["stage_detail"]["ust"] if n["kind"] == "Note"), key=lambda n: n["on"])
        out = [[m, n["on"], n["part"]] for n in notes for m in M.ust_lyric_moras(n["lyric"])]
        json.dump({"id": d["id"], "moras": out}, open(os.path.join(ref_dir, d["id"] + ".json"), "w"),
                  ensure_ascii=False)
        print(d["id"], len(out), "moras")


def score_one(ref, hyp, unread, extra=None):
    s, i, d, pairs, ops = align([r[0] for r in ref], [h[0] for h in hyp])
    rec = {"n_ref": len(ref), "n_hyp": len(hyp), "S": s, "I": i, "D": d, "unread": unread, "runs": runs(ops),
           "minutes": (ref[-1][1] - ref[0][1]) / 60 if ref and len(ref[0]) > 1 else None}
    rec.update(extra or {})
    return rec, pairs


def text(lyric_dir, ref_dir, out_dir, other):
    read = other or M.read_moras
    os.makedirs(out_dir, exist_ok=True)
    for rp in sorted(glob.glob(os.path.join(ref_dir, "*.json"))):
        ref = json.load(open(rp))
        sid = ref["id"]
        src = os.path.join(lyric_dir, sid + ".readings.txt")
        if not os.path.exists(src):
            src = os.path.join(lyric_dir, sid + ".txt")
        lines = [ln for ln in open(src, encoding="utf-8").read().splitlines() if not ln.startswith("#")]
        hyp, unread = read("\n".join(lines))
        rec, _ = score_one(ref["moras"], [[h] for h in hyp], unread, {"source": os.path.basename(src)})
        json.dump(rec, open(os.path.join(out_dir, sid + ".json"), "w"))


def asr(tr_dir, ref_dir, out_dir, other):
    os.makedirs(out_dir, exist_ok=True)
    for tp in sorted(glob.glob(os.path.join(tr_dir, "*.json"))):
        tr = json.load(open(tp))
        sid = tr["id"]
        ref = json.load(open(os.path.join(ref_dir, sid + ".json")))["moras"]
        hyp, unread = [], 0
        for seg in tr["segments"]:
            # read the segment as one text (a word alone may be one kanji, read wrongly out of context),
            # then give each mora the start of the timestamped word holding its token's first character
            words = seg["words"] or [{"start": seg["start"], "word": seg["text"]}]
            text, owner = "", []
            for w in words:
                piece = unicodedata.normalize("NFKC", w["word"])
                text += piece
                owner += [w["start"]] * len(piece)
            if other:
                ms, u = other(text)
                hyp += [[m, None, False] for m in ms]
                unread += u
                continue
            toks = M.read_tokens(text)
            unread += sum(u for _, _, u in toks)
            ms, tags = M.tagged_moras([(k, t) for t, (_, k, _) in enumerate(toks)])
            for k, (m, t) in enumerate(zip(ms, tags)):
                start = toks[t][0]
                initial = k == 0 or (bool(seg["words"]) and tags[k - 1] != t)
                hyp.append([m, owner[start] if start < len(owner) else seg["start"], initial])
        rec, pairs = score_one(ref, hyp, unread, {k: tr[k] for k in ("audio_s", "wall_s", "cpu_s",
                                                                     "load_start", "load_end", "threads")})
        # timing of correctly recognised word-initial moras against the UST note onset
        errs = [hyp[j][1] - ref[i][1] for i, j in pairs if hyp[j][2] and hyp[j][1] is not None]
        rec["onset_err"] = errs
        chars = [c for c in unicodedata.normalize("NFKC", tr["text"]) if c.isalnum()]
        rec["kanji_share"] = sum("一" <= c <= "鿿" or c == "々" for c in chars) / max(1, len(chars))
        json.dump(rec, open(os.path.join(out_dir, sid + ".json"), "w"))


def boot(recs, reps=10000, seed=5):
    rnd = random.Random(seed)
    ids = list(recs)
    vals = []
    for _ in range(reps):
        pick = [recs[rnd.choice(ids)] for _ in ids]
        vals.append(sum(r["S"] + r["I"] + r["D"] for r in pick) / sum(r["n_ref"] for r in pick))
    vals.sort()
    return vals[int(0.025 * reps)], vals[int(0.975 * reps) - 1]


def paired(ref_dir, a_dir, b_dir, reps=10000, seed=5):
    """Pooled MER of B minus A over the songs both have, with a paired song-bootstrap 95% interval."""
    ids = [os.path.basename(p)[:-5] for p in sorted(glob.glob(os.path.join(ref_dir, "*.json")))]
    ids = [s for s in ids if all(os.path.exists(os.path.join(d, s + ".json")) for d in (a_dir, b_dir))]
    A = {s: json.load(open(os.path.join(a_dir, s + ".json"))) for s in ids}
    B = {s: json.load(open(os.path.join(b_dir, s + ".json"))) for s in ids}

    def diff(pick):
        n = sum(A[s]["n_ref"] for s in pick)
        return (sum(B[s]["S"] + B[s]["I"] + B[s]["D"] - A[s]["S"] - A[s]["I"] - A[s]["D"] for s in pick)) / n
    rnd = random.Random(seed)
    vals = sorted(diff([rnd.choice(ids) for _ in ids]) for _ in range(reps))
    lo, hi = vals[int(0.025 * reps)], vals[int(0.975 * reps) - 1]
    print(f"{os.path.basename(os.path.normpath(b_dir))} − {os.path.basename(os.path.normpath(a_dir))}: "
          f"{diff(ids):+.1%} [{lo:+.1%}, {hi:+.1%}] over {len(ids)} songs")


def table(ref_dir, out_dirs):
    ids = sorted(os.path.basename(p)[:-5] for p in glob.glob(os.path.join(ref_dir, "*.json")))
    print("| run | songs | ref moras | MER | 95% CI | S | I | D | unread chars | xRT wall | xRT cpu |"
          " onset |err| median s | within 0.5 s | edits/min | fix spots/min | edits in runs ≥ 5 |")
    print("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    per = {}
    for od in out_dirs:
        recs = {sid: json.load(open(os.path.join(od, sid + ".json")))
                for sid in ids if os.path.exists(os.path.join(od, sid + ".json"))}
        if not recs:
            continue
        n = sum(r["n_ref"] for r in recs.values())
        S, I, D = (sum(r[k] for r in recs.values()) for k in "SID")
        lo, hi = boot(recs)
        xw = xc = med = within = ""
        if all("wall_s" in r for r in recs.values()):
            a = sum(r["audio_s"] for r in recs.values())
            xw = f"{sum(r['wall_s'] for r in recs.values()) / a:.2f}"
            xc = f"{sum(r['cpu_s'] for r in recs.values()) / a:.2f}"
            errs = [abs(e) for r in recs.values() for e in r["onset_err"]]
            if errs:
                med = f"{statistics.median(errs):.2f}"
                within = f"{sum(e <= 0.5 for e in errs) / len(errs):.0%}"
        name = os.path.basename(os.path.normpath(od))
        mins = sum(r["minutes"] for r in recs.values())
        rl = [x for r in recs.values() for x in r["runs"]]
        print(f"| {name} | {len(recs)} | {n} | {(S + I + D) / n:.1%} | {lo:.1%}–{hi:.1%} | {S / n:.1%} |"
              f" {I / n:.1%} | {D / n:.1%} | {sum(r['unread'] for r in recs.values())} | {xw} | {xc} |"
              f" {med} | {within} | {(S + I + D) / mins:.0f} | {len(rl) / mins:.0f} |"
              f" {sum(x for x in rl if x >= 5) / max(1, sum(rl)):.0%} |")
        per[name] = {sid: (r["S"] + r["I"] + r["D"]) / r["n_ref"] for sid, r in recs.items()}
    print()
    print("| song | " + " | ".join(per) + " |")
    print("|---|" + "---|" * len(per))
    for sid in ids:
        print(f"| {sid} | " + " | ".join(f"{per[r][sid]:.1%}" if sid in per[r] else "" for r in per) + " |")


if __name__ == "__main__":
    mode, args = sys.argv[1], [a for a in sys.argv[2:] if not a.startswith("--")]
    kak = M.kakasi_moras if "--kakasi" in sys.argv else M.openjtalk_moras if "--openjtalk" in sys.argv else None
    if mode == "refs":
        refs(*args)
    elif mode == "text":
        text(*args, kak)
    elif mode == "asr":
        asr(*args, kak)
    elif mode == "table":
        table(args[0], args[1:])
    elif mode == "paired":
        paired(args[0], args[1], args[2])
    elif mode == "selftest":
        ref = list("あいうえおかきくけこ")
        assert align(ref, ref)[:3] == (0, 0, 0)
        assert align(ref, ref[:3] + ref[5:])[:3] == (0, 0, 2)
        assert align(ref, ref[:4] + ["x"] + ref[4:])[:3] == (0, 1, 0)
        assert align(ref, ["さ"] + ref[1:])[:4] == (1, 0, 0, [(i, i) for i in range(1, 10)])
        assert runs(align(ref, ref[:2] + ref[7:])[4]) == [5]
        print("ok")
