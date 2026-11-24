# 14 — Flag the doubtful notes in the editor

Status: claimed
Type: task
Blocked by: 13

## What

13 (ADR-0033) gave cell-stage notes a confidence that finds the expensive errors: flagging each song's
lowest 30% catches 47.5% of the dev edit cost (random 30%). But the editor's flag is a fixed threshold
(0.50 by default), which outlines only 2.2% of stage notes, and the inspector's "why" names the weakest
sub-score, which no longer explains a stage note's score.

- Flag by share: an option "flag the doubtful N%" per song, default 20%, alongside the threshold. The
  threshold stays for pYIN and GAME-only results, whose scores mean something else.
- The inspector says why a stage note is doubtful in the user's terms: "long note, may hide several
  syllables" (length term dominates) or "the model was unsure here" (probability term dominates).
- Behaviour tests through the public interface (web unit tests plus an e2e driver), and the statechart
  or TLA+ model if the flag control changes the editor's states.

## Acceptance

- On a stage result, the default outlines about 20% of notes, the longest and least sure first.
- pYIN and GAME-only results look as before.
- `npm test`, `tsc`, e2e and the TLA+ checks pass; a mutant that ranks by the old score fails a test.

## Comments
