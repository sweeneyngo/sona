# 05 — Transcribing the lyric when none is pasted

Status: claimed
Type: research
Blocked by: none

## What

Find a local speech recogniser that turns a separated Japanese vocal into hiragana well enough to be the
fallback. Candidates to check, not assume: Whisper and its Japanese fine-tunes (kotoba-whisper, etc.),
ReazonSpeech, and anything trained on singing. For each: licence (can a user download and run it? sona
won't redistribute weights), size, CPU speed on a 3-minute vocal, and how kanji output becomes hiragana
(a reading step and its errors).

Score on dev: the mora error rate against the lead UST's moras (insertions, deletions, substitutions), and
the same after 01's fit, so a wrong mora is counted where the user will see it. Run everything locally; no
audio or lyric leaves the machine. Song-level results stay in the scratchpad; only pooled numbers go in
the report.

## Acceptance

A report with a table per candidate, a recommendation, and an estimate of how much fixing a fallback lyric
costs the user compared with pasting one.

## Comments
