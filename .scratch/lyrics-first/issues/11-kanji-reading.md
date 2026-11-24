# 11 — Reading kanji lyrics to hiragana (opt-in, reviewed)

Status: needs-triage
Type: task
Blocked by: 08, 10

Needs the user's decision first. The spec says hiragana only and "no silent conversion".

## What

Users paste lyrics from lyric sites, which mix kanji and kana. This ticket adds a "Read kanji for me"
action in the lyric box. It replaces each kanji word with its hiragana in place and underlines it, so
the user reviews the reading before starting. Nothing changes without the click.

- **Reader:** an OpenJTalk-family frontend. On dev, pyopenjtalk-plus, vanilla OpenJTalk and
  fugashi+unidic-lite tie for best; the numbers are in `crates/sona-eval/reports/language-and-units.md`.
  For sona, use **jpreprocess**: a Rust port of the OpenJTalk frontend, BSD-3, with naist-jdic (BSD-3)
  embedded. It runs inside `sona serve`, behind an endpoint the lyric box calls, so no Python is
  needed. Before wiring it in, confirm on dev that it reproduces the vanilla-OpenJTalk prototype's
  mora error rate.
- **Units:** use the reader's pronunciation for particles (は→わ, へ→え) and long-vowel tails, which
  become optional units (08).
- **Ruby:** accept `漢字(かな)` and `漢字《かな》` as an explicit reading. Lyric sites mark gikun this
  way (gikun, e.g. 本気→マジ).
- **Flags:** flag words with more than one common reading (何, 今日, 一日, names) for review.
- **Licences:** nothing GPL. pykakasi is GPL-3+, and it also reads worst on dev.

## Acceptance

- The dev mora error rate of the Rust reader matches the prototype's within the bootstrap noise.
- Unit tests for ruby and particles.
- An e2e driver: paste kanji, read, edit one reading, start.
- An ADR for the reader choice and the review step.

## Comments
