# 08 — Pull note interpretation and timing edits out of `web/main.ts` so they can be tested

Status: resolved
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

## Answer

Done on branch `audit-a08-main-seam` (from 52d2851, rebased on 25f6c54). Code moved, not rewritten; no
behaviour change. `web/main.ts` keeps the DOM, canvas, state and event wiring and calls three pure modules:

- `web/lib/interpret.ts`: `effectiveNotes(working, frame, {flagThreshold, snapKey, key})` (quantize with the
  escape hatch, manual notes never escape, `resolveOverlaps`, flags, key snap), `firstOnset` (the "anchor on
  the first onset" rule), and the shapes the roll hands on: `exportNotes` (to `exportSequence`),
  `playbackNotes`, `renderNotes`, `notesInTimeOrder`, `noteSpan`. `main.ts` only caches the result.
- `web/lib/noteEdits.ts`: `movedNote`, `resizedNote` (the half-cell minimum), `newNote` (one cell on the
  nearest grid line, not before 0 s), `withoutNote`. `main.ts` applies them with `Object.assign` to the
  note under the pointer, so selection and drag identity are unchanged.
- `web/lib/session.ts`: `normalizeAnnotation`, `workingNotesFrom`, `baselineFrom`, `settingsOnLoad` (fresh
  annotation vs saved `session` block, including the anchor fallback that used to be in `restoreSession`),
  `sessionFile`, and the undo snapshot `docText`/`parseDoc`.

What the ticket listed and where it stands: `effectiveNotes`, the anchor rule and its session restore, and
the move/resize/add snapping are in the modules above. The beat/grid line copy was already gone (A02:
`frame.ts` `gridLines`). BPM: `scaleBpm` already uses `startFlow`'s `usableBpm`; `setBpm`'s `> 0 && finite`
test is the same as `updateFrame`'s tempo check, left in place.

`main.ts`: 2,873 → 2,680 lines. The ticket's "under 2,500" was set against 2,775 before A01/A02/A12 added
~100 lines; what is left is DOM, drawing and interaction. The expression bar's OpenUtau clamping
(`applyExprBar`) and lyric fill are the remaining pure-ish note logic; not moved (not timing, and the
expr-bar TLA+ model describes that code where it is).

Tests (vitest 161 → 180): `interpret.test.ts` property tests over random documents and 25 random edits each
(move, resize, add, delete, grid drag, BPM/grid change, snap-to-key and flag changes, applied the way
`main.ts` applies them): one voice and no note lost or reordered in 5,000 states; manual notes never escape;
`flagged` = low confidence or escaped; with nothing escaped every edge is on a grid line and `exportSequence`
puts every note on the tick, length and tone the roll shows (checked in >500 states); snap-to-key puts every
pitch in the key; undo steps back through every state exactly (document text and exported sequence) and redo
replays them. `noteEdits.test.ts`, `session.test.ts`: the edit rules, a save → reopen round trip (200 random
sessions), fresh-annotation defaults, bad session fields. `exportCases.ts` (`snapLikeTheEditor`) now calls
`effectiveNotes` instead of its own copy, so the A02 byte-identity test (420 sessions, `frame.test.ts`
unchanged) pins the editor's real interpretation; it still passes byte for byte.

Mutants, all caught: manual notes may escape (property + A02 golden), no `resolveOverlaps` (one-voice and
export properties), resize without the half-cell minimum, undo snapshot drops `snapKey` (undo property),
session anchor fallback 0 instead of the first onset, a move not marking the note manual, snap-to-key ignored.

Verification: `tsc --noEmit`, vitest 180/180, `scripts/ci/e2e.sh` 10/10 drivers pass, all five
`verification/*/check.sh` pass, corpus guard passes. No TLA+ model names a moved function (`expr-bar` and
`lyric-editor` name `syncExprBar`, `applyExprBar`, `openLyricEditor`, `commitLyric`, … which stay in
`main.ts`; the lyric editor's Enter order now comes from `interpret.notesInTimeOrder` through the same
`main.ts` wrapper).

One visible difference, by construction harmless: loading now writes all five settings controls (BPM, grid,
flag slider and readout, snap-to-key) from the state; before, a fresh annotation wrote only BPM and a session
wrote the grid and flag only when valid. The controls already mirror the state on every path, so the writes
are no-ops.

Found, not fixed (behaviour had to stay): a saved or autosaved session on the 1/16T grid (division 24)
reopens on the previous grid, because the session restore accepts only 4/8/16/32 while the grid menu and
`sona serve` jobs offer 24. The notes then snap and export differently after a reload. Worth a ticket.
`web/README.md`'s module list doesn't name the three new modules (left for the docs owner).
