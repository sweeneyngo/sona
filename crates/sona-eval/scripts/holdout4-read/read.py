"""utau-edit-cost 17: the one pre-registered holdout4 read, run once, end to end (rehearse it on dev).

Steps, in order; nothing is scored before step 1 has dropped the songs it flags:
  0. preflight, reads nothing: binaries, models, ear labels (`labels.py check`), every lyric input present
     for every song of the split, a clean tree at or after the rules' commit, and no earlier read of the set;
  1. the alignment check (`sona-eval-ust-align --split S --write`: 35 ms shift / 30 ms drift) — the read's
     ledger entry; flagged songs are dropped, and fewer than MIN_SONGS aligned songs stop the read;
  2. bar 1 (`sona-eval-frame --estimate-bar-one`, arms A today / B harmony / C Beat This) on the ear labels;
  3. the shipped stage D (`sona-eval-cellstage --model cellstage-D --edit-cost --dump`);
  4. stage C with T and 14's fit (`--model cellstage-19C --edit-cost --settle-near-ties --lyric-placement
     --lyric-lines --lyric-line-times --dump`);
  5. the lyric arm L = 14's fit + 17's S1 on C's notes: the one replaceable stage (`LYRIC_STAGES`);
  6. every pre-registered rule's verdict (issue 17, "Pre-registered rules"), printed and written to
     OUT/verdicts.json. The Python rows are checked against the Rust reports before any verdict.

Steps 2–4 log "same sitting" re-reads, as earlier holdout reads did. On dev nothing is logged and the
manifest must come back unchanged (checked).

Usage (see issue 17 for the full command):
  python3 -I read.py --split holdout4 --reason "…" --rules-commit SHA --labels FILE --out DIR \\
      --lyric-lines DIR --lyric-times DIR --lyric-work DIR --lyric-onsets DIR [--lyric-stage python|rust] \\
      [--s1-producer CMD] [--lyric-row TITLE] [--bin DIR] [--stage-d DIR] [--stage-c DIR] [--beat-this PROGRAM] [--resume]
`--s1-producer CMD` runs after step 4 with S1_DUMP (C's dump), S1_SPLIT and S1_OUT (= --lyric-onsets) in its
environment and must write S1_OUT/<id>.json for every scored song: for an onset producer that needs sona's own
notes (the lattice prior). With `--lyric-stage rust` the C run is then repeated with `--lyric-onsets` (4b).
Everything it writes under OUT holds UST- or lyric-derived content: keep OUT outside the repo.
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import json, os, re, shlex, subprocess, time  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
sys.path.insert(0, HERE)
import labels as LB  # noqa: E402

MIN_SONGS = 20
MIN_CONFIRMED = 5
S1_VARIANT = "file-m3+adaptmora+abst3.0:split+start1"
BOOT_ITERS = 10_000
TITLES = {
    "C": "learned cell stage, onset phase (ships)",
    "C+T": "learned cell stage, near ties settled (opt-in)",
    "C+L14": "learned cell stage, lyric fitted per aligned lyric line and space, rests hold moras",
}


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def say(msg=""):
    print(msg, flush=True)


def git(*a):
    return subprocess.run(["git", "-C", REPO, *a], capture_output=True, text=True).stdout.strip()


def ledger_lines():
    p = os.path.join(os.path.dirname(manifest_path()), "holdout-reads.jsonl")
    return open(p).read().splitlines() if os.path.exists(p) else []


def manifest_path():
    return os.environ.get("SONA_UST_MANIFEST") or os.path.join(REPO, "crates/sona-eval/corpus/ust-manifest.json")


def song_set(split):
    return ",".join(sorted(s["id"] for s in LB.songs_of(split)))


def tool(cmd, out_path, env_extra=None):
    """Run one Rust tool, its stdout to out_path; stop the read if it fails."""
    say("  $ " + " ".join(cmd))
    env = dict(os.environ, **(env_extra or {}))
    t = time.time()
    with open(out_path, "w") as f:
        r = subprocess.run(cmd, stdout=f, stderr=subprocess.PIPE, text=True, env=env)
    if r.returncode:
        sys.exit(f"read stopped: {cmd[0]} failed ({r.returncode}): {r.stderr.strip()[-2000:]}")
    say(f"    {time.time() - t:.0f} s, {out_path}")
    return r.stderr


# ---- step 0 ----

def preflight(o):
    errs = []
    for b in ("sona-eval-ust-align", "sona-eval-frame", "sona-eval-cellstage"):
        if not os.access(os.path.join(o["bin"], b), os.X_OK):
            errs.append(f"no {b} in {o['bin']} (cargo build --release -p sona-eval --features game --bins)")
    for m in (o["stage_d"], o["stage_c"]):
        if not os.path.exists(os.path.join(m, "cellstage.json")):
            errs.append(f"no model in {m}")
    if not os.access(o["beat_this"], os.X_OK):
        errs.append(f"no Beat This program {o['beat_this']} (scripts/setup-separators.sh beat-this)")
    for k in ("SONA_GAME_DIR", "ORT_DYLIB_PATH"):
        if not os.environ.get(k):
            errs.append(f"set {k}")
    lab_errs, st = LB.problems(json.load(open(o["labels"])), o["split"])
    errs += lab_errs
    if st["unlabelled"]:
        errs.append(f"labels: {len(st['unlabelled'])} songs not labelled yet ({', '.join(st['unlabelled'])})")
    if o["holdout"] and len(st["confirmed"]) < MIN_CONFIRMED:
        errs.append(f"labels: {len(st['confirmed'])} confirmed by the user, the rule needs {MIN_CONFIRMED}")
    for sid in (s["id"] for s in LB.songs_of(o["split"]) if not (s.get("alignment_status") or "").startswith("flagged")):
        for d, f in LYRIC_STAGES[o["lyric_stage"]].inputs(o, sid):
            if o["s1_producer"] and d == o["lyric_onsets"]:
                continue
            if not os.path.exists(os.path.join(d, f)):
                errs.append(f"lyric input missing: {os.path.join(d, f)}")
    if o["holdout"]:
        if git("status", "--porcelain", "--untracked-files=no"):
            errs.append("the tree has uncommitted changes: commit first, so the ledger's commit is the code that ran")
        if not o["rules_commit"] or subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", o["rules_commit"], "HEAD"]).returncode:
            errs.append(f"--rules-commit {o['rules_commit']!r} is not an ancestor of HEAD")
        prior = [l for l in ledger_lines() if json.loads(l).get("set") == song_set(o["split"])]
        if prior and not o["resume"]:
            errs.append(f"{o['split']} was already read {len(prior)} time(s): this is not the one read (--resume only after a failed run)")
        if not o["reason"]:
            errs.append("--reason is required")
    return errs, st


# ---- step 1 ----

def alignment(o):
    before = open(manifest_path(), "rb").read()
    cmd = [os.path.join(o["bin"], "sona-eval-ust-align"), "--split", o["split"], "--write"] + o["ledger_args"]("alignment check")
    tool(cmd, os.path.join(o["out"], "align.txt"))
    songs = LB.songs_of(o["split"])
    aligned = [s["id"] for s in songs if (s.get("alignment_status") or "").startswith("aligned")]
    flagged = {s["id"]: s.get("alignment_status") for s in songs if s["id"] not in aligned}
    if not o["holdout"] and open(manifest_path(), "rb").read() != before:
        open(manifest_path(), "wb").write(before)
        sys.exit("rehearsal: the alignment check changed dev's manifest entries (restored); the check is not reproducible")
    return aligned, flagged


# ---- step 2: the frame ----

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


def frame(o):
    rows_path = os.path.join(o["out"], "frame-rows.json")
    cmd = [os.path.join(o["bin"], "sona-eval-frame"), "--estimate-bar-one", "--split", o["split"], "--music-frames", o["labels"],
           "--beat-this", o["beat_this"], "--beat-this-out", os.path.join(o["out"], "beat-this"), "--rows", rows_path] + o["ledger_args"]("bar 1")
    tool(cmd, os.path.join(o["out"], "frame.txt"))
    return json.load(open(rows_path))["songs"]


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
        sys.exit(f"read stopped: the Python bootstrap of B - A {out['B-A']} does not match the Rust report ({m and m.groups()})")
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


# ---- steps 3-5: edit cost ----

def cellstage(o, name, model, extra):
    dump = os.path.join(o["out"], "dump-" + name)
    report = os.path.join(o["out"], f"cellstage-{name}.txt")
    cmd = [os.path.join(o["bin"], "sona-eval-cellstage"), "--model", model, "--edit-cost", "--split", o["split"], "--dump", dump] + extra
    tool(cmd + o["ledger_args"](f"stage {name}"), report)
    return dump, report


class PythonS1:
    """Today's lyric stage: 14's fit (`fit_lines`) with 17's S1 start moves, in Python (`scripts/mora-split/split.py`,
    variant S1_VARIANT), on stage C's dumped notes, with the onsets read from per-song files."""
    name = "python"

    @staticmethod
    def inputs(o, sid):
        w = o["lyric_work"]
        return [(o["lyric_lines"], sid + ".json"), (os.path.join(w, "text"), sid + ".json"), (os.path.join(w, "align", "lyrics-lead"), sid + ".json"),
                (o["lyric_times"], sid + ".json"), (o["lyric_onsets"], sid + ".json")]

    @staticmethod
    def cellstage_args(o):
        return []

    @staticmethod
    def rows(o, M, songs, report):
        data = {s["id"]: M.S.song_data(s) for s in songs}
        return [M.S.score(s, M.notes_for(s, data, "stage", "L"))[0] for s in songs]


class RustS1:
    """After ticket 21: the same arm from `sona-eval-cellstage --lyric-onsets DIR` (its row title in `--lyric-row`)."""
    name = "rust"

    @staticmethod
    def inputs(o, sid):
        return [(o["lyric_lines"], sid + ".json"), (o["lyric_times"], sid + ".json"), (o["lyric_onsets"], sid + ".json")]

    @staticmethod
    def cellstage_args(o):
        return ["--lyric-onsets", o["lyric_onsets"]]

    @staticmethod
    def rows(o, M, songs, report):
        title = o["lyric_row"]
        rust = M.rust_full_rows(report).get(title) or sys.exit(f"read stopped: no Rust row {title!r} (--lyric-row)")
        plain = {s["id"]: M.S.score(s, M.notes_for(s, {}, "stage", ""))[0] for s in songs}
        return [dict(counts=dict(zip(M.ec.CLASSES, rust[s["id"]][0])), span_s=plain[s["id"]]["span_s"], measures=rust[s["id"]][2], clean=rust[s["id"]][1])
                for s in songs]


LYRIC_STAGES = {"python": PythonS1, "rust": RustS1}


class Model:
    """The edit-cost rows from the dumps, through the dev scripts (`bundle.py`, `split.py`, `lib01.py`)."""

    def __init__(self, o):
        onsets, times = os.path.abspath(o["lyric_onsets"]), os.path.abspath(o["lyric_times"])
        if o["lyric_stage"] == "python" and (os.path.basename(os.path.dirname(onsets)) != "onsets" or os.path.dirname(times) != os.path.join(os.path.abspath(o["lyric_work"]), "times")):
            sys.exit("the Python lyric stage reads WORK/times/<name> and L17/onsets/<name>: pass --lyric-times and --lyric-onsets in those layouts")
        os.environ["L14_WORK"] = o["lyric_work"]
        os.environ["L17_WORK"] = os.path.dirname(os.path.dirname(onsets))
        os.environ["L16_TIMES"] = os.path.basename(times)
        sys.path.insert(0, os.path.join(HERE, "..", "bundle"))
        import bundle as B  # noqa: E402
        self.B, self.S, self.L, self.ec = B, B.S, B.L, B.ec
        self.B.LYRIC["L"] = "file-" + os.path.basename(o["lyric_onsets"].rstrip("/")) + S1_VARIANT[S1_VARIANT.index(":"):]

    def load(self, dump):
        self.L.DUMP = dump
        return self.L.load()

    def notes_for(self, s, data, key, lyric):
        return self.B.notes_for(s, data, key, lyric)

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
        sys.exit("read stopped: the Python rows do not reproduce the Rust report:\n  " + "\n  ".join(bad))


def edit_cost_rules(M, rows, ids):
    B = M.B

    def without_largest(ref, new):
        """The paired Δ without the song whose cost moved most (20's amaten check): reported, never deciding."""
        moved = [abs(M.B.cost_w(n["counts"]) - M.B.cost_w(r["counts"])) for r, n in zip(ref, new)]
        k = max(range(len(moved)), key=moved.__getitem__)
        keep = [i for i in range(len(ref)) if i != k]
        d, lo, hi = M.L.paired([ref[i] for i in keep], [new[i] for i in keep], B.cpm, pool_fn=M.S.pool_e)
        return {"song": ids[k], "delta": d, "ci": [lo, hi]}

    def judge(ref, new):
        ok, hi, d = B.passes(ref, new)
        _, lo, _ = M.L.paired(ref, new, B.cpm, pool_fn=M.S.pool_e)
        _, _, hi4 = M.L.paired(ref, new, B.cpm4, pool_fn=M.S.pool_e)
        dc, dclo, dchi = M.L.paired(ref, new, B.clean, pool_fn=M.S.pool_e)
        miss = lambda rs: M.S.pool_e(rs)["counts"].get("missing", 0)
        return {"ref": B.cpm(M.S.pool_e(ref)), "new": B.cpm(M.S.pool_e(new)), "delta": d, "ci": [lo, hi], "upper_at_missing_4": hi4,
                "missing": [miss(ref), miss(new)], "clean": [B.clean(M.S.pool_e(ref)), B.clean(M.S.pool_e(new))], "clean_delta": [dc, dclo, dchi],
                "criteria": {"upper bound < 0": hi < 0, "missing does not rise, or upper bound < 0 at missing 4": miss(new) <= miss(ref) or hi4 < 0},
                "passes": ok, "without_largest": without_largest(ref, new)}

    r1 = judge(rows["D"], rows["C"])
    r2 = judge(rows["C"], rows["C+L"])
    r3 = judge(rows["C"], rows["C+T"])
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
    extra = {"D -> C+L (the bundle vs today)": judge(rows["D"], rows["C+L"]), "C -> C+L14 (14's fit alone)": judge(rows["C"], rows["C+L14"]),
             "C+L14 -> C+L (S1 on top)": judge(rows["C+L14"], rows["C+L"])}
    return {"1: C vs D": r1, "2: L on C vs C": r2, "3: T on C vs C (only if L fails)": r3}, extra


def fmt_rule(name, r):
    lo, hi = r["ci"]
    return (f"  {name}: {r['ref']:.1f} -> {r['new']:.1f} cost/min, Δ {r['delta']:+.1f} [{lo:+.1f}, {hi:+.1f}], up@missing4 {r['upper_at_missing_4']:+.1f}, "
            f"missing {r['missing'][0]} -> {r['missing'][1]}, clean {r['clean'][0]:.1f}% -> {r['clean'][1]:.1f}%  => "
            + ("PASS" if r["passes"] else "FAIL")
            + f"\n      without {r['without_largest']['song']} (moved most): Δ {r['without_largest']['delta']:+.1f} [{r['without_largest']['ci'][0]:+.1f}, {r['without_largest']['ci'][1]:+.1f}] (reported)"
            + "".join(f"\n      [{'x' if v else ' '}] {k}" for k, v in r["criteria"].items())
            + (f"\n      verdict: {r['verdict']}" if "verdict" in r else ""))


def main():
    split = arg("--split", "dev")
    o = {"split": split, "holdout": split != "dev", "reason": arg("--reason"), "rules_commit": arg("--rules-commit"), "labels": arg("--labels"),
         "out": arg("--out"), "bin": arg("--bin", os.path.join(REPO, "target/release")), "resume": "--resume" in sys.argv,
         "stage_d": arg("--stage-d", os.path.expanduser("~/.cache/sona/cellstage-D")),
         "stage_c": arg("--stage-c", os.path.expanduser("~/.cache/sona/cellstage-19C")),
         "beat_this": arg("--beat-this", os.path.expanduser("~/.local/share/sona/separator-venv/bin/sona-beat-this")),
         "lyric_stage": arg("--lyric-stage", "python"), "lyric_lines": arg("--lyric-lines"), "lyric_times": arg("--lyric-times"),
         "lyric_work": arg("--lyric-work"), "lyric_onsets": arg("--lyric-onsets"), "lyric_row": arg("--lyric-row"),
         "s1_producer": arg("--s1-producer")}
    if not o["out"] or not o["labels"] or not o["lyric_lines"] or not o["lyric_times"] or not o["lyric_onsets"]:
        sys.exit(__doc__)
    os.makedirs(o["out"], exist_ok=True)
    sitting = 0

    def ledger_args(step):
        nonlocal sitting
        if not o["holdout"]:
            return []
        sitting += 1
        first = sitting == 1 and not o["resume"]
        reason = o["reason"] + ("" if first else f" (same sitting: {step})") + (" (resumed after a failed run)" if o["resume"] else "")
        return ["--reason", reason] + ([] if first else ["--reread"])

    o["ledger_args"] = ledger_args
    started = time.time()
    say(f"== the {split} read, {time.strftime('%Y-%m-%d %H:%M')}, commit {git('rev-parse', '--short', 'HEAD')} ==")
    say("\n0. preflight (reads nothing)")
    errs, st = preflight(o)
    for e in errs:
        say("  " + e)
    if errs:
        sys.exit("read not started: fix the above")
    say(f"  ok: {len(LB.songs_of(split))} songs; labels {len(st['4/4'])} in 4/4 ({len(st['confirmed'])} confirmed), {len(st['other'])} not 4/4; "
        f"lyric stage {o['lyric_stage']}")
    ledger0 = len(ledger_lines())

    say("\n1. alignment check (35 ms shift / 30 ms drift) — flagged songs are dropped before any score")
    aligned, flagged = alignment(o)
    say(f"  {len(aligned)} aligned, {len(flagged)} dropped" + "".join(f"\n    dropped {k}: {v}" for k, v in flagged.items()))
    verdicts = {"split": split, "commit": git("rev-parse", "--short", "HEAD"), "rules_commit": o["rules_commit"], "aligned": aligned, "dropped": flagged}
    if len(aligned) < MIN_SONGS:
        verdicts["stopped"] = f"{len(aligned)} aligned songs < {MIN_SONGS}: the read stops (ticket 08)"
        json.dump(verdicts, open(os.path.join(o["out"], "verdicts.json"), "w"), indent=1)
        say(f"\nREAD STOPS: {verdicts['stopped']}" if o["holdout"] else f"\n  (rehearsal on {split}: {len(aligned)} < {MIN_SONGS} is expected; going on)")
        if o["holdout"]:
            sys.exit(1)

    say("\n2. bar 1 (A today, B harmony, C Beat This) against the ear labels")
    frame_rows = frame(o)
    labelled = [r["song"] for r in frame_rows]
    fr = frame_rules(frame_rows, open(os.path.join(o["out"], "frame.txt")).read())
    verdicts["frame"] = fr
    if len(labelled) < MIN_SONGS:
        fr["verdict"] = f"{len(labelled)} aligned songs labelled in 4/4 < {MIN_SONGS}: the frame rules decide nothing"
        if o["holdout"]:
            fr["default"] = "A stays (too few labelled songs)"

    say("\n3. stage D (shipped)")
    dump_d, report_d = cellstage(o, "D", o["stage_d"], [])
    say("\n4. stage C, with T and 14's fit")
    c_args = ["--settle-near-ties", "--lyric-placement", "--lyric-lines", o["lyric_lines"], "--lyric-line-times", o["lyric_times"]]
    stage = LYRIC_STAGES[o["lyric_stage"]]
    dump_c, report_c = cellstage(o, "C", o["stage_c"], c_args + ([] if o["s1_producer"] else stage.cellstage_args(o)))
    if o["s1_producer"]:
        say("\n4b. S1's onsets from C's notes (--s1-producer)")
        tool(shlex.split(o["s1_producer"]), os.path.join(o["out"], "s1-producer.txt"),
             {"S1_DUMP": dump_c, "S1_SPLIT": split, "S1_OUT": o["lyric_onsets"]})
        missing = [sid for sid in aligned if not os.path.exists(os.path.join(o["lyric_onsets"], sid + ".json"))]
        if missing:
            sys.exit(f"read stopped: the S1 producer wrote no onsets for {missing}")
        if stage.cellstage_args(o):
            _, report_c = cellstage(o, "C-L", o["stage_c"], c_args + stage.cellstage_args(o))

    say(f"\n5. the lyric arm on C ({o['lyric_stage']} S1)")
    M = Model(o)
    songs_d, songs_c = M.load(dump_d), M.load(dump_c)
    if [s["id"] for s in songs_d] != [s["id"] for s in songs_c] or sorted(s["id"] for s in songs_c) != sorted(aligned):
        sys.exit(f"read stopped: scored songs differ from the aligned ones: D {[s['id'] for s in songs_d]} C {[s['id'] for s in songs_c]}")
    rows = {"D": [M.S.score(s, M.notes_for(s, {}, "stage", ""))[0] for s in songs_d],
            "C": [M.S.score(s, M.notes_for(s, {}, "stage", ""))[0] for s in songs_c],
            "C+T": [M.S.score(s, M.notes_for(s, {}, "stage_settled", ""))[0] for s in songs_c]}
    data = {s["id"]: M.S.song_data(s) for s in songs_c} if o["lyric_stage"] == "python" else {}
    rows["C+L14"] = [M.S.score(s, M.notes_for(s, data, "stage", "L14"))[0] for s in songs_c] if data else None
    rows["C+L"] = LYRIC_STAGES[o["lyric_stage"]].rows(o, M, songs_c, report_c)
    check_rows(M, {"C": rows["D"]}, songs_d, report_d, ["C"])
    check_rows(M, rows, songs_c, report_c, ["C", "C+T"] + (["C+L14"] if data else []))
    say(f"  Python rows = Rust reports on {len(songs_c)} songs (D, C, C+T{', C+L14' if data else ''})")
    if rows["C+L14"] is None:
        rows["C+L14"] = rows["C+L"]

    say("\n6. verdicts (rules: issue 17, 'Pre-registered rules')")
    rules, extra = edit_cost_rules(M, rows, [s["id"] for s in songs_c])
    verdicts["edit_cost"], verdicts["edit_cost_reported"] = rules, extra
    for k, r in rules.items():
        say(fmt_rule(k, r))
    say("  reported, deciding nothing:")
    for k, r in extra.items():
        say(fmt_rule(k, r).replace("=> PASS", "").replace("=> FAIL", "").split("\n      [")[0])
    say(f"\n  4: bar 1 on {fr['songs']} labelled songs: A {fr['A']}, B {fr['B']}, C {fr['C']} pass; on the beat A {fr['beat_A']}, B {fr['beat_B']}, C {fr['beat_C']}")
    for arm in ("B", "C"):
        x = fr[arm + "-A"]
        say(f"    {arm} - A {x['diff']:+d} [{x['ci'][0]:+.0f}, {x['ci'][1]:+.0f}]: " + ("QUALIFIES" if x["qualifies"] else "does not qualify")
            + "".join(f"\n      [{'x' if v else ' '}] {k}" for k, v in x["criteria"].items()))
    say(f"    C - B {fr['C-B']['diff']:+d} [{fr['C-B']['ci'][0]:+.0f}, {fr['C-B']['ci'][1]:+.0f}] (tie-break only)")
    say(f"    by confidence / vote share (passing/songs): {fr['by_confidence']}; B and C disagree on {fr['B_and_C_disagree'] or 'none'}")
    say(f"    verdict: {fr.get('verdict', fr['default'])}")

    if rows:
        say("\n  per song (cost/min): D · C · C+T · C+L")
        for i, s in enumerate(songs_c):
            say(f"    {s['id']:34} " + "  ".join(f"{M.B.cpm(M.S.pool_e([rows[k][i]])):6.1f}" for k in ("D", "C", "C+T", "C+L")))
    new_ledger = ledger_lines()[ledger0:]
    verdicts["ledger"] = [json.loads(l) for l in new_ledger]
    say(f"\nledger: {len(new_ledger)} new entr{'y' if len(new_ledger) == 1 else 'ies'}" + "".join(f"\n  {l}" for l in new_ledger))
    if o["holdout"]:
        say("commit crates/sona-eval/corpus/holdout-reads.jsonl and the manifest's alignment statuses with the result; the split is now spent")
    json.dump(verdicts, open(os.path.join(o["out"], "verdicts.json"), "w"), indent=1)
    say(f"\n{time.time() - started:.0f} s; verdicts in {os.path.join(o['out'], 'verdicts.json')}")


if __name__ == "__main__":
    main()
