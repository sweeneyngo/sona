# 16 — Beat This! downbeats as a second bar-1 source (opt-in until holdout4)

Status: claimed
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
