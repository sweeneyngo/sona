# 15 — Show each line's place and let the user drag a misplaced one

Status: needs-triage
Type: task
Blocked by: 04, 14

## What

After transcription, the editor shows where each pasted line starts and marks lines the aligner was unsure
of. Dragging a line's start re-fits that line's (and its neighbours') notes with `fit_span_moras`, as one
undo step. Behaviour tests, an e2e driver, and the lyric-editor statechart/TLA+ model if states change.

## Comments
