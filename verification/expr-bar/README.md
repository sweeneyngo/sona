# Pitch expression bar model (TLA+)

`ExprBar.tla` models the review surface's pitch expression bar (`web/main.ts`:
`syncExprBar`, `applyExprBar`, its field `change` listeners, and note selection
on the canvas). You type in a field (glide, vibrato depth, …), and the value
commits on Enter or when the field loses focus.

It also models dragging a pitch handle on the roll (ADR-0021), which rewrites the
note from the expression it captured at mousedown.

Rules, checked on every path with 3 notes:
- **EditLandsOnTheNoteShown:** a value you type goes to the note whose values the bar was showing.
- **NoEditIsLost:** a handle drag never overwrites a value committed after it captured the note.

`./check.sh` runs TLC on the rules and on three mutants, each of which must be caught.
`capture-before-blur` is a drag that captures the note before the bar's typed value
commits; the mousedown handler commits the bar first (`applyExprBar`, then blur).
`commit-to-selected` is the bug the pstack statechart pass found on 2026-11-22:
1. **Why it happened.** Clicking a note selects it on mousedown, before the field you were editing blurs and commits.
2. **The effect.** Committing to the *selected* note moved your typed value onto the note you just clicked.
3. **Confirmation.** The path was replayed in the browser before the fix.
4. **The fix.** The bar is bound to the note it shows (`noteShownInExprBar` in `web/main.ts`, `exprBarNote` in the model).

CI runs `check.sh` in the `model-check` job.
