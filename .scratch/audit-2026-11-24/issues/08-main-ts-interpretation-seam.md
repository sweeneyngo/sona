# 08 — Pull note interpretation and timing edits out of `web/main.ts` so they can be tested

Status: ready-for-agent
Type: task
Blocked by: 01, 02

## Evidence

- `web/main.ts` is **2,775 lines** (`wc -l`), not the ~2,400 the audit brief assumed. It holds DOM refs,
  state, drawing, interaction, sessions, export, lyrics, pitch handles, the start flow and playback in one
  module. Rough map: load 288–423, interpretation 424–608, drawing 819–1174, interaction 1175–1621,
  session/undo 1622–1843, export 1844–1898, start flow 2389–2571.
- The logic that decides **where notes land in OpenUtau** lives here, where no unit test can reach it:
  - `effectiveNotes` (`main.ts:550-575`): quantize, `resolveOverlaps`, flags and key snap, all reading
    module state;
  - the anchor rule "first onset" (`main.ts:365`) and its session restore (`main.ts:1823-1827`);
  - the beat/grid line enumeration (`main.ts:935-948`), a second copy of `web/lib/grid.ts` `gridLines`;
  - move/resize/add-note snapping (`main.ts:1287-1298`, `~1396`), with `spacing()/2` as a minimum length;
  - BPM range checks (`main.ts:640-645`), duplicating `startFlow.ts`'s constants.
- 01 and 02 found two timing bugs in exactly this code (MIDI unshifted; beat lines vs export). Both were
  invisible to the 141 vitest tests because the tests stop at `web/lib`.
- The e2e drivers (`web/e2e/`) do exercise it, but run locally only (roadmap 6.4).

## Proposal

1. A pure `web/lib/interpret.ts`: `effectiveNotes(working, frame, opts)` and the edit-snapping
   helpers, with `frame` the object from 02. `main.ts` keeps the DOM and state and calls it.
2. Property tests through it: after any random sequence of move/resize/add/delete edits, the effective
   notes are monophonic, on the grid, and export (via 01's seam) to the same ticks the roll shows.
3. No behaviour change: the e2e drivers and the golden UST (`crates/sona-eval/fixtures/ust/
   sona-export.ust`) stay identical.

## Cost (S/M/L) and how to tell it worked

M. It worked when `main.ts` loses the interpretation code (target: under 2,500 lines), the new property
tests catch a mutant (e.g. skip `resolveOverlaps` on manual notes), and `web/e2e/run_all.sh` passes
unchanged.

## Comments

- Do not split `main.ts` further for its own sake. The value is in the timing logic, which decides the
  OpenUtau result.
