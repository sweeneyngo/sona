# 02 — Make OpenUtau's beats and measures match the editor's (export tick 0 is chosen modulo one cell)

Status: ready-for-agent
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
