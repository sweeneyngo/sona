# 01 — Lyric placement metric and line oracle

Status: resolved
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

### 2026-11-24 — claimed; metric, oracles and decision rule (declared before measuring)

**Lyric placement** (`sona_eval::correction::lyric_placement`). Both sides go on the UST's cells exactly
as in the edit cost (same rounding, overlaps cut, only output notes in the UST's span). A mora is
compared after taking a VCV lyric's last word and folding katakana to hiragana.
- A UST note `r` is **matched** to the output note `o` that shares the most cells with it (ties: the
  earlier note), when `r` is also the UST note that shares the most cells with `o`. One match per note.
- `r` is **placed** when it is matched and `o` carries `r`'s mora. Every other UST note is a mora to
  retype or shift: **wrong moras per minute** = unplaced UST notes ÷ the UST span in minutes (the edit
  cost's span). **Placed** = placed ÷ UST notes.
- A UST phrase (a run of touching UST notes) **lands** when all its notes are placed and no other output
  note whose midpoint lies in the phrase carries a mora. **Phrases landed** = landed ÷ UST phrases.
- Intervals: song bootstrap (10 000 resamples) of the pooled value, and paired against (d) for the
  differences.

**Truth and the pasted text.** The truth is the lead UST's lyric per note. Every oracle pastes the UST's
own mora stream (perfect text) so that only the phrasing differs. For (b) the line breaks come from the
lyric text: its reading (fugashi + unidic-lite, kana kept) is aligned to the UST's mora stream by edit
distance, and each text line start maps to the UST note it aligns with. Kept out of the repo.

**Oracles** (GAME lead on the onset phase, out of sample; learned stage, in-sample on dev). The fit is
07's `sona_core::lyric_fit` as shipped, extended to say which mora each fitted note gets; a note given
no mora carries none, and a mora the DP leaves unsung goes nowhere.
- (a) phrases = the UST's phrases: the output notes whose midpoint lies in a UST phrase's cells get that
  phrase's moras, fitted (07's ceiling).
- (a′) 07 as ported: phrases = runs of touching output notes, moras = UST notes whose midpoint lies inside
  (for reference).
- (b) phrases = the lyric text's lines: a line's span runs from its first UST note's start to its last
  UST note's end; the output notes whose midpoint lies inside get the line's moras, fitted. This places
  the lines in time perfectly; mapping them to sona's notes without the UST is 02's work.
- (c) the whole song as one phrase: every output note in the UST span and every mora, fitted.
- (d) today: the moras pasted in order onto the output notes from the first one in the UST span, no fit
  (`web/lib/lyrics.ts` `assignLyrics`).
- Diagnostics: how many line breaks fall on a sona rest (a gap between output notes within one cell of
  the UST's gap or boundary between the lines), and how many sona rests are line breaks; also how many
  line breaks are UST phrase breaks, and the reverse.

**Decision rule for 02.** Lines alone are good enough if (b)'s GAME lead edit cost keeps at least three
quarters of (a)'s gain over the unfitted output and its wrong moras per minute are within (a)'s
interval. Otherwise 02 splits lines further at sona's rests (DP over lines × rests), and this ticket
measures that bound as well: (b) with each line also cut at the UST's own rests inside it is (a) by
construction, so the gap (a) − (b) is the most such a split could recover.

## Answer

Full report: `crates/sona-eval/reports/lyric-placement.md`. Dev only; GAME lead (out of sample) first, stage
(in-sample) second. Δ = edit cost against the same output with no lyric, paired song bootstrap.

| landing | lead cost/min (Δ) | lead wrong moras/min · placed | stage cost/min (Δ) · placed |
|---|---|---|---|
| (d) pasted in order, today | 188.7 (0) | 183.2 · 4.9% | 180.2 (0) · 4.3% |
| (a′) 07 as ported | 174.9 (−13.9) | 71.6 · 62.9% | 162.4 (−17.8) · 69.8% |
| (a) UST phrases, shipped fit | 205.2 (+16.5 [+3.4, +29.6]) | 76.6 · 60.3% | 176.5 (−3.7) · 68.2% |
| (a) UST phrases, rests hold moras | 174.7 (−14.1 [−31.1, −3.5]) | 74.0 [48.9, 102.1] · 61.6% | 162.0 (−18.2) · 67.2% |
| (b) lines, rests hold moras | 176.7 (−12.0 [−30.2, −1.1]) | 93.1 [70.4, 118.3] · 51.7% | 167.4 (−12.8) · 50.1% |
| (b+) lines and spaces, rests | 175.2 (−13.5 [−31.1, −2.7]) | 83.4 [58.7, 110.7] · 56.7% | 167.4 (−12.8) · 55.3% |
| lines cut at UST phrases, rests | 168.9 (−19.8 [−38.7, −7.9]) | 66.1 · 65.7% | 155.8 (−24.4) · 71.0% |
| (c) whole song, rests | 189.0 (+0.3) | 179.4 · 6.9% | 180.7 (+0.5) · 8.0% |

- **Pasting in order places 4.9% of moras**: one missing or extra note shifts the rest of the song.
- **The shipped `fit_phrase` needs the rests.** 07's −13.9 relied on a simulated user who pastes onto each run of
  sona's notes only the moras sung inside it. With honest phrases (the UST's), moras sona missed are forced
  into neighbours: +16.5. Letting the phrase's rests hold moras (`sona_core::lyric_fit::fit_span_moras`, new,
  07's 1b DP) gives −14.1, the same as 07's number.
- **Decision rule: lines pass.** With rests, lines keep 85% of the UST phrases' gain on GAME lead (rule: 75%),
  with wrong moras inside (a)'s interval; lines plus spaces keep 96% and lose fewer moras. The stage keeps 70%.
  The rule's "lines cut at UST rests is (a) by construction" was wrong: lines break inside UST phrases too, and
  cutting at both beats either (−5.8 [−10.4, −1.9] against (a)).
- **Lines and sona's rests disagree**: 49% of line breaks fall on a GAME lead rest, 36% of its rests are line
  breaks; 41% of line breaks have no rest in the UST either. 02 cannot snap lines to rests.
- **amaten carries most of the edit-cost gain** (−85 of 394 cost/min); the placement gain is on every song.
- **Ported:** `sona_eval::correction::lyric_placement` (metric, report), `lyric_fit::{fit_phrase_moras,
  fit_span_moras, mora_blocks}`, and `sona-eval-cellstage --lyric-placement [--lyric-lines DIR]`. The Rust
  reproduces the prototype on all 198 song × output × landing rows; the default run and
  `--fit-phrase-lyrics` are byte-identical to 07's.

**Recommendation for 02:** phrases from the pasted lines and the spaces in them; place each line on a run of
sona's notes by a song-level DP on mora counts and the fit's cost (a break may fall where notes touch); fit each
line with `fit_span_moras`. Ceiling (lines placed by the UST): −13.5 cost/min and 83 wrong moras/min on GAME
lead.
