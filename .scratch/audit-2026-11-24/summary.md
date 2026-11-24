# Audit 2026-11-24: summary

Read-only audit of sona against the OpenUtau edit-cost goal (`../utau-edit-cost/spec.md`), branch
`audit-2026-11-24` (79edef3, built on 2b7cbde). Drafts in `issues/`. Triaged the same day (statuses in each
file); the order and grouping below are the triage.

| # | Title | Why it matters for the OpenUtau goal | Cost | Triage |
|---|---|---|---|---|
| 01 | MIDI export lands off the grid, on a different timeline from UST/USTX | Every MIDI note sits up to one cell after its grid line; README and ADR-0018 claim otherwise | S | ready-for-agent, with 02 |
| 02 | OpenUtau's beats and measures match the editor's | Export tick 0 is the anchor mod one cell, so editor beats match OpenUtau's 1 time in 4; utau-05 has nowhere to put beat 1 | S–M | ready-for-agent, with 01 |
| 03 | The bar offset is sub-beat; "nearest grid line to 0 s" as a candidate prior | Even a perfect phase lands bar 1 one 16th late on 10/10 dev songs, unless the alignment offsets are biased | S | ready-for-agent (research), merged with utau-02's drums finding |
| 04 | Re-transcribing costs a full re-run; job dirs never deleted | A BPM fix should be a quick re-run, but repeats demucs + UVR (~16 min for 2.6 min of audio); `/tmp` is RAM on Fedora | M | ready-for-agent, with 05 |
| 05 | `sona serve` worker failure modes | A panic kills the only worker silently; a hung child blocks the queue; 48 kHz stems break GAME | S–M | ready-for-agent, with 04 |
| 06 | Output invariants skip the engines sona recommends | GAME and cell-stage note assembly are pure functions CI could fuzz without ONNX | S | ready-for-agent |
| 07 | Delete knobs and paths that were measured and lost | Project rule; the release sub-state needs a Vocadito re-gate | S | ready-for-agent, after 05 |
| 08 | Pull note interpretation and timing out of `web/main.ts` (2,775 lines) | Both export-timing bugs live where no unit test reaches | M | ready-for-agent, blocked by 01/02 |
| 09 | Docs that disagree with the code | MIDI claim, CONTEXT.md grid-mode and phase-drag lines, ADR-0016, `--help` | S | ready-for-agent, last |

**Checked and fine:** the UST writer (rests, CRLF, Shift-JIS/UTF-8, Mode2 pitch round-trip, Rust golden),
the USTX writer (480 ppq, one tempo, 4/4), UST tempo rounding (~8 ms over 4 min), silent stems, 24/32-bit
and float WAVs, stereo downmix, no O(n²) on long songs, serve's Host check, path guard and cancel,
`grid.ts` ↔ `quantize.rs` shared fixtures. `cargo test --workspace --release` 259 passed; vitest 141/141.

**Dismissed:** Rust/TS correction parity (utau-01 covers it); the off-grid escape hatch after a BPM change
(never fires: 0/735 notes qualify; re-snapping instead of re-decoding is folded into 04); five
grid-phase estimators (differ by design); the GAME deterministic-kernels mutant (needs weights, not
CI-able); e2e in CI (roadmap 6.4); USTX without a singer (OpenUtau asks anyway). Not ticketed: the source
song as an aligned USTX `wave_part`, worth a look after 02.

**Measured:** pYIN grid on a 281 s stem: 155 s wall on one core of 16 (pYIN front end dominates; profile
before acting). Full serve job on a 158 s song: demucs 546 s, UVR 330 s, transcribe 93 s (machine loaded,
so separation is inflated); job dir 134 MB, never deleted.

## Progress

- 2026-11-24: **A01, A02 resolved** (merged; ADR-0029): one export seam; MIDI on the UST/USTX ticks with
  lyrics and 4/4; bar lines in the editor; Shift-click the ruler or ♩ → Bar 1 sets beat 1; default exports
  byte-identical (420 sessions vs c93a284's own output).
- 2026-11-24: **A03 resolved** (merged; `crates/sona-eval/reports/bar-one.md`).
- 2026-11-24: **A04 (items 1, 4, 5), A05 resolved** (merged; ADR-0030): stem cache by upload hash
  (re-run 17–23 s vs 223 s), jobs under `~/.cache/sona/jobs` with keep-20/keep-10 cleanup, panic guard,
  tool deadlines, Origin check, 48 kHz resampling, separator progress in the UI. Left over → 10, 11, 12.
- Needs a human in OpenUtau: MIDI import of UTF-8 lyric events and tick 0; a UST with a chosen beat 1
  lands on an OpenUtau bar line.
- 2026-11-24: **A06 resolved** (merged): invariants now fuzz GAME's `snap_to_grid` (+ octave check), the cell stage's `snap_notes`/`cells_to_notes`, and the stage minus the model; no violations found; runs in CI without weights.
- 2026-11-24: **A13 resolved** (merged via PR #1, CI green including the new `e2e` job): the 10 editor browser drivers run in CI through `scripts/ci/e2e.sh`.
- 2026-11-24: **A12 resolved** (merged): the start screen and `sona serve` say what's missing (ffmpeg, venvs) and what downloads on first use (demucs weights, UVR model); tempo errors show the server's reason. GAME/cell-model greying still unexplained (left).
- 2026-11-24: **A07 resolved** (merged): release sub-state, spectral-flux onsets, `crepe`, `--salience`/`--spectral-cue` (now unknown options), `from_reference`, test-only web exports removed (~1,250 lines); grid path byte-identical; Vocadito COnP 0.6463 → 0.6466. New ticket 14 (dev cache not bit-exact).
