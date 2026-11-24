# 01 — MIDI export lands off the grid and on a different timeline from UST/USTX

Status: resolved
Type: task
Blocked by: —

## Evidence

- UST and USTX go through `toSequence` (`web/main.ts:1859-1861`), which moves every note earlier by the
  grid phase `shiftS = gridPhase(anchor, spacing)` so notes sit on whole cells (`web/lib/sequence.ts:53-57`).
- MIDI does not: `exportMidi` (`web/main.ts:1844-1848`) passes `midiNotes()` (`main.ts:578-585`), which is the
  snapped notes in **stem seconds**, to `encodeMidiFile`. That function converts seconds to ticks with no shift
  (`web/lib/midi.ts:48-49`).
- Reproduced with the real modules (`vite-node`, 120 BPM, 16th grid of 120 ticks, grid phase 0.05 s, two
  notes snapped by `quantizeNote`):

  ```
  UST/USTX ticks  [[960,240],[1200,240]]      shiftS 0.05
  MIDI events     note-on 1008, off 1248, on 1248, off 1488
  ```

  So every MIDI note is 48 ticks, or 0.4 of a cell, after a grid line. Imported into OpenUtau or a DAW, no
  note sits on the grid, and the MIDI and UST of one session disagree by `shiftS`.
- The phase is nonzero in general. A fresh annotation anchors the grid on its first onset
  (`main.ts:365`), and grid-mode onsets sit at sona's estimated phase (ADR-0027), anywhere in
  `[0, cell)`.
- The docs claim the opposite. `README.md:95` says UST/USTX "write the same snapped notes as the MIDI export",
  and `docs/adr/0018-scope-utau-authoring.md:15` says "exactly the notes the MIDI export uses".
- Untested. `web/lib/midi.test.ts` only feeds `encodeMidiFile` hand-made seconds. Nothing exports the same
  session three ways and compares the results, and the whole export assembly lives in `main.ts`, where no
  test reaches it.
- `toSequence` also trims overlaps that appear after tick rounding (`sequence.ts:62-66`). The MIDI path
  does not, so a MIDI note can overlap the next one by a tick.
- Smaller gaps in the MIDI writer: it writes no lyric meta events, so pasted lyrics are lost (ADR-0018 lyrics
  only reach UST/USTX), and no time-signature event.

Why this matters for the goal: the map's Fog asks whether OpenUtau's MIDI and UST imports place notes the
same way. sona's own MIDI and UST exports already disagree before OpenUtau reads either one. A user who
takes the MIDI has to nudge **every** note onto the grid, the most expensive kind of edit in the spec's
table.

## Proposal

1. Move the export assembly out of `main.ts` into one pure function in `web/lib` (e.g.
   `exportSequence(notes, frame)`). All three writers consume its `Sequence`, so MIDI goes through
   `toSequence` like the others. Give `encodeMidiFile` a `Sequence` (ticks), not seconds.
2. Write lyrics as SMF lyric meta events (`FF 05`), plus a 4/4 time-signature event (`FF 58`).
3. Add a property test: for random note sets, anchors and BPMs, the UST, USTX and MIDI of one session give
   the same `(tick, length, tone)` list, and every tick is a multiple of the cell. Check it catches two
   mutants: MIDI unshifted (today's code), and a dropped overlap trim.
4. Fix README:95 and ADR-0018:15 to describe the result.
5. Open question to check by hand once (map Fog): does OpenUtau's MIDI import read `FF 05` lyrics and
   keep tick 0? Record the answer in the map.

## Cost (S/M/L) and how to tell it worked

S. It worked when the three-format property test passes and the mutant (no shift) fails it, and a MIDI
exported from a session where `shiftS` is not 0 imports into OpenUtau with every note on the 1/16 grid and
in the same place as the UST import.

## Comments

- Ticket 02 changes *which* grid line becomes tick 0. Do 01 first: it creates the single place where that
  rule lives.

## Answer

Done on branch `audit-a01-a02-export-frame` (from c93a284), with 02; decision in
`docs/adr/0029-export-frame-bar-one.md`.

- **One seam.** `web/lib/sequence.ts` `exportSequence(notes, frame)` builds the `Sequence`; MIDI, UST and
  USTX are all written from it. `encodeMidiFile(seq, { bpm })` takes ticks (480 ppq), not seconds, so the
  MIDI gets the shift and the overlap trim the UST had. `main.ts` builds one sequence per export
  (`sequenceForExport`); the synth keeps stem seconds (`playbackNotes`).
- **Lyrics and 4/4.** Each note's lyric is an `FF 05` event (UTF-8) at its note-on, and the track starts with
  `FF 58 04 02 18 08`.
- **Test** (`web/lib/frame.test.ts`, "MIDI, UST and USTX share one timeline"): 300 sessions (editor-snapped
  and raw overlapping notes, with and without a bar 1). MIDI (read back, normalised by its ppq), UST and USTX
  give the same `(tick, length, tone)` list, and MIDI and UST the same lyrics. The audit's case is a fixed
  test: MIDI `[960,240],[1200,240]`, where the stem-seconds MIDI wrote 1008/1248. Every tick is a multiple of
  the cell: the A02 property checks it for every line kind.
- **Mutants caught:**
  - MIDI shifted back to stem seconds (today's bug): 2 tests fail;
  - overlap trim dropped: 2;
  - MIDI without lyrics: 2.
- **Browser** (`web/e2e/bar_one.py`, synthetic annotation, phase 50 ms): the default UST and the MIDI both
  start at 1200 ticks (the old MIDI wrote 1248).
- README:95 and ADR-0018:15 now describe this.
- **Not done (needs OpenUtau by hand, map Fog):** whether OpenUtau's MIDI import reads UTF-8 `FF 05` lyrics
  and keeps tick 0. Not recorded in the map: this ticket was told not to edit map.md.
