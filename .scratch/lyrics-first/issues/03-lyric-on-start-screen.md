# 03 — The lyric box on the start screen

Status: claimed
Type: task
Blocked by: none

## What

- The start screen asks for the lyric next to the BPM (ADR-0028's flow): a multi-line box, hiragana, one
  sung line per line.
- Validation as in `spec.md`: hiragana, ー, small kana and っ; spaces and line breaks separate. Anything
  else gets a message naming the offending characters.
- Starting needs either a lyric or an explicit "transcribe the lyric for me" choice (the fallback, ticket
  06; until it exists, that choice is shown disabled with a short note).
- The lyric travels with the job to `sona serve`, is kept with the session and on re-transcribe
  (ADR-0032), and is laid on the notes in order by today's `assignLyrics` until ticket 04 lands.
- The start-flow statechart and TLA+ model (`verification/start-flow/`) gain the lyric state.
- An ADR for "lyrics are required" (the user's decision, quoted).

## Acceptance

Web unit tests and an e2e driver (pasting a lyric, a rejected katakana line, the session reload); TLA+
passes and a mutant (starting with neither a lyric nor the fallback) is caught.

## Comments
