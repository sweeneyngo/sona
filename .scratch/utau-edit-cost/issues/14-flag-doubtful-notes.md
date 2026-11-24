# 14 — Flag the doubtful notes in the editor

Status: resolved
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

## Answer

Done ([ADR-0034](../../../docs/adr/0034-flag-doubtful-stage-notes-by-share.md)).

**Which results.** The web knows a learned-stage result by its provenance (`settings.engine == "game+cells"`,
which `sona transcribe/serve --cell-model` write) or, for an older file, by notes that carry `stage_doubt`.
Everything else keeps the threshold.

**Flag by share.** Settings shows **Flag the doubtful N%** (0–50%, step 5, default 20%) instead of the
threshold for a stage result: each song's `round(N% × notes)` least confident notes, ties to the longer,
are outlined. A note the user edits keeps its rank but loses its outline, so fixing one never outlines
another. The share is saved in the session and undone like the threshold.

**Why.** ADR-0033's log-odds split into a length term `0.58·ln(cells)` and an uncertainty term
`4.44·(1 − mean class prob)`, both 0 for a one-cell note the model is certain of. The core now emits both
per stage note (`cellstage::note_doubt` → optional `Note.stage_doubt { length, unsure }`; absent for every
other producer, so their JSON is byte-identical; `SCHEMA_VERSION` stays 1, as for `provenance` and
`bar_one`). An outlined stage note's tooltip says "⚠ long note, may hide several syllables" or "⚠ the model
was unsure here", never a sub-score; a stage note that isn't outlined gives no reason.

**Does the reason mean anything?** (dev only, in sample, `doubt_reasons.py` on 13's dump; no holdout read.)
20% per song as the editor ranks catches 33.3% [29.6, 38.1] of the cost (ADR-0033: 33.2%). 59% of flagged
notes say "long", 41% "unsure", and the reasons sort the errors: "long" flags catch 25.9% of the joined cost
and 8.8% of the chopped; "unsure" flags 6.8% and 27.4%. At 30%: 47.7% [43.9, 52.7].

**Verified.** Rust: `stage_confidence.rs` (the two doubts per cause, carried to the app note by note), the
output invariant (every stage note reaches the app with a finite, non-negative doubt), the contract fixture
(`stage_doubt` round-trips; `annotation.test.ts` reads it). Web: `lib/doubt.test.ts` through
`effectiveNotes`, `flagReading`, `doubtReason` and the session functions (16 tests, one a 300-song
property). e2e `web/e2e/doubtful_notes.py` (in `run_all.sh`/CI): 10 of 48 outlined with 8 "long" + 2
"unsure", 0% → 0, 30% → 14, a dragged note loses its outline, the pYIN fixture keeps the threshold and
"⚠ soft onset (0.90)". **pYIN and GAME-only look exactly as before:** the base commit (1f6d5fa) and this
branch, side by side in Chromium, draw identical canvas pixels and tooltips for every outlined note at
thresholds 0.50/0.70/0.95, and the same Settings text (scratch comparison, not committed); the unit test
also asserts identical effective notes at every share. Mutants, each caught: web (`doubt.test.ts`) ranking by the old soft-min of the sub-scores,
flagging the most confident, a stage result keeping the threshold, every result treated as a stage one, the
two reasons swapped, ties to the shorter note, edited notes staying outlined, edited notes dropping out of
the ranking (another gets outlined), the share floored instead of rounded; e2e the inspector naming the
weakest sub-score for stage notes; Rust the two doubts swapped, uncertainty from `p` instead of `1 − p`,
length from cells instead of ln(cells), the app never getting the doubt, the doubts shifted by one note.
TLA+: no editor state changed (the share is a value like the threshold, not a mode), so no model change;
every `verification/*/check.sh` passes.

**For the user.** (1) 20% is a starting point: the catch grows about linearly with the share, so the right
share is how many notes a producer will actually check; it's one slider away. (2) The inspector still lists
a stage note's sub-scores, which don't explain its score; hiding them for stage notes is a small follow-up if
they confuse. (3) An older stage annotation (before this change) is flagged by share but its tooltip falls
back to the weakest sub-score until re-transcribed.

## Comments
