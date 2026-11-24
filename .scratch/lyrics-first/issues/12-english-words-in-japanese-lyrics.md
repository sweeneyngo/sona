# 12 — English words in a Japanese lyric

Status: needs-triage
Type: task
Blocked by: 10, 11

## What

Four of the 9 dev songs sing English words or lines. The UST writes them as Japanese-style kana sung by
the Japanese bank, so a Japanese CV bank needs kana for them. Ticket 10 flags the Latin island; this
ticket offers a katakana reading for it. The user reviews it, as with 11.

Candidates measured on dev (report: `crates/sona-eval/reports/language-and-units.md`, "English inside
Japanese"):

- **e2k**: Unlicense, a small RNN with a Rust port.
- **alkana**: GPL-2, dictionary, separate process only.
- An OpenJTalk reader alone, which spells the letters out.

English sung in a Japanese song is often re-moraed freely (the UST's kana are the singer's, not a
dictionary's). Expect a high error rate whatever the reader, and keep the user in the loop.

The alternative is OpenUtau's own EN→JA phonemizer (`ENtoJAPhonemizer`: an English word plus `+` on a
Japanese bank). The export could write the English word there instead of kana. That is a product choice
for the user.

## Acceptance

- The dev mora error rate on the Latin lines for the chosen converter, against the UST.
- Unit tests.
- An e2e driver: paste a mixed lyric, accept the katakana, start.

## Comments

- 2026-11-24, lead: the user chose the English word on its first note, then `+` (for OpenUtau's EN-to-JA phonemizer), not katakana.

- 2026-11-24, lead: 08 made the lyric box and Edit → Lyrics… hiragana-only. The user chose the English word on its first note, then `+`, so this ticket must let Latin words through the check (as one unit spanning its syllables) without loosening anything else.
