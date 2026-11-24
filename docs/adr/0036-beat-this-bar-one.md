# ADR-0036: Bar 1 from Beat This!, an opt-in tool beside the harmony

Status: accepted (opt-in), 2026-11-24. Ticket: `.scratch/utau-edit-cost/issues/16-beat-this-bar-one.md`.
Builds on [ADR-0031](0031-bar-one-from-the-music.md) (bar 1 from the music) and
[ADR-0030](0030-stem-cache-cleanup-and-job-failures.md) (tool deadlines). Study:
`crates/sona-eval/reports/beat-trackers.md` (ticket 15).

## Context

Ticket 15 measured downbeat trackers against the dev songs' music frames. Beat This! (CPJKU, ISMIR 2024;
code **and weights MIT**) put bar 1 on the music's bar line on 9/9 dev songs (today's rule: 5/9), the same
as ADR-0031's harmonic estimator. On dev the two never disagree, and the dev labels are the harmonic
estimator's own output, so dev can't say which is better. Holdout4, labelled by ear, can, if Beat This can be
run on it the same way the app would run it. The user chose the integration: a user-installed tool in the
separator venv, the least code, with no ML runtime in the default build (ADR-0005).

## Decision

1. **The tool.** `scripts/sona-beat-this.py` runs `Audio2Beats(final0, cpu, dbn=False)` on the mono mix,
   ticket 15's measured configuration, pinned to `--threads N` (torch and the OMP/MKL/BLAS pools), and
   writes `{beats, downbeats, checkpoint, checkpoint_path, beat_this, torch, threads, cpu_s, wall_s, load}`.
   `scripts/setup-separators.sh beat-this` (not part of `all`) installs `beat-this==1.1.0` and torchaudio
   into the lead separator's venv, constrained to the venv's own torch so audio-separator keeps working.
   It copies the script in as `bin/sona-beat-this` and fetches the checkpoint (81 MB) into torch's cache.
2. **The mapping** (`sona_core::bar_one::tracked_bar`): the beat is ADR-0031's (`first_beat`: the
   vocals' cell line nearest `grid_phase(mix, one beat)`, the first at or after 0 s). Each downbeat votes for
   its whole-beat offset from it, mod 4 (ticket 15's `fold_bar`). The most votes win, ties going to the
   offset voted for first. Bar 1 is that beat, so it lies within 4 beats of 0 s, and the vote share is its
   confidence. No downbeats: bar 1 is the beat, with confidence 0. There is no confidence threshold: none
   was measured, and holdout4 scores the bar as voted.
3. **The annotation.** `bar_one` gains `source: "beat-this"` and `harmonic_time_s` (the harmony's bar 1
   under ADR-0031's rule, computed beside it), both absent for the harmony. `harmonic` keeps its JSON name
   and now means "a bar the source found, not the beat fallback". Default and harmonic JSON are unchanged
   byte for byte.
4. **Running it.**
   - `sona transcribe --bar-one MIX --bar-one-tracker beat-this [--beat-this PROGRAM]
     [--beat-this-threads N] [--tool-timeout S]`: one thread by default. The deadline is 600 s, or 20 s per
     second of the mix if longer, as for `sona serve`'s tools. A missing tool names its path and
     `scripts/setup-separators.sh beat-this`.
   - `sona serve`: a job with `bar_one=beat-this` (400 with `separation=none`) runs the tool on the job's
     `mix.wav` as a job tool. It shows the tool's last line, stops at `--tool-timeout`/`--tool-timeout-ratio`,
     and can be cancelled, under a new phase `finding-bar-one` before `transcribing`. It takes
     `--beat-this PROGRAM` (default `<separator venv>/bin/sona-beat-this`) and `--beat-this-threads N`
     (default 1). `bar_one=1` is still the harmony.
   - `/api/capabilities` gains `bar_one: {harmonic, "beat-this"}`. A missing tool is a setup note with the
     new status `optional`: it blocks nothing, its message says how to install it, and the job is refused
     with 409 and the same fix. An installed tool without its checkpoint is a `first-use` note.
   - Provenance: `bar_one: mix` as before, plus `bar_one_source: beat-this`, `beat_this` (version),
     `beat_this_checkpoint`, `beat_this_threads` (CLI), and the checkpoint's SHA-256 as model
     `bar_one/beat-this`.
5. **The app.** The start screen's **Options** has **Bar 1**: *First grid line* (today's rule, the default),
   *From the harmony* (ADR-0031), and *From Beat This*, which is listed only when the server has it. When the
   server lacks it, its install line shows under the options, not among the start screen's setup notes. The ♩
   popover reads "1.260 s · Beat This (97%) · the harmony agrees", or "… · the harmony says 0.260 s" in the
   warning colour: a disagreement is the case to check by ear. A re-transcription keeps the source, and falls
   back to the harmony where Beat This is gone.
6. **The eval.** `sona-eval-frame --estimate-bar-one --beat-this PROGRAM` runs the same tool on each song's
   mix and scores bar 1 from it in a second table. That table also prints the downbeats' vote against the
   music's bar line and the beats' cell vote, the two numbers ticket 15's `score.py` computes. The first
   table is unchanged. Holdout4's read can then compare all three rules against ear labels (ticket 05's
   pre-registration, now with a third arm).

## Evidence (dev only; no holdout audio opened, ledger unchanged)

- **Speed, one thread** (`final0`, the installed tool, the 10 dev mixes one after another; load average
  27–39 on 16 cores when each started):
  - CPU: 57–101 s per song, median 88 s per 4 minutes of song (range 77–98);
  - wall: 80–157 s per song, median **133 s per 4 minutes** (range 112–148).

  The machine was never unloaded, so wall time is CPU time plus waiting for a core. One thread can't use
  more than one core, so on an idle machine wall ≈ CPU ≈ 1.5 minutes per 4-minute song. That would pass
  ticket 15's 120 s gate, but it was **not measured idle here**. Ticket 15 measured the same two songs at
  57–60 s CPU at load 14–19, so contention inflates the CPU seconds too.
- **Same output as ticket 15**: beats and downbeats identical to ticket 15's default-thread run on 10/10 dev
  songs, with the separator venv's `rotary-embedding-torch` 0.6.5 (audio-separator pins < 0.7; ticket 15
  had 0.9.2).
- **Port check**: on the same tool output, `sona-eval-frame`'s bar vote and beat-cell vote equal
  `score.py`'s on 9/9 songs, and the shares agree to 0.01.
- **Accuracy (circular, as ADR-0031's)**: Beat This 9/9 [9, 9], +4 [+1, +7] vs today, +0 vs the harmony;
  the two agree on every dev song.
- **Tests**:
  - core: synthetic songs at 150–240 BPM with downbeats on each beat of the bar; half-tempo downbeats;
    a 10-to-3 split vote and its share; a pickup before the first beat; ties; no downbeats;
  - CLI and server: fake tool scripts for the mapping, the default thread count and the mix path, a
    missing tool, a hung tool stopped at its deadline (the queue moves on), a failing tool, the capability
    report and its 409;
  - web: the choices offered by capability, the query, the re-transcription fallback, the label and its
    flag, the new phase.

  Mutants, each caught, are listed in the ticket's answer.

## Consequences

- Nothing changes unless the user installs the tool and picks it. The default build links no ML runtime.
- A Beat This job costs about one more CPU minute and a half per 4-minute song, run before transcription.
  Separation costs more than that. The downbeats aren't cached, so a re-transcription at a new BPM runs the
  tool again.
- Making Beat This (or the harmony) the default bar 1 needs holdout4 with ear-labelled frames under ticket
  05's rule, which now scores three arms. madmom (CC BY-NC-SA models) and Essentia (AGPL) stay eval
  references (ticket 15).
- Beat This's README notes that some of its *training data* is copyrighted or under limited licences, "and
  it is up to the user to assess whether this may impact their use case". The tool is installed by the user,
  not shipped.
