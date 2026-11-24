# 13 — Units for languages beyond Japanese (English first)

Status: needs-triage
Type: task
Blocked by: 08, 10

Needs the user's decision on scope first. The spec supports Japanese hiragana only "for now".

## What

Add more languages to the units module from 08, one module per language. The table is in
`crates/sona-eval/reports/language-and-units.md`.

- **English** (OpenUtau EN ARPA / ARPA+ / VCCV / X-SAMPA / DiffSinger EN):
  - The note unit is a syllable, but the lyric OpenUtau expects is the whole word on the word's first
    note, then `+` on each following note. `+~` holds a vowel. OpenUtau groups a word only when the
    notes touch (`UPart.cs`).
  - Syllable counts come from CMUdict (BSD-2) vowel counts. Words CMUdict lacks fall back to a
    permissive syllabifier (hypher's en patterns), flagged for review.
  - Exports write `word`, `+`, `+`. A UST for a classic English VCCV bank may need per-syllable aliases
    instead; check before choosing.
- **Chinese** (ZH CVVC, CVV, VCV, DiffSinger ZH): one hanzi per note, and the lyric is the hanzi.
  OpenUtau converts it to pinyin itself (csharp-pinyin, Apache-2.0). Pinyin display needs pinyin-pro
  (MIT, JS) or the pinyin crate (MIT).
- **Korean** (KO CV/CVC/CVVC/VCV, DiffSinger KO): one hangul syllable per note, the lyric as typed.
  OpenUtau applies the sound changes.
- **The fit:** `lyric_fit`'s mora kinds become per-language unit kinds (an open syllable vs a consonant
  onset).

## Acceptance

Per language:
- unit tests;
- a synthetic fixture exported to USTX and loaded in OpenUtau with that language's phonemizer (manual
  check, noted in the PR);
- README and ADR updates.

## Comments
