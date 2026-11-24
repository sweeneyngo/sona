# ADR-0029: One export seam, and a frame whose bar lines are OpenUtau's

Status: accepted, 2026-11-24. Amends [ADR-0018](0018-scope-utau-authoring.md) (grid alignment). Tickets:
`.scratch/audit-2026-11-24/issues/01-midi-export-off-grid.md` (A01) and `02-export-frame-follows-editor.md` (A02).

## Context

The goal is notes that are cheap to fix in OpenUtau, and a passage in the wrong measures is the hardest fix
there is (`.scratch/utau-edit-cost/spec.md`). Two bugs worked against it:
- **A01.** UST and USTX moved the notes back by the grid phase so they sat on whole cells. The MIDI export
  didn't: it converted the notes' stem seconds to ticks, so every MIDI note sat up to one cell after its grid
  line (48 ticks in the audit's case), on a different timeline from the UST of the same session. README and
  ADR-0018 claimed the opposite.
- **A02.** Export tick 0 was the grid anchor **modulo one cell**. The editor counted beat lines from the anchor
  (the first sung onset) and drew no bars. So a note the editor showed on a beat landed on a beat in OpenUtau
  only when `floor(anchor / cell)` was a multiple of 4, and on the right bar line only one time in 16. There
  was no way to say where beat 1 is, and nowhere for a downbeat estimate (utau-edit-cost 05) to go.

## Decision

1. **One export seam.** `web/lib/sequence.ts` `exportSequence(notes, frame)` turns the snapped notes into ticks
   (sorted, monophonic, at least a tick long). MIDI, UST and USTX are all written from that `Sequence`.
   `encodeMidiFile` takes the `Sequence`, at 480 ppq, and now also writes each note's lyric (`FF 05`, UTF-8)
   and a 4/4 time signature (`FF 58`).
2. **The frame** (`web/lib/frame.ts`): BPM, division, the grid anchor and **bar 1**, a downbeat in stem seconds
   or `null`. It is a pure module: `gridLines` classifies each grid line as cell, beat or bar; `tickZero`
   places the export's tick 0; `updateFrame` applies the frame's events (tempo, grid, ruler drag, bar 1,
   auto).
   - **Bar 1 unset (the default)** keeps the old rule exactly: tick 0 is the first grid line at or after
     0 s. The editor now counts beats and bars from that line, so what it shows is what the export writes.
   - **Bar 1 set:** tick 0 is the first bar line at or after 0 s, moved back by whole bars while a note
     starts before it. A pickup sung before the first bar line goes in the bar before (tick 0 is then
     before the stem starts, as in many hand-made USTs) instead of being clamped onto tick 0.
   - Bar 1 is kept as a time in the stem, on the grid line nearest it, so a new BPM or grid keeps it where
     the user heard it.
3. **The editor** draws bar lines (brightest, numbered in the ruler as OpenUtau numbers them, tick 0 = bar 1)
   and beat lines from the frame. **Shift-click on the ruler** makes the nearest grid line a bar line; the ♩
   popover shows **Bar 1** with **At playhead** and **Auto**. A ruler drag still shifts the grid phase, and now
   carries bar 1 with it (the first drag pins the current bar 1), so the bars don't jump a cell when the
   phase wraps. Bar 1 is saved in the session (`session.barOne`, absent = unset) and is one undo step.
4. **4/4 only.** Time signatures are roadmap 2.2. Which line the default bar 1 should be (a downbeat estimate)
   stays with A03 / utau-edit-cost 05; this ADR gives it a place to go.

## Evidence

Web tests 153/153 (`cd web && npx vitest run`), `tsc --noEmit` clean.

- **Default unchanged, byte for byte.** `lib/frame.test.ts` compares the new seam with SHA-256 digests of the
  UST and USTX that the pre-frame code (c93a284) wrote for 420 sessions:
  - the 5 committed synthetic annotations, each at 4 tempos/grids/anchors;
  - 400 random sessions: fresh and dragged anchors, all five grids, integer and decimal BPMs, escaped and
    manual notes, lyrics that need UTF-8, pitch expressions.

  The golden (`lib/fixtures/export-default-frame.json`) was written by the old `toSequence`, `ustText` and
  `ustxText` and is read back by the test. 415 of the 420 have a non-zero shift, so the shift rule is
  exercised. All 420 match.
- **Three formats, one timeline (A01).** On 300 sessions (snapped ones, and raw overlapping ones), with and
  without a bar 1, MIDI (normalised by its ppq), UST and USTX give the same `(tick, length, tone)` list, and
  MIDI and UST the same lyrics.
- **Beats on beats (A02).** For 1 000 random frames (BPM 40–300, every grid, any anchor, bar 1 anywhere in
  −5…25 s) and 500 default frames, every note drawn on a line exports to a multiple of the cell. Beat
  lines go to multiples of 480 ticks and bar lines to multiples of 1 920, and no other line does.
- **Frame events.** 200 random sequences of 25 events: bar 1 stays on a grid line, tick 0 is a bar line, the
  line the user chose is a bar line, a drag moves every line and its kind with the grid, tempo and grid
  changes keep bar 1 in the stem, Auto is the default rule, and the beats-on-beats property holds after
  every step.
- **Mutants**, each caught by the tests above:
  - tick 0 modulo a cell (the old rule) with bar 1 set: 3 tests fail;
  - beats counted from the anchor (the old drawing): 3;
  - a drag that leaves bar 1 behind: 1;
  - no step back for a pickup: 3;
  - the default path through the bar rule: the byte-identity test;
  - MIDI shifted back to stem seconds (the A01 bug): 2;
  - MIDI without lyrics: 2;
  - no overlap trim: 2.
- **Browser** (`web/e2e/bar_one.py`, Chromium, a synthetic annotation at 120 BPM with the grid phase at
  50 ms):
  - the default UST put the first note at 1200 ticks, as before, and the MIDI did too (the old MIDI wrote
    1248);
  - Shift-click on the note at 2.3 s: the ruler's bar ticks ran every 2.00 s through it, and the UST and
    MIDI put that note at 1920 and every other note on a beat;
  - Ctrl+Z / Ctrl+Shift+Z switched between the two exports;
  - a saved session reloaded to the same UST bytes;
  - a 40 ms ruler drag left every exported tick where it was;
  - At playhead put the 2.8 s note on a bar line.

  The other editor drivers pass on a synthetic `sona transcribe --grid 16` annotation, except
  `pitch_handles` (fixed screen positions, utau-edit-cost 09).

## Why no TLA+ model

The frame has one writer: synchronous UI events on one thread, with no jobs or answers arriving late. What
could go wrong is arithmetic: which line is a bar, where tick 0 falls. The random event sequences check that
on the real code. A model would check an abstraction of the same thing.

## Consequences

- MIDI output changes (A01 was a bug): its notes move earlier by the grid phase onto the UST's ticks, and it
  gains lyric and time-signature events. UST and USTX bytes change only when the user sets bar 1 or drags the
  ruler.
- The editor's beat lines move for sessions whose anchor wasn't a whole beat from the first grid line. They
  now show the beats OpenUtau will show; before, they showed beats the export ignored.
- Open, to check by hand once: does OpenUtau's MIDI import read UTF-8 `FF 05` lyrics, and does it keep
  tick 0?
