# 04 — Transcription fits the notes to the lyric

Status: needs-triage
Type: task
Blocked by: 02, 03

## What

`sona transcribe --lyric FILE` and the server job use 02's fit: the notes are split and joined to the
lyric, and each note carries its mora into the annotation and every export (UST `Lyric=`, USTX, MIDI lyric
meta events). Check that OpenUtau imports the lyrics from each format (the user checks by hand).

## Comments

- 2026-11-24 (02): placing lines from sona's notes failed; the line timing must come from the audio or from the
  user marking each line's first note. Pick one (user's call) before this ticket; the fit after it is
  `lyric_fit::fit_span_moras`.
