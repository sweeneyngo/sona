# 02 — Frame metrics: bar offset, tempo multiple, BPM drift

Status: ready-for-agent
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
