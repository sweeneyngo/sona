# 10 — Detecting the lyric's language on paste

Status: needs-triage
Type: task
Blocked by: 03, 08

## What

When a lyric is pasted, decide its language and show it. Design and dev numbers are in
`crates/sona-eval/reports/language-and-units.md` ("Language detection").

- **Song level, by script.** Kana means Japanese, and hangul means Korean. All-Han text is ambiguous
  between Chinese and Japanese, so ask. Latin-only text goes to a detector (lingua-rs or whatlang,
  both permissive, pure Rust; or a JS port), and the app says the language isn't supported yet.
- **Line level, inside the song's language.** A Han-only line in a Japanese song is Japanese: lingua
  calls 8 such dev lines Chinese, which is why lines don't get their own detector. A Latin line or word
  in a Japanese song is an "English island", 22 of the 368 dev lines. The editor highlights it with
  "the Japanese voicebank needs kana here", and ticket 12 offers a katakana reading.
- **What the user sees** (no silent decisions):
  - Confident: a chip, "Japanese", that the user can change.
  - Unsure (Han only, or nothing but symbols): "Is this Japanese or Chinese?" Start stays disabled
    until the user answers.
  - Unsupported: "Only Japanese is supported for now", naming the detected language.
- The chosen language travels with the job and the session, next to the lyric (03).

## Acceptance

- Unit tests per state, including the Han-only line inside a Japanese song.
- An e2e driver: paste a mixed lyric, see the English line flagged, change the language chip.
- If this adds UI states, the start-flow statechart and its TLA+ model gain them.

## Comments
