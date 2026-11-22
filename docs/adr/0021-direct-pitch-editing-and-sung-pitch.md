# ADR-0021: Direct pitch editing on the roll, and the singer's pitch as a curve

Status: accepted (2026-11-22)

## Context

ADR-0019 fitted OpenUtau's portamento points and vibrato from the stem's pitch, and made them editable
through a row of number fields. In use, numbers were the wrong tool: a glide or vibrato is a shape, and
OpenUtau itself edits it by dragging nodes on the roll. Separately, the singer's tracked pitch (the
white line) carries more than points and vibrato can say, and OpenUtau already has a place for that: the
`pitd` expression curve its pitch pen draws.

## Decision

1. **Handles on the roll.** The selected note shows its expression as handles on the orange curve
   (`web/lib/pitchHandles.ts`): one circle per pitch point, and four diamonds for the vibrato (start =
   `length`, the top corners = fade-in/fade-out and depth, the lower one = period). Drags map straight
   back to OpenUtau's parameters, clamped to `UVibrato`'s limits; points never pass their neighbours,
   and a first point under `snap_first` keeps its height. Double-click adds a point, Alt-click or
   hover + Delete removes one, and a curve keeps at least two. The number bar stays for exact values.
2. **The singer's pitch as `pitd`.** *Export → USTX with the singer's pitch* writes
   `curves: [{abbr: pitd, xs, ys}]`: the tracked f0 minus what the exported notes and expressions already
   sing (`renderPitch` over the notes as ticks, not the editor's seconds), so OpenUtau's total pitch is
   the white line. It is written every 5 ticks (OpenUtau's pen interval), simplified to within 2 cents,
   with zeros at each voiced run's ends. Unvoiced frames, rests and frames > 600 cents off (octave slips,
   stray consonants) are left to the notes. It is a separate export so the plain USTX stays exactly as
   before (checked byte-for-byte in the browser).
3. **A pitch pen.** **P** toggles a pen that redraws the white line: a stroke overwrites the contour
   frames it crosses with a straight line in cents; Alt-drag marks them unvoiced so the notes sing there.
   Edits live in the session's contour, so fit-pitch and the sung-pitch export both see them.

## Evidence

- Unit tests: handle positions match OpenUtau's geometry; 5 000 random drags keep points ordered, inside
  the note's neighbours, `in + out ≤ 100` and every vibrato field in range. Notes + `pitd` reproduce a
  synthetic sung contour (glide, vibrato, offsets) within 4 cents; unvoiced, out-of-note and octave-slip
  frames give 0. A pen click between frames marks the nearer one. A packed contour round-trips within a
  cent. Mutants: 9/9 of the handle maths and 9/9 of the pen, the `pitd` builder and the packing are caught.
- Model check: `verification/expr-bar` now includes handle drags. **NoEditIsLost**: a drag never
  overwrites a value committed after it captured the note. The `capture-before-blur` mutant (the drag
  capturing the note before the bar's typed value commits) is caught; the shipped code commits first.
- Browser (Playwright on the real app): dragging, adding and removing points; all four vibrato handles; a
  value typed in the bar but not committed survives a handle drag. On *Denpa*, the sung-pitch export is
  7 185 points (median |pitd| 18 cents), valid YAML with OpenUtau's `pitd` descriptor, and identical to
  the plain export apart from `curves`. A flat pen stroke at 6674 cents exports so that the notes plus
  `pitd` sing exactly 6674 over it.
- Fixed on the way:
  - The canvas backing store did not follow the roll's height when the expression bar appeared, so
    pointer and drawing coordinates drifted apart (now a `ResizeObserver`).
  - Review (comment-sicko pass): Delete removed a point from a stale hover after a wheel pan; it now
    tests what is under the mouse at the moment of the key press. Replayed in the browser: the old code
    removes a point that is no longer under the cursor, the new code deletes the selected note.
  - Review: a "lite" autosave (storage quota hit) dropped the contour and with it every pen edit. It now
    keeps the contour packed (`[t, hz, voicing]`, rounded), and only as a last resort drops it, with a
    status warning. Replayed with a forced quota: the strokes survive a reload and resume.

## Consequences

- UST still cannot carry the free curve; Mode1 `PitchBend` is legacy and not written.
- The `pitd` export bakes in whatever the tracker heard, including consonant dips inside notes; the pen
  is the way to clean them up before export.
- If even the packed contour does not fit in browser storage, the autosave drops it and says so; *File →
  Save session* always keeps pen edits.
