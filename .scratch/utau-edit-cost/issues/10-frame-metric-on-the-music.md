# 10 — Frame metric against the music, not the UST's grid

Status: ready-for-agent
Type: task
Blocked by: —

## Evidence

A03 (`crates/sona-eval/reports/bar-one.md`): Pifuyuu's bar lines match the music's bar on 1/9 dev songs and
his beat lines are off the music's beat on 7/9, while `alignment_offset_s` is unbiased. `sona-eval-frame`
(ticket 02) scores bar offset against the UST's tick 0, so it measures agreement with the file's start,
not with what users hear.

## Proposal

- Add manifest fields `music_beat_shift_cells` and `music_bar_shift_cells` (cells from the UST's lines to
  the music's beat and bar), seeded from the A03 table for dev, marked `unverified` until checked by ear.
- `sona-eval-frame` reports bar offset and frame pass against the music's beat and bar when the fields are
  present; the UST-based columns stay for comparison.
- For holdout4 (ticket 08), label beat and bar 1 **by ear** (click overlays, as in A03) before any read;
  a human (the user) confirms a sample. No sona output is involved in labelling.

## How to tell

Tests with click fixtures where the music's bar is shifted from the file's start by 1–3 beats; the music-
anchored offset is right and the UST-anchored one is not. A mutant that reads the UST's tick 0 fails.

## Comments
