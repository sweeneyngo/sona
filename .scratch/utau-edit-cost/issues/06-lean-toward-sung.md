# 06 — Lean toward a note when unsure

Status: needs-triage
Type: prototype
Blocked by: 01, 03

## What

Deleting an extra note (weight 1) is cheaper than drawing a missing one (weight 2). Add an opt-in
sung/rest threshold to the learned cell stage and sweep it on dev against weighted edit cost. Check the
lyric-shift Fog in `map.md` before claiming a win: extras shift pasted lyrics.

## Acceptance

- A dev sweep table (cost, clean measures, cell accuracy) with song-bootstrap intervals.
- Default unchanged; the knob stays opt-in until holdout4.

## Comments
