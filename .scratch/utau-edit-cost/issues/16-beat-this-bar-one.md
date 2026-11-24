# 16 — Beat This! downbeats as a second bar-1 source (opt-in until holdout4)

Status: resolved
Type: task
Blocked by: 15

## Why

Ticket 15 (`crates/sona-eval/reports/beat-trackers.md`): Beat This! (CPJKU, ISMIR 2024; code and weights MIT)
puts bar 1 on the music's bar line on 9/9 dev songs (today 5/9; +4 [+1, +7]), 5/5 on the corroborated songs,
and its beats on the music's beat cell on 9/9. It agrees with ADR-0031's harmonic estimator on every dev song,
including the 4 whose labels had weak corroboration, so dev can't say which is better; holdout4 can.

It passed the accuracy part of 15's rule and **failed the speed gate as measured** (median 160 s wall per
4-minute song on a machine at load 40–62, default threads). Controlled runs put it at 57–60 s CPU per song on
one thread and 43–100 s wall per 4-minute song at load 14–32, so the failure is mostly the machine. madmom
matched it (9/9) but is slower and its models are non-commercial. Hence needs-triage: the user decides
whether the cost is acceptable and how it runs (15's "For the user to decide").

## What

1. **Speed first.** Measure `final0` and `small0` with a fixed thread count (1 and 4) on a machine that isn't
   loaded, and the Rust port (`beat-this` crate, `rten`) if linking is chosen. Record CPU and wall per 4-minute
   song. `small0` must reproduce `final0`'s dev beat and bar votes to replace it.
2. **Integration (the user's call):** a user-installed venv like the separators, or ONNX through the `game`
   feature's `ort` (ADR-0017), or the Rust port behind a feature. Weights are MIT, so they may be downloaded at
   setup; the default build stays free of an ML runtime.
3. **Use:** the user's BPM; the beat from `grid_phase(mix)` as in `bar_one::estimate`; the bar from the vote
   of Beat This's downbeats mod 4 beats at that BPM (15's `score.py` `fold_bar`); confidence = the vote share.
   Report the harmonic bar beside it; a disagreement is what the app should flag. `bar_one` in the annotation
   gains a `source` (absent by default, so the default JSON is byte-identical).
4. **Port check:** the Rust fold reproduces `score.py`'s bar and beat votes on dev 9/9 songs (same input
   beats), and its vote shares to 0.01.
5. Opt-in only (`sona transcribe --bar-one-tracker beat-this`, a start-screen option). A default needs holdout4
   under the rule pre-registered in 05, extended to three sources (today, harmonic, Beat This) scored against
   ear-labelled frames.

## Optional second use (needs its own pre-registered test)

With sona's octave and Beat This's tempo (60 / mean of the beat intervals within 10% of the median), the
start screen's whole-BPM hint contains the exact UST BPM on 10/10 dev songs, against 6/10 for sona's estimate
(post hoc; the corpus has only integer tempos). If adopted, pre-register it on holdout4-tempo before reading.

## Acceptance

Speed numbers recorded; integration chosen by the user; Rust fold = prototype on dev; default path
byte-identical (ADR-0029's golden, `sona-eval-frame` default columns); tests that fail on a mutant (vote
taken mod 3, share ignored); README and an ADR.

## Comments

- 2026-11-24, lead: triaged ready. Build it opt-in as a user-installed tool in the separator venv (least code; MIT code and weights), so the holdout4 read can compare bar 1 from today's rule, the harmonic estimate (ADR-0031) and Beat This against ear labels. Its CPU minute is small next to separation. Speed gate re-measured with one thread pinned.

## Answer

Status: resolved (2026-11-24), opt-in, [ADR-0036](../../../docs/adr/0036-beat-this-bar-one.md). Dev only: no holdout audio was
opened, and the ledger is unchanged. The setup script was run against a reflink copy of the separator venv
(`~/.cache/sona/beat-trackers/separator-venv-t16`); the user's own venv is untouched.

**Built (integration: a user-installed tool, as the lead chose).**
- `scripts/setup-separators.sh beat-this` installs `beat-this==1.1.0` and torchaudio into the lead venv,
  constrained to the venv's torch (2.14.1 stayed; `rotary-embedding-torch` stayed 0.6.5, audio-separator's
  pin). It writes `scripts/sona-beat-this.py` in as `bin/sona-beat-this` and fetches `final0` into torch's cache.
- Core: `bar_one::first_beat`, `downbeat_vote` (fold mod 4, ties to the first vote) and `tracked_bar`. The
  annotation's `bar_one` gains `source` and `harmonic_time_s`, both absent for the harmony.
- `sona transcribe --bar-one MIX --bar-one-tracker beat-this [--beat-this P] [--beat-this-threads N]
  [--tool-timeout S]`.
- `sona serve`: `bar_one=beat-this`, phase `finding-bar-one`, the tool run as a job tool (deadline,
  cancel, last line). Also `--beat-this`, `--beat-this-threads` (default 1), capabilities `bar_one`, a setup
  note with the new status `optional`, and a 409 with the fix.
- App: Options → **Bar 1**: First grid line (default) / From the harmony / From Beat This (only when
  installed; otherwise its install line shows under Options). The ♩ popover says "Beat This (N%) · the
  harmony agrees", or "… · the harmony says X s" in red. A re-transcription keeps the source, falling back
  to the harmony.
- Eval: `sona-eval-frame --estimate-bar-one --beat-this PROGRAM [--beat-this-threads N] [--beat-this-out
  DIR]` adds a second table. The first table is byte-identical to ADR-0031's.
- Ticket 05's pre-registration gains arm C with the same three thresholds, plus a tie-break written before
  any read.

**Speed, one thread, the installed tool, the 10 dev mixes in sequence** (load average 27–39 on 16 cores):

| | per song | per 4 minutes of song |
|---|---|---|
| CPU | 57–101 s (median 83) | 77–98 s (median 88) |
| wall | 80–157 s (median 123) | 112–148 s (median 133) |

Not an idle machine, so this isn't the unloaded number item 1 asked for. One thread can't use more than
one core, so idle wall time should be about the CPU time, roughly 1.5 minutes per 4-minute song. The
eval's own run (below, at load 33–50, overlapping my test builds) gave 52–94 s CPU and 123–187 s wall per 4
minutes. `small0`, 4 threads and the Rust port were not measured: the integration was decided, and the lead
narrowed the speed check to one thread.

**Port check (item 4).**
- The tool's beats and downbeats are identical to ticket 15's run on 10/10 dev songs.
- On the same JSON, `sona-eval-frame`'s bar vote and beat-cell vote equal `score.py`'s on 9/9 songs, and the
  shares agree to 0.01 (bar: 0.63–1.00; beat: 0.73–1.00).

**Dev result** (`sona-eval-frame --estimate-bar-one --beat-this <venv>/bin/sona-beat-this`, end to end):
Beat This 9/9 [9, 9], +4 [+1, +7] vs today, +0 vs the harmony (the two agree on 9/9). This is circular in
the same way as ADR-0031's: the reference bar is the harmonic estimate.

**Tests** (through the public interfaces):
- core: 5 new tests in `tests/bar_one.rs`;
- annotation: harmonic JSON unchanged, Beat This JSON, the contract fixture;
- CLI: the downbeat → bar-1 mapping with a fake tool (5 of 7 downbeats on beat 2 → 1.1 s, confidence 5/7,
  one thread, on the mix); the missing tool; the time limit (stopped after 1 s); a failing tool; bad flags;
- serve: the mapping, steps and provenance; missing tool → `optional` note, `bar_one` false, 409; a hung
  tool stopped at 2 s and the queue moves on;
- web: choices by capability, the install line, the query, the re-transcription fallback, the label and its
  flag, the phase;
- browser: `web/e2e/bar_one_choice.py`, passing against `sona serve` without and with a stand-in tool.

**Mutants (11, all caught):**

| mutant | caught by |
|---|---|
| vote taken mod 3 | 4 core tests |
| share ignored | 2 core tests |
| vote ignored | 2 core tests |
| CLI writes the harmony's bar | CLI mapping test |
| no deadline | CLI time-limit test |
| Beat This always offered | serve missing-tool test |
| harmonic JSON gains `source: null` | annotation test |
| web offers Beat This regardless | vitest |
| web sends `bar_one=1` for Beat This | vitest |
| a disagreement is never flagged | vitest |
| re-transcription asks for a missing Beat This | vitest |

**Verification:**
- `cargo test --workspace --release`: 375 passed;
- `--features game game::`: 8 passed;
- clippy `-D warnings`, plain and `--features game`: clean;
- web: `tsc` clean, vitest 228/228, the 12 editor e2e drivers pass;
- all 5 `verification/*/check.sh` pass (no start-screen state changed: the choice is an option, like the
  engine);
- `check-no-corpus-data.sh` passes.

**For the user:**
1. Run `scripts/setup-separators.sh beat-this` to install it into your real venv.
2. Delete the test copy `~/.cache/sona/beat-trackers/separator-venv-t16` when you like.
3. Review ticket 05's tie-break for arm C.
4. Holdout4 still needs a `--split` path in `--estimate-bar-one` that goes through the ledger.
5. The downbeats aren't cached, so a re-transcription runs Beat This again.
6. The optional tempo-hint use is not done.
