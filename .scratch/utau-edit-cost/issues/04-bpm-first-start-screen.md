# 04 — Ask for the BPM first

Status: resolved
Type: task
Blocked by: —

## What

Users usually know their song's exact BPM (user, 2026-11-24). Change the start screen (ADR-0022) so the
BPM field asks for the song's BPM; the estimate becomes a visible hint ("detected ≈ 246, ½× = 123") rather
than a pre-filled value. Accept decimal BPMs. Transcribe still needs a usable tempo.

- Rules live in the pure reducer `web/lib/startFlow.ts`; update its tests and the TLA+ model
  `verification/start-flow` (mutants must still be caught).
- Update the browser driver for the start screen in `web/e2e`.
- Record the change as an ADR amending ADR-0022.

## Acceptance

- Reducer tests and the 3 000 random event orders pass; TLA+ model passes and its mutants are caught.
- `web/e2e/run_all.sh` passes.
- `npx tsc --noEmit`, vitest green.

## Comments

## Answer

Done: [ADR-0028](../../../docs/adr/0028-ask-for-the-bpm-first.md), which amends ADR-0022's decision 2.

**What the user sees.** Dropping a song opens the song panel at once. It reads "Tempo [ e.g. 128 ] BPM",
and the field is empty and focused, so a known BPM can be typed straight away. Decimals work with a point
or a comma. Beside the field, the hint reads "finding the tempo…", then "detected ≈ `246` `½× 123`", or
"no steady beat found". Each hint value is a button that fills the field with that whole BPM. The ½×/2×
buttons are gone from the start screen. With the field empty, Transcribe says "Enter the song's BPM
(20–400): it sets the grid the notes snap to." and starts nothing.

**Reducer** (`web/lib/startFlow.ts`):
- The `detecting` state is gone. A dropped song is `ready` with `detected: "pending"`.
- `halve`/`double` are replaced by `use-hint {multiple: 1 | 0.5}`.
- `parseBpm` reads a typed BPM, and `hintBpm` rounds the hint to a whole BPM.
- A tempo answer only sets `detected`, for its own song, and is also taken while a job runs. Cancel and
  failure keep the hint.

**Checks:**
- **Reducer tests:** 12 tests, including the 3 000 random event orders. New per-step checks: the hint is
  the shown song's estimate, a tempo answer never changes the BPM, and a new song starts empty. Two
  broken reducers were caught: answer fills BPM (4 tests failed), answer for any song (2 failed).
- **TLA+:** new invariant `BpmIsYours`, and 2 new mutants (`estimate-becomes-bpm`, `bpm-carries-over`).
  All 4 mutants are caught. All five `verification/*/check.sh` pass.
- **`web/e2e/run_all.sh`**, run against a local `sona serve` (pyin only, no GAME) and Vite proxied to it:
  - `song_to_notes` and `cancel_and_retry` (both rewritten for this) pass.
  - 8 of the 9 editor drivers pass.
  - `pitch_handles` fails at its fixed-position vibrato step (`StopIteration`, line 40). It fails
    identically on the untouched d261e6b web code with the same locally generated Denpa annotation. The
    original fixture annotation wasn't available, and the driver's screen positions are tuned to it
    (web/e2e/README.md). The cause is the fixture, not this change.
