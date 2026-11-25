"""utau-edit-cost 21: S1's onset files, with the per-class offsets frozen from dev.

S1 (lyrics-first 17) moves the fitted notes to per-mora vowel onsets. Each onset starts from three CTC forced
alignments of the song's moras on the lead stem (14's lyrics, hiragana and jg aligners). Each aligner's onset is
moved by its consonant class's offset: the median signed error of that aligner on that class against the UST's
note starts. The offsets read the UST, so they are fitted once, on dev, and frozen.

- `freeze OUT`: fits the offsets on every dev song in $L14_WORK (reads their UST onsets) and writes OUT
  (`~/.cache/sona/lyric-onsets/offsets.json`: UST-derived, local only, never in the repo). Refuses a song
  outside dev.
- `produce --offsets FILE --out DIR [ID …]`: writes `DIR/<id>.json` = {"sources": [aligner …], "members":
  [[seconds or null, one per mora] per aligner]}, the aligners' onsets with the frozen offsets applied. It
  reads the aligners' output, each mora's kana and the offsets file: no UST time and no dump. The rest of the
  consensus (median, adaptive sigma, lattice prior, snap, abstention) runs inside the scorer:
  `sona-eval-cellstage --lyric-onsets DIR` (`sona_core::lyric_fit::consensus_onsets`).
- `check --offsets FILE`: fails if `produce` reads a UST time (it runs again on kana-only copies of the text
  files and must write the same bytes), if it ignores the offsets file, or if the offsets were fitted on a
  song outside dev.

Env: L14_WORK (`text/<id>.json`, `align/<aligner>/<id>.json`; lyric-derived, outside the repo).
"""
import json, os, shutil, statistics, sys, tempfile, unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "..", "..", "corpus", "ust-manifest.json")
ALIGNERS = ("lyrics-lead", "hira-lead", "jg-lead")

# Consonant class of a mora's first hiragana, by its Unicode name (`HIRAGANA LETTER KA` -> KA); the same
# table as precision.py's CLASSES. Anything else (katakana, small ya/yu/yo/tu, wi, we, vu) is "other".
CLASSES = {
    "vowel": "A I U E O SMALL_A SMALL_I SMALL_U SMALL_E SMALL_O",
    "N": "N",
    "unvoiced stop": "KA KI KU KE KO TA TI TU TE TO PA PI PU PE PO",
    "voiced stop": "GA GI GU GE GO DA DI DU DE DO BA BI BU BE BO",
    "fricative": "SA SI SU SE SO HA HI HU HE HO ZA ZI ZU ZE ZO",
    "nasal": "NA NI NU NE NO MA MI MU ME MO",
    "approximant": "RA RI RU RE RO YA YU YO WA WO",
}
CLASS_OF = {r: c for c, rs in CLASSES.items() for r in rs.split()}


def mora_class(kana):
    name = unicodedata.name(kana[0], "") if kana else ""
    if not name.startswith("HIRAGANA LETTER "):
        return "other"
    return CLASS_OF.get(name[len("HIRAGANA LETTER "):].replace(" ", "_"), "other")


def work():
    return os.environ["L14_WORK"]


def song_ids(w=None):
    return sorted(f[:-5] for f in os.listdir(os.path.join(w or work(), "text")) if f.endswith(".json"))


def kanas(sid, w=None):
    """The song's moras' kana: the only field of the text file `produce` reads."""
    return [m["kana"] for m in json.load(open(os.path.join(w or work(), "text", sid + ".json")))["moras"]]


def raw(aligner, sid, w=None):
    """The aligner's onset (seconds) per mora."""
    return [x[0] for x in json.load(open(os.path.join(w or work(), "align", aligner, sid + ".json")))["moras"]]


def debiased(onsets, kana, offsets):
    """Each onset minus its mora's class offset (0 for a class the table lacks)."""
    return [None if t is None else t - offsets.get(mora_class(k), 0.0) for t, k in zip(onsets, kana)]


def dev_ids():
    m = json.load(open(MANIFEST))
    return {s["id"] for s in m["songs"] if s.get("split") == "dev"}


def load_offsets(path):
    o = json.load(open(path))
    if o.get("split") != "dev" or not set(o["fitted_on"]) <= dev_ids():
        sys.exit(f"{path}: offsets must be fitted on dev songs only")
    return o


def freeze(out):
    ids = song_ids()
    if not ids or not set(ids) <= dev_ids():
        sys.exit(f"freeze fits on dev songs only; not dev: {sorted(set(ids) - dev_ids())}")
    table = {}
    for a in ALIGNERS:
        errs = {}
        for sid in ids:
            text = json.load(open(os.path.join(work(), "text", sid + ".json")))["moras"]
            for m, t in zip(text, raw(a, sid)):
                if t is not None:
                    errs.setdefault(mora_class(m["kana"]), []).append(t - m["on"])
        table[a] = {k: statistics.median(v) for k, v in sorted(errs.items())}
    os.makedirs(os.path.dirname(os.path.abspath(out)), exist_ok=True)
    json.dump({"split": "dev", "fitted_on": ids, "offsets_s": table}, open(out, "w"), indent=1)
    print(f"froze per-class offsets of {', '.join(ALIGNERS)} from {len(ids)} dev songs into {out}")


def produce(offsets, out, ids, w=None):
    os.makedirs(out, exist_ok=True)
    for sid in ids:
        kana = kanas(sid, w)
        members = [debiased(raw(a, sid, w), kana, offsets["offsets_s"][a]) for a in ALIGNERS]
        if any(len(m) != len(kana) for m in members):
            sys.exit(f"{sid}: an aligner's moras differ from the text's")
        json.dump({"sources": list(ALIGNERS), "members": members}, open(os.path.join(out, sid + ".json"), "w"))


def check(offsets_path):
    """Exit non-zero unless `produce` is blind to UST times and reads the frozen offsets."""
    offsets = load_offsets(offsets_path)
    ids = song_ids()
    tmp = tempfile.mkdtemp(prefix="s1-check-")
    try:
        produce(offsets, os.path.join(tmp, "real"), ids)
        blind = os.path.join(tmp, "work")
        os.makedirs(os.path.join(blind, "text"))
        os.symlink(os.path.join(work(), "align"), os.path.join(blind, "align"))
        for sid in ids:
            # Kana only: a UST time (`on`, `off`), lines or pieces read by `produce` would be a KeyError here.
            json.dump({"moras": [{"kana": k} for k in kanas(sid)]}, open(os.path.join(blind, "text", sid + ".json"), "w"))
        try:
            produce(offsets, os.path.join(tmp, "blind"), ids, blind)
        except KeyError as e:
            print(f"check: produce read {e} from a text file, which only the UST has")
        shifted = {**offsets, "offsets_s": {a: {k: v + 0.05 for k, v in t.items()} for a, t in offsets["offsets_s"].items()}}
        produce(shifted, os.path.join(tmp, "shifted"), ids)
        bytes_of = lambda d, s: open(os.path.join(tmp, d, f"{s}.json"), "rb").read() if os.path.exists(os.path.join(tmp, d, f"{s}.json")) else None
        same = [bytes_of("real", s) == bytes_of("blind", s) for s in ids]
        moved = [bytes_of("real", s) != bytes_of("shifted", s) for s in ids]
    finally:
        shutil.rmtree(tmp)
    ok = all(same) and all(moved)
    print(f"check: kana-only text gives the same files on {sum(same)}/{len(ids)} songs; the offsets file moves {sum(moved)}/{len(ids)}; fitted on {len(offsets['fitted_on'])} dev songs -> {'ok' if ok else 'FAILS'}")
    return ok


def main():
    a = sys.argv[1:]
    opt = lambda k: a[a.index(k) + 1] if k in a else None
    if a[:1] == ["freeze"] and len(a) == 2:
        freeze(a[1])
    elif a[:1] == ["produce"] and opt("--offsets") and opt("--out"):
        ids = [x for i, x in enumerate(a[1:], 1) if not x.startswith("--") and a[i - 1] not in ("--offsets", "--out")]
        produce(load_offsets(opt("--offsets")), opt("--out"), ids or song_ids())
    elif a[:1] == ["check"] and opt("--offsets"):
        sys.exit(0 if check(opt("--offsets")) else 1)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
