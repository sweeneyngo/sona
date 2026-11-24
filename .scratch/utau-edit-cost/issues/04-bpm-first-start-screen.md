# 04 — Ask for the BPM first

Status: ready-for-agent
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
