# 09 — Browser drivers need a committed fixture, not a lost annotation

Status: resolved
Type: task
Blocked by: —

## Evidence

- `web/e2e/README.md` says the drivers were tuned on "the *Denpa* annotation at 125 BPM" with a 1440–1500 ×
  860 viewport. That annotation is not in the repo and no longer exists on this machine.
- During ticket 04, `pitch_handles.py` failed at its vibrato step (`StopIteration`, line 40) with a fresh
  `sona transcribe` annotation, **identically on the untouched d261e6b code**. It clicks fixed screen
  positions, so any other material breaks it.

## Proposal

- Commit a **synthetic** annotation + stem (generated, not corpus audio: `crates/*/fixtures/` is where the
  corpus guard allows them) with notes long enough for vibrato and pitch points, and make it the drivers'
  default when `SONA_E2E_ANNOTATION` is unset.
- Make `pitch_handles.py` (and any other driver that clicks coordinates) find targets from the DOM/canvas
  model instead of fixed pixels where practical.

## How to tell

`web/e2e/run_all.sh` passes on a clean checkout with only `SONA_E2E_SONG` set (or none, with
`SONA_E2E_SKIP_SERVE=1`). Break one handle's hit-test and see the driver fail.

## Comments

## Answer

Resolved 2026-11-24. `SONA_E2E_SKIP_SERVE=1 web/e2e/run_all.sh` passes with no annotation, stem or song
set: all ten editor drivers pass (`bar_one`, `pitch_handles`, `pitch_pen`, `sung_pitch_export`,
`lyrics_export`, `fit_and_export`, `stale_hover`, `lite_autosave`, `motion`, `undo_redo`).

**The fixture is generated, not committed.** `web/e2e/fixture.py` writes it the first time a driver
imports `_env.py` without `SONA_E2E_ANNOTATION`, into `$SONA_E2E_OUT/fixture-<hash of fixture.py>/`. That
takes about 1.5 s and happens once per version of the file. It holds eight two-bar phrases at 125 BPM in
C major: 48 notes, with four short legato notes gliding into a 1.68 s held note. The held note has
5.5 Hz ±35-cent vibrato and a falling tail before a rest. The annotation has a 10 ms f0 contour and an
energy envelope. The stem is a 22.05 kHz three-harmonic tone that follows that contour. Fit pitch finds
48 glides and 8 vibratos on it. Nothing is tracked, so the corpus guard has nothing to allow.

**No fixed screen positions.**
- `_env.py` gains:
  - `FIND_NOTES`: note bars as rectangles, widest first. It tolerates the contour drawn under the
    translucent fill, and merges the pieces of one bar.
  - `watch`: collects console and page errors, skipping failed `/api` calls (the capability probe
    without `sona serve`).
  - `check` and `finish`.
- `pitch_handles`:
  - It zooms, then pans until two whole notes at least 100 px wide are in view.
  - It reads a note's row from its own first handle, and finds the notes again after selecting. The
    pitch bar that appears on selection shrinks the roll, which is what broke the old clicks.
  - Each step now asserts on the saved expression: a double-click on the curve adds a point (not a
    note), the point drags, Alt-click removes the middle point, V adds the default vibrato, the
    fade-out drag raises depth and fade-out, the fade-in drag changes fade-in only, and the period and
    start drags take effect. A typed depth is committed by touching a handle.
  - It passes at 1024 × 700, 1280 × 720, 1500 × 860 and 1920 × 1080.
- `stale_hover` and `pitch_pen` use the same targeting. `pitch_pen` now checks that a stroke one row
  above a note draws that note's pitch + 100 cents inside it, and `lite_autosave` draws relative to the
  roll.
- `sung_pitch_export` now checks its README promise: the sung USTX equals the plain one apart from the
  `pitd` curve. `lyrics_export`, `fit_and_export`, `undo_redo` and `motion` turn their printouts into
  checks.
- No app change and no test hook were needed.

**Mutant.** Skipping `vibrato-period` in `handleAtPoint` (`web/main.ts`) makes `pitch_handles` fail with
`FAIL  period handle →: period 175 → 175` and exit 1. The code was restored afterwards.

**Still needs real material.** `song_to_notes` and `cancel_and_retry` need `sona serve` and a song
(`SONA_E2E_SONG`). The fixture cannot show how the drivers behave on a dense real transcription with
short notes. `SONA_E2E_ANNOTATION`/`SONA_E2E_STEM` still override the fixture, but that path was not run
here because no real annotation was at hand. The targeting needs some notes at least about 100 px wide
after two zoom steps.
