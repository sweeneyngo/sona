# 02 — Frame metrics: bar offset, tempo multiple, BPM drift

Status: resolved
Type: task
Blocked by: —

## What

Per song, measure whether sona's timing frame matches the UST's:

- **Bar offset:** with the UST's true BPM given, how many beats sona's bar 1 sits from the UST's bar 1
  (UST tick 0 is bar 1; audio time = UST time + manifest `alignment_offset_s`; sona's tick 0 comes from
  the grid phase, ADR-0018/0027). Report the offset in beats mod 4 (assume 4/4; see the map's Fog).
- **Tempo multiple:** estimated / true BPM, classified as 1×, 2×, ½×, other (`sona_core::tempo`).
- **BPM drift:** |estimated − true| / true × song length in beats. Report it for songs whose multiple is
  1×.
- **Frame pass:** bar offset 0 and drift below a quarter beat.

Extend `sona-eval-tempo` / `sona-eval-phase`, or add one `sona-eval-frame` binary.

## Acceptance

- Tests on click-track fixtures with known BPM and a known bar-1 position (shifted by 1, 2, 3 beats).
- A mutant that drops the phase shift is caught.
- Runs on `dev`. No holdout read here (03 does the re-reads).

## Comments

## Answer

Resolved 2026-11-24. Code: `sona_eval::frame` (definitions and `measure`), the `sona-eval-frame` binary
(dev songs only; it has no split flag), tests in `crates/sona-eval/tests/frame.rs`.

### How sona chooses tick 0 today

- **Export (UST, USTX):** `web/lib/sequence.ts` `toSequence` puts tick 0 at
  `shiftS = gridAnchor mod spacing`, which is the first grid line at or after audio time 0. Every note
  becomes `round((t − shiftS) · ticks per second)`.
- **`gridAnchor`** (`web/main.ts`) is the earliest note onset of a fresh annotation, or the value saved
  in the session, and the user can drag it. The notes sit on the decoder's grid, so `gridAnchor mod
  spacing` is the decoder's phase:
  - learned cell stage: `sona_core::tempo::grid_phase` on the vocals stem (ADR-0027);
  - GAME only: `snap_to_grid`'s circular mean of the onsets;
  - pYIN grid mode: the decoder's own phase search.
- **MIDI export** (`encodeMidiFile`) does not shift anything: tick 0 is audio time 0 and the notes sit
  `shiftS` off its grid.
- **sona never looks for beat 1.** Its bar 1 is wherever the audio file starts, less than one cell
  later. So the bar offset depends only on how the file's start lines up with the song's bars, never on
  anything sona hears.
- **Edge case**, seen on a click track whose clicks start exactly at audio time 0: the phase estimate
  can land a hair below a full cell. It then wraps, and tick 0 moves one cell late.

### Definitions (4/4 assumed; `sona_eval::frame`)

- **sona's tick 0:** `grid_phase(vocals, spacing at the UST's exact BPM) mod spacing`, matching the
  cell stage plus the export.
- **UST's tick 0:** the manifest's `alignment_offset_s` (stem time = UST time + offset).
- **Bar offset:** `(sona tick 0 − UST tick 0) / beat`, taken mod 4.
  - The nearest whole beat (`0..4`) is the bar offset.
  - What remains, in `[−0.5, 0.5)` beat, is the residual.
  - Positive means sona's bar 1 is late.
- **Tempo multiple:** `estimate_bpm(full mix)` / UST BPM, within 2% (as in `sona-eval-tempo`):
  1×, 2×, ½× or other.
- **Drift:** `|est − true| / true` × the mix's length in beats at the true tempo. Reported for 1× only.
- **Frame pass:** 1×, drift < 0.25 beat and bar offset 0 (the residual is not part of it, as the ticket
  defines it).
- `drift@int` is the drift had the estimate been rounded to a whole BPM. The corpus is integer-tempo by
  construction, so this column flatters rounding.

### Dev table (`cargo run --release -p sona-eval --bin sona-eval-frame`)

9 aligned dev songs. plus-space is skipped because it is `flagged`.

| song | grid | UST BPM | est | mult | drift (beats) | drift@int | bar | residual (beats) | frame |
|---|---|---|---|---|---|---|---|---|---|
| innocent-addict | 16 | 198 | 198.74 | 1× | 3.44 | 4.68 | 0 | +0.255 | FAIL |
| kamaitachi-ni-yoru | 16 | 144 | 143.61 | 1× | 1.28 | 0.00 | 0 | +0.259 | FAIL |
| telescope-nevermore | 16 | 123 | 246.13 | 2× | – | – | 0 | +0.255 | FAIL |
| atamaro-deluxe | 24 | 244 | 244.57 | 1× | 1.91 | 3.34 | 0 | +0.171 | FAIL |
| azemichi-ni-ochita-ginga | 16 | 196 | 195.90 | 1× | 0.39 | 0.00 | 0 | +0.249 | FAIL |
| yamineko | 16 | 250 | 249.14 | 1× | 2.50 | 2.91 | 0 | +0.259 | FAIL |
| my-axiom | 16 | 195 | 194.98 | 1× | 0.07 | 0.00 | 0 | +0.254 | pass |
| amaten | 16 | 163 | 162.80 | 1× | 0.52 | 0.00 | 0 | +0.262 | FAIL |
| fukagyakuteki-henyou | 16 | 236 | 235.01 | 1× | 3.83 | 3.86 | 0 | +0.247 | FAIL |

**Frame pass: 1/9** (11%, song bootstrap 95% interval 0–33%). The other counts:

- 1× tempo: 8/9;
- drift < 0.25 beat: 1/9;
- bar offset 0: 9/9;
- |residual| < ⅛ beat: 0/9.

### What the numbers say

- **With sona's own BPM, drift fails the frame, not the bar.** The estimate is a 1× within 2% on 8 songs.
  Even so, 0.01–0.42% of tempo error over a whole song slips 0.07–3.8 beats.
  - Every 1× song but my-axiom fails on drift.
  - Rounding the estimate to an integer zeroes drift on 4 of 8. That leans on the corpus's
    integer-tempo bias.
  - The user typing the true BPM (spec: they usually know it) removes drift entirely. This is
    ticket 04's case.
- **Bar offset is 0 on all 9, but not because sona finds bar 1.**
  - On every dev song the UST's bar 1 maps to 16 beats (4 bars) plus 0.08–0.24 beat before audio
    time 0. That is 28–100 ms before the file starts, mod a bar.
  - So the files start just after a UST bar line, and "first grid line after 0" lands in the right
    measure.
  - This is a property of these files (official uploads that open on a bar line, and Pifuyuu's 4-bar
    lead-in). Real users' audio, with an intro, a pickup or a trimmed start, will not have it. The 9/9
    says nothing about them.
- **A uniform one-cell residual on all 9 songs** (+0.25 beat; +0.17 = one 24th-grid cell on atamaro).
  - The UST's bar line sits about 0.2 beat before audio 0, and sona takes the next cell line after 0.
  - So sona's export puts every note one cell earlier in the measure than the UST does.
  - The ticket's frame pass ignores this, and the strict version (|residual| < ⅛ beat) passes 0/9.
  - Whether it costs a select-all-and-move in OpenUtau, like a whole-beat offset, is for 01/03 to
    weigh.
  - Taking the grid line just *before* audio 0 instead would zero it on all 9. That fits this corpus's
    convention and is ticket 05's question, not a fix to adopt from 9 songs.
- **Premise check on the UST frame as truth** (a diagnostic run, not committed):
  - In 6 of the 9 dev USTs (innocent-addict, kamaitachi, telescope, yamineko, amaten, fukagyakuteki),
    71–100% of the lead's note starts sit on the 2nd or 4th 16th of the beat. Examples: yamineko 0 of
    597 on a beat or an 8th; innocent-addict 641 of 679 on odd 16ths.
  - On those 6 songs, `grid_phase` on the drums stem at one-beat spacing peaks 1 cell (yamineko:
    3 cells) after the UST's beat lines.
  - So on those songs Pifuyuu's beat grid sits a 16th off the music's beats. On 5 of them, sona's
    tick 0 lands on the drums' beat.
  - On azemichi and my-axiom (notes on beats and 8ths, drums at 0 cells) the UST frame matches the
    music, and sona's one-cell residual is a real miss.
  - The note-to-vocals alignment is not in question: it was verified by onset deltas, and dev cell
    accuracy is 92%. What is in question is the UST's own bar grid. The residual column measures
    agreement with the UST, not with the music.
- **Tests:**
  - Click tracks at 150/180/200 BPM, with bar 1 shifted 0, 1, 2 and 3 beats into the audio, and
    before it, give offsets (4−k) mod 4 and k, with |residual| < 0.02 beat.
  - A mutant that drops the phase shift (tick 0 = audio 0) fails 2 of 5 tests, with residual −0.15
    beat.
  - The end-to-end `measure` on a 180 BPM click track passes. Shifting it by one beat gives bar
    offset 3, and a 110 BPM track reads 2×.
