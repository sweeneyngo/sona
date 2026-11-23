# ADR-0025: Undo and redo for every edit

Status: accepted (2026-11-23)

## Context

There was no undo. A mis-drag, a wrong lyric fill or a pen stroke in the wrong place could only be
reverted by hand, or by reloading and losing everything since the last save.

## Decision

**Snapshots, at the one place every edit already reports.** Each edit calls `markDirty()`. The undo
controller (`web/lib/undo.ts`, independent of the DOM) compares the document with the last state it
recorded and keeps the difference as one step.

- **The document:** the notes (timing, pitch, lyric, glides, vibrato), the tempo, grid, grid phase,
  flag threshold, key snapping and the pitch line.
- **One step per action:**
  - every drag (note, handle, pen, ruler) is a gesture, recorded when the mouse is released, and a
    gesture that changed nothing records nothing;
  - repeated changes from one source within 1.5 s (typing a tempo, the flag slider) merge into one
    step, and a burst that ends where it started records nothing;
  - a new edit after an undo drops the redo branch, and history keeps the last 200 steps.
- **The pitch line** (26 000 frames, edited in place by the pen) is shared by every step until an
  element actually changes (`SharedSnapshot`). Steps without pen edits cost no copy, and identity
  stands for equality.
- **Keys:** Ctrl/⌘+Z, and Ctrl/⌘+Shift+Z or Ctrl+Y. Inside a text field the browser's own text undo
  applies. *Edit → Undo / Redo* show when each is available.
- **History starts fresh** whenever an annotation, a session, an autosave or a transcription loads.

## Evidence

- **Unit tests (9), including:**
  - 300 random edit histories (gestures, inserts, deletes, typing bursts with pauses) must undo
    back to the start and redo every state exactly, with no empty steps;
  - a change made just before Undo is kept as its own step, not lost.
- **Mutants caught:** recording inside a gesture, not dropping redo, losing an unreported change,
  skipping the equality check, forgetting the restored state, merging across a pause, and forgetting
  the restored pitch line.
  - Clearing a step's merge key on redo was deleted rather than kept: no test could tell it apart,
    because the step's old timestamp already falls outside the merge window.
- **Browser** (`web/e2e/undo_redo.py`):
  - a tempo burst, a note drag, a lyric and a pen stroke undo one step each, back to the original
    file, and redo exactly;
  - Ctrl+Z in a text field leaves the document alone, *Edit → Undo* works, and reopening a file
    leaves Undo and Redo disabled.
- **Found by the browser driver:**
  - a burst that returned to its starting tempo left an empty step;
  - after an undo, the restored pitch line was a fresh copy, so the next undo first recorded a
    spurious step and then consumed it (one Ctrl+Z did nothing);
  - choosing the same file again in the file picker did nothing, because the input kept its value and
    never fired `change`. This bug predates undo.

  All three are fixed and tested.
- **Found by review** (comment-sicko pass), fixed, and each caught by its own browser check when its fix
  is reverted:
  - undoing with focus in the pitch bar left the bar showing a note the undo had replaced, so edits
    made there vanished. A restore now drops focus there and unbinds the bar;
  - opening a malformed file kept the previous song's history live. A failed load, or a failed
    autosave restore, now resets it;
  - Ctrl+Z did nothing while a dropdown or checkbox had focus. Only text-entry fields now keep the
    browser's own undo.
- **Kept against the review's advice:** Undo first records any change that wasn't reported. The
  reviewer flagged it as a workaround, but without it a change on a path that forgot to report would
  be lost on undo. The phantom step it produced in the malformed-file case came from the missing
  reset, which is fixed at its source.

## Consequences

- Undo history lives in memory only: a reload or a resumed autosave starts with an empty history.
- Selection is cleared on undo and redo; the notes are restored as new objects.
