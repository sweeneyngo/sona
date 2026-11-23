# Spec: estimate a song's tempo so a user needn't type one

Status: **rule committed before any estimator exists or any test song is read.**

## Question

Can sona read a song's BPM from the full mix well enough to make it the default in one-click generation
(the user can still override it)? Grid mode and the UST/USTX export both depend on it: a wrong BPM puts
every note on the wrong grid.

## Data

The 42 corpus songs, each a full mix (`data/pifuyuu-audio/<id>.wav`) whose exact tempo is in its lead
UST (`bpm_from_ust`, the manifest). Every song has a single tempo.
- **Dev:** the 10 `dev` songs: iterate freely.
- **Test:** the other 32 (holdout, holdout2, holdout3), read **once**. They were spent for *cell accuracy*,
  but no tempo estimate has ever been made or tuned on them, so they are fresh for this question.

## Metric

- **Exact:** the estimate is within 2% of the UST tempo.
- **Octave-equivalent (reported, not the rule):** within 2% of the tempo, its double or its half.

## Pre-registered decision rule

Ship the estimator as one-click generation's default tempo (shown to the user, editable) if, on the
32 test songs, **exact accuracy ≥ 80%**. Otherwise ship it only as a suggestion in the BPM field, and
keep BPM required.
