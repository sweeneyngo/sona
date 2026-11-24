# 08 — Japanese sound units behind the lyric box

Status: needs-triage
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
