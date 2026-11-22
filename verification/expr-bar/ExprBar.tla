---- MODULE ExprBar ----
\* Clicking the canvas selects on mousedown; the field being edited fires
\* `change` after that, on blur.
EXTENDS Naturals
CONSTANTS N
VARIABLES selected, exprBarNote, editing, editFor, appliedTo
vars == <<selected, exprBarNote, editing, editFor, appliedTo>>
Notes == 1..N

Init == selected = 0 /\ exprBarNote = 0 /\ editing = FALSE /\ editFor = 0 /\ appliedTo = 0

YouClickANote(i) == /\ selected' = i
                    /\ exprBarNote' = IF editing THEN exprBarNote ELSE i
                    /\ UNCHANGED <<editing, editFor, appliedTo>>
YouStartEditingAField == /\ selected # 0 /\ ~editing /\ exprBarNote = selected
                         /\ editing' = TRUE /\ editFor' = exprBarNote
                         /\ appliedTo' = 0 /\ UNCHANGED <<selected, exprBarNote>>
TheFieldCommits == /\ editing
                   /\ appliedTo' = exprBarNote
                   /\ editing' = FALSE
                   /\ exprBarNote' = selected
                   /\ UNCHANGED <<selected, editFor>>
Next == \/ \E i \in Notes : YouClickANote(i) \/ YouStartEditingAField \/ TheFieldCommits

EditLandsOnTheNoteShown == appliedTo # 0 => appliedTo = editFor
====
