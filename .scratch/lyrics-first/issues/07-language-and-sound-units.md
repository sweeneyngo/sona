# 07 — Language detection and sound units (phonemization)

Status: resolved
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

## Answer

Report: `crates/sona-eval/reports/language-and-units.md`. Harness: `crates/sona-eval/scripts/language-units/`.
Dev only, 9 songs; per-song text stayed in the scratchpad.

- **The lyric on each note differs by language in OpenUtau.**
  - Japanese: one hiragana mora; the phonemizer builds the VCV/CVVC aliases.
  - English and other Latin-script languages: the whole word on the first note, then `+`.
  - Chinese: one hanzi or pinyin; OpenUtau converts hanzi itself.
  - Korean: one hangul syllable.
- **Japanese units.** っ is never a note: 0 of 5,336 UST notes, against 137 in the texts. A long-vowel
  tail has its own note only 18% of the time.
- **Hiragana input**, mora error rate against the UST on Japanese lines:
  - today's splitter: 12.4%;
  - with the UST's conventions: 9.0%;
  - plus optional tails from a rule with no dictionary: 5.3%;
  - with an OpenJTalk-family analyser: 4.3%.
- **Kanji input.** The best readers tie: pyopenjtalk-plus 3.8% [2.1, 5.9], vanilla OpenJTalk 3.9%,
  fugashi+unidic-lite 4.1%, with tails optional; 4.5% with tails merged.
  - Kanji-word moras err at 5.3%, kana-word moras at 1.8%. The 1.8% is the floor from the text
    differing from what is sung.
  - pykakasi (GPL) is worst at 11.1%.
  - The kanji errors are:
    - wrong sense (a common noun read as a name);
    - the lyricist's own reading (ruby, gikun);
    - sung variants (言う→ゆう);
    - 何.
- **English islands.** 22 of 368 lines are Latin. e2k (Unlicense) brings their error from 88% to 25%.
- **Language detection.** A script rule decides the song's language, and each line inherits it.
  Per-line lingua mislabels 8 Han-only lines as Chinese.
- **Recommendation.** Build 08 (units), 10 (detection) and 09 (optional tails in the fit). Build 11
  (kanji via jpreprocess in `sona serve`, reviewed by the user) and 13 (more languages) only if the user
  decides to.
- **New tickets:** 08–13.
