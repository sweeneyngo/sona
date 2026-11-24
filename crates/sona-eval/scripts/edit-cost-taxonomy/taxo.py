"""Edit-cost taxonomy, oracles and calibration from `sona-eval-cellstage --dump` output
(`reports/edit-cost-baseline.md`).

usage: python3 -I taxo.py DUMP_DIR [lead|stage]
"""
import json, os, sys, random, collections
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ec

VOWELS = {}
for row, vs in {
    "あいうえお": "aiueo", "かきくけこ": "aiueo", "さしすせそ": "aiueo", "たちつてと": "aiueo", "なにぬねの": "aiueo",
    "はひふへほ": "aiueo", "まみむめも": "aiueo", "やゆよ": "auo", "らりるれろ": "aiueo", "わを": "ao",
    "がぎぐげご": "aiueo", "ざじずぜぞ": "aiueo", "だぢづでど": "aiueo", "ばびぶべぼ": "aiueo", "ぱぴぷぺぽ": "aiueo",
    "ぁぃぅぇぉ": "aiueo", "ゃゅょ": "auo", "ゔ": "u",
}.items():
    for k, v in zip(row, vs):
        VOWELS[k] = v
PURE_VOWEL = set("あいうえおぁぃぅぇぉ")


def kata_to_hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def mora(lyric):
    """(vowel, kind) of a UST lyric: kind in vowel / cv / n / ext / other."""
    l = lyric.strip()
    if " " in l:  # VCV "a か"
        l = l.split()[-1]
    l = kata_to_hira(l)
    if not l:
        return None, "other"
    if l in ("ー", "-", "～", "~"):
        return None, "ext"
    if l in ("ん", "n", "N"):
        return "n", "n"
    if l in ("っ",):
        return None, "other"
    last = l[-1]
    v = VOWELS.get(last)
    if v is None:
        # romaji
        lo = l.lower()
        if lo and lo[-1] in "aiueo":
            return lo[-1], ("vowel" if len(lo) == 1 else "cv")
        return None, "other"
    if len(l) == 1 and last in PURE_VOWEL:
        return v, "vowel"
    return v, "cv"


def load(d):
    out = []
    for f in sorted(os.listdir(d)):
        if f.endswith(".json"):
            out.append(json.load(open(os.path.join(d, f))))
    return out


def ust_lookup(song, field="lyric"):
    """UST Note blocks on the reference grid, keyed by (start cell, pitch) -> lyric (or part)."""
    sp, an = song["spacing"], song["ust_anchor"]
    m = {}
    for u in song["stage_detail"]["ust"]:
        if u["kind"] != "Note":
            continue
        s = ec.rround((u["on"] - an) / sp)
        m.setdefault((s, u["midi"]), u[field])
    return m


def starts(notes_cells):
    return {c[0] for c in notes_cells}


def near(cellset, k, tol=1):
    return any((k + d) in cellset for d in range(-tol, tol + 1))


def song_sources(song):
    """Each stage source's notes on the UST cell grid: lead (snapped), vocals GAME (snapped on the onset
    phase), sona pYIN (onset phase)."""
    sp, an, ph = song["spacing"], song["ust_anchor"], song["onset_phase"]

    def snap(ns):
        # sona_core::cellstage::snap_notes (tune ignored for boundaries)
        out = []
        for a, b, m, v in ns:
            if not v:
                continue
            on = ph + round((a - ph) / sp) * sp
            off = ph + round((b - ph) / sp) * sp
            if off <= on + 1e-9:
                off = on + sp
            out.append((on, off, m))
        out.sort()
        for i in range(1, len(out)):
            if out[i][0] < out[i - 1][1] - 1e-9:
                if out[i][0] > out[i - 1][0] + 1e-9:
                    out[i - 1] = (out[i - 1][0], out[i][0], out[i - 1][2])
                else:
                    out[i] = (out[i - 1][1], max(out[i][1], out[i - 1][1] + sp), out[i][2])
        return out

    to = lambda ns: ec.to_cells(ns, sp, an)
    return {
        "lead": to(song["outputs"]["lead"]),
        "vocals": to(snap(song["stage_detail"]["game_vocals"])),
        "sona": to(song["sona_onset"]),
    }


def joined_boundaries(song, A):
    """One row per hidden UST note (joined count): the boundary between consecutive refs under one output note."""
    rows = []
    lyr = ust_lookup(song)
    part = ust_lookup(song, "part")
    src = song_sources(song)
    srcstarts = {k: starts(v) for k, v in src.items()}
    refs, outs = A["refs"], A["outs"]
    for oi, rs in enumerate(A["adj_o"]):
        if len(rs) < 2:
            continue
        rs = sorted(rs, key=lambda r: refs[r][0])
        o = outs[oi]
        for a, b in zip(rs, rs[1:]):
            ra, rb = refs[a], refs[b]
            la, lb = lyr.get((ra[0], ra[2]), "?"), lyr.get((rb[0], rb[2]), "?")
            va, ka = mora(la)
            vb, kb = mora(lb)
            k = rb[0]
            rows.append(dict(
                song=song["id"], same=ra[2] == rb[2], interval=rb[2] - ra[2], gap=rb[0] - ra[1],
                len_a=ra[1] - ra[0], len_b=rb[1] - rb[0], out_pitch=o[2], out_len=o[1] - o[0],
                lyric_a=la, lyric_b=lb, kind_b=kb,
                cross_part=part.get((ra[0], ra[2])) != part.get((rb[0], rb[2])),
                vowel_cont=(kb == "vowel" and va is not None and va == vb) or kb == "ext",
                vowel_change=(kb == "vowel" and va is not None and va != vb),
                n_b=kb == "n",
                cand={s: near(srcstarts[s], k) for s in srcstarts},
                cand_exact={s: k in srcstarts[s] for s in srcstarts},
            ))
    return rows


def chopped_cuts(song, A, out="stage"):
    """One row per extra piece (chopped count): the cut between consecutive outputs under one UST note."""
    if out != "stage":
        rows = []
        refs, outs = A["refs"], A["outs"]
        srcstarts = {k: starts(v) for k, v in song_sources(song).items()}
        for ri, os_ in enumerate(A["adj_r"]):
            if len(os_) < 2:
                continue
            os_ = sorted(os_, key=lambda o: outs[o][0])
            r = refs[ri]
            for a, b in zip(os_, os_[1:]):
                oa, ob = outs[a], outs[b]
                iv = ob[2] - oa[2]
                why = "rest" if oa[1] < ob[0] else "pitch" if iv else "source restarts (lead)"
                rows.append(dict(song=song["id"], why=why, interval=iv, gap=ob[0] - oa[1], ref_len=r[1] - r[0],
                                 len_a=oa[1] - oa[0], len_b=ob[1] - ob[0], first_piece_short=outs[os_[0]][1] - outs[os_[0]][0] == 1 and a == os_[0],
                                 wrong_a=oa[2] != r[2], wrong_b=ob[2] != r[2], octave=abs(iv) == 12, srcpair="lead>lead",
                                 at={s: ob[0] in srcstarts[s] for s in srcstarts}))
        return rows
    rows = []
    refs, outs = A["refs"], A["outs"]
    det = song["stage_detail"]
    k0 = det["k0"]
    sp, an, ph = song["spacing"], song["ust_anchor"], song["onset_phase"]
    # stage cell index -> UST cell: stage cell c starts at ph + (k0 + c) sp
    shift = ec.rround((ph + k0 * sp - an) / sp)
    classes, cands = det["classes"], det["cands"]
    lyr = ust_lookup(song)
    srcstarts = {k: starts(v) for k, v in song_sources(song).items()}
    NAMES = ["rest", "lead", "vocals", "sona", "lead+12", "lead-12"]
    for ri, os_ in enumerate(A["adj_r"]):
        if len(os_) < 2:
            continue
        os_ = sorted(os_, key=lambda o: outs[o][0])
        r = refs[ri]
        for a, b in zip(os_, os_[1:]):
            oa, ob = outs[a], outs[b]
            k = ob[0]
            c = k - shift
            why = "?"
            if 0 < c < len(classes):
                ca, cb = classes[c - 1], classes[c]
                pa = cands[c - 1][ca] if ca else -1
                pb = cands[c][cb] if cb else -1
                if oa[1] < ob[0]:
                    why = "rest"
                elif pa != pb:
                    why = "pitch"
                elif ca == cb:
                    why = "source restarts (" + ["rest", "lead", "vocals", "sona", "lead+12", "lead-12"][cb] + ")"
                else:
                    why = "other"
            pieces = [outs[o] for o in os_]
            iv = ob[2] - oa[2]
            rows.append(dict(
                song=song["id"], why=why, interval=iv, gap=ob[0] - oa[1], ref_len=r[1] - r[0],
                len_a=oa[1] - oa[0], len_b=ob[1] - ob[0],
                first_piece_short=pieces[0][1] - pieces[0][0] == 1 and a == os_[0],
                wrong_a=oa[2] != r[2], wrong_b=ob[2] != r[2], octave=abs(iv) == 12,
                lyric=lyr.get((r[0], r[2]), "?"),
                classes=(classes[c - 1], classes[c]) if 0 < c < len(classes) else None,
                srcpair=f"{NAMES[classes[c - 1]]}>{NAMES[classes[c]]}" if 0 < c < len(classes) else "?",
                at={s: k in srcstarts[s] for s in srcstarts},
            ))
    return rows


def out_notes_seconds(cells, sp, an):
    return [(an + c[0] * sp, an + c[1] * sp, c[2]) for c in cells]


def replace_oracle(song, out_name, A, pick):
    """Output with every component for which pick(comp) holds replaced by its UST notes."""
    sp, an = song["spacing"], song["ust_anchor"]
    drop_out, add_ref = set(), []
    for c in A["comps"]:
        if pick(c):
            drop_out.update(A["outs"][o][3] for o in c["outs"])
            add_ref.extend(A["refs"][r] for r in c["refs"])
    orig = song["outputs"][out_name]
    new = [tuple(n) for i, n in enumerate(orig) if i not in drop_out] + out_notes_seconds(add_ref, sp, an)
    new.sort()
    return ec.analyse(song["reference"], new, sp, an, song["division"])


def has(k):
    return lambda c: c["counts"][k] > 0


def pool(rows):
    c = collections.Counter()
    meas = clean = 0
    span = 0.0
    for r in rows:
        c.update(r["counts"])
        meas += r["measures"]
        clean += r["clean"]
        span += r["span_s"]
    return dict(counts=dict(c), measures=meas, clean=clean, span_s=span)


def per_min(p):
    return ec.cost(p["counts"]) / (p["span_s"] / 60)


def boot(rows, stat, iters=10000, seed=1):
    rng = random.Random(seed)
    v = []
    for _ in range(iters):
        v.append(stat(pool([rows[rng.randrange(len(rows))] for _ in rows])))
    v.sort()
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def fmt_pool(rows):
    p = pool(rows)
    lo, hi = boot(rows, per_min)
    clo, chi = boot(rows, lambda q: 100 * q["clean"] / q["measures"])
    return f"{per_min(p):6.1f} [{lo:.1f}, {hi:.1f}]  clean {100 * p['clean'] / p['measures']:5.1f}% [{clo:.1f}, {chi:.1f}]"


def comp_label(c):
    k = c["counts"]
    if k["joined"] and k["chopped"]:
        return "joined+chopped"
    for name in ("joined", "chopped", "missing", "extra"):
        if k[name]:
            return name
    errs = [n for n in ("pitch", "length", "shift", "moved") if k[n]]
    return "+".join(errs) if errs else "ok"


def calibration(song, A):
    """Per stage output note: shipped confidence, mean/min max-prob over its cells, length, label, attributed cost."""
    det = song["stage_detail"]
    sp, an, ph = song["spacing"], song["ust_anchor"], song["onset_phase"]
    conf = {}
    for on, off, p, cf, sub in det["confidence"]:
        conf[(ec.rround((on - an) / sp), p)] = (cf, sub)
    shift = ec.rround((ph + det["k0"] * sp - an) / sp)
    probs = det["probs"]
    comp_of = {}
    for c in A["comps"]:
        for o in c["outs"]:
            comp_of[o] = c
    rows = []
    for oi, o in enumerate(A["outs"]):
        c = comp_of[oi]
        cf, sub = conf.get((o[0], o[2]), (None, None))
        pm = [max(probs[k - shift]) for k in range(o[0], o[1]) if 0 <= k - shift < len(probs)]
        rows.append(dict(song=song["id"], conf=cf, sub=sub, pmean=sum(pm) / len(pm), pmin=min(pm), len=o[1] - o[0],
                         label=comp_label(c), cost=c["cost"] / len(c["outs"])))
    return rows


def comp_hidden(a):
    """Joined components -> their hidden boundaries as (ref before, ref after)."""
    rows = {}
    refs = a["refs"]
    for c in a["comps"]:
        if not c["counts"]["joined"]:
            continue
        hidden = []
        for o in c["outs"]:
            rs = sorted(a["adj_o"][o], key=lambda r: refs[r][0])
            hidden.extend((refs[x], refs[y]) for x, y in zip(rs, rs[1:]))
        rows[id(c)] = hidden
    return rows


def share(rows, f):
    return f"{100 * sum(1 for r in rows if f(r)) / max(1, len(rows)):5.1f}%"


def main():
    d = sys.argv[1]
    songs = load(d)
    for s in songs:
        for name in ("pyin", "lead", "stage"):
            a = ec.analyse(s["reference"], s["outputs"][name], s["spacing"], s["ust_anchor"], s["division"])
            got = [a["counts"][k] for k in ec.CLASSES]
            assert got == s["edit_cost"][name], (s["id"], name, got, s["edit_cost"][name])
    print(f"port check: Python counts equal Rust counts on {len(songs)} songs x 3 outputs\n")

    OUT = sys.argv[2] if len(sys.argv) > 2 else "stage"
    A = {s["id"]: ec.analyse(s["reference"], s["outputs"][OUT], s["spacing"], s["ust_anchor"], s["division"]) for s in songs}
    base = [A[s["id"]] for s in songs]
    P = pool(base)
    tot = ec.cost(P["counts"])
    print(f"== output: {OUT} (onset phase) ==")
    print("pooled", fmt_pool(base))
    for name in ("pyin", "lead", "stage"):
        rows = [ec.analyse(s["reference"], s["outputs"][name], s["spacing"], s["ust_anchor"], s["division"]) for s in songs]
        print(f"{name:6}", fmt_pool(rows))
    print("class shares:", " ".join(f"{k} {100 * ec.W[k] * P['counts'].get(k, 0) / tot:.1f}%" for k in ec.CLASSES))
    print()
    print(f"{'song':28} {'min':>5} {'cost/min':>8} {'clean':>6} {'notes/s':>7} {'1-cell%':>7} " + " ".join(f"{k:>7}" for k in ec.CLASSES))
    for s in songs:
        a = A[s["id"]]
        refs = a["refs"]
        rate = len(refs) / a["span_s"]
        one = 100 * sum(1 for r in refs if r[1] - r[0] == 1) / len(refs)
        print(f"{s['id']:28} {a['span_s'] / 60:5.2f} {ec.cost(a['counts']) / (a['span_s'] / 60):8.1f} {100 * a['clean'] / a['measures']:5.1f}% {rate:7.2f} {one:6.1f}% " + " ".join(f"{a['counts'][k] * ec.W[k]:7d}" for k in ec.CLASSES))

    J = [r for s in songs for r in joined_boundaries(s, A[s["id"]])]
    assert len(J) == P["counts"]["joined"], (len(J), P["counts"]["joined"])
    print(f"\n== joined: {len(J)} hidden UST notes ==")
    print(f"{'song':28} {'n':>5} {'same':>6} {'gap>0':>6} {'b 1cell':>7} {'a 1cell':>7} {'vowel-cont':>10} {'vow-chg':>7} {'n':>6} {'cv':>6} {'cand any':>8} {'lead':>6} {'vocals':>6} {'sona':>6}")
    for sid in [s["id"] for s in songs] + ["ALL"]:
        rows = [r for r in J if sid in ("ALL", r["song"])]
        print(f"{sid:28} {len(rows):5d} {share(rows, lambda r: r['same'])} {share(rows, lambda r: r['gap'] > 0)} {share(rows, lambda r: r['len_b'] == 1):>7} {share(rows, lambda r: r['len_a'] == 1):>7} {share(rows, lambda r: r['vowel_cont']):>10} {share(rows, lambda r: r['vowel_change']):>7} {share(rows, lambda r: r['n_b'])} {share(rows, lambda r: r['kind_b'] == 'cv')} {share(rows, lambda r: any(r['cand'].values())):>8} {share(rows, lambda r: r['cand']['lead'])} {share(rows, lambda r: r['cand']['vocals'])} {share(rows, lambda r: r['cand']['sona'])}")
    print("  artefact checks: hidden note from another lead part", share(J, lambda r: r["cross_part"]), "; hidden note with a rest before it", share(J, lambda r: r["gap"] > 0))
    for s in songs:
        notes = [u for u in s["stage_detail"]["ust"] if u["kind"] == "Note"]
        parts = len({u["part"] for u in notes})
        print(f"    {s['id']:28} UST notes {len(notes):5d}, on the grid {len(A[s['id']]['refs']):5d} (lost or merged by rounding/monophony {len(notes) - len(A[s['id']]['refs'])}), lead parts {parts}")
    print("  a source starts a note exactly at the hidden start:", " ".join(f"{k} {share(J, lambda r, k=k: r['cand_exact'][k])}" for k in ("lead", "vocals", "sona")), " any", share(J, lambda r: any(r["cand_exact"].values())))
    print("  boundary recall: of the UST's legato boundaries (next note starts where the last ended), the share hidden in a joined note")
    lt = collections.Counter()
    for s in songs:
        a = A[s["id"]]
        lyr = ust_lookup(s)
        hidden = set()
        for oi, rs in enumerate(a["adj_o"]):
            rs = sorted(rs, key=lambda r: a["refs"][r][0])
            hidden.update(a["refs"][y][0] for y in rs[1:])
        refs = a["refs"]
        for x, y in zip(refs, refs[1:]):
            if y[0] != x[1]:
                continue
            kb = mora(lyr.get((y[0], y[2]), "?"))[1]
            for key in (("all",), ("same" if x[2] == y[2] else "diff",), ("same" if x[2] == y[2] else "diff", "next 1 cell" if y[1] - y[0] == 1 else "next 2+ cells"), ("same" if x[2] == y[2] else "diff", kb), ("song", s["id"], "same" if x[2] == y[2] else "diff")):
                lt[key + ("n",)] += 1
                lt[key + ("hid",)] += y[0] in hidden
    for key in sorted({k[:-1] for k in lt}, key=str):
        n = lt[key + ("n",)]
        print(f"    {' / '.join(key):48} {n:6d} boundaries, {100 * lt[key + ('hid',)] / n:5.1f}% hidden")
    print("same-pitch x lyric kind of the hidden note:")
    for same in (True, False):
        rows = [r for r in J if r["same"] == same]
        c = collections.Counter(("vowel-cont" if r["vowel_cont"] else "vowel-change" if r["vowel_change"] else r["kind_b"]) for r in rows)
        print(f"  same={same!s:5} n={len(rows):4d} " + " ".join(f"{k} {100 * v / max(1, len(rows)):.1f}%" for k, v in c.most_common()))
    iv = collections.Counter(min(abs(r["interval"]), 13) for r in J)
    print("  |interval| (13 = >12):", dict(sorted(iv.items())))
    lb = collections.Counter(min(r["len_b"], 9) for r in J)
    print("  hidden note length in cells (9 = 9+):", dict(sorted(lb.items())))
    print("  sample same-pitch lyrics a>b:", " ".join(f"{r['lyric_a']}>{r['lyric_b']}" for r in [r for r in J if r["same"]][:30]))
    print("  sample diff-pitch lyrics a>b:", " ".join(f"{r['lyric_a']}>{r['lyric_b']}({r['interval']:+d})" for r in [r for r in J if not r["same"]][:30]))

    C = [r for s in songs for r in chopped_cuts(s, A[s["id"]], OUT)]
    assert len(C) == P["counts"]["chopped"], (len(C), P["counts"]["chopped"])
    print(f"\n== chopped: {len(C)} extra pieces ==")
    why = collections.Counter(r["why"] for r in C)
    print("  stage cause:", " ".join(f"{k} {100 * v / len(C):.1f}%" for k, v in why.most_common()))
    iv = collections.Counter(("octave" if abs(r["interval"]) == 12 else "same" if r["interval"] == 0 else "1-2 st" if abs(r["interval"]) <= 2 else "3-11 st" if abs(r["interval"]) < 12 else ">12") for r in C)
    print("  pitch step at the cut:", " ".join(f"{k} {100 * v / len(C):.1f}%" for k, v in iv.most_common()))
    print("  a piece off the UST pitch:", share(C, lambda r: r["wrong_a"] or r["wrong_b"]), " gap at cut:", share(C, lambda r: r["gap"] > 0),
          " 1-cell piece:", share(C, lambda r: r["len_a"] == 1 or r["len_b"] == 1), " UST note >= 8 cells:", share(C, lambda r: r["ref_len"] >= 8))
    print("  a source starts a note exactly at the cut:", " ".join(f"{k} {share(C, lambda r, k=k: r['at'][k])}" for k in ("lead", "vocals", "sona")))
    sp_ = collections.Counter(r["srcpair"] for r in C)
    print("  chosen source before>after the cut:", " ".join(f"{k} {100 * v / len(C):.1f}%" for k, v in sp_.most_common(8)))
    def chop_kind(r):
        if r["why"] == "rest":
            return "gap (rest inside a UST note)"
        if abs(r["interval"]) == 12 or abs(r["interval"]) == 24:
            return "octave flip"
        if r["interval"] == 0:
            return "same pitch, source restarts"
        if r["len_a"] == 1 and r["first_piece_short"]:
            return "1-cell head (scoop/consonant)"
        if abs(r["interval"]) <= 2:
            return "1-2 st step (vibrato/glide/drift)"
        return "other pitch step"
    ck = collections.Counter(chop_kind(r) for r in C)
    print("  chopped kinds:", " | ".join(f"{k} {100 * v / len(C):.1f}%" for k, v in ck.most_common()))
    print(f"{'song':28} {'n':>5} " + " ".join(f"{k[:14]:>14}" for k, _ in why.most_common()))
    for sid in [s["id"] for s in songs]:
        rows = [r for r in C if r["song"] == sid]
        print(f"{sid:28} {len(rows):5d} " + " ".join(f"{sum(1 for r in rows if r['why'] == k):14d}" for k, _ in why.most_common()))

    print("\n== oracles (components replaced by the UST's notes; the real metric re-run) ==")
    src = {s["id"]: {k: starts(v) for k, v in song_sources(s).items()} for s in songs}
    H = {sid: comp_hidden(a) for sid, a in A.items()}

    def same_only(sid):
        h = H[sid]
        return lambda c: id(c) in h and all(x[2] == y[2] for x, y in h[id(c)])

    def diff_any(sid):
        h = H[sid]
        return lambda c: id(c) in h and any(x[2] != y[2] for x, y in h[id(c)])

    def cand_all(sid, keys=("lead", "vocals", "sona"), tol=0):
        h, st = H[sid], src[sid]
        return lambda c: id(c) in h and all(any(near(st[k], y[0], tol) for k in keys) for x, y in h[id(c)])

    picks = [
        ("baseline", lambda sid: lambda c: False),
        ("joined (any)", lambda sid: has("joined")),
        ("  joined, all hidden same-pitch", same_only),
        ("  joined, some hidden diff-pitch", diff_any),
        ("  joined, a source has every start", cand_all),
        ("  joined, ... within 1 cell", lambda sid: cand_all(sid, tol=1)),
        ("  joined, GAME lead has every start", lambda sid: cand_all(sid, ("lead",))),
        ("chopped (any)", lambda sid: has("chopped")),
        ("joined or chopped", lambda sid: lambda c: c["counts"]["joined"] or c["counts"]["chopped"]),
        ("missing", lambda sid: has("missing")),
        ("extra", lambda sid: has("extra")),
        ("length (pairs)", lambda sid: has("length")),
        ("shift/moved (pairs)", lambda sid: lambda c: c["counts"]["shift"] or c["counts"]["moved"]),
        ("pitch (pairs)", lambda sid: has("pitch")),
        ("every pair with an error", lambda sid: lambda c: len(c["refs"]) == 1 and len(c["outs"]) == 1 and c["cost"] > 0),
    ]
    base_cost = per_min(P)
    print(f"  {'fixed perfectly':38} {'cost/min [95%]':>22} {'clean measures [95%]':>26}  cost cut")
    for name, mk in picks:
        rows = [replace_oracle(s, OUT, A[s["id"]], mk(s["id"])) for s in songs]
        print(f"  {name:38} {fmt_pool(rows)}  {100 * (1 - per_min(pool(rows)) / base_cost):5.1f}%")

    if OUT != "stage":
        return
    print("\n== confidence calibration (stage notes) ==")
    R =[r for s in songs for r in calibration(s, A[s["id"]])]
    print(f"  notes {len(R)}, unmatched to a shipped confidence {sum(1 for r in R if r['conf'] is None)}")
    vals = sorted(set(round(r["conf"], 4) for r in R if r["conf"] is not None))
    print(f"  distinct shipped confidence values (4 dp): {len(vals)}; range {vals[0]}..{vals[-1]}")
    labels = ["ok", "joined", "chopped", "joined+chopped", "extra", "pitch", "length", "shift", "other pair"]

    def lab(r):
        return r["label"] if r["label"] in labels else "other pair"

    allc = sum(r["cost"] for r in R)

    def table(key, edges):
        print(f"  {key:>6} bucket {'notes':>6} {'err%':>6} " + " ".join(f"{l[:8]:>8}" for l in labels[1:]) + f" {'cost%':>6}")
        for lo, hi in zip(edges, edges[1:]):
            rows = [r for r in R if r[key] is not None and lo <= r[key] < hi]
            if not rows:
                continue
            c = collections.Counter(lab(r) for r in rows)
            print(f"  [{lo:.2f},{hi:.2f}) {len(rows):6d} {100 * (1 - c['ok'] / len(rows)):5.1f}% " + " ".join(f"{100 * c[l] / len(rows):7.1f}%" for l in labels[1:]) + f" {100 * sum(r['cost'] for r in rows) / allc:5.1f}%")

    table("conf", [0, 0.2, 0.4, 0.5, 0.55, 0.6, 0.65, 1.01])
    table("pmean", [0, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.01])
    print("  flag the lowest x% of notes by a signal: share of attributable cost caught (joined / chopped / all)")
    jc = sum(r["cost"] for r in R if "joined" in r["label"])
    cc = sum(r["cost"] for r in R if "chopped" in r["label"])
    sig = {
        "shipped confidence": lambda r: r["conf"] if r["conf"] is not None else 1,
        "stage mean max-prob": lambda r: r["pmean"],
        "stage min max-prob": lambda r: r["pmin"],
        "longest notes first": lambda r: -r["len"],
    }
    rng = random.Random(7)
    for name, f in sig.items():
        order = sorted(R, key=lambda r: (f(r), rng.random()))
        out = []
        for x in (10, 20, 30):
            top = order[: len(order) * x // 100]
            out.append(f"{x}%: {100 * sum(r['cost'] for r in top if 'joined' in r['label']) / jc:4.1f}/{100 * sum(r['cost'] for r in top if 'chopped' in r['label']) / cc:4.1f}/{100 * sum(r['cost'] for r in top) / allc:4.1f}")
        print(f"  {name:22} " + "  ".join(out))
    print(f"  (random flags catch x% of each; attributable to an output note: {100 * allc / tot:.1f}% of all cost, the rest is missing notes)")


if __name__ == "__main__":
    main()
