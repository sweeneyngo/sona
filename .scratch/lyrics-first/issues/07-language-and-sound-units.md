# 07 — Language detection and sound units (phonemization)

Status: claimed
Type: research
Blocked by: none

## What

The user (2026-11-24): "Part of the pipeline is being able to recognize the language of the song and
convert it into blocks of sound based on the language. I believe it's called phonemization or
phonemizers."

In UTAU a note carries one sung unit: a mora for Japanese (CV/VCV/CVVC banks), a syllable for English
(ARPAsing, VCCV), Chinese and others. OpenUtau then turns a note's lyric into the voicebank's phonemes
with a per-language phonemizer. So sona needs, per language:

1. **Language detection**: from the pasted lyric's script and words, or from the audio when the lyric is
   transcribed (the recogniser's language ID; see 05).
2. **Units**: split the lyric into the per-note units OpenUtau's phonemizer for that language expects
   (Japanese kana moras; English words split into syllables; Chinese one character per note as pinyin or
   hanzi), and say which voicebank types each format suits.
3. **Readings**: for scripts that need one (kanji → kana, hanzi → pinyin, English spelling → syllables),
   which G2P or dictionary does it.

## Research

- OpenUtau's phonemizers: which languages, what note lyric each expects (read OpenUtau's source and docs).
- Libraries per language: language ID, syllabifiers and G2P (e.g. pyopenjtalk for Japanese readings, CMUdict
  or g2p_en for English, pypinyin for Chinese, espeak-ng / phonemizer). **Licences matter**: sona is MIT;
  GPL or AGPL code can't be linked in, though a separate process the user installs may be possible. Say which.
- Can it run in Rust or the browser, or does it need the Python venv (as the separators do)?
- Measure what you can on dev: for Japanese, how often a kanji lyric read to kana by each reader matches
  the UST's moras (the corpus lyric texts are kanji-mixed). Dev only; per-song text stays in the
  scratchpad, never the repo.

## Acceptance

A report (`crates/sona-eval/reports/language-and-units.md`): a table per language (unit, OpenUtau
phonemizer, library, licence, how it would run), the Japanese reading accuracy, a design for a per-language
"units" module behind the lyric box (Japanese first), and the tickets to build it.

## Comments
