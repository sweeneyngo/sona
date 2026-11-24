# 02 — Make OpenUtau's beats and measures match the editor's (export tick 0 is chosen modulo one cell)

Status: resolved
Type: task
Blocked by: — (do after 01, which creates the single export seam; utau-edit-cost 05 needs this to have any effect)

## Evidence

- Export tick 0 is placed at `gridPhase(anchor, spacing)`, the anchor **modulo one cell**
  (`web/lib/sequence.ts:43-46, 55`). Which beat, or which cell of the beat, the anchor marked is thrown
  away.
- The editor draws a beat line every `division/4` cells, counted **from the anchor**
  (`web/main.ts:935-948`), and the anchor starts on the first sung onset (`main.ts:365`). It draws no bar
  lines at all; there is no "bar" or "measure" anywhere in `main.ts`.
- So the editor and the export agree on beats only when `floor(anchor / cell)` is a multiple of 4.
  Reproduced with the real `toSequence` (`vite-node`, 120 BPM, 16th grid, 480 ticks per beat):

  ```
  first note at 1.05s : editor shows it ON a beat; export tick  960 -> on a beat in OpenUtau
  first note at 1.175s: editor shows it ON a beat; export tick 1080 -> 1 cell after a beat
  first note at 1.3s  : editor shows it ON a beat; export tick 1200 -> 2 cells after a beat
  first note at 1.425s: editor shows it ON a beat; export tick 1320 -> 3 cells after a beat
  anchor +0 / +1 beat / +1 cell -> export position 960 960 960
  ```

  Three times in four, what looks right in sona lands on the off-beat 16ths in OpenUtau. Dragging the ruler
  (`main.ts:1181, 1280`) to put the beat lines where the user hears them changes nothing in the export
  unless the drag is a fraction of a cell.
- For bars it is worse: tick 0 is the bar line, so the measure is right only when
  `floor(anchor / cell) ≡ 0 (mod 16)`.
- This is upstream of utau-edit-cost ticket 05 ("shift sona's tick 0 to beat 1"). Today there is no place
  to put a downbeat. Even a perfect downbeat estimate fed in as the anchor would be reduced modulo a cell.
  Ticket 02 there (frame metrics) would score the export rule, not the estimator.

Why this matters for the goal: the spec ranks "a passage in the wrong measures" as the hardest fix in
OpenUtau. Today a user can't fix it in sona either. sona shows beat lines, but the export doesn't honour
them.

## Proposal

1. One **frame** (BPM, cell, bar-1 time) owned by the export seam from 01. The export places tick 0 at
   `gridPhase(bar1, barSeconds)`, not modulo a cell. Keep the 4/4 assumption and note it in the README;
   time signatures are roadmap 2.2.
2. The editor draws bar lines from the same frame, and gives a direct control for beat 1: "this beat is
   beat 1" on a ruler click, or ruler drags snap by cells while the bar number follows. This replaces the
   implicit "first onset is a downbeat".
3. The default bar 1 stays whatever 03 / utau-05 decide. This ticket only makes the frame explicit and
   carries it through to export. With the default unchanged, the UST bytes for existing sessions must stay
   identical: store the frame in the session, and default it to today's rule when absent.
4. Property test through the seam: for random anchors, BPMs and divisions, a note the editor draws on a beat
   line exports to a tick ≡ 0 (mod 480), and on a bar line to ≡ 0 (mod 1920). Mutant: today's
   modulo-a-cell rule must fail it.

## Cost (S/M/L) and how to tell it worked

S–M (pure function, session field, ruler drawing; a browser driver for the bar-1 control). It worked when
the property test passes and catches the mutant, and when choosing beat 1 in sona and exporting a UST
puts that beat on an OpenUtau bar line (one manual check).

## Comments

- If the app moves to "ask for the BPM first" (utau-04), asking "where is beat 1?" on the same screen, or
  after transcription, is the natural place for the control.

## Answer

Done on branch `audit-a01-a02-export-frame` (from c93a284), with 01; decision in
`docs/adr/0029-export-frame-bar-one.md`.

- **The frame** (`web/lib/frame.ts`, pure): BPM, division, anchor and `barOne` (stem seconds, or `null`).
  - `gridLines` classifies every grid line as cell, beat or bar.
  - `tickZero` places tick 0 on a bar line: with `barOne` unset, exactly today's rule (the first grid line,
    the anchor modulo a cell); with it set, the first bar line at or after 0 s, moved back whole bars while
    a note starts before it (a pickup goes in the bar before, not onto tick 0).
  - `updateFrame` applies the events: tempo, grid, ruler drag (bar 1 moves with the grid), bar 1, auto.
- **Editor.**
  - Bar lines (brightest) and beat lines come from the frame, and the ruler numbers the bars as OpenUtau will
    (tick 0 = bar 1).
  - **Shift-click on the ruler** makes the nearest grid line a bar line. The ♩ popover has **Bar 1** with
    **At playhead** and **Auto**.
  - Bar 1 is saved as `session.barOne` (absent = unset) and is one undo step.
  - 4/4 only; noted in the README.
- **Default unchanged.** Byte-identity test against SHA-256 digests the pre-frame code (c93a284) wrote:
  - 420 sessions, all matching: the 5 committed synthetic annotations × 4 settings, plus 400 random sessions;
  - 415 of them have a non-zero shift;
  - golden in `web/lib/fixtures/export-default-frame.json`.
- **Property** (`frame.test.ts`): for 1 000 random frames with a bar 1 and 500 default frames, a note on a
  beat line exports to a multiple of 480, on a bar line to a multiple of 1920, and on no other line to
  either. Random event sequences (200 × 25 steps) check the same after every event, plus:
  - bar 1 stays on the grid;
  - the chosen line is a bar line;
  - a drag moves every line with the grid.
- **Mutants caught:**
  - today's modulo-a-cell tick 0: 3 tests fail;
  - beats counted from the anchor (today's drawing): 3;
  - a drag that leaves bar 1 behind: 1;
  - no pickup step-back: 3;
  - the default going through the bar rule: the byte-identity test fails.
- **TLA+:** not added. There is one synchronous writer and no async events; the random sequences check the
  arithmetic on the real code (ADR-0029, "Why no TLA+ model").
- **Browser** (`web/e2e/bar_one.py`):
  - Shift-click on the 2.3 s note: ruler bar ticks every 2.00 s through it; UST and MIDI put it at 1920 and
    every other note on a beat;
  - undo/redo, the session reload (same UST bytes), a 40 ms ruler drag (ticks unchanged) and At playhead
    all pass.
- **Not done:** the manual check in OpenUtau (choose beat 1, export, see it on an OpenUtau bar line). The
  default bar 1 stays with 03 / utau-05.
