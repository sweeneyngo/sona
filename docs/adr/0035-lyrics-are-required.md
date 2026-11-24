# ADR-0035: A transcription needs the song's lyric, given on the start screen in hiragana

Status: accepted, 2026-11-24. Amends [ADR-0028](0028-ask-for-the-bpm-first.md) (the start screen) and
[ADR-0032](0032-retranscribe-by-upload-hash.md) (re-transcription). Ticket:
`.scratch/lyrics-first/issues/03-lyric-on-start-screen.md`; spec `.scratch/lyrics-first/spec.md`.

## Context

The user's decision (2026-11-24):

> "Lyrics should be a required part of the process of transcription. We should have the user post it, but
> lyric transcription should be a natural fallback. For now, support Japanese hiragana."

Why: UTAU writes one note per mora, and a singer runs same-pitch moras together, so the costliest error in
OpenUtau (joined notes) can only be fixed with the lyric (utau-edit-cost 07). Until now the lyric came last:
pasted into **Edit → Lyrics…** after transcribing, and never seen by the server.

## Decision

1. **The start screen asks for the lyric beside the BPM.** A multi-line box, one sung line per line. Below
   it, a line says what sona reads ("4 lines · 30 moras") or which characters it can't take.
2. **What it takes** (`checkLyric` in `web/lib/lyrics.ts`): hiragana (U+3041–U+3096, so small kana, っ and
   ゔ), ー, and whitespace (spaces and line breaks separate phrases). Anything else, katakana (full or half
   width), kanji, romaji, punctuation, iteration marks, is named by line: "Hiragana only for now. Line 2:
   カ ゼ". Invisible characters are named by code (`U+200B`). Nothing is converted. Windows line breaks
   become plain ones, line ends are trimmed and blank lines at the ends dropped; inner lines are kept, since
   ticket 02 uses them as phrases.
3. **Transcribe needs a lyric.** With none it says "Paste the song's lyric in hiragana, one sung line per
   line." and starts nothing; with one it can't take, it shows the same message as the box. The fallback,
   "Transcribe the lyric for me", is shown disabled with "(not yet: paste it for now)" until ticket 06
   builds it, so for now a lyric is required.
4. **The reducer** (`web/lib/startFlow.ts`) holds the lyric as typed (`set-lyric`) beside the BPM. A new
   song starts without one; Cancel and a failure keep it. A running job holds the checked lyric it sends.
5. **The job carries it.** `POST /api/jobs?…&lyric=<text>` (UTF-8, percent-encoded), by upload or by hash.
   The server applies the same rule (400 naming the characters; a blank lyric or one over 6000 characters
   is a 400 too) and keeps it as `job.lyric` in the job's record and the annotation's provenance (optional
   in the annotation contract). The parameter is optional, so `curl`, scripts and the tests' jobs run as
   before. The request head's limit rises from 16 to 64 KiB to fit 6000 percent-encoded kana. A query
   parameter, not a body: the body is the audio, and a job named by hash has none.
6. **The notes take it.** Fresh notes from a job (and a re-transcription's) take the lyric a mora each, in
   time order, with today's `assignLyrics`, before the pitch fit, so a re-transcription stays one undo
   step. Ticket 04 replaces this with fitting phrase by phrase. **Edit → Lyrics…** holds the lyric.
7. **It stays with the song.** A saved session or the autosave keeps the annotation, so the lyric comes
   back with it. **Re-transcribe at N BPM** sends the transcription's lyric again. Notes transcribed
   without one (files from before this) send the user to the start screen, song and BPM filled in, to
   paste it; the job then goes by hash as before.

## Evidence

- **Unit tests** (`web/lib/*.test.ts`, 209 tests): the lyric rule on hiragana with ー/small kana/っ, empty
  and blank boxes, katakana, kanji, romaji, punctuation, an invisible character, half-width katakana, ゝ,
  CRLF; the reducer's fixed paths and its 3 000 random event orders, which now include valid, refused and
  empty lyrics and a re-transcription without one, and check at every step that a running job holds a
  lyric sona takes and a new song starts without one; the job query keeps line breaks and leaves the
  parameter out when there is no lyric; provenance and re-transcription carry it; the annotation contract
  fixture has the new field.
- **Mutants** (each applied and caught): a Transcribe that skips the lyric check (2 tests fail), a lyric
  carried to a new song (2), a re-transcription without a lyric (2), katakana accepted (4), a job query
  without the lyric (1), a re-transcription job without it (1); server: a lyric accepted unchecked, a lyric
  not kept in the record (1 test each).
- **Server** (`crates/sona-cli/tests/serve.rs`, stub separators): a lead job with a lyric gives it back,
  line breaks and all, in the annotation and the record; a job named by hash keeps its own; a job without
  one runs as before and has no `lyric`; a 6000-character lyric fits in the request; katakana, kanji,
  romaji and `、` are 400s naming them; blank and over-long lyrics are 400s; nothing ran for a refused job.
- **Model** (`verification/start-flow`, TLA+): new rules `LyricIsForTheShownSong` and `JobsHaveALyric`,
  and opening a file with or without a lyric as an action. All 12 mutants are caught; the new ones are
  `start-without-lyric` (Transcribe with neither a lyric nor the fallback), `lyric-carries-over`,
  `retranscribe-without-lyric` and `lyric-of-another-song`.
- **Browser** (`web/e2e/lyric_start.py`, against `sona serve`, the synthetic fixture's stem sent as a stem,
  pYIN; a made-up lyric): Transcribe with no lyric asked for it and sent nothing; a pasted katakana line
  showed "Hiragana only for now. Line 2: マ チ ア ル ク" as typed, Transcribe refused it the same way and the
  box was unchanged; the fixed lyric read "4 lines · 30 moras", went with both requests (hash, then file)
  with its line breaks, and the server kept it; the 73 notes took the 30 moras in order ("73 notes · lyric
  on 30 notes"); Edit → Lyrics… held it; Re-transcribe at ½× sent it again and the new notes took it; after
  a reload and Resume the notes' lyrics, the song's lyric and the Lyrics box were all still there. The
  other three serve drivers (`song_to_notes`, `cancel_and_retry`, `retranscribe`) now paste a lyric and
  pass. `retranscribe.py` caught a bug on the way: laying the lyric after the pitch fit split a
  re-transcription into two undo steps.

## Consequences

- Every new transcription starts with the lyric in hand, ready for tickets 02 and 04 to fit the notes to
  it. Until 04 lands, laying it in order is only as good as the note count: joined or split notes shift
  every mora after them, as pasting into Edit → Lyrics… always did.
- A user without a lyric can't transcribe until ticket 06 (lyric transcription). That is the user's call
  (hiragana and a lyric for now), but it blocks songs nobody has written out.
- Users with lyrics in kanji or katakana must convert them first; sona names what to change but converts
  nothing. A reading converter is a later choice.
- The annotation now carries the user's lyric. Annotations stay local, but anyone sharing a session file
  shares the lyric too.
