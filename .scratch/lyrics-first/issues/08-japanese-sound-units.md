# 08 — Japanese sound units behind the lyric box

Status: resolved
Type: task
Blocked by: 03

## What

Replace the Japanese path of `web/lib/lyrics.ts` with a units module (`web/lib/units/`, design in
`crates/sona-eval/reports/language-and-units.md`, "The units module"). It turns one pasted line into the
per-note units a Japanese CV/VCV/CVVC bank expects, as the UST writes them:

- One hiragana mora per note; small kana join the mora before (きゃ, うぃ, ふぉ); ん is its own unit.
- **っ is not a note.** 0 of the 5,336 dev UST notes carry っ, while the 9 dev lyric texts hold 137. It
  marks the mora before as clipped (a flag the fit and export can use later). Today's splitter gives it
  a note of its own.
- **A long-vowel tail is optional.** The UST gives a ー, or a spelled-out long vowel (とう, せい), its own
  note only 18% of the time (33 of 187 on dev). The unit carries `optional: true`, and the fit decides
  (ticket 09).
- Each unit carries its mora kind (`Consonant` / `Open`, as `sona_core::lyric_fit::mora_of`), so the
  fit gets the units without re-parsing.
- を stays as typed. OpenUtau's JA phonemizers look it up as an alias. Pifuyuu writes お, so the dev
  metric treats を = お, ぢ = じ and づ = ず as equal.
- The rejection rule in `spec.md` is unchanged (hiragana, ー, small kana, っ, separators). The module
  reports the offending characters with their positions, for 03's message.

The lyric box still accepts hiragana only. Reading kanji is ticket 11, and it needs the user's decision.

## Acceptance

- Unit tests through the public interface: each convention above, plus a mutant (っ as a unit) that
  must be caught.
- The README's "Paste lyrics" line is updated: っ is no longer "its own".
- On dev, the scratchpad harness (`crates/sona-eval/scripts/language-units/`) scores the module's units
  for the hiragana text at least as well as the prototype's `pron-merge` mode. Per-song text stays in
  the scratchpad.

## Comments

## Answer

Built `web/lib/units/` (`unitsFor("ja")`: `validate`, `split`, `inOrder`; `index.ts` holds the
language-neutral types, `ja.ts` the Japanese rules; another language adds a module). `lyrics.ts` now
checks with `validate` and lays with `inOrder`; the old splitter is gone.

- **Units.** A hiragana mora a note; small kana join the mora before (not ん, っ, ー, a tail or a space);
  ん its own (open). **っ makes no unit** and marks the unit before `clipped`. A **long-vowel tail** is
  `optional: true`: ー (sings the vowel before: きゃー → きゃ, あ), or a vowel kana after its own vowel,
  おう or えい, within a word (a space or っ breaks it). Every unit has `kind` (`consonant`/`open`, as
  `mora_of`) and a `span`. を and は stay as typed. `validate` gives each refused character with
  `[start, end)` in its line; `checkLyric`'s messages are unchanged.
- **In order** (no fit, until 09/04): the dev USTs hold ー (14 of 16) and おう (105 of 131) in the note
  before, but give **えい (69 of 103) and a repeated vowel (45 of 66) a note of their own**. So `inOrder`
  holds only ー and おう. Skipping every optional tail, as first planned, is worse (below). `optional` is
  still set on all four, for the fit.
- **Edit → Lyrics…** takes the start screen's rule: anything but hiragana, ー and spaces is named in the
  preview, and the Fill buttons stay off. Romaji, English and kanji no longer fill notes there (typing a
  note's lyric with **L** still takes anything).

**Dev score** (9 songs, ja-only lines, 07's hiragana stand-in, rule.py's groups and metric, song bootstrap):

| Units | Mora error rate | 95% CI |
|---|---|---|
| prototype `split` (UST conventions, no tails) | 0.0903 | [0.060, 0.132] |
| prototype `rule-opt` | 0.0525 | [0.031, 0.083] |
| prototype `pron-merge` (the bar) | 0.0514 | [0.030, 0.079] |
| prototype `pron-opt` | 0.0444 | [0.024, 0.072] |
| **module, tails optional** | **0.0535** | [0.031, 0.086] |
| module, particles scored equal (は=わ, へ=え, both sides) | 0.0364 vs pron-merge 0.0507 | paired [−0.022, −0.008] |
| module laid in order (`inOrder`) | 0.0672 | [0.041, 0.106] |
| module, every optional tail skipped | 0.0831 | [0.061, 0.115] |

- Against `pron-merge` the module is 8 errors behind in 4,244 moras: paired [−0.006, +0.012], so the two
  can't be told apart. **The whole gap is the particle は**: 49 は→わ substitutions, which a
  dictionary-free splitter can't tell from a は word. With particles scored equal, the module is clearly
  better (−1.4 points). So it meets the bar on everything but particles. Closing that needs the
  analyser (jpreprocess, ticket 11's engine) or the user typing わ.
- `inOrder` vs skipping every tail: paired [−0.025, −0.006]. The port reproduces the prototype on the
  same groups exactly (rule-opt 0.0525, pron-opt 0.0444, as 07 reported), and the module's own `inOrder`
  reproduces the scorer's held-tail rule exactly (285 errors).
- What else the numbers could be measuring: an optional unit is free in the opt metric, so over-marking
  tails is never penalised there. The in-order row is the check that marking is sensible. The stand-in text is
  pyopenjtalk's orthographic reading with no spaces. A user's own spacing and long-vowel spelling
  (せんせい vs せんせー) will differ.

**Tests.** `web/lib/units/units.test.ts` (19) and `lyrics.test.ts` (11) through the public interface;
241 web tests, `tsc` clean. Mutants caught: っ as a unit (6 tests), tails never optional, `inOrder`
skipping every tail, small kana never joining. `lyrics_export.py` gains a Lyrics-box check, which also
catches the っ mutant. All 12 editor e2e drivers pass, with no expectation changed (their lyrics have no
っ or tails), and so does `lyric_start.py` against this branch's `sona serve`. The lyric-editor TLA+ model doesn't count units, and it passes.

Scorer, bridge and per-song text: session scratchpad only (`units08/`), never committed.
