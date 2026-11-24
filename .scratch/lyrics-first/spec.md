# Lyrics first

## Goal (user, 2026-11-24)

"Lyrics should be a required part of the process of transcription. We should have the user post it, but
lyric transcription should be a natural fallback. For now, support Japanese hiragana."

Why it matters: utau-edit-cost 07 found that joined notes, the costliest error in OpenUtau, come from
UTAU's one note per mora. Only the lyric can fix them, fitted phrase by phrase (`sona_core::lyric_fit`; dev
GAME lead 188.7 → 174.9 cost/min). A lyric that lands one mora per note also saves the user pasting and
re-aligning it in OpenUtau.

## The flow

1. Start screen: the user gives the BPM (ADR-0028) and the lyric, in hiragana, one sung line per line.
   With no lyric, sona transcribes one (the fallback) and the user reviews it before notes are fitted.
2. Transcription fits the notes to the lyric: each line is a phrase, and each note gets one mora.
3. The editor shows each note's mora; exports carry it (UST `Lyric=`, USTX, MIDI lyric events).

## Input rules (for now)

- Hiragana, plus ー, small kana and っ, which are parts of moras. Spaces and line breaks separate phrases.
- Anything else (kanji, katakana, romaji) gets a clear message naming the characters. No silent conversion.

## Metrics

- **Edit cost** (utau-edit-cost spec), as before, for the notes.
- **Lyric placement**: the share of the UST's notes whose matched sona note carries the right mora, and
  the moras per minute a user must retype or shift. Defined in ticket 01.
- **Fallback lyric accuracy**: the mora error rate of transcribed lyrics against the UST's lyrics.

## Rules

As in `docs/agents/rules.md`: dev for iteration; holdout4 only with a pre-registered rule, bundled with the
utau-edit-cost winners; new behaviour opt-in until a holdout confirms it, except product flow the user has
decided (the lyric box itself). Pifuyuu's lyrics and USTs never enter the repo, and neither does anything
derived from them. ASR models run locally: no audio or lyric leaves the machine.
