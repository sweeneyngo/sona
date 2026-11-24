# 01 — Lyric placement metric and line oracle

Status: ready-for-agent
Type: research
Blocked by: none

## What

- Add a lyric placement metric to `sona-eval` (`sona_eval::correction`): match sona's notes to the lead
  UST's notes as the edit cost does, and count the matched notes whose mora is wrong. Report moras per
  minute to retype, and the share of phrases whose pasted lyric lands one mora per note. Unit tests,
  including the case where one extra note shifts the rest of a phrase.
- Hiragana lyric per dev song, with line breaks. The UST's own moras are the truth. The lines come from
  the lyric text in `crates/sona-eval/data/pifuyuu-lyrics/` (a lyric site's lines, kanji mixed in):
  align its reading to the UST's mora stream to place the line breaks, and keep the result out of the repo.
- Oracles on dev for the fitted notes and their moras, using 07's `lyric_fit`:
  (a) phrases = the UST's own phrases (07's ceiling);
  (b) phrases = the lyric text's lines;
  (c) whole song, no lines (07: no gain);
  (d) today: the lyric pasted in order with no fit (`web/lib/lyrics.ts`).
  How often does a line break fall on one of sona's rests?

## Acceptance

Table on dev: edit cost and lyric placement for (a)–(d), with song-bootstrap intervals. Recommend how
lines become phrases for 02.

## Comments
