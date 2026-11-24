# 04 — Transcription fits the notes to the lyric

Status: needs-triage
Type: task
Blocked by: 02, 03

## What

`sona transcribe --lyric FILE` and the server job use 02's fit: the notes are split and joined to the
lyric, and each note carries its mora into the annotation and every export (UST `Lyric=`, USTX, MIDI lyric
meta events). Check that OpenUtau imports the lyrics from each format (the user checks by hand).

## Comments
