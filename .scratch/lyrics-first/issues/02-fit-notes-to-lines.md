# 02 — Fit the notes to the pasted lines

Status: needs-triage
Type: research
Blocked by: 01

## What

Build the mechanism 01 recommends: split the lyric into lines, map each line onto a run of sona's notes
(a DP over lines × rests, if lines and rests disagree), then fit each line's moras with `lyric_fit`. Port
it to `sona-core`. Dev numbers for edit cost and lyric placement; opt-in in eval until holdout4.

## Comments
