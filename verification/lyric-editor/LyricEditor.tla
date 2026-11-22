---- MODULE LyricEditor ----
\* The review surface's inline lyric editor (web/main.ts: openLyricEditor,
\* scrollNoteIntoViewBothAxes, commitLyric, closeLyricEditor and the editor's
\* keydown/blur handlers). Notes are culled by time only, so a note outside the
\* pitch rows still has a hitbox and the box always opens; opening scrolls both
\* axes so the note being edited is on screen.
EXTENDS Naturals
CONSTANTS N, Words
VARIABLES lyric, open, target, box, selected, inPitchView
vars == <<lyric, open, target, box, selected, inPitchView>>
Notes == 1..N
None == "none"

Init == /\ lyric = [i \in Notes |-> None]
        /\ open = FALSE /\ target = 0 /\ box = None /\ selected = 0
        /\ inPitchView \in [Notes -> BOOLEAN]

Open(i) == /\ selected' = i
           /\ open' = TRUE /\ target' = i /\ box' = lyric[i]
           /\ inPitchView' = [inPitchView EXCEPT ![i] = TRUE]
Close == open' = FALSE /\ target' = 0 /\ box' = None
Commit == lyric' = [lyric EXCEPT ![target] = box]

YouClickANote(i) == ~open /\ inPitchView[i] /\ selected' = i /\ UNCHANGED <<lyric, open, target, box, inPitchView>>
YouPressL == ~open /\ selected # 0 /\ Open(selected) /\ UNCHANGED lyric
YouType(w) == open /\ box' = w /\ UNCHANGED <<lyric, open, target, selected, inPitchView>>
YouPressEnter == open /\ Commit /\
                 IF target < N THEN Open(target + 1)
                 ELSE Close /\ UNCHANGED <<selected, inPitchView>>
YouPressEscape == open /\ Close /\ UNCHANGED <<lyric, selected, inPitchView>>
YouClickAway == open /\ Commit /\ Close /\ UNCHANGED <<selected, inPitchView>>
YouScrollPitch(i) == ~open /\ inPitchView' = [inPitchView EXCEPT ![i] = ~@] /\ UNCHANGED <<lyric, open, target, box, selected>>

Next == \/ \E i \in Notes : YouClickANote(i)
        \/ YouPressL
        \/ \E w \in Words : YouType(w)
        \/ YouPressEnter
        \/ YouPressEscape
        \/ YouClickAway
        \/ \E i \in Notes : YouScrollPitch(i)

\* While the lyric box is open, the highlighted note is the note it edits.
HighlightedIsEdited == open => selected = target
\* You can see the note you are typing a lyric for.
NoteBeingEditedIsOnScreen == open => inPitchView[target]
\* The box is never open on nothing.
BoxHasANote == open => target \in Notes
====
