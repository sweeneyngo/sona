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

**Disclosure (dec0416 + 1):** while listing the dev tempos I also printed the test songs' UST tempos
(labels only: range 92–204 BPM in test, 108–250 in dev). No estimate was made on any test song. Every
choice of method, prior or range is made on dev alone.

## Result (2026-11-23): the bar is not met; ships as a suggestion

Test (32 songs, read once, logged): exact 24/32 (75.0%), within an octave 31/32 (96.9%), half-tempo
errors 0. Below the 80% bar, so per the rule the estimate is pre-filled and the BPM stays required
(the start screen's tempo field, ½×/2× beside it). Dev: 8/10, 10/10, 0. ADR-0022.
