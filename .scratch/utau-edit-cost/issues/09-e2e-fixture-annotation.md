# 09 — Browser drivers need a committed fixture, not a lost annotation

Status: ready-for-agent
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
