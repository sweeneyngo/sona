# 10 — Frame metric against the music, not the UST's grid

Status: resolved
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

## Answer

Resolved 2026-11-24. Dev only: no `--split`, and the holdout ledger is unchanged. Holdout entries are not
touched.

**What changed**

- **Manifest.** There are three new optional fields per song: `music_beat_shift_cells`,
  `music_bar_shift_cells` and `music_frame_status`.
  - They are seeded for the 9 aligned dev songs from `reports/bar-one.md` (beat +1, +1, +1, +2, 0, +3,
    0, +1, +1; bar +13, +9, +1, +2, 0, +3, +4, +9, +1, in table order), all `unverified-estimate`.
  - plus-space is flagged and has no values.
  - `Song::music_frame()` reads them. The manifest test requires all three fields or none, a known
    status, and a bar line that falls on the beat. It also checks that `save()` rewrites the committed
    JSON byte for byte, and the new fields are skipped when absent.
- **`sona_eval::frame`.**
  - `MusicFrame` gives `bar_line_s` and `beat_line_s` as whole cells after the UST's lines.
  - `measure(…, music)` now returns `music_bar_offset` beside `bar_offset`, and
    `Frame::passes_on_music()` sits beside `passes()`.
  - `click_times` and `overlay_clicks` build the ear-check overlays.
- **`sona-eval-frame`.**
  - The default table adds the music columns (bar, residual, frame, `est`/`ear`) next to the UST ones.
    This tool is new and nothing parses its output.
  - `--bar-one` is unchanged and its output is identical.
  - `--clicks SONG [--bar-shift CELLS]` writes the mix with clicks on a candidate grid.
  - The how-to for holdout4 is in `reports/bar-one.md`, "Ear-labelling the music frame". The tool was run
    on one dev song only (kamaitachi).

**Dev table** (`cargo run --release -p sona-eval --bin sona-eval-frame`; tempo from sona's own estimate)

| song | grid | BPM | est | mult | drift | UST bar | UST residual | UST frame | music bar | music residual | music frame |
|---|---|---|---|---|---|---|---|---|---|---|---|
| innocent-addict | 16 | 198 | 198.74 | 1x | 3.44 | 0 | +0.255 | FAIL | 1 | +0.005 | FAIL |
| kamaitachi-ni-yoru | 16 | 144 | 143.61 | 1x | 1.28 | 0 | +0.259 | FAIL | 2 | +0.009 | FAIL |
| telescope-nevermore | 16 | 123 | 246.13 | 2x | - | 0 | +0.255 | FAIL | 0 | +0.005 | FAIL |
| atamaro-deluxe | 24 | 244 | 244.57 | 1x | 1.91 | 0 | +0.171 | FAIL | 0 | −0.162 | FAIL |
| azemichi-ni-ochita-ginga | 16 | 196 | 195.90 | 1x | 0.39 | 0 | +0.249 | FAIL | 0 | +0.249 | FAIL |
| yamineko | 16 | 250 | 249.14 | 1x | 2.50 | 0 | +0.259 | FAIL | 0 | −0.491 | FAIL |
| my-axiom | 16 | 195 | 194.98 | 1x | 0.07 | 0 | +0.254 | **pass** | 3 | +0.254 | FAIL |
| amaten | 16 | 163 | 162.80 | 1x | 0.52 | 0 | +0.262 | FAIL | 2 | +0.012 | FAIL |
| fukagyakuteki-henyou | 16 | 236 | 235.01 | 1x | 3.83 | 0 | +0.247 | FAIL | 0 | −0.003 | FAIL |
| **pooled (9)** | | | | 1x 8/9 | < ¼ beat 1/9 | **0 on 9/9** | < ⅛ beat 0/9 | **1/9** | **0 on 5/9** | < ⅛ beat 5/9 | **0/9** |

**Reading the table**

- The UST-anchored columns reproduce ticket 02: frame pass 1/9, bar offset 0 on 9/9.
- Against the music, the frame passes on 0/9. Drift still fails 8/9 with sona's own BPM. With a typed
  BPM (ticket 04), frame pass equals bar offset 0, which is **5/9 against the music, not 9/9**.
  - On 4 songs sona's bar 1 sits whole beats off the music's: innocent-addict 1, kamaitachi 2, amaten 2,
    my-axiom 3.
  - my-axiom's only "pass" was against Pifuyuu's lines.
- **The residual flips too.** Against the UST, sona's tick 0 is a cell late on 9/9 (it is the export's
  first cell line after 0 s). Against the music, it is on the beat (|residual| < ⅛) on 5/9: innocent,
  kamaitachi, telescope, amaten, fukagyakuteki.
- yamineko's −0.491 is two cells, half a beat, which sits on the boundary between bar offsets 0 and 3.
  Its bar call (0/4 cues) is also the weakest in A03.
- All music values are `unverified-estimate`: the bar is a harmonic downbeat with no ground truth. The
  music columns are the right *reference*, but their numbers are only as good as the A03 estimates until
  they are ear-checked.

**Tests** (`crates/sona-eval/tests/frame.rs`)

The click fixtures accent the music's downbeat 1–3 beats after the file's start, at 150, 180 and 200 BPM,
with the music's beat also a cell after the UST's lines in one test.

- The music-anchored offset is `4 − k` beats with residual ≈ 0, or −¼ beat with the extra cell.
- The UST-anchored offset says 0 beats, which is wrong.
- The overlay's clicks sit where `click_times` says (2 kHz on bar lines, 1 kHz on the other beats), and
  sona's `grid_phase` hears them there within 0.05 beat.

**Mutants**

- `MusicFrame::bar_line_s` returning the UST's tick 0 fails 3 tests:
  `the_music_s_bar_shifted_from_the_file_s_start`, `the_music_s_beat_and_bar_both_off_the_ust_s_lines`
  and `music_lines_are_whole_cells_after_the_ust_s`.
- `measure` scoring the music offset against the UST's tick 0 fails 2 tests (the first two).
- After restoring, all 14 pass.
