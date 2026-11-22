# Pitch expression bar model (TLA+)

`ExprBar.tla` models the review surface's pitch expression bar (`web/main.ts`:
`syncExprBar`, `applyExprBar`, its field `change` listeners, and note selection
on the canvas). You type in a field (glide, vibrato depth, …), and the value
commits on Enter or when the field loses focus.

Rule, checked on every path with 3 notes:
- **EditLandsOnTheNoteShown:** a value you type goes to the note whose values the bar was showing.

`./check.sh` runs TLC on the rule and on two mutants, each of which must be caught.
`commit-to-selected` is the bug the pstack statechart pass found on 2026-11-22:
1. **Why it happened.** Clicking a note selects it on mousedown, before the field you were editing blurs and commits.
2. **The effect.** Committing to the *selected* note moved your typed value onto the note you just clicked.
3. **Confirmation.** The path was replayed in the browser before the fix.
4. **The fix.** The bar is bound to the note it shows (`exprBarNote`).

CI runs `check.sh` in the `model-check` job.
