"""utau-edit-cost 17: the pre-registered holdout4 reads (rehearse them on dev). Two invocations, each run once:

The edit-cost read (rules 1-3), in order; nothing is scored before step 1 has dropped the songs it flags:
  0. preflight, reads nothing: binaries, models, every song's lyric text and the audio-only lyric inputs
     (`audio_inputs.py`: the three CTCs' emissions, the lead's loudness), the frozen S1 offsets, a clean tree at
     or after the rules' commit, and no earlier read of the set;
  1. the alignment check (`sona-eval-ust-align --split S --write`: 35 ms shift / 30 ms drift): the read's ledger
     entry; flagged songs are dropped, and fewer than MIN_SONGS aligned songs stop the read;
  3. the shipped stage D (`sona-eval-cellstage --model cellstage-D --edit-cost --dump`);
  3b. the lyric inputs, built only now (option B: no UST content before the read), for the aligned songs that
     have a lyric text, from D's dump (the UST's moras): 01's lines (`align_lines.py`), 14's texts (`texts.py`),
     the three Viterbis on the cached emissions (`ctc_align.py`), 14's energy line ends (`line_times.py`, no
     scoring), then S1's member onsets (`s1_onsets.py check` + `produce`, offsets frozen from dev);
  4. stage C with T (`--model cellstage-19C --edit-cost --settle-near-ties --dump`) and, in the same run when
     every aligned song has a lyric text, 14's fit and L (`--lyric-placement --lyric-lines --lyric-line-times
     --lyric-onsets`; C+L is the row ending "(S1)"); otherwise L runs on the songs with a text (`SONA_ONLY`);
  5. the rows: D, C and C+T from the dumps (Python, checked equal to the Rust reports), C+L14 and C+L from the
     Rust report;
  6. rules 1-3's verdicts (issue 17, "Pre-registered rules"), printed and written to OUT/verdicts.json.

The frame read (rule 4), `--frame-only`, later, once `labels.py check` passes: bar 1 (`sona-eval-frame
--estimate-bar-one`, arms A today / B harmony / C Beat This) on the songs step 1 kept (read back from the
manifest, never re-checked), its own ledger entry; verdict in OUT/verdicts-frame.json.

Ledger: the first tool of the edit-cost read logs the read; the others log "same sitting" re-reads. `--resume`
(after a failed run, only when the ledger already holds this set) re-runs from step 1 if it never finished, else
from step 3, every tool logging a re-read marked "(resumed after a failed run)". On dev nothing is logged, the
alignment check must leave the manifest unchanged (checked), and `--stop-after STEP` simulates a crash.

Usage (see issue 17 for the full commands):
  python3 -I read.py --split holdout4 --reason "…" --rules-commit SHA --out DIR --audio-work DIR [--resume]
  python3 -I read.py --split holdout4 --frame-only --reason "…" --rules-commit SHA --labels FILE --out DIR
Options: --allow-partial-lyrics (holdout: let rule 2 read only the songs with a lyric text), --lyrics DIR (lyric texts and index.json; default $SONA_LYRICS_DIR, else data/pifuyuu-lyrics),
  --s1-offsets FILE, --bin DIR, --stage-d DIR, --stage-c DIR, --beat-this PROGRAM, --fugashi-python, --ctc-python.
Everything it writes under OUT holds UST- or lyric-derived content: keep OUT outside the repo.
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import json, os, re, subprocess, time  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
SCRIPTS = os.path.join(HERE, "..")
sys.path.insert(0, HERE)
import labels as LB  # noqa: E402

MIN_SONGS = 20
MIN_CONFIRMED = 5
S1_MODELS = ("lyrics", "hira", "jg")
BOOT_ITERS = 10_000
STEPS = ("1", "3", "3b", "4")
LEDGER_FILES = ("crates/sona-eval/corpus/ust-manifest.json", "crates/sona-eval/corpus/holdout-reads.jsonl")
TITLES = {
    "C": "learned cell stage, onset phase (ships)",
    "C+T": "learned cell stage, near ties settled (opt-in)",
    "C+L14": "learned cell stage, lyric fitted per aligned lyric line and space, rests hold moras",
    "C+L": "learned cell stage, lyric fitted per aligned lyric line and space, rests hold moras, split and started at the moras' onsets (S1)",
}


class ReadStopped(SystemExit):
    pass


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def say(msg=""):
    print(msg, flush=True)


def git(*a):
    return subprocess.run(["git", "-C", REPO, *a], capture_output=True, text=True).stdout.rstrip("\n")


def manifest_path():
    return os.environ.get("SONA_UST_MANIFEST") or os.path.join(REPO, "crates/sona-eval/corpus/ust-manifest.json")


def ledger_lines():
    p = os.path.join(os.path.dirname(manifest_path()), "holdout-reads.jsonl")
    return open(p).read().splitlines() if os.path.exists(p) else []


def song_set(split):
    return ",".join(sorted(s["id"] for s in LB.songs_of(split)))


def status(s):
    return s.get("alignment_status") or ""


def tool(cmd, out_path, env_extra=None):
    """Run one tool, its stdout to out_path; stop the read if it fails."""
    say("  $ " + " ".join(cmd))
    env = dict(os.environ, **(env_extra or {}))
    t = time.time()
    with open(out_path, "w") as f:
        r = subprocess.run(cmd, stdout=f, stderr=subprocess.PIPE, text=True, env=env)
    if r.returncode:
        raise ReadStopped(f"read stopped: {os.path.basename(cmd[0])} failed ({r.returncode}): {r.stderr.strip()[-2000:]}")
    say(f"    {time.time() - t:.0f} s, {out_path}")
    return r.stderr


# ---- the ledger: who may read, and what each tool logs (pure; tests in test_read.py) ----

def ledger_policy(mode, holdout, resume, prior_reads, unverified, songs):
    """Whether this invocation may start, and where: (errors, first step). `prior_reads`: the ledger's entries for
    this set; `unverified`: the split's songs whose alignment step 1 has not settled; `songs`: the split's size."""
    if not holdout:
        return [], "2" if mode == "frame" else "3" if resume else "1"
    if mode == "frame":
        if resume:
            return ["--resume is for the edit-cost read; the frame read runs once"], None
        if prior_reads == 0 or unverified:
            return ["the frame read reuses step 1's alignment: run the edit-cost read first (and commit its ledger entry)"], None
        return [], "2"
    if not resume:
        if prior_reads:
            return [f"this set was already read {prior_reads} time(s): this is not the one read (--resume only after a failed run)"], None
        if unverified != songs:
            return ["some songs' alignment is already settled although the set was never read: the manifest is not the split's"], None
        return [], "1"
    if not prior_reads:
        return ["nothing to resume: the ledger has no read of this set"], None
    return [], ("1" if unverified else "3")


class Ledger:
    """The `--reason`/`--reread` flags of each tool in one invocation: the first tool of a fresh edit-cost read
    logs the read, every other tool a re-read naming why."""

    def __init__(self, holdout, reason, mode, resume):
        self.holdout, self.reason, self.mode, self.resume, self.calls = holdout, reason, mode, resume, 0

    def args(self, step):
        if not self.holdout:
            return []
        self.calls += 1
        if self.mode == "edit" and not self.resume and self.calls == 1:
            return ["--reason", self.reason]
        why = {"frame": "rule 4, bar 1: the frame read", "edit": "same sitting"}[self.mode]
        if self.resume:
            why = "resumed after a failed run"
        return ["--reason", f"{self.reason} ({why}: {step})", "--reread"]


# ---- step 0 ----

def lyric_index(o):
    p = os.path.join(o["lyrics"], "index.json")
    return {e["id"]: e for e in json.load(open(p))} if os.path.exists(p) else {}


def has_text(o, sid):
    return any(os.path.exists(os.path.join(o["lyrics"], sid + ext)) for ext in (".readings.txt", ".txt"))


def preflight(o):
    errs = []
    for b in ("sona-eval-ust-align", "sona-eval-frame", "sona-eval-cellstage"):
        if not os.access(os.path.join(o["bin"], b), os.X_OK):
            errs.append(f"no {b} in {o['bin']} (cargo build --release -p sona-eval --features game --bins)")
    for k in ("SONA_GAME_DIR", "ORT_DYLIB_PATH"):
        if not os.environ.get(k):
            errs.append(f"set {k}")
    songs = LB.songs_of(o["split"])
    st = None
    if o["mode"] == "frame":
        if not os.access(o["beat_this"], os.X_OK):
            errs.append(f"no Beat This program {o['beat_this']} (scripts/setup-separators.sh beat-this)")
        if not o["labels"] or not os.path.exists(o["labels"]):
            errs.append("--labels FILE is required")
        else:
            lab_errs, st = LB.problems(json.load(open(o["labels"])), o["split"])
            errs += lab_errs
            if st["unlabelled"]:
                errs.append(f"labels: {len(st['unlabelled'])} songs not labelled yet")
            if o["holdout"] and len(st["confirmed"]) < MIN_CONFIRMED:
                errs.append(f"labels: {len(st['confirmed'])} confirmed by the user, the rule needs {MIN_CONFIRMED}")
    else:
        for m in (o["stage_d"], o["stage_c"]):
            if not os.path.exists(os.path.join(m, "cellstage.json")):
                errs.append(f"no model in {m}")
        if not os.path.exists(o["s1_offsets"]):
            errs.append(f"no frozen S1 offsets {o['s1_offsets']} (ticket 21: s1_onsets.py freeze, on dev)")
        for py in (o["fugashi_python"], o["ctc_python"]):
            if not os.access(py, os.X_OK):
                errs.append(f"no python {py}")
        live = [s["id"] for s in songs if not status(s).startswith("flagged")]
        o["texts"] = [sid for sid in live if has_text(o, sid)]
        if o["holdout"] and len(o["texts"]) < len(live) and not o["partial_lyrics"]:
            errs.append(f"{len(live) - len(o['texts'])} of {len(live)} songs have no lyric text, so rule 2 would read only "
                        f"{len(o['texts'])}: fetch them, or pass --allow-partial-lyrics if the rules say so")
        for sid in o["texts"]:
            need = [os.path.join("rms", sid + ".json")] + [os.path.join("emis", f"{m}-lead", sid + ".npy.json") for m in S1_MODELS]
            errs += [f"audio input missing: {os.path.join(o['audio_work'], n)}" for n in need if not os.path.exists(os.path.join(o["audio_work"], n))]
    if o["holdout"]:
        dirty = [l[3:] for l in git("status", "--porcelain", "--untracked-files=no").splitlines()]
        allowed = LEDGER_FILES if o["resume"] else ()
        if [f for f in dirty if f not in allowed]:
            errs.append(f"uncommitted changes ({', '.join(dirty)}): commit first, so the ledger's commit is the code that ran")
        if not o["rules_commit"] or subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", o["rules_commit"], "HEAD"]).returncode:
            errs.append(f"--rules-commit {o['rules_commit']!r} is not an ancestor of HEAD")
        if not o["reason"]:
            errs.append("--reason is required")
    prior = sum(json.loads(l).get("set") == song_set(o["split"]) for l in ledger_lines())
    unverified = sum(status(s).startswith("unverified") for s in songs)
    pol, start = ledger_policy(o["mode"], o["holdout"], o["resume"], prior, unverified, len(songs))
    return errs + pol, st, start


# ---- step 1 ----

def alignment(o, run):
    before = open(manifest_path(), "rb").read()
    if run:
        cmd = [os.path.join(o["bin"], "sona-eval-ust-align"), "--split", o["split"], "--write"] + o["ledger"].args("alignment check")
        tool(cmd, os.path.join(o["out"], "align.txt"))
        if not o["holdout"] and open(manifest_path(), "rb").read() != before:
            open(manifest_path(), "wb").write(before)
            raise ReadStopped("rehearsal: the alignment check changed dev's manifest entries (restored); the check is not reproducible")
    songs = LB.songs_of(o["split"])
    aligned = [s["id"] for s in songs if status(s).startswith("aligned")]
    return aligned, {s["id"]: status(s) for s in songs if s["id"] not in aligned}


# ---- step 3b: the lyric inputs, inside the read ----

def build_lyric_inputs(o, dump_d, ids):
    """01's lines, 14's texts, the three Viterbis, 14's energy line ends and S1's member onsets for `ids`, under
    OUT/lyric (the cached emissions and loudness linked in from --audio-work)."""
    w = os.path.join(o["out"], "lyric")
    l01, l14 = os.path.join(w, "l01"), os.path.join(w, "l14")
    os.makedirs(l01, exist_ok=True)
    os.makedirs(l14, exist_ok=True)
    for d in ("emis", "rms"):
        link = os.path.join(l14, d)
        if not os.path.islink(link):
            os.symlink(os.path.join(os.path.abspath(o["audio_work"]), d), link)
    env = {"L01_DUMP": dump_d, "L01_WORK": l01, "L14_WORK": l14, "SONA_LYRICS_DIR": o["lyrics"], "SONA_STEMS": os.environ.get("SONA_UST_STEMS_DIR", "")}
    log = lambda name: os.path.join(w, name + ".txt")
    tool([o["fugashi_python"], "-I", "-B", os.path.join(SCRIPTS, "lyric-placement", "align_lines.py")] + ids, log("align_lines"), env)
    tool([o["ctc_python"], "-I", "-B", os.path.join(SCRIPTS, "line-alignment", "texts.py")], log("texts"), env)
    for m in S1_MODELS:
        tool([o["ctc_python"], "-I", "-B", os.path.join(SCRIPTS, "line-alignment", "ctc_align.py"), "--model", m, "--stem", "lead", "--threads", "4"] + ids,
             log("ctc_align-" + m), env)
    tool([o["ctc_python"], "-I", "-B", os.path.join(HERE, "line_times.py")] + ids, log("line_times"), env)
    s1 = [o["ctc_python"], "-I", "-B", os.path.join(SCRIPTS, "vowel-onsets", "s1_onsets.py")]
    tool(s1 + ["check", "--offsets", o["s1_offsets"]], log("s1_check"), env)
    onsets = os.path.join(w, "s1-onsets")
    tool(s1 + ["produce", "--offsets", o["s1_offsets"], "--out", onsets] + ids, log("s1_produce"), env)
    missing = [sid for sid in ids for p in (os.path.join(l01, "lines", sid + ".json"), os.path.join(l14, "times", "lyrics-lead-energy", sid + ".json"),
                                            os.path.join(onsets, sid + ".json")) if not os.path.exists(p)]
    if missing:
        raise ReadStopped(f"read stopped: lyric inputs not built for {sorted(set(missing))}")
    return ["--lyric-placement", "--lyric-lines", os.path.join(l01, "lines"), "--lyric-line-times", os.path.join(l14, "times", "lyrics-lead-energy"),
            "--lyric-onsets", onsets]


# ---- the frame (rule 4) ----

def song_bootstrap(scores):
    """sona_eval::frame::song_bootstrap, exactly (xorshift64, the same seed): 95% interval of the summed score."""
    if not scores:
        return 0.0, 0.0
    m, state, n, draws = (1 << 64) - 1, 0x9E3779B97F4A7C15, len(scores), []
    for _ in range(BOOT_ITERS):
        total = 0.0
        for _ in range(n):
            state ^= (state << 13) & m
            state ^= state >> 7
            state ^= (state << 17) & m
            total += scores[state % n]
        draws.append(total)
    draws.sort()
    q = lambda p: draws[int((len(draws) - 1) * p + 0.5)]
    return q(0.025), q(0.975)


def frame_rules(rows, report):
    """Ticket 05's rule (B) and 16's arm C, per song from the frame rows; checks the bootstrap against the report."""
    n = len(rows)
    pa = [r["rules"]["today"]["beats"] == 0 for r in rows]
    pb = [r["rules"]["shipped"]["beats"] == 0 for r in rows]
    pc = [r["beat_this"]["offset"]["beats"] == 0 for r in rows]
    on_beat = lambda xs: sum(abs(x) < 0.125 for x in xs)
    beat_a = on_beat([r["rules"]["today"]["residual_beats"] for r in rows])
    beat_b = on_beat([r["rules"]["shipped"]["residual_beats"] for r in rows])
    beat_c = on_beat([r["beat_this"]["offset"]["residual_beats"] for r in rows])
    out = {"songs": n, "A": sum(pa), "B": sum(pb), "C": sum(pc), "beat_A": beat_a, "beat_B": beat_b, "beat_C": beat_c}
    for arm, p, beat in (("B", pb, beat_b), ("C", pc, beat_c)):
        lo, hi = song_bootstrap([float(x) - float(a) for x, a in zip(p, pa)])
        crit = {"1: passes on >= 3 more songs than A": sum(p) - sum(pa) >= 3, "2: bootstrap lower bound of the paired difference >= +1": lo >= 1,
                f"3: on the music's beat on >= as many songs as A ({beat} vs {beat_a})": beat >= beat_a}
        out[arm + "-A"] = {"diff": sum(p) - sum(pa), "ci": [lo, hi], "criteria": crit, "qualifies": all(crit.values()),
                           "fewer_than_A": sum(p) < sum(pa)}
    lo, hi = song_bootstrap([float(c) - float(b) for c, b in zip(pc, pb)])
    out["C-B"] = {"diff": sum(pc) - sum(pb), "ci": [lo, hi]}
    # The Rust table prints the same intervals: they must agree before any verdict.
    m = re.search(r"shipped\s+\d+/\d+\s+\[.*?\]\s+vs today ([+-]?\d+)\s+\[([+-]?\d+), ([+-]?\d+)\]", report)
    if not m or (int(m.group(1)), float(m.group(2)), float(m.group(3))) != (out["B-A"]["diff"], *out["B-A"]["ci"]):
        raise ReadStopped(f"read stopped: the Python bootstrap of B - A {out['B-A']} does not match the Rust report ({m and m.groups()})")
    qualified = [a for a in ("B", "C") if out[a + "-A"]["qualifies"]]
    if qualified == ["B"]:
        default = "B (harmony) becomes the default bar 1"
    elif qualified == ["C"]:
        default = "C (Beat This) becomes the default bar 1 where Beat This is installed; elsewhere A stays"
    elif qualified:
        default = ("B" if out["B"] >= out["C"] else "C (where installed)") + " becomes the default (both qualify: more songs; a tie goes to B)"
    else:
        default = "A stays the default; B and C stay opt-in"
    out["default"] = default
    out["by_confidence"] = {"B conf >= 0.75": [sum(p for p, r in zip(pb, rows) if r["confidence"] >= 0.75), sum(r["confidence"] >= 0.75 for r in rows)],
                            "B conf < 0.75": [sum(p for p, r in zip(pb, rows) if r["confidence"] < 0.75), sum(r["confidence"] < 0.75 for r in rows)],
                            "C share >= 0.75": [sum(p for p, r in zip(pc, rows) if r["beat_this"]["share"] >= 0.75), sum(r["beat_this"]["share"] >= 0.75 for r in rows)],
                            "C share < 0.75": [sum(p for p, r in zip(pc, rows) if r["beat_this"]["share"] < 0.75), sum(r["beat_this"]["share"] < 0.75 for r in rows)]}
    out["B_and_C_disagree"] = [r["song"] for r, b, c in zip(rows, pb, pc) if r["rules"]["shipped"]["beats"] != r["beat_this"]["offset"]["beats"]]
    return out


def frame_read(o, aligned):
    rows_path = os.path.join(o["out"], "frame-rows.json")
    cmd = [os.path.join(o["bin"], "sona-eval-frame"), "--estimate-bar-one", "--split", o["split"], "--music-frames", o["labels"],
           "--beat-this", o["beat_this"], "--beat-this-out", os.path.join(o["out"], "beat-this"), "--rows", rows_path] + o["ledger"].args("bar 1")
    tool(cmd, os.path.join(o["out"], "frame.txt"))
    rows = json.load(open(rows_path))["songs"]
    if [r["song"] for r in rows if r["song"] not in aligned]:
        raise ReadStopped("read stopped: the frame scored a song step 1 dropped")
    fr = frame_rules(rows, open(os.path.join(o["out"], "frame.txt")).read())
    if len(rows) < MIN_SONGS:
        fr["verdict"] = f"{len(rows)} aligned songs labelled in 4/4 < {MIN_SONGS}: the frame rules decide nothing"
        if o["holdout"]:
            fr["default"] = "A stays (too few labelled songs)"
    say(f"\n  4: bar 1 on {fr['songs']} labelled songs: A {fr['A']}, B {fr['B']}, C {fr['C']} pass; on the beat A {fr['beat_A']}, B {fr['beat_B']}, C {fr['beat_C']}")
    for arm in ("B", "C"):
        x = fr[arm + "-A"]
        say(f"    {arm} - A {x['diff']:+d} [{x['ci'][0]:+.0f}, {x['ci'][1]:+.0f}]: " + ("QUALIFIES" if x["qualifies"] else "does not qualify")
            + "".join(f"\n      [{'x' if v else ' '}] {k}" for k, v in x["criteria"].items()))
    say(f"    C - B {fr['C-B']['diff']:+d} [{fr['C-B']['ci'][0]:+.0f}, {fr['C-B']['ci'][1]:+.0f}] (tie-break only)")
    say(f"    by confidence / vote share (passing/songs): {fr['by_confidence']}; B and C disagree on {fr['B_and_C_disagree'] or 'none'}")
    say(f"    verdict: {fr.get('verdict', fr['default'])}")
    return fr


# ---- steps 3-6: edit cost ----

def cellstage(o, name, model, extra, only=None):
    dump = os.path.join(o["out"], "dump-" + name)
    report = os.path.join(o["out"], f"cellstage-{name}.txt")
    cmd = [os.path.join(o["bin"], "sona-eval-cellstage"), "--model", model, "--edit-cost", "--split", o["split"], "--dump", dump] + extra
    tool(cmd + o["ledger"].args(f"stage {name}"), report, {"SONA_ONLY": ",".join(only)} if only is not None else None)
    return dump, report


class Model:
    """The edit-cost rows from the dumps, through the dev scripts (`bundle.py`, `split.py`, `lib01.py`)."""

    def __init__(self, o):
        os.environ.setdefault("L14_WORK", os.path.join(o["out"], "lyric", "l14"))  # split.py reads it at import; unused here
        sys.path.insert(0, os.path.join(SCRIPTS, "bundle"))
        import bundle as B  # noqa: E402
        self.B, self.S, self.L, self.ec = B, B.S, B.L, B.ec

    def rust_rows(self, report, name, songs, plain):
        """Rows of one Rust report row, the span from the plain Python row of the same song (the UST's span)."""
        rust = self.rust_full_rows(report).get(TITLES[name])
        if not rust:
            raise ReadStopped(f"read stopped: no Rust row {TITLES[name]!r} in {report}")
        return [dict(counts=dict(zip(self.ec.CLASSES, rust[s["id"]][0])), span_s=p["span_s"], measures=rust[s["id"]][2], clean=rust[s["id"]][1])
                for s, p in zip(songs, plain)]

    def load(self, dump):
        self.L.DUMP = dump
        return self.L.load()

    def plain(self, songs, key):
        return [self.S.score(s, self.B.notes_for(s, {}, key, ""))[0] for s in songs]

    @staticmethod
    def rust_full_rows(path):
        """{title: {song: (counts, clean, measures)}} from a `sona-eval-cellstage --edit-cost` report."""
        out, title = {}, None
        for line in open(path):
            if line.startswith("edit cost, "):
                title = line[len("edit cost, "):].split(" (weights:")[0].strip()
                out[title] = {}
            elif title and line.startswith("  ") and not line.lstrip().startswith(("song", "POOLED", "pooled")):
                f = line.split()
                if len(f) == 15:
                    clean, meas = (int(x) for x in f[5].split("/"))
                    out[title][f[0]] = ([int(x) for x in f[6:15]], clean, meas)
            elif not line.startswith("  "):
                title = None
        return out


def check_rows(M, rows, songs, report, keys):
    """The Python rows equal the Rust report's per-song counts (as bundle.py's --check)."""
    rust, bad = M.rust_full_rows(report), []
    for name in keys:
        title = TITLES[name]
        for s, r in zip(songs, rows[name]):
            py = [r["counts"].get(k, 0) for k in M.ec.CLASSES]
            if title not in rust or rust[title].get(s["id"], (None,))[0] != py:
                bad.append(f"{name} {s['id']}: Rust {rust.get(title, {}).get(s['id'])} Python {py}")
    if bad:
        raise ReadStopped("read stopped: the Python rows do not reproduce the Rust report:\n  " + "\n  ".join(bad))


def edit_cost_rules(M, rows, ids, lyric_ids, low_ids):
    B = M.B

    def judge(ref, new, names):
        ok, hi, d = B.passes(ref, new)
        _, lo, _ = M.L.paired(ref, new, B.cpm, pool_fn=M.S.pool_e)
        _, _, hi4 = M.L.paired(ref, new, B.cpm4, pool_fn=M.S.pool_e)
        dc, dclo, dchi = M.L.paired(ref, new, B.clean, pool_fn=M.S.pool_e)
        miss = lambda rs: M.S.pool_e(rs)["counts"].get("missing", 0)
        moved = [abs(B.cost_w(n["counts"]) - B.cost_w(r["counts"])) for r, n in zip(ref, new)]
        k = max(range(len(moved)), key=moved.__getitem__)
        keep = [i for i in range(len(ref)) if i != k]
        wd, wlo, whi = M.L.paired([ref[i] for i in keep], [new[i] for i in keep], B.cpm, pool_fn=M.S.pool_e)
        return {"songs": len(ref), "ref": B.cpm(M.S.pool_e(ref)), "new": B.cpm(M.S.pool_e(new)), "delta": d, "ci": [lo, hi], "upper_at_missing_4": hi4,
                "missing": [miss(ref), miss(new)], "clean": [B.clean(M.S.pool_e(ref)), B.clean(M.S.pool_e(new))], "clean_delta": [dc, dclo, dchi],
                "criteria": {"upper bound < 0": hi < 0, "missing does not rise, or upper bound < 0 at missing 4": miss(new) <= miss(ref) or hi4 < 0},
                "passes": ok, "without_largest": {"song": names[k], "delta": wd, "ci": [wlo, whi]}}

    sub = [ids.index(i) for i in lyric_ids]
    pick = lambda rs: [rs[i] for i in sub]
    r1 = judge(rows["D"], rows["C"], ids)
    r2 = judge(pick(rows["C"]), rows["C+L"], lyric_ids)
    r3 = judge(rows["C"], rows["C+T"], ids)
    r1["verdict"] = "C becomes the default stage" if r1["passes"] else "D stays the default stage; C stays opt-in"
    if not r1["passes"]:
        r2["verdict"] = f"recorded only: L {'passes' if r2['passes'] else 'fails'} on C, but C does not ship, so nothing ships from L"
        r3["verdict"] = "recorded only (C does not ship)"
    elif r2["passes"]:
        r2["verdict"] = "L (14's fit + S1) ships on C; the lyric runtime (three CTCs) is the user's call"
        r3["verdict"] = "not judged (L passed); T stays opt-in"
    else:
        r2["verdict"] = "L fails: stays opt-in"
        r3["verdict"] = "T ships on C" if r3["passes"] else "T fails: stays opt-in"
    extra = {"D -> C+L (the bundle vs today)": judge(pick(rows["D"]), rows["C+L"], lyric_ids),
             "C -> C+L14 (14's fit alone)": judge(pick(rows["C"]), rows["C+L14"], lyric_ids),
             "C+L14 -> C+L (S1 on top)": judge(rows["C+L14"], rows["C+L"], lyric_ids)}
    sure = [j for j, sid in enumerate(lyric_ids) if sid not in low_ids]
    if low_ids and sure:
        extra[f"L on C without the low-confidence lyric texts ({', '.join(low_ids)})"] = judge(
            [pick(rows["C"])[j] for j in sure], [rows["C+L"][j] for j in sure], [lyric_ids[j] for j in sure])
    else:
        extra["L on C without the low-confidence lyric texts"] = {"note": "no lyric text is low-confidence: the same as rule 2" if not low_ids else "every text is low-confidence"}
    return {"1: C vs D": r1, "2: L on C vs C": r2, "3: T on C vs C (only if L fails)": r3}, extra


def fmt_rule(name, r):
    if "note" in r:
        return f"  {name}: {r['note']}"
    lo, hi = r["ci"]
    w = r["without_largest"]
    return (f"  {name} ({r['songs']} songs): {r['ref']:.1f} -> {r['new']:.1f} cost/min, Δ {r['delta']:+.1f} [{lo:+.1f}, {hi:+.1f}], "
            f"up@missing4 {r['upper_at_missing_4']:+.1f}, missing {r['missing'][0]} -> {r['missing'][1]}, clean {r['clean'][0]:.1f}% -> {r['clean'][1]:.1f}%"
            + ("  => " + ("PASS" if r["passes"] else "FAIL") if "verdict" in r else "")
            + f"\n      without {w['song']} (moved most): Δ {w['delta']:+.1f} [{w['ci'][0]:+.1f}, {w['ci'][1]:+.1f}] (reported)"
            + ("".join(f"\n      [{'x' if v else ' '}] {k}" for k, v in r["criteria"].items()) + f"\n      verdict: {r['verdict']}" if "verdict" in r else ""))


def edit_read(o, start, verdicts):
    say("\n1. alignment check (35 ms shift / 30 ms drift): flagged songs are dropped before any score")
    if start != "1":
        say("  resumed: step 1 finished in the failed run; its result is read back from the manifest")
    aligned, flagged = alignment(o, start == "1")
    say(f"  {len(aligned)} aligned, {len(flagged)} dropped" + "".join(f"\n    dropped {k}: {v}" for k, v in flagged.items()))
    verdicts.update(aligned=aligned, dropped=flagged)
    if len(aligned) < MIN_SONGS:
        if o["holdout"]:
            raise ReadStopped(f"READ STOPS: {len(aligned)} aligned songs < {MIN_SONGS} (ticket 08)")
        say(f"  (rehearsal on {o['split']}: {len(aligned)} < {MIN_SONGS} is expected; going on)")
    stop_at(o, "1")

    say("\n3. stage D (shipped)")
    dump_d, report_d = cellstage(o, "D", o["stage_d"], [])
    stop_at(o, "3")
    lyric_ids = [sid for sid in aligned if has_text(o, sid)]
    say(f"\n3b. the lyric inputs, built now from D's dump, for the {len(lyric_ids)} aligned songs with a lyric text"
        + (f" (none for {', '.join(sid for sid in aligned if sid not in lyric_ids)})" if len(lyric_ids) < len(aligned) else ""))
    if not lyric_ids:
        raise ReadStopped("read stopped: no aligned song has a lyric text")
    lyric_args = build_lyric_inputs(o, dump_d, lyric_ids)
    stop_at(o, "3b")

    say("\n4. stage C with T" + (", 14's fit and L in one run" if len(lyric_ids) == len(aligned) else "; then 14's fit and L on the songs with a text"))
    if len(lyric_ids) == len(aligned):
        dump_c, report_c = cellstage(o, "C", o["stage_c"], ["--settle-near-ties"] + lyric_args)
        report_l = report_c
    else:
        dump_c, report_c = cellstage(o, "C", o["stage_c"], ["--settle-near-ties"])
        _, report_l = cellstage(o, "C-L", o["stage_c"], lyric_args, only=lyric_ids)
    stop_at(o, "4")

    say("\n5. rows")
    M = Model(o)
    songs_d, songs_c = M.load(dump_d), M.load(dump_c)
    ids = [s["id"] for s in songs_c]
    if [s["id"] for s in songs_d] != ids or sorted(ids) != sorted(aligned):
        raise ReadStopped(f"read stopped: scored songs differ from the aligned ones: D {[s['id'] for s in songs_d]} C {ids}")
    lyric_ids = [sid for sid in ids if sid in lyric_ids]
    rows = {"D": M.plain(songs_d, "stage"), "C": M.plain(songs_c, "stage"), "C+T": M.plain(songs_c, "stage_settled")}
    check_rows(M, {"C": rows["D"]}, songs_d, report_d, ["C"])
    check_rows(M, rows, songs_c, report_c, ["C", "C+T"])
    songs_l = [s for s in songs_c if s["id"] in lyric_ids]
    plain_l = [rows["C"][ids.index(s["id"])] for s in songs_l]
    if report_l != report_c:
        check_rows(M, {"C": plain_l}, songs_l, report_l, ["C"])
    rows["C+L14"] = M.rust_rows(report_l, "C+L14", songs_l, plain_l)
    rows["C+L"] = M.rust_rows(report_l, "C+L", songs_l, plain_l)
    say(f"  D, C, C+T: Python rows = Rust reports on {len(ids)} songs; C+L14 and C+L from the Rust report on {len(lyric_ids)}")

    say("\n6. verdicts (rules 1-3: issue 17, 'Pre-registered rules')")
    index = lyric_index(o)
    low = [sid for sid in lyric_ids if index.get(sid, {}).get("confidence") == "low"]
    rules, extra = edit_cost_rules(M, rows, ids, lyric_ids, low)
    verdicts.update(lyric_songs=lyric_ids, low_confidence_lyrics=low, edit_cost=rules, edit_cost_reported=extra)
    for k, r in rules.items():
        say(fmt_rule(k, r))
    say("  reported, deciding nothing:")
    for k, r in extra.items():
        say(fmt_rule(k, r))
    say("\n  per song (cost/min): D · C · C+T · C+L")
    for i, sid in enumerate(ids):
        cl = f"{M.B.cpm(M.S.pool_e([rows['C+L'][lyric_ids.index(sid)]])):6.1f}" if sid in lyric_ids else "     -"
        say(f"    {sid:34} " + "  ".join(f"{M.B.cpm(M.S.pool_e([rows[k][i]])):6.1f}" for k in ("D", "C", "C+T")) + "  " + cl)


def stop_at(o, step):
    if o["stop_after"] == step:
        raise ReadStopped(f"rehearsal: stopped after step {step} (--stop-after), as a crash would")


def main():
    split = arg("--split", "dev")
    mode = "frame" if "--frame-only" in sys.argv else "edit"
    o = {"split": split, "holdout": split != "dev", "mode": mode, "reason": arg("--reason"), "rules_commit": arg("--rules-commit"),
         "labels": arg("--labels"), "out": arg("--out"), "bin": arg("--bin", os.path.join(REPO, "target/release")), "resume": "--resume" in sys.argv,
         "stage_d": arg("--stage-d", os.path.expanduser("~/.cache/sona/cellstage-D")),
         "stage_c": arg("--stage-c", os.path.expanduser("~/.cache/sona/cellstage-19C")),
         "beat_this": arg("--beat-this", os.path.expanduser("~/.local/share/sona/separator-venv/bin/sona-beat-this")),
         "lyrics": arg("--lyrics", os.environ.get("SONA_LYRICS_DIR") or os.path.join(REPO, "crates/sona-eval/data/pifuyuu-lyrics")),
         "audio_work": arg("--audio-work"), "stop_after": arg("--stop-after"), "partial_lyrics": "--allow-partial-lyrics" in sys.argv,
         "fugashi_python": arg("--fugashi-python", os.path.expanduser("~/.cache/sona/lyric-asr/venv/bin/python")),
         "ctc_python": arg("--ctc-python", os.path.expanduser("~/.cache/sona/line-align/venv/bin/python")),
         "s1_offsets": arg("--s1-offsets", os.path.expanduser("~/.cache/sona/lyric-onsets/offsets.json"))}
    if not o["out"] or (mode == "edit" and not o["audio_work"]):
        sys.exit(__doc__)
    if o["stop_after"] and (o["holdout"] or o["stop_after"] not in STEPS):
        sys.exit(f"--stop-after is for rehearsals on dev, one of {', '.join(STEPS)}")
    os.makedirs(o["out"], exist_ok=True)
    o["ledger"] = Ledger(o["holdout"], o["reason"], mode, o["resume"])
    started = time.time()
    name = "edit-cost read (rules 1-3)" if mode == "edit" else "frame read (rule 4)"
    say(f"== the {split} {name}{' (resumed)' if o['resume'] else ''}, {time.strftime('%Y-%m-%d %H:%M')}, commit {git('rev-parse', '--short', 'HEAD')} ==")
    say("\n0. preflight (reads nothing)")
    errs, st, start = preflight(o)
    for e in errs:
        say("  " + e)
    if errs:
        sys.exit("read not started: fix the above")
    ledger0 = len(ledger_lines())
    verdicts = {"split": split, "mode": mode, "commit": git("rev-parse", "--short", "HEAD"), "rules_commit": o["rules_commit"], "resumed": o["resume"]}
    out_name = "verdicts.json" if mode == "edit" else "verdicts-frame.json"
    try:
        if mode == "edit":
            say(f"  ok: {len(LB.songs_of(split))} songs, {len(o['texts'])} with a lyric text; starting at step {start}")
            edit_read(o, start, verdicts)
        else:
            aligned, _ = alignment(o, False)
            say(f"  ok: labels {len(st['4/4'])} in 4/4 ({len(st['confirmed'])} confirmed), {len(st['other'])} not 4/4; step 1 kept {len(aligned)} songs")
            say("\n2. bar 1 (A today, B harmony, C Beat This) against the ear labels, on the songs step 1 kept")
            verdicts.update(aligned=aligned, frame=frame_read(o, aligned))
    except ReadStopped as e:
        verdicts["stopped"] = str(e)
        say(f"\n{e}")
    new_ledger = ledger_lines()[ledger0:]
    verdicts["ledger"] = [json.loads(l) for l in new_ledger]
    say(f"\nledger: {len(new_ledger)} new entr{'y' if len(new_ledger) == 1 else 'ies'}" + "".join(f"\n  {l}" for l in new_ledger))
    if o["holdout"] and new_ledger:
        say("commit crates/sona-eval/corpus/holdout-reads.jsonl (and the manifest's alignment statuses) with the result")
    json.dump(verdicts, open(os.path.join(o["out"], out_name), "w"), indent=1)
    say(f"\n{time.time() - started:.0f} s; {os.path.join(o['out'], out_name)}")
    if "stopped" in verdicts:
        sys.exit(1)


if __name__ == "__main__":
    main()
