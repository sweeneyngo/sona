"""utau-edit-cost 20: the holdout4 bundle measured together on dev, the stage scored out of sample.

Reads two `sona-eval-cellstage --edit-cost --dump` runs of dev, one per stage (19's fold models, `--fold-models`),
each made with `--settle-near-ties --merge-unheard-pieces` so the dump holds the stage with those applied
(`stage_settled`, `stage_merged`, `stage_settled_merged`). The lyric arm runs here, on those notes: 14's fit on
the aligned lines and spaces (`tail`) and 17's S1 start moves on top (`file-m3+adaptmora+abst3.0:split+start1`),
through `scripts/mora-split/split.py`.

Arms: T (near ties settled), M (unheard pieces merged), L (14's fit + S1). Prints every subset on the candidate
stage, the comparator stage's plain row, the ticket's backward elimination (rule in issue 20), the taxonomy by
class, and per-song rows. `--check RUST_REPORT` compares the Python rows with the Rust report's per-song counts,
L too when the report has S1's row (`--lyric-onsets`; utau-edit-cost 21). L's onsets are
`$L17_WORK/onsets/m3+adaptmora+abst3.0/`: since 21, `sources.py --offsets` (frozen from dev) writes them.

Usage:
  L01_DUMP=<candidate dump> L01_WORK=<l01> L14_WORK=<l14> L17_WORK=<l17> \\
    python3 bundle.py --base-dump <comparator dump> [--check <rust report of the candidate run>] [--per-song]
Everything it reads is UST- or lyric-derived and stays outside the repo; it prints numbers only.
"""
import collections, itertools, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "mora-split"))
import split as S  # noqa: E402

L, O, ec = S.L, S.O, S.ec
S1 = "file-m3+adaptmora+abst3.0:split+start1"
LYRIC = {"": None, "L14": "tail", "L": S1}
STAGE_KEY = {"": "stage", "T": "stage_settled", "M": "stage_merged", "TM": "stage_settled_merged"}
ARMS = ("T", "M", "L")


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def cost_w(counts, missing=2):
    return sum((missing if k == "missing" else ec.W[k]) * v for k, v in counts.items())


def cpm(p, missing=2):
    return cost_w(p["counts"], missing) / (p["span_s"] / 60)


def cpm4(p):
    return cpm(p, 4)


def clean(p):
    return 100 * p["clean"] / p["measures"]


def notes_for(s, data, key, lyric):
    cells = O.out_cells(s, key)
    v = LYRIC[lyric]
    if v is None:
        return [(a, b, p, None) for a, b, p in cells]
    src, spans, cfg = S.parse_variant(v)
    text, al, times = data[s["id"]]
    on_c = [None if t is None else O.g_of(s, t) for t in S.onsets(text, al, src)]
    return S.fit_timed(s, cells, S.groups_of(s, text, times, spans), on_c, cfg)


def name(arms):
    return "+".join(a for a in ARMS if a in arms) or "C"


def config_rows(songs, data):
    """{(stage arms 'T'/'M'/'TM'/'', lyric ''/'L14'/'L'): [per-song edit-cost row]}."""
    rows = {}
    for tm, lyric in itertools.product(STAGE_KEY, LYRIC):
        rows[(tm, lyric)] = [S.score(s, notes_for(s, data, STAGE_KEY[tm], lyric))[0] for s in songs]
    return rows


def subset_rows(rows, arms):
    tm = "".join(a for a in "TM" if a in arms)
    return rows[(tm, "L" if "L" in arms else "")]


def fmt(rows, ref=None):
    p = S.pool_e(rows)
    lo, hi = S.boot(rows, cpm, S.pool_e)
    c = p["counts"]
    out = f"{cpm(p):6.1f} [{lo:5.1f}, {hi:5.1f}]  clean {clean(p):5.1f}%  missing {c.get('missing', 0):4d}"
    if ref is not None:
        d, dlo, dhi = L.paired(ref, rows, cpm, pool_fn=S.pool_e)
        d4, _, d4hi = L.paired(ref, rows, cpm4, pool_fn=S.pool_e)
        dc, dclo, dchi = L.paired(ref, rows, clean, pool_fn=S.pool_e)
        dm = c.get("missing", 0) - S.pool_e(ref)["counts"].get("missing", 0)
        out += f"  Δ {d:+6.1f} [{dlo:+6.1f}, {dhi:+6.1f}]  Δclean {dc:+5.1f} [{dclo:+5.1f}, {dchi:+5.1f}]  Δmissing {dm:+4d}  up@4 {d4hi:+6.1f}"
    return out


def passes(ref, rows):
    """The ticket's rule 2 for one arm: S (rows) against S without the arm (ref)."""
    _, _, hi = L.paired(ref, rows, cpm, pool_fn=S.pool_e)
    _, _, hi4 = L.paired(ref, rows, cpm4, pool_fn=S.pool_e)
    dm = S.pool_e(rows)["counts"].get("missing", 0) - S.pool_e(ref)["counts"].get("missing", 0)
    d = cpm(S.pool_e(rows)) - cpm(S.pool_e(ref))
    return hi < 0 and (dm <= 0 or hi4 < 0), hi, d


def eliminate(rows):
    bundle = set(ARMS)
    while bundle:
        print(f"\n  S = {{{', '.join(a for a in ARMS if a in bundle)}}}: {fmt(subset_rows(rows, bundle))}")
        verdicts = {}
        for a in ARMS:
            if a in bundle:
                ok, hi, d = passes(subset_rows(rows, bundle - {a}), subset_rows(rows, bundle))
                verdicts[a] = (ok, hi, d)
                print(f"    leave out {a}: S vs S\\{{{a}}}: {fmt(subset_rows(rows, bundle), subset_rows(rows, bundle - {a}))}  -> {'passes' if ok else 'fails'}")
        failing = [a for a, (ok, _, _) in verdicts.items() if not ok]
        if not failing:
            return bundle
        drop = max(failing, key=lambda a: (verdicts[a][1], verdicts[a][2]))
        print(f"    drop {drop}")
        bundle.discard(drop)
    return bundle


def taxonomy(title, rows):
    p = S.pool_e(rows)
    tot = cost_w(p["counts"])
    parts = " · ".join(f"{k} {ec.W[k] * p['counts'].get(k, 0)} ({100 * ec.W[k] * p['counts'].get(k, 0) / tot:.0f}%)" for k in ec.CLASSES)
    print(f"  {title:34} {cpm(p):6.1f}/min: {parts}")


# ---- the Rust report, for --check ----

def rust_rows(path):
    """{title: {song: [counts by class]}} from a `sona-eval-cellstage --edit-cost` report."""
    out, title = {}, None
    for line in open(path):
        if line.startswith("edit cost, "):
            title = line[len("edit cost, "):].split(" (weights:")[0].strip()
            out[title] = {}
        elif title and line.startswith("  ") and not line.startswith("  song") and not line.startswith("  POOLED") and not line.startswith("  pooled"):
            f = line.split()
            if len(f) == 15:
                out[title][f[0]] = [int(x) for x in f[6:15]]
        elif not line.startswith("  "):
            title = None
    return out


def check(rows, songs, path, dump_rows):
    rust = rust_rows(path)
    pairs = [
        ("learned cell stage, onset phase (ships)", ("", "")),
        ("learned cell stage, near ties settled (opt-in)", ("T", "")),
        ("learned cell stage, unheard pieces merged (opt-in)", ("M", "")),
        ("learned cell stage, near ties settled, unheard pieces merged (opt-in)", ("TM", "")),
        ("learned cell stage, lyric fitted per aligned lyric line and space, rests hold moras", ("", "L14")),
    ]
    s1 = "learned cell stage, lyric fitted per aligned lyric line and space, rests hold moras, split and started at the moras' onsets (S1)"
    if s1 in rust:
        # utau-edit-cost 21: S1 in Rust (`--lyric-onsets`), against split.py on the onsets in $L17_WORK.
        pairs.append((s1, ("", "L")))
    else:
        print("  check: no S1 row in the Rust report (run without --lyric-onsets); L not checked")
    bad = 0
    for title, key in pairs:
        if title not in rust:
            print(f"  check: no Rust row {title!r}")
            bad += 1
            continue
        for s, r in zip(songs, rows[key]):
            py = [r["counts"].get(k, 0) for k in ec.CLASSES]
            if rust[title].get(s["id"]) != py:
                print(f"  check: {title} {s['id']}: Rust {rust[title].get(s['id'])} Python {py}")
                bad += 1
        print(f"  check: {title}: {'same on every song' if not bad else 'DIFFERS'}")
    for key, rs in dump_rows.items():
        for s, r in zip(songs, rs):
            dumped = list(s["edit_cost"][key])
            py = [r["counts"].get(k, 0) for k in ec.CLASSES]
            if dumped != py:
                print(f"  check: dump {key} {s['id']}: Rust {dumped} Python {py}")
                bad += 1
    print(f"  check: dump edit costs of {', '.join(dump_rows)}: {'same on every song' if not bad else 'DIFFERS'}")
    return bad


def main():
    songs = [s for s in L.load() if os.path.exists(os.path.join(S.WORK, "text", s["id"] + ".json"))]
    data = {s["id"]: S.song_data(s) for s in songs}
    rows = config_rows(songs, data)
    L.DUMP = arg("--base-dump")
    base = {s["id"]: s for s in L.load()}
    base_rows = [S.score(base[s["id"]], notes_for(base[s["id"]], data, "stage", ""))[0] for s in songs]
    c = rows[("", "")]

    if arg("--check"):
        dump_rows = {k: rows[(tm, "")] for tm, k in STAGE_KEY.items()}
        if check(rows, songs, arg("--check"), dump_rows):
            sys.exit("the Python composition does not reproduce the Rust rows")

    print(f"\n{len(songs)} dev songs; cost/min [95%] · clean · missing · Δ [paired 95%] · Δclean · Δmissing · upper bound of Δ at missing 4")
    print(f"  {'B (today, fold models)':34} {fmt(base_rows)}")
    print(f"  {'C (fold models)':34} {fmt(c, base_rows)}   (vs B)")
    print("\n  each arm alone on C, vs C:")
    for arms in [{"T"}, {"M"}, {"L"}]:
        print(f"  {'C+' + name(arms):34} {fmt(subset_rows(rows, arms), c)}")
    print(f"  {'C+L14 (14 fit only; diagnostic)':34} {fmt(rows[('', 'L14')], c)}")
    print(f"  {'C+L14 vs C+L (S1 on top of 14)':34} {fmt(rows[('', 'L')], rows[('', 'L14')])}")
    print("\n  every subset, vs C and vs B:")
    for k in range(4):
        for arms in itertools.combinations(ARMS, k):
            r = subset_rows(rows, set(arms))
            print(f"  {'C+' + name(set(arms)):34} {fmt(r, c)}")
    print(f"  {'C+T+M+L vs B':34} {fmt(subset_rows(rows, set(ARMS)), base_rows)}")

    print("\nbackward elimination (rule in issue 20):")
    final = eliminate(rows)
    print(f"\n  bundle: C + {{{', '.join(a for a in ARMS if a in final)}}}")

    print("\ntaxonomy by class (weighted units, share of cost):")
    taxonomy("B", base_rows)
    taxonomy("C", c)
    for arms in [{"T"}, {"M"}, {"L"}, {"T", "M"}, set(ARMS), final]:
        taxonomy("C+" + name(arms), subset_rows(rows, arms))
    taxonomy("C+L14", rows[("", "L14")])
    pc, pl = S.pool_e(c)["counts"], S.pool_e(subset_rows(rows, final))["counts"]
    print("\n  counts C -> bundle: " + " · ".join(f"{k} {pc.get(k, 0)}→{pl.get(k, 0)}" for k in ec.CLASSES))
    for arms in [{"L"}, {"T", "M", "L"}]:
        pa = S.pool_e(subset_rows(rows, arms))["counts"]
        pb = S.pool_e(subset_rows(rows, arms - {"L"}))["counts"]
        print(f"  joined removed by L on top of C+{name(arms - {'L'})}: {pb['joined']} → {pa['joined']} ({100 * (pb['joined'] - pa['joined']) / pb['joined']:.1f}%); chopped {pb['chopped']} → {pa['chopped']}; missing {pb['missing']} → {pa['missing']}")

    if "--per-song" in sys.argv:
        b = subset_rows(rows, final)
        print("\nper song: B · C · C+T · C+M · C+L · bundle (cost/min, clean %)")
        cols = [base_rows, c, subset_rows(rows, {"T"}), subset_rows(rows, {"M"}), subset_rows(rows, {"L"}), b]
        for i, s in enumerate(songs):
            print(f"  {s['id']:28} " + "  ".join(f"{cpm(S.pool_e([r[i]])):6.1f} {clean(S.pool_e([r[i]])):5.1f}%" for r in cols))
    if arg("--dump-rows"):
        json.dump({f"{tm}|{ly}": [[s["id"], [r["counts"].get(k, 0) for k in ec.CLASSES], r["measures"], r["clean"], r["span_s"]] for s, r in zip(songs, rs)] for (tm, ly), rs in rows.items()} | {"B": [[s["id"], [r["counts"].get(k, 0) for k in ec.CLASSES], r["measures"], r["clean"], r["span_s"]] for s, r in zip(songs, base_rows)]}, open(arg("--dump-rows"), "w"))


if __name__ == "__main__":
    main()
