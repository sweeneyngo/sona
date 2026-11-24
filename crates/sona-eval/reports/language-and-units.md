# Language detection and sound units (lyrics-first 07)

Date: 2026-11-24. **Dev only**: the 9 dev songs with both a lyric text and a 03 dump; `plus-space` has
neither. No holdout split was read.

Pifuyuu's lyric texts, USTs and everything derived from them stayed on this machine, under the session
scratchpad and `~/.cache/sona/`. That includes every reading, matched span and per-song text. This report
carries pooled numbers and per-song rates only. The only downloads were public packages and their
dictionaries.

Harness: `crates/sona-eval/scripts/language-units/`. Run `run_all.sh` with `SONA_LANG_DUMP` set to 03's dev
dumps; per-song outputs go to `SONA_LANG_OUT` (default `~/.cache/sona/lang-units/out`). The committed copy
reproduces the prototype's numbers exactly: pyopenjtalk-plus `pron-opt` 169/4467, fugashi+unidic-lite
175/4284.

## Answer

- **OpenUtau's phonemizers want different note lyrics per language.**
  - **Japanese:** one kana mora per note, in hiragana. The phonemizer builds VCV and CVVC aliases itself
    from a plain `か`.
  - **English and the other Latin-script languages:** the **whole word on the first note**, then `+` on
    each following note of the word.
  - **Chinese:** one hanzi (or pinyin syllable) per note. OpenUtau converts hanzi to pinyin itself.
  - **Korean:** one hangul syllable per note.

  So sona's "unit" is a mora or a syllable for counting notes, but the exported lyric is per language
  (table 1).
- **Japanese units need two corrections to today's splitter, whatever the reader.**
  - **っ is never a note.** It is in 0 of the 5,336 dev UST notes, but the 9 lyric texts hold 137.
  - **A long-vowel tail usually isn't a note.** A ー or a spelled-out long vowel (とう, せい) gets its own
    UST note only 33 of 187 times (18%).

  On hiragana input, today's splitter misses **12.4%** of the UST's moras [8.6, 16.9]. Applying the UST
  conventions gives **9.0%**. Adding an OpenJTalk-family analyser with tails merged gives **5.0%**; with
  tails left to the fit, **4.3%** (table 3).
- **Kanji input: the best readers are the OpenJTalk family and fugashi+unidic-lite, and they tie.** Read
  from the lyric site's kanji text, pyopenjtalk-plus misses **3.8%** of the UST's moras [2.1, 5.9] on
  Japanese lines with tails optional, or **4.5%** with tails merged. Vanilla OpenJTalk gets 3.9% and
  fugashi+unidic-lite 4.1%. The paired difference to fugashi is [−0.2, +0.9] points.
  - Kanji words, 60% of the moras, err at **5.3%**. Kana words err at **1.8%**: that is the floor from
    text vs sung differences.
  - pykakasi (GPL-3+) is worst at 11.1%, ipadic 5.3%, and Sudachi 7.1% (it has no pronunciation field).
- **Kanji errors are mostly about the lyric, not the dictionary.** The kinds are:
  - wrong-sense readings (one common noun read as a personal name costs 20 edits across 5 lines);
  - gikun, the lyricist's own ruby (illustration, not from the corpus: 本気→マジ);
  - sung variants (illustrations: 言う→ゆう, 行く→ゆく, rendaku);
  - 何 read なに or なん.

  Numbers were too rare in the texts to measure. Using the wiki's ruby for 3 songs moves 3.8 → 3.7%.
- **English words inside Japanese songs** need a katakana reading for a Japanese bank. 22 of 368 dev
  lines are Latin, across 4 songs. OpenJTalk spells the letters out, missing 88% of those lines' UST
  moras. e2k (Unlicense, Rust port) or alkana (GPL-2) bring that to 25%, still a review job.
- **Language detection from text is a script rule plus one detector for Latin text.** All 9 songs are
  Japanese by both the rule and lingua. Lingua per line calls 8 kanji-only lines in Japanese songs
  Chinese, so lines must inherit the song's language.
- **Licences:**
  - Everything recommended is MIT, BSD or Apache: jpreprocess with naist-jdic, lindera, lingua and
    whatlang, CMUdict, pypinyin and pinyin-pro, e2k.
  - GPL tools (pykakasi, espeak-ng, `phonemizer`, alkana) could only run as a separate user-installed
    process. Nothing here needs them.

**Recommendation.**
- Build the units module (08) and language detection (10) now. Neither needs a dictionary.
- Make long-vowel tails optional in the fit (09). The dictionary-free rule gets 5.3% this way; it fails
  (8.4%, no better than the split) if it merges tails outright.
- Read kanji (11) only if the user accepts it. The reader would be jpreprocess, inside `sona serve`, with
  a review step. Expect about 4–5% of moras wrong vs the UST, roughly 20 per song, about half from kanji.
  The other half is there with hand-typed hiragana too.

## 1. Per language: unit, OpenUtau phonemizer, library, licence, runtime

Sources:
- OpenUtau `master` at `08222cb`. The repo is now `openutau/OpenUtau`.
- The OpenUtau wiki "Phonemizers" page.
- Each library's LICENSE, PyPI, crates.io or npm metadata.
- Licences of the packages installed for the measurement, checked from their installed metadata.

I re-read these OpenUtau claims myself:
- Note grouping, in `OpenUtau.Core/Ustx/UPart.cs`: a note extends the previous one only if it touches
  it and its lyric starts with `+`.
- JA VCV alias building: `JapaneseVCVPhonemizer.cs` tries `"{vow} {lyric}"`, then `* `, the bare
  lyric, then `- `.
- The wiki: EN ARPA takes "the whole word in the first note, then use `+`"; JA CVVC and VCV take
  "Lyrics should be written in hiragana".

| Language | Note unit | OpenUtau phonemizers (tag) | Lyric OpenUtau expects | Reading / G2P library | Licence | How it runs |
|---|---|---|---|---|---|---|
| Japanese | mora (っ not a note; long-vowel tail optional) | Default (CV, lyric = alias), JA VCV, JA CVVC, JA VCV & CVVC (presamp), DIFFS JA, ENUNU, VOICEVOX | hiragana, one mora per note. VCV/CVVC aliases are built by the phonemizer. `[hint]` overrides. Presamp converts whole-lyric romaji | Kanji→kana: jpreprocess (OpenJTalk frontend port) + naist-jdic; or lindera/vibrato + ipadic/unidic. Measured as pyopenjtalk(-plus) and fugashi | jpreprocess BSD-3; naist-jdic BSD-3; lindera MIT; UniDic tri-licence (take BSD) | Rust (jpreprocess, lindera) in `sona serve`; browser via lindera-wasm (17.8 MB ipadic) or kuromoji.js (41 MB, stale); Python only for the prototype |
| English | syllable | EN ARPA, ARPA+, EN VCCV, EN X-SAMPA, EN C+V, DIFFS EN, ENUNU EN; EN to JA for Japanese banks | **word on the first syllable's note, `+` on the rest** (`+n` aligns to a phoneme, `+~` holds a vowel); `word[ph o n]` or `[ph o n]` hints | Syllable count: CMUdict vowel count; out-of-vocabulary words: hypher en patterns (hyphenation ≠ syllables) or pysle/ISLEdict. OpenUtau itself uses CMUdict 0.07 + an ONNX G2P | CMUdict BSD-2; hypher MIT/Apache (en patterns permissive); pysle MIT. **GPL:** espeak-ng, `phonemizer`, `syllables`, big-phoney, Python `cmudict` | Rust (CMUdict file 3.6 MB + hypher) or browser; nothing GPL needed |
| Chinese (Mandarin) | one hanzi | ZH CVVC, ZH CVV, ZH CVV+, ZH VCV, DIFFS ZH, VOGEN ZH | hanzi or toneless pinyin per note; OpenUtau romanizes hanzi with csharp-pinyin (Apache-2.0) | Display or check only: pinyin-pro (JS), `pinyin` crate (no phrase context), pypinyin | MIT (all three); **g2pM bundles CC-CEDICT (CC-BY-SA)** | browser or Rust; sona needn't convert |
| Cantonese | one hanzi | ZH-YUE CVVC, ZH-YUE SYO, DIFFS ZH-YUE, VOGEN ZH-YUE | hanzi or jyutping | (OpenUtau's csharp-pinyin) | Apache-2.0 | — |
| Korean | one hangul syllable | KO CV / CVC / CVVC / VCV / CVCCV / CBNN, DIFFS KO, ENUNU KO; KO to JA | hangul syllable per note; OpenUtau applies sound changes across notes | none needed (U+AC00–D7A3 = one block per note) | — | trivial |
| Spanish, Italian (SYL), French, Portuguese, Russian, Ukrainian, German, Filipino, Finnish | syllable | ES SYL / VCCV / MAKKU, IT SYL, FR CVVC / VCCV / SPHINX, PT-BR CVC, RU CVC / VCCV, DE VCCV / DIPHONE, FIL, DIFFS × | word + `+`, like English | OpenUtau bundles per-language G2P packs (licences not stated in the zips) | — | out of scope for now |
| Vietnamese | syllable | VIE CVVC / VCV / VINA | one quốc ngữ syllable per note (tones stripped by the phonemizer) | none | — | — |
| Thai | syllable | TH VCCV | one Thai-script syllable per note | none | — | — |
| Italian CVVC, Turkish, Polish | romanized syllable | IT CVVC, TR CVVC, PL CVC | romanized syllable per note | none | — | — |

Language ID from text:
- lingua (Rust `lingua-rs` / Python): Apache-2.0, models Apache-2.0. The ja model is 82 KB, en 2.6 MB.
- whatlang: MIT, 83 KB.
- franc: MIT, JS, weak on short text.
- cld3: Apache-2.0, archived.
- fastText lid.176: **model CC-BY-SA 3.0**.

Script ranges settle most cases (section 5).

## 2. Japanese reading accuracy (kanji text → hiragana moras vs the lead UST)

**Method.**
- **Input:** the lyric site's text `<id>.txt`, as a user would paste it.
- **Readers:** each reads the text to kana. The kana become UST-style moras:
  - small kana join the mora before;
  - っ is dropped (the UST never writes it);
  - ー repeats the vowel;
  - を = お, ぢ = じ, づ = ず (sung alike; the UST writes お and じ, ず).
- **Matching:** lines are grouped into units of ≥ 12 moras within a paragraph. Each unit is matched to
  its best span anywhere in the lead UST's mora stream (an approximate-substring edit distance). Repeats
  the lyric writes once cost nothing.
- **Rate:** edits ÷ matched UST moras, pooled over songs, with a 10,000-draw song bootstrap. "Japanese
  lines" excludes units with Latin letters (section 4).
- **Modes:**
  - `read`: the orthographic reading, with particles は/へ sung as わ/え (needs the reader's part of
    speech).
  - `read-raw`: the same without the particle fix.
  - `pron`: the pronunciation field, ー as a vowel.
  - `pron-merge`: the pronunciation with long-vowel tails dropped.
  - `pron-opt`: tails free either way. This is the fit deciding, so it is an upper bound for ticket 09.

Pooled, 9 songs, Japanese lines (4,467 UST moras for pyopenjtalk-plus). The paired column is the
difference in this rate against pyopenjtalk-plus `pron-opt`.

| Reader (mode) | Mora error rate | 95% CI | Kanji-word moras | Kana-word moras | Paired Δ vs best |
|---|---|---|---|---|---|
| pyopenjtalk-plus (`pron-opt`) | **0.038** | [0.021, 0.059] | 0.053 | 0.018 | — |
| OpenJTalk vanilla (`pron-opt`) | 0.039 | [0.023, 0.060] | 0.056 | 0.018 | [+0.000, +0.003] |
| fugashi + unidic-lite (`pron-opt`) | 0.041 | [0.024, 0.062] | 0.064 | 0.026 | [−0.002, +0.009] |
| fugashi + ipadic (`pron-opt`) | 0.053 | [0.038, 0.077] | 0.079 | 0.033 | [+0.010, +0.022] |
| pyopenjtalk-plus (`pron-merge`) | 0.045 | [0.027, 0.068] | 0.064 | 0.024 | [+0.004, +0.011] |
| fugashi + unidic-lite (`pron-merge`) | 0.051 | [0.033, 0.072] | 0.078 | 0.035 | [+0.007, +0.020] |
| pyopenjtalk-plus (`pron`, tails as vowels) | 0.071 | [0.042, 0.105] | 0.098 | 0.035 | [+0.018, +0.049] |
| pyopenjtalk-plus (`read`) | 0.072 | [0.046, 0.102] | 0.094 | 0.041 | [+0.022, +0.046] |
| pyopenjtalk-plus (`read-raw`, は as は) | 0.089 | [0.060, 0.128] | 0.094 | 0.077 | [+0.036, +0.071] |
| SudachiPy + core (`read`, no pronunciation field) | 0.071 | [0.045, 0.100] | 0.096 | 0.043 | [+0.021, +0.045] |
| fugashi + unidic-lite (`read`) | 0.075 | [0.049, 0.106] | 0.103 | 0.052 | [+0.023, +0.052] |
| fugashi + ipadic (`read`) | 0.084 | [0.061, 0.113] | 0.114 | 0.055 | [+0.034, +0.057] |
| pykakasi (`read`; no part of speech, no pronunciation) **GPL-3+** | 0.111 | [0.085, 0.142] | 0.134 | 0.090 | [+0.056, +0.094] |

**Per song, Japanese lines.** Song order: innocent-addict, kamaitachi-ni-yoru, telescope-nevermore,
atamaro-deluxe, azemichi-ni-ochita-ginga, yamineko, my-axiom, amaten, fukagyakuteki-henyou.

| Reader (mode) | inn | kam | tel | ata | aze | yam | axi | ama | fuk |
|---|---|---|---|---|---|---|---|---|---|
| pyopenjtalk-plus `pron-opt` | 0.018 | 0.012 | 0.017 | 0.048 | 0.023 | 0.038 | 0.107 | 0.025 | 0.023 |
| pyopenjtalk-plus `pron-merge` | 0.026 | 0.021 | 0.019 | 0.054 | 0.041 | 0.046 | 0.117 | 0.028 | 0.023 |
| fugashi+unidic-lite `pron-opt` | 0.000 | 0.035 | 0.019 | 0.052 | 0.019 | 0.031 | 0.106 | 0.041 | 0.023 |
| pyopenjtalk-plus `read` | 0.037 | 0.033 | 0.039 | 0.087 | 0.053 | 0.046 | 0.166 | 0.094 | 0.059 |
| SudachiPy `read` | 0.031 | 0.037 | 0.040 | 0.093 | 0.049 | 0.041 | 0.157 | 0.097 | 0.057 |
| pykakasi `read` | 0.183 | 0.121 | 0.071 | 0.124 | 0.079 | 0.066 | 0.194 | 0.136 | 0.082 |

**Error kinds**, pyopenjtalk-plus, Japanese lines. In `read` there are 322 edits:

| Kind | Count |
|---|---|
| long vowel, reader writes the tail the UST merges | 160 |
| kanji reading | 104 |
| kana words, text vs sung | 30 |
| long vowel, other vowel | 21 |
| long vowel, UST holds a vowel the text doesn't write | 9 |

`read-raw` adds 53 particle errors: は/へ written as read. In `pron-opt` there are 169 edits: kanji
reading 99, kana text vs sung 25, other vowel 19, tail 17, held vowel 9.

The kanji-reading edits break down as:
I sorted the 99 kanji-reading edits by hand, by word:

| Kind | Edits | What it is |
|---|---|---|
| other dictionary reading | 63 | The word has two dictionary readings and the reader picked the unsung one (on/kun, なに/なん, あす/あした). One common noun read as a personal name accounts for 20 edits, 5 lines in one song. |
| sung variants | 15 | The singer's form (illustrations, not from the corpus: 言う→ゆう, 行く→ゆく, よい/いい, rendaku). |
| gikun / lyricist's ruby | 11 | The lyric writes one word and sings another; the wiki's ruby marks some of them. |
| word-level text vs sung | 10 | A mora added or dropped inside a kanji word. |

Corpus words are not quoted here, because Pifuyuu's lyrics stay out of the repo.
- **何:** なに or なん.

There were too few numbers to count (6 digits in 3 songs, all in mixed lines).

**What else could these numbers be measuring?** Each check below was run.

- **Text vs sung differences.** A lyric site's line is not always what is sung: kana words err at
  1.8%. That floor is in every reader's number, and the paired column cancels it.
- **Matcher leniency.** A unit could match a wrong place in the song. Matching whole paragraphs
  instead of ≥12-mora line groups gives the same ordering: pyopenjtalk-plus `pron-opt` 0.041, unidic
  0.041, ipadic 0.057, Sudachi 0.069, pykakasi 0.105. The paired Δ for unidic is [−0.005, +0.007].
- **One outlier song.** my-axiom is 2–3× the others for every reader. The name misreading and a
  katakana line sung differently account for it. Its rank order matches the pooled one.
- **Readers that read nothing.** fugashi returns no reading for Latin words, so its rate over all
  lines (0.049) looks better than OpenJTalk's (0.152, which spells letters out). That is why the table
  uses Japanese lines. Readers that read nothing are scored on fewer moras (N 4,284 vs 4,467).
- **Harness bugs found and fixed.**
  - Sudachi read a space as キゴウ ("symbol"). Its first rate was 0.129; the fixed rate is 0.071.
  - ・ was counted as a mora.
  - A scratch file named `inspect.py` shadowed the standard library and silently dropped three
    readers.

  The numbers above are after the fixes.
- **Ruby.** Using `<id>.readings.txt`, the wiki's ruby for 3 songs, gives pyopenjtalk-plus `pron-opt`
  0.037 [0.022, 0.058] vs 0.038. Ruby is rare in these texts but exact where present.

## 3. Units from hiragana (what the spec's lyric box gets)

The spec asks the user for hiragana, so this is the case to get right first. As a stand-in for a
user's hiragana, I used pyopenjtalk-plus's orthographic reading of each text:
- particles as written (は, へ);
- long vowels spelled out;
- Latin words left as Latin.

This text carries that reader's kanji errors, the same in every row. The rows differ only in how
hiragana becomes units.

Japanese lines, 9 songs. The Δ column is the paired difference to the row before.

| Units from hiragana | Mora error rate | 95% CI | Δ |
|---|---|---|---|
| today's `web/lib/lyrics.ts` (っ and ー are notes) | 0.124 | [0.086, 0.169] | — |
| + UST conventions (っ not a note, ー repeats the vowel) | 0.090 | [0.060, 0.132] | [−0.047, −0.022] |
| + dictionary-free tail rule, tails **dropped** | 0.084 | [0.062, 0.114] | [−0.025, +0.012] vs the row above: no gain |
| + dictionary-free tail rule, tails **optional** | 0.053 | [0.031, 0.083] | [−0.054, −0.021] vs conventions |
| analyser on the hiragana (pyopenjtalk-plus `pron`) | 0.068 | [0.041, 0.099] | |
| analyser, tails dropped (`pron-merge`) | 0.050 | [0.030, 0.077] | |
| analyser, tails optional (`pron-opt`) | 0.043 | [0.024, 0.069] | [−0.066, −0.033] vs conventions |
| fugashi + unidic-lite on the hiragana, `pron-opt` | 0.044 | [0.027, 0.066] | [−0.004, +0.005] vs the row above |

Notes on these rows:
- The dictionary-free rule calls a vowel kana a tail when it continues the mora before (おう, えい, or
  the same vowel).
- An analyser reads hiragana nearly as well as kanji text: 4.3% vs 3.8%. It finds the particles
  (2 particle errors left) and the true long vowels.

Long-vowel tails and っ (pyopenjtalk-plus pronunciation, Japanese lines):
- **Tails:** a tail has its own UST note 33 times and is merged into the note before 154 times
  (**18% own note**). There are 19 more substitutions.
- **っ:** 137 in the 9 texts, **0** in 5,336 UST notes.

## 4. English inside Japanese songs

Units containing Latin letters: 4 songs, about 650 UST moras. The UST writes these words in kana for
the Japanese bank, as the singer chose to sing them.

| Latin words read by | Mora error rate on those units | 95% CI |
|---|---|---|
| pyopenjtalk-plus alone (spells the letters) | 0.883 | [0.477, 1.021] |
| e2k C2K → katakana, then pyopenjtalk-plus (`pron-opt`) | 0.250 | [0.111, 0.421] |
| alkana dictionary → katakana, then pyopenjtalk-plus (`pron-opt`) **GPL-2** | 0.253 | [0.171, 0.444] |

innocent-addict is most of it: 489 of the e2k run's 656 moras. One of my-axiom's English lines is not
in the lead UST at all. Either way, a converted English word is a proposal to review, not a reading.

## 5. Language detection (dev texts)

The script rule, per text:
- any kana → Japanese;
- hangul → Korean;
- Han only → "Chinese or Japanese?";
- Latin only → run a detector.

| | Song level (9) | Line level (368 lines) |
|---|---|---|
| Script rule | 9 × Japanese | 337 Japanese, 22 Latin, 8 Han-only, 1 mixed |
| lingua (all languages) | 9 × Japanese | the 22 Latin lines → English; the **8 Han-only lines → Chinese**; 1 mixed line → Romanian |

So:
- The song's language comes from the whole text.
- A line inherits it.
- Only a Latin island inside a Japanese song is flagged, as English. lingua is right on all 22.

## 6. Design: a per-language units module behind the lyric box

```
web/lib/units/
  index.ts   detect(text) -> Detection; unitsFor(lang): LanguageUnits
  ja.ts      Japanese (first)
  en.ts, zh.ts, ko.ts (later, ticket 13)

type Lang = "ja" | "en" | "zh" | "ko" | …
interface Detection { lang: Lang | null; confidence: "sure" | "ask" | "unsupported"; islands: Island[] }
interface Island { line: number; start: number; end: number; script: "latin" | "han" | …; guess?: Lang }
interface Unit {
  text: string;            // the sung unit (mora, syllable, hanzi)
  lyric: string;           // what the export writes on its note: "か"; "hello" then "+"; "你"
  kind: "consonant" | "open";  // for lyric_fit (mora_of today)
  optional: boolean;       // ja: long-vowel tail; the fit may give it no note
  clipped?: boolean;       // ja: followed by っ (no note of its own)
  span: [number, number];  // where it came from in the line, for highlighting
}
interface LanguageUnits {
  validate(line: string): Problem[];          // offending characters + positions (03's message)
  split(line: string): Unit[];                // per-note units, one line = one phrase
  read?(line: string): Promise<Reading[]>;    // ja: kanji → kana via sona serve (ticket 11), opt-in
}
```

**Japanese first (ticket 08).** `split` follows the UST's conventions as measured:
- one hiragana mora per unit; small kana join the mora before; ん is its own unit;
- っ marks the unit before as `clipped` and makes no unit;
- ー and dictionary-rule tails become `optional` units.

The fit (09) gives an optional unit no note cheaply. Without a dictionary this reaches 5.3% on dev,
against 12.4% today.

Optional analyser: jpreprocess in `sona serve`, the same engine as ticket 11. It adds:
- particles (は → わ in the exported lyric; the count and kind are unchanged);
- better tails (4.3%).

**Where each part runs:**
- Splitting, validation and detection run in the browser. Detection needs no library for
  ja/ko/zh-or-ja.
- A Latin-only text is checked by lingua-rs or whatlang in `sona serve`, or franc in the browser, only
  to name the unsupported language.
- Readings (kanji → kana, English → katakana) run in `sona serve` in Rust: jpreprocess with naist-jdic
  (BSD-3, ~20 MB), e2k-rs (Unlicense). No Python, and nothing GPL.
- The prototype's Python stays in `~/.cache/sona/lang-units`, for evaluation only.

**What the user sees:**
- **Confident** (kana present): a "Japanese" chip next to the lyric box, which the user can change.
  Units are drawn under each line, so the user sees how many notes it asks for. Optional tails are
  dimmed. っ shows as a small mark on its mora.
- **Unsure:**
  - Han-only text: "Is this Japanese or Chinese?", with Start disabled until the user answers.
  - A Han-only line inside a Japanese song: no question, it is Japanese.
- **An island:** an English line or word in a Japanese song is underlined. "The Japanese voicebank
  sings kana: [Suggest katakana] [I'll type it]". The suggestion is e2k's, shown for review (ticket 12).
- **Kanji** under today's rule: "Kanji isn't supported yet: <the kanji found>" (03). If the user accepts
  ticket 11, a "Read kanji for me" button replaces each kanji word with its hiragana in place and
  underlines it. Ambiguous words (何, 今日, names) are marked. `漢字(かな)` ruby overrides the reading.
  Nothing converts without the click.
- **Unsupported language:** "Only Japanese is supported for now (this looks like English)."

## 7. Licences and what may ship

- **Safe to link or ship:**
  - jpreprocess and naist-jdic (BSD-3);
  - lindera (MIT) with ipadic;
  - vibrato (MIT/Apache);
  - UniDic under its BSD option;
  - lingua (Apache-2.0) and whatlang (MIT);
  - CMUdict (BSD-2);
  - hypher (MIT/Apache; its en patterns are permissive);
  - pypinyin, pinyin-pro and the `pinyin` crate (MIT);
  - e2k (Unlicense);
  - wanakana (MIT).
- **Measured here only, separate process if ever used:** pykakasi (GPL-3+) and alkana (GPL-2). Both
  were measured and neither is needed.
- **Never needed:** espeak-ng and `phonemizer` (GPL-3+). Running a user-installed `espeak-ng` binary as
  a subprocess would not bind sona's code; linking libespeak-ng would.
- **Data not to ship:**
  - fastText lid.176 (CC-BY-SA 3.0);
  - g2pM's CC-CEDICT (CC-BY-SA 4.0);
  - the `arpabet` crate (BSD-4, no LICENSE in its repo).
- **Unconfirmed:**
  - the licence of the UniDic-CSJ part of pyopenjtalk-plus's dictionary (it was measured, not
    proposed for shipping);
  - the licences of OpenUtau's non-CMU G2P packs.

## 8. Negative results

- **Merging tails by a spelling rule doesn't help:** 8.4% vs 9.0% for the split, Δ [−0.025, +0.012].
  It merges real vowels, such as a verb ending in おう. Tails must be optional, decided by the fit, or recognised by an
  analyser.
- **Per-line language ID is harmful inside a song:** 8 false Chinese and 1 false Romanian.
- **pykakasi:** the worst `read`-mode reader on all 9 songs, and GPL. Drop it.
- **SudachiPy** reads as well as OpenJTalk in `read` mode. It has no pronunciation field, so it can't
  find tails or particles by itself.

## Tickets

| Ticket | Blocked by |
|---|---|
| 08 Japanese units | 03 |
| 09 optional moras in the fit | 01, 08 |
| 10 language detection | 03, 08 |
| 11 kanji reading (user decision) | 08, 10 |
| 12 English words in Japanese lyrics | 10, 11 |
| 13 more languages (user decision) | 08, 10 |

All are in `.scratch/lyrics-first/issues/`, Status `needs-triage`.
