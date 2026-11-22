---- MODULE ExprBar ----
\* Clicking the canvas selects on mousedown; the field being edited fires
\* `change` after that, on blur. Pressing a pitch handle starts a drag that
\* rewrites the note from the expression it captured at mousedown.
EXTENDS Naturals
CONSTANTS N
VARIABLES selected, exprBarNote, editing, editFor, appliedTo, version, dragNote, dragBase, dragWrote, lost
vars == <<selected, exprBarNote, editing, editFor, appliedTo, version, dragNote, dragBase, dragWrote, lost>>
Notes == 1..N
MaxEdits == 2

Init == /\ selected = 0 /\ exprBarNote = 0 /\ editing = FALSE /\ editFor = 0 /\ appliedTo = 0
        /\ version = [n \in Notes |-> 0] /\ dragNote = 0 /\ dragBase = 0 /\ dragWrote = FALSE /\ lost = FALSE

YouClickANote(i) == /\ dragNote = 0
                    /\ selected' = i
                    /\ exprBarNote' = IF editing THEN exprBarNote ELSE i
                    /\ UNCHANGED <<editing, editFor, appliedTo, version, dragNote, dragBase, dragWrote, lost>>
YouStartEditingAField == /\ dragNote = 0 /\ selected # 0 /\ ~editing /\ exprBarNote = selected
                         /\ editing' = TRUE /\ editFor' = exprBarNote
                         /\ appliedTo' = 0
                         /\ UNCHANGED <<selected, exprBarNote, version, dragNote, dragBase, dragWrote, lost>>
TheFieldCommits == /\ editing /\ version[exprBarNote] < MaxEdits
                   /\ appliedTo' = exprBarNote
                   /\ version' = [version EXCEPT ![exprBarNote] = @ + 1]
                   /\ editing' = FALSE
                   /\ exprBarNote' = selected
                   /\ UNCHANGED <<selected, editFor, dragNote, dragBase, dragWrote, lost>>
\* The press blurs the field first, so its value commits before the drag captures the note.
YouPressAHandle == /\ dragNote = 0 /\ selected # 0 /\ version[selected] < MaxEdits
                   /\ ~(editing /\ version[exprBarNote] >= MaxEdits)
                   /\ LET v == IF editing THEN [version EXCEPT ![exprBarNote] = @ + 1] ELSE version
                      IN /\ version' = v
                         /\ dragBase' = v[selected]
                   /\ appliedTo' = IF editing THEN exprBarNote ELSE appliedTo
                   /\ editing' = FALSE
                   /\ exprBarNote' = selected
                   /\ dragNote' = selected
                   /\ dragWrote' = FALSE
                   /\ UNCHANGED <<selected, editFor, lost>>
\* Every move writes the captured expression plus the drag so far; anything
\* else that changed the note since the capture would be overwritten.
TheHandleMoves == /\ dragNote # 0
                  /\ lost' = (lost \/ version[dragNote] # (IF dragWrote THEN dragBase + 1 ELSE dragBase))
                  /\ version' = [version EXCEPT ![dragNote] = dragBase + 1]
                  /\ dragBase' = dragBase
                  /\ dragWrote' = TRUE
                  /\ UNCHANGED <<selected, exprBarNote, editing, editFor, appliedTo, dragNote>>
YouReleaseTheHandle == /\ dragNote # 0 /\ dragNote' = 0
                       /\ UNCHANGED <<selected, exprBarNote, editing, editFor, appliedTo, version, dragBase, dragWrote, lost>>
Next == \/ \E i \in Notes : YouClickANote(i) \/ YouStartEditingAField \/ TheFieldCommits
        \/ YouPressAHandle \/ TheHandleMoves \/ YouReleaseTheHandle

EditLandsOnTheNoteShown == appliedTo # 0 => appliedTo = editFor
NoEditIsLost == ~lost
====
