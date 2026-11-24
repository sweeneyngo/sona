# 03 — The lyric box on the start screen

Status: resolved
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

## Answer

Status: resolved, 2026-11-24. [ADR-0035](../../../docs/adr/0035-lyrics-are-required.md).

- **Start screen:** a lyric box under the BPM (one sung line per line) with a live line below it: "4 lines ·
  30 moras", or "Hiragana only for now. Line 2: マ チ ア ル ク". Transcribe with no lyric says "Paste the
  song's lyric in hiragana, one sung line per line." "Transcribe the lyric for me" is shown disabled, "(not
  yet: paste it for now)", until 06.
- **Rule** (`checkLyric`, and the same on the server): hiragana U+3041–U+3096, ー, whitespace. Everything
  else is named by line, never converted. Line breaks are kept (CRLF → LF, line ends trimmed).
- **Job:** `lyric=` on `POST /api/jobs` (optional; by upload or by hash), kept as `job.lyric` in the record
  and the annotation's provenance; a 400 names the characters, and a blank lyric or one over 6000
  characters is a 400 too.
- **Editor:** fresh notes and a re-transcription's take it a mora each in time order (`assignLyrics`);
  Edit → Lyrics… holds it; sessions and Re-transcribe keep it; notes without one go back to the start
  screen to ask for it.
- **Checks:** web unit tests (209), server tests (2 new), TLA+ (`LyricIsForTheShownSong`, `JobsHaveALyric`;
  12/12 mutants caught, incl. `start-without-lyric`), 8 code mutants caught, `web/e2e/lyric_start.py` and
  the other three serve drivers pass against `sona serve`.
- **For 04:** today's in-order laying puts っ and ー on notes of their own (as `splitLyrics` always did);
  07 found っ is never a UST note and a tail has one 18% of the time. The fit should replace it, not build
  on it.
