# Lyric placement and where a pasted lyric's phrases come from (lyrics-first 01)

Date: 2026-11-24. Dev only (9 songs, 27.5 min, 5 245 UST notes, 817 UST phrases). No holdout was read; the
spent splits were not re-read. GAME lead + octave is out of sample; the learned stage is in-sample on dev.
Baselines (unchanged, byte for byte): GAME lead 188.7 cost/min, stage 180.2. The metric, oracles and decision
rule were committed before measuring (02a11c6).

## The metric (`sona_eval::correction::lyric_placement`)

Both sides go on the UST's cells as in the edit cost. A UST note is **placed** when the output note that
shares the most cells with it (and for which it is also the best UST note) carries its mora. **Wrong moras
per minute** counts the UST notes that are not placed: the moras a user must retype or shift. A UST phrase (a
run of touching UST notes) **lands** when all its notes are placed and no other output note in it carries a
mora. `sona-eval-cellstage --edit-cost --lyric-placement [--lyric-lines DIR]` prints it, with the edit cost,
for every landing below; the default output is unchanged.

## Answer

- **Pasting in order (the app today) places 4.9% of the moras** (GAME lead; 4.3% on the stage). One missing or
  extra note shifts the rest of the song, so 183 moras per minute must be retyped. Any per-phrase fit places
  51–70%.
- **The shipped fit is wrong for honest phrases.** 07's −13.9 came from a simulated user who pastes onto each
  run of sona's touching notes exactly the moras sung inside it, so moras sona missed (a rest inside a sung
  phrase) were never pasted. Given the UST's own phrases, which hold those moras, `fit_phrase` forces each
  into a neighbouring note, and the split costs more than it saves: GAME lead **+16.5 [+3.4, +29.6]**.
  Letting the phrase's rests hold moras (`fit_span_moras`, 07's own 1b DP) brings it back to **−14.1 [−31.1,
  −3.5]** (stage −18.2), the same as 07's ported number (−0.2 [−3.4, +2.6] apart). Whatever gives the phrases,
  the fit must see the rests between them.
- **A lyric text's lines give phrases nearly as well as the UST's phrases, when they are placed in time.**
  With the rests holding moras, lines alone keep 85% of the UST phrases' edit-cost gain on GAME lead (−12.0
  [−30.2, −1.1]; paired against UST phrases +2.0 [−0.6, +4.8]), and lines plus spaces keep 96% (−13.5).
  Placement is worse: 93.1 and 83.4 wrong moras per minute against 74.0 (lines +19.1 [+9.0, +29.7]; lines and
  spaces +9.5 [−1.6, +22.5]). By the rule declared before measuring (three quarters of (a)'s gain, wrong moras
  within (a)'s interval [48.8, 102.5]), **lines are good enough, and lines plus spaces are better**. On the
  stage, lines keep 70% (−12.8 against −18.2).
- **Cutting lines further helps.** Lines cut at the UST's own phrase breaks beat either alone: −19.8 [−38.7,
  −7.9] (paired against UST phrases −5.8 [−10.4, −1.9]) and the fewest wrong moras (66.1). More anchors are
  better, so 02 should keep the user's spaces and let sona's rests cut a line too.
- **The whole song as one phrase does not work** (+37.8 with the shipped fit, +0.3 with rests holding moras;
  12% and 7% placed), as 07 found.
- **Lines and sona's rests disagree.** Only 49% of line breaks fall on a rest of GAME lead (60% for the stage),
  and only 36% of lead's rests are line breaks. Pifuyuu writes 41% of line breaks with no rest at all (the last
  note of a line touches the next line's first). So 02 cannot snap lines to rests: it must place each line on
  a run of notes by its moras, with a rest only a hint.
- **The gain is one song's.** amaten gives −85 of its 394 cost/min for every phrasing; the other songs move
  −19 … +4. Placement improves on every song (2–12% placed pasting in order, 38–78% with lines and spaces).

**Recommendation for 02:** take the phrases from the pasted lines and the spaces within them; place each line on
a run of sona's notes by a DP over the whole song (line mora counts and the fit's own cost; a break where notes
touch is allowed); fit each line with `fit_span_moras`. 01's ceiling for that (lines placed by the UST) is
−13.5 cost/min and 83 wrong moras/min on GAME lead.

## Table (dev; Δ against the same output with no lyric; paired song bootstrap, 10 000 resamples)

"rests" = `fit_span_moras` (rests inside the phrase may hold moras, at no cost); otherwise `fit_phrase_moras`.

| landing | lead cost/min | Δ [95%] | wrong moras/min [95%] | placed | landed | stage cost/min | Δ [95%] | wrong/min | placed |
|---|---|---|---|---|---|---|---|---|---|
| no lyric | 188.7 | | 192.7 | 0% | 0% | 180.2 | | 192.7 | 0% |
| (d) pasted in order (today) | 188.7 | 0 | 183.2 [158.1, 213.4] | 4.9% | 3.3% | 180.2 | 0 | 184.4 | 4.3% |
| (a′) runs of sona's notes, moras inside (07 as ported) | 174.9 | −13.9 [−30.2, −3.8] | 71.6 [47.4, 97.2] | 62.9% | 37.5% | 162.4 | −17.8 [−38.0, −4.8] | 58.2 | 69.8% |
| (a) UST phrases | 205.2 | +16.5 [+3.4, +29.6] | 76.6 [51.2, 104.4] | 60.3% | 36.5% | 176.5 | −3.7 [−12.9, +4.4] | 61.4 | 68.2% |
| (a) UST phrases, rests | 174.7 | −14.1 [−31.1, −3.5] | 74.0 [48.9, 102.1] | 61.6% | 36.0% | 162.0 | −18.2 [−40.2, −4.4] | 63.2 | 67.2% |
| (b) lyric lines | 199.5 | +10.7 [+0.4, +20.7] | 82.9 [62.3, 106.1] | 57.0% | 31.7% | 180.8 | +0.6 [−6.3, +6.5] | 77.5 | 59.8% |
| (b) lyric lines, rests | 176.7 | −12.0 [−30.2, −1.1] | 93.1 [70.4, 118.3] | 51.7% | 27.7% | 167.4 | −12.8 [−31.8, −1.1] | 96.2 | 50.1% |
| (b+) lines and spaces | 197.1 | +8.4 [−0.8, +16.6] | 74.7 [53.0, 98.2] | 61.3% | 34.8% | 178.5 | −1.7 [−7.7, +2.9] | 68.5 | 64.5% |
| (b+) lines and spaces, rests | 175.2 | −13.5 [−31.1, −2.7] | 83.4 [58.7, 110.7] | 56.7% | 32.2% | 167.4 | −12.8 [−31.4, −1.7] | 86.2 | 55.3% |
| lines cut at UST phrases, rests | 168.9 | −19.8 [−38.7, −7.9] | 66.1 [44.8, 89.9] | 65.7% | 36.1% | 155.8 | −24.4 [−47.2, −9.5] | 55.8 | 71.0% |
| (c) whole song | 226.6 | +37.8 [+18.4, +59.3] | 169.7 [147.7, 196.9] | 12.0% | 4.5% | 214.9 | +34.7 [+15.8, +57.5] | 169.3 | 12.1% |
| (c) whole song, rests | 189.0 | +0.3 [+0.0, +0.9] | 179.4 [152.6, 209.9] | 6.9% | 3.2% | 180.7 | +0.5 [+0.0, +1.7] | 177.4 | 8.0% |

Wrong-moras intervals and pooled values are `sona-eval-cellstage`'s; the Δ intervals are the prototype's
paired bootstrap. Phrases landed are out of 817.

**The price of a mora in a rest** (prototype, lead Δ / wrong moras per minute): free −14.1 / 74.0 for UST
phrases and −12.0 / 93.1 for lines; 0.5 per mora −9.8 / 68.7 and −8.2 / 83.8; 1 per mora −4.5 / 69.5 and +0.8
/ 82.1; 2 per mora as without rests. A free rest saves the most edits and loses some placement (a mora put in
a rest is on no note). `MORA_IN_REST` is 0, 07's value.

**Line breaks against rests** (rests of at least one cell; within one cell of the break):

| | breaks on a UST rest | UST rests that are breaks | breaks on a lead rest | lead rests that are breaks | breaks on a stage rest | stage rests that are breaks |
|---|---|---|---|---|---|---|
| dev, 383 breaks | 58.7% | 27.8% of 808 | 49.3% | 35.7% of 535 | 60.6% | 28.0% of 836 |

By song, breaks on a UST rest range from 31% (my-axiom) to 100% (telescope, yamineko). With rests of at least
4 cells, 39% of breaks fall on a lead rest and 47% of lead rests are breaks.

## How the lines were placed

The UST's moras are the truth and every landing pastes them (perfect text), so only the phrasing differs. For
(b), each dev song's lyric text (`data/pifuyuu-lyrics`, the `.readings.txt` where it exists) is read into moras
(fugashi + unidic-lite for kanji, Latin words as wildcards) and aligned to the UST's moras by edit distance;
each line's first mora gives the UST note that starts the line. The text matches 91–100% of the UST's moras on
seven songs; amaten (67%; the text writes a chorus of about 85 moras once) and innocent-addict (52%; English lines) are
rough. A UST run of 8+ moras the text never reached is a repeat written once: it is aligned again alone and
takes that stretch's breaks (139 moras on amaten, 48 on innocent-addict, 15 on fukagyakuteki). A space breaks a
line only between Japanese words. The line files are lyric-derived and stay out of the repo.

## What else these numbers could be measuring

- **The lines' timing.** In (b) a line spans its UST notes' time: the lines are placed in time perfectly. That
  is the ceiling for 02, which must place them from sona's notes alone.
- **Perfect text.** Real pasted text differs from the UST (repeats written once, long vowels, English, the
  fallback's errors). Here only the line breaks come from the text.
- **Alignment errors** put a few line breaks in the wrong place; they count against (b), so (b) is if anything
  understated. Seven songs align at 91–100%.
- **Chance matches.** Pasting in order places 4.9% partly by coincidence (ん, い and て are 10–15% of moras).
  The fits place 51–70%, far above that.
- **A matcher artefact.** Placement needs a mutual best match; a joined note places one of its two moras even
  when both are the same (tested). The edit cost is unchanged by the metric; the fit's numbers reproduce 07's
  byte for byte.
- **One song.** amaten carries most of the edit-cost gain (above). The placement gain is on every song.
- **Selection on dev.** The rest price was swept on dev (four values) and kept at 07's 0; the oracles are not
  tuned. Confirmation needs holdout4, with the mechanism 02 builds.
- **The port.** The Rust landings reproduce the prototype on all 198 song × output × landing rows (edit-cost
  class counts and placement counts), and the default run and `--fit-phrase-lyrics` are byte-identical to 07's.

## Reproduce

```sh
# Rust (dev): edit cost and lyric placement for every landing; --lyric-lines adds (b), (b+) and the cut lines.
ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
    --model ~/.cache/sona/cellstage-D --edit-cost --lyric-placement --lyric-lines "$L01_WORK/lines"
# Prototype (needs a --dump of dev; L01_WORK holds the line files, lyric-derived: keep out of the repo)
export L01_DUMP=… L01_WORK=…
python3 -I crates/sona-eval/scripts/lyric-placement/align_lines.py <ids>   # a Python with fugashi + unidic-lite
python3 crates/sona-eval/scripts/lyric-placement/oracles.py --dump-rows rows.json [--rest-cost C --variants …]
python3 crates/sona-eval/scripts/lyric-placement/breaks.py
python3 crates/sona-eval/scripts/lyric-placement/compare.py rows.json lead "b/r" "a/r"
python3 crates/sona-eval/scripts/lyric-placement/check_port.py rust.out rows.json
```
