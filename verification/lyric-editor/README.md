# Lyric editor model (TLA+)

`LyricEditor.tla` models the review surface's inline lyric editor (`web/main.ts`:
`openLyricEditor`, `scrollNoteIntoViewBothAxes`, `commitLyric`,
`closeLyricEditor`, and the editor's keydown and blur handlers). You select a
note, press **L**, type, then press Enter (save and go to the next note), Esc
(stop), or click elsewhere (save and stop).

Rules, checked on every path with 3 notes:
- **HighlightedIsEdited:** while the box is open, the highlighted note is the one it edits.
- **NoteBeingEditedIsOnScreen:** you can see the note you are typing a lyric for.
- **BoxHasANote:** the box is never open on nothing.

`./check.sh` runs TLC on the rules and on three mutants, each of which must be caught.
`no-pitch-scroll` is the bug the pstack statechart pass found on 2026-11-22:
1. **The bug.** Opening the box scrolled the roll in time but not in pitch.
2. **Why it went unseen.** Notes are culled by time only, so a note above or below the visible rows still had a hitbox.
3. **The effect.** You typed a lyric for a note you couldn't see.
4. **Confirmation.** The path was replayed in the browser before the fix.

CI runs `check.sh` in the `model-check` job.
