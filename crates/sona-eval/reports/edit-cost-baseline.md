# Edit-cost baseline: where the OpenUtau editing cost comes from (utau-edit-cost 03)

Date: 2026-11-24. Branch `utau-03-taxonomy`. Dev is the working split. The three spent splits (holdout, holdout2,
holdout3) were re-read once each **for diagnosis only**; they confirm nothing (ledger entries at 0217da0).

## Answer

**Which numbers are in-sample.** The learned cell stage (`cellstage-D`, what ships) was trained on all 28 songs
of dev, holdout and holdout2 (`.scratch/game-holdout3/spec.md`; the model file lists the 28), so **every
learned-stage number on those three splits is in-sample**. It never saw holdout3, which is its only
out-of-sample view. **GAME lead + octave** was not trained on the corpus (its presence threshold and octave rule
were tuned on dev), so it is the primary reference for the taxonomy, oracles and ranking below, with the stage
beside it.

- **Joined notes are the largest class on every split, and they are the UST's one-note-per-mora writing.**
  GAME lead: joined is **51.5%** of dev cost, 39.7% / 36.0% / 32.0% on holdout / holdout2 / holdout3. The
  learned stage is within 4 points on every split (52.8% dev, 34.6% holdout3 out of sample): the two stacks
  hide the same boundaries.
  - **62–78% of hidden UST notes have the same pitch as the note before them** (GAME lead; 74% on dev). Of
    those, 60–82% are moras with no consonant to hear: a bare vowel (さ>い) or ん (げ>ん).
  - The UST's same-pitch boundaries are hidden by mora type: **ん 53–79%, bare vowel 49–57%, consonant 7–18%**;
    different-pitch boundaries only 3–6%.
  - Hidden notes are short: median 92–125 ms by split (one 16th at 120–163 BPM).
  - **Mora rate explains the song-to-song spread.** Joined cost per minute follows notes per second with a
    Spearman of +0.87 on dev and +0.72 / +0.79 / +0.68 on the spent splits. amaten (4.7 notes/s, 69% of its
    notes one cell long) and atamaro (4.2 notes/s) top dev; noizugoto, cinema-mitaina, denpa and zero-zero
    (3.7–4.5 notes/s) top the spent splits. It is the same mechanism at lower rates elsewhere: without amaten
    and atamaro, joined is still 51% of GAME lead's dev cost.
  - **No source proposes most hidden boundaries.** GAME lead never starts a note exactly there; GAME on the
    vocals 8–15%, sona's pYIN grid 23–33%; any source 26–37%. So better selection among today's candidates
    (what the learned stage does) fixes at most **10–16%** of cost, against a full joined oracle of **32–53%**.
    New boundary evidence is needed.
- **Chopped (12–24% of GAME lead's cost) is GAME's segmentation of held notes.** Across splits: 29–41% of extra
  pieces are same-pitch restarts, 33–38% are 1–2 semitone steps (vibrato, glide or drift), 17–28% a one-cell
  head (scoop or consonant), 2–8% a larger step, octave flips 0–3%. The learned stage chops more than GAME lead
  on every split (dev 240 against 167, holdout3 175 against 133) and adds octave flips (2–16% of its pieces).
- **Oracle ranking** (GAME lead, share of cost removed if the class were fixed perfectly; dev · holdout ·
  holdout2 · holdout3):

  | fixed | dev | holdout | holdout2 | holdout3 |
  |---|---|---|---|---|
  | joined | **52.5%** | **40.2%** | **36.5%** | **32.4%** |
  | joined or chopped (07) | 64.3% | 63.2% | 49.7% | 44.3% |
  | every pair error (length, shift, pitch) | 21.7% | 19.1% | 21.0% | 25.7% |
  | chopped | 13.8% | 24.1% | 14.0% | 12.7% |
  | missing (06's ceiling) | 11.4% | 13.7% | 25.3% | 24.4% |
  | extra | 1.4% | 3.6% | 3.0% | 4.2% |

  On the shipped stage, missing is smaller (8.0% dev, 11.7% holdout3 out of sample), because the stage
  already fills many rests; pitch pairs reach 20.5% on holdout3, mostly two songs (anata, ironication).
- **Confidence: today's note confidence carries no information for the shipped stack.** 91–99% of the
  learned stage's notes score 0.60–0.65, because the cell stage hands its notes to the scorer with fixed
  onset, offset, pitch-stability and voicing inputs. Flagging the lowest 30% catches 30–32% of the cost on
  every split, as random flags would. The stage's own class probability does better (lowest 30% catch 44–48%
  of cost, 42–53% of chopped cost), and "longest notes first" catches the joined notes (60–78% of joined cost
  at 30% flagged), which are long and confident. Highlighting doubtful notes by today's score would not
  catch the expensive errors.
- **Recommendation, in order:**
  1. **07, re-scoped to "split notes at moras"** (joined oracle 32–53% by split, the largest everywhere). Two
     levers, both needing evidence no current source has: (a) the lyrics, when the user pastes them, give the
     mora count per phrase, so the number of notes is known and the splits go where the timbre changes; (b)
     without lyrics, a same-pitch boundary detector for vowel and ん changes (spectral change, not pitch).
     Measure (a)'s oracle first: notes per phrase equal to moras per phrase.
  2. **07's chopped half**: merge same-pitch restarts and 1–2 semitone wobbles inside a held note (13–24%).
  3. **New ticket: boundary placement and pitch of one-to-one pairs** (19–26%; length alone 16–18%). Cheap
     edits, but many, and they lift clean measures almost as much as joined does (dev 16% → 35%).
  4. **06 after those, not dropped**: its ceiling is 7–18% on the shipped stage (24–25% on GAME lead on
     holdout2/3; ironication alone is 720 of holdout3's 1 090 missing cost). The realistic gain is at most half
     of that (a missing note becomes an extra at best, 2 → 1), and filling a rest between two same-pitch notes
     joins them (4).
  5. **Replace the confidence score for cell-stage notes** before any "doubtful note" UI: stage probability and
     note length, both measured here, beat today's score.

## Setup

- `sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost [--dump DIR]`, built with `--features game`,
  GAME-1.0.3-medium. Outputs: pYIN grid default on the vocals (its own phase), **GAME lead + octave** on the
  onset phase, **learned cell stage** on the onset phase (what ships). Weights from `spec.md`.
- **GAME cache** (new): the two GAME runs and the pYIN grid notes are cached in `target/ust-dev-cache/`, keyed by
  song, a fingerprint of each stem, the GAME folder, the GAME settings (seed included) and the transcriber's
  settings, like `sona-eval-ust-dev`. On dev the first run took **2 h 57 min** (on a shared, loaded machine) and
  the second **68 s**; the two outputs are **byte-identical**, and both equal ticket 01's numbers (180.2 / 188.7 /
  338.4 cost/min; 92.69% / 86.76% cells). Two tests read a cached run back exactly and refuse a torn file; a
  mutant of each (voiced flag dropped, trailing bytes accepted) fails them.
- **`--dump DIR`** (new) writes each song's UST notes with lyrics, the three outputs with their Rust edit
  costs, the stage's cells (candidates, sources, class probabilities), each shipped note's confidence (scored as
  `sona transcribe` scores it) and GAME's raw notes. It holds UST content and stays outside the repo.
- The taxonomy, oracles and calibration are Python on the dumps: `scripts/edit-cost-taxonomy/taxo.py DUMP lead|stage`
  and `extra.py` (per-song drivers). Its port of the edit cost (`ec.py`) **reproduces the Rust class counts
  exactly** on every song and output of every split.
- Intervals are song-bootstrap 95% (10 000 resamples).

## Dev baseline (9 songs, 27.5 min)

| output | cost/min [95%] | clean measures [95%] | cell accuracy |
|---|---|---|---|
| pYIN grid default | 338.4 [257.3, 447.5] | 3.1% [1.1, 5.4] | — |
| **GAME lead + octave** | **188.7 [127.3, 262.1]** | **16.2% [9.4, 23.2]** | 86.76% |
| learned stage (ships; in-sample) | 180.2 [115.9, 256.5] | 23.8% [13.8, 33.9] | 92.69% |

Frame pass rate (ticket 02/10, dev only, `sona-eval-frame` has no spent-split mode): 1/9 with sona's own BPM
(drift), 5/9 against the music with a typed BPM.

Cost share by class:

| output | joined | chopped | length | missing | shift | pitch | extra | moved |
|---|---|---|---|---|---|---|---|---|
| GAME lead + octave | 51.5% | 12.9% | 13.8% | 12.0% | 4.2% | 3.9% | 1.4% | 0.3% |
| learned stage (in-sample) | 52.8% | 19.4% | 10.5% | 8.6% | 4.2% | 3.3% | 0.7% | 0.4% |

So the joined share does not depend on the stack (52–53%); the learned stage trades missing, length and pitch
cost for more chopping.

Per song, GAME lead (cost per minute by class, weighted):

| song | notes/s | same-pitch legato boundaries/min | joined | chopped | total cost/min | clean |
|---|---|---|---|---|---|---|
| amaten | 4.66 | 151.7 | 220.7 | 4.9 | 394.1 | 0.0% |
| atamaro-deluxe | 4.19 | 81.7 | 169.8 | 33.2 | 347.0 | 4.4% |
| fukagyakuteki-henyou | 2.92 | 47.3 | 154.3 | 15.4 | 256.3 | 8.9% |
| my-axiom | 3.45 | 77.7 | 124.0 | 26.5 | 190.1 | 15.4% |
| innocent-addict | 2.90 | 40.7 | 97.3 | 8.5 | 171.9 | 11.3% |
| yamineko | 3.72 | 59.7 | 52.3 | 34.4 | 128.1 | 26.2% |
| telescope-nevermore | 2.51 | 23.5 | 32.5 | 39.5 | 93.8 | 28.3% |
| kamaitachi-ni-yoru | 2.64 | 32.9 | 30.4 | 41.9 | 100.5 | 20.8% |
| azemichi-ni-ochita-ginga | 2.45 | 49.7 | 27.4 | 16.0 | 75.1 | 31.1% |

Spearman over songs: joined/min vs notes/s **+0.87**; vs same-pitch legato boundaries/min +0.68.

## Why notes are joined

Each joined count is one hidden UST note: the boundary between two consecutive UST notes under one output
note. 667 on GAME lead, 654 on the learned stage.

| | GAME lead | learned stage (in-sample) |
|---|---|---|
| hidden note has the previous note's pitch | **74.1%** | **79.7%** |
| … and is a bare vowel / ん / consonant mora | 36.0% / 24.7% / 39.3% of those | 35.7% / 24.2% / 40.1% |
| different pitch: 1–2 semitones / 3+ | 19.9% / 6.0% | 16.1% / 4.3% |
| hidden note one cell long / median duration | 41.4% / 92 ms | 42.2% / 92 ms |
| a rest before the hidden note | 1.6% | 0.2% |
| a source starts a note exactly there: GAME lead / GAME vocals / sona pYIN / any | 0% / 7.8% / 30.9% / 33.7% | 4.9% / 8.4% / 30.4% / 34.1% |

Recall of the UST's legato boundaries (the next note starts where the last ends), share hidden in a joined
note, GAME lead (stage within 2 points everywhere):

| boundary | count | hidden |
|---|---|---|
| different pitch | 2832 | 5.8% |
| same pitch, next mora starts with a consonant | 1098 | 17.6% |
| same pitch, next mora is ん | 231 | 52.8% |
| same pitch, next mora is a bare vowel | 316 | 56.0% |
| same pitch, next note one cell long / two or more | 544 / 1101 | 43.9% / 23.0% |

Per song (GAME lead): same-pitch share of hidden notes runs from 37% (yamineko) to 85–88% (atamaro,
kamaitachi); the hidden notes are one cell long on amaten (97%) and telescope (79%), two cells (a triplet
8th) on atamaro.

**Reading.** In UTAU every mora is its own note, so Pifuyuu writes げ+ん, さ+い, and runs of short moras on one
pitch, as separate notes. A singer runs them together: no pitch change, and for ん or a bare vowel no consonant
either; only the timbre moves. Pitch-and-onset transcribers (GAME, sona's pYIN grid) cannot see such a
boundary, and the learned stage cannot draw a boundary no source proposes. This is the cost a user pays in
OpenUtau: to place a lyric per mora they must split the note.

Does Pifuyuu's fast one-note-per-mora writing explain amaten and atamaro? Yes for those two (4.7 and 4.2
notes/s; hidden notes of 92 ms and a triplet 8th; same-pitch 79–85%), and it is the same mechanism elsewhere at
lower rates: the correlation with mora rate holds across all 9 songs.

## Why notes are chopped

Each chopped count is one extra piece: a cut between two output notes under one UST note.

| | GAME lead (167) | learned stage (240, in-sample) |
|---|---|---|
| same pitch on both sides (a source restarts the note) | 41.3% | 32.1% |
| 1–2 semitone step (vibrato, glide, drift) | 32.9% | 30.4% |
| one-cell head at the note's start (scoop, consonant) | 16.8% | 25.8% |
| larger step / octave flip / rest inside | 8.4% / 0% / 0.6% | 7.1% / 2.1% / 2.5% |
| a piece off the UST's pitch | 58.7% | 67.5% |

On the learned stage the cut falls where GAME lead starts a note in 69% of cases, and 64% of cuts are
lead → lead: the stage inherits GAME's segmentation and adds pitch steps from sona and the vocals run
(lead→sona 9%, vocals→lead 6%). Chopped cost is spread evenly over songs (5–42 per minute), unlike joined.

## Oracles

Components replaced by their UST notes, then the real metric re-run (dev). Intervals here come from the
Python bootstrap: same method as the tool's, different random draws, so the baseline row differs from the table
above by a few tenths.

| fixed perfectly | GAME lead cost/min [95%] | clean | cut | learned stage (in-sample) cost/min | clean | cut |
|---|---|---|---|---|---|---|
| nothing | 188.7 [126.8, 261.5] | 16.2% | — | 180.2 [115.5, 255.4] | 23.8% | — |
| **joined** | **89.7 [65.7, 120.2]** | 31.7% | **52.5%** | 82.2 [55.0, 115.5] | 43.4% | 54.4% |
| … only all-same-pitch joins | 118.4 | 23.8% | 37.3% | 105.2 | 36.4% | 41.6% |
| … only where a source proposed every hidden start | 159.5 | 20.5% | 15.5% | 150.6 | 28.7% | 16.4% |
| … within one cell | 121.7 | 25.1% | 35.5% | 113.3 | 34.4% | 37.1% |
| chopped | 162.8 [98.6, 237.4] | 21.7% | 13.8% | 142.5 | 31.9% | 20.9% |
| joined or chopped (07) | 67.3 [39.4, 101.2] | 38.9% | 64.3% | 50.0 | 54.0% | 72.3% |
| every pair with an error | 147.8 | 35.4% | 21.7% | 147.9 | 37.0% | 17.9% |
| … length | 154.5 | 32.2% | 18.1% | 153.1 | 34.1% | 15.0% |
| … shift / moved | 170.0 | 18.4% | 9.9% | 161.6 | 26.5% | 10.3% |
| … pitch | 179.0 | 18.4% | 5.2% | 171.8 | 25.8% | 4.6% |
| missing (06's ceiling) | 167.2 [116.5, 225.7] | 18.9% | 11.4% | 165.8 | 25.7% | 8.0% |
| extra | 186.2 | 16.8% | 1.4% | 179.0 | 24.1% | 0.7% |

Clean measures move most with joined (16% → 32%) and with pair errors (→ 35%): pair errors are cheap but
touch many measures.

## Confidence calibration (shipped stack, dev, in-sample)

Each output note of the learned stage gets the error class of its component and an equal share of its cost
(91.4% of cost is attributable to an output note; the rest is missing notes).

| shipped confidence | notes | with an error | joined | chopped | share of cost |
|---|---|---|---|---|---|
| < 0.55 | 34 | 94% | 9% | 26% | 1.4% |
| 0.55–0.60 | 17 | 76% | 6% | 47% | 0.6% |
| 0.60–0.65 | 4649 | 36% | 13% | 8% | 97.9% |

| stage mean class probability | notes | with an error | joined | chopped | share of cost |
|---|---|---|---|---|---|
| < 0.70 | 227 | 80% | 6% | 34% | 8.8% |
| 0.70–0.90 | 483 | 70% | 19% | 15% | 18.6% |
| 0.90–0.95 | 385 | 56% | 22% | 9% | 13.1% |
| ≥ 0.95 | 3605 | 27% | 11% | 6% | 59.4% |

Flag the lowest x% of notes by a signal; share of joined / chopped / all attributable cost caught:

| signal | 10% | 20% | 30% |
|---|---|---|---|
| shipped confidence | 10 / 19 / 13 | 21 / 28 / 23 | 30 / 39 / 32 |
| stage mean class probability | 10 / 32 / 17 | 29 / 44 / 35 | 42 / 53 / 48 |
| stage min class probability | 18 / 24 / 20 | 34 / 39 / 37 | 47 / 51 / 49 |
| longest notes first | 25 / 10 / 18 | 45 / 18 / 34 | 60 / 19 / 44 |

Why the shipped confidence is flat: `sona transcribe`'s cell stage passes its notes through `grid_segments`,
which sets onset strength 1, drop steepness 0.5, dispersion 0 and voicing 1, so only the bleed factor
(note peak against stem peak) varies. The scalar is the soft-min of {1, bleed, 0.67, 0.5}.

## Spent splits: diagnostic re-reads (confirm nothing)

One run per split at 0217da0, `--split NAME --reread --reason "edit-cost baseline, diagnosis only"`, logged in
`corpus/holdout-reads.jsonl`. Songs with stems: holdout 9, holdout2 10, holdout3 10. The learned stage is
**in-sample on holdout and holdout2** (trained on them) and **out of sample on holdout3**. Nothing here was
tuned on these songs, and nothing is confirmed by them.

| split | output | cells | cost/min [95%] | clean measures [95%] |
|---|---|---|---|---|
| holdout | pYIN grid default | — | 262.6 [205.0, 334.4] | 3.6% [1.3, 5.8] |
| | GAME lead + octave | 84.90% | 139.8 [94.9, 197.3] | 15.0% [5.9, 25.0] |
| | learned stage (in-sample) | 90.95% | 132.9 [91.3, 184.2] | 19.8% [8.8, 32.3] |
| holdout2 | pYIN grid default | — | 296.5 [254.0, 348.6] | 0.8% [0.0, 1.7] |
| | GAME lead + octave | 78.88% | 182.3 [142.8, 229.5] | 8.2% [4.1, 13.4] |
| | learned stage (in-sample) | 86.24% | 178.9 [138.9, 226.7] | 12.3% [6.8, 18.9] |
| holdout3 | pYIN grid default | — | 272.2 [223.5, 320.0] | 1.9% [0.8, 3.6] |
| | GAME lead + octave | 76.64% | 161.1 [110.4, 227.9] | 14.8% [8.8, 21.4] |
| | learned stage (**out of sample**) | 78.84% | 156.3 [111.5, 209.7] | 17.1% [9.9, 24.7] |

holdout3's stage cell accuracy reproduces the published 78.8% (ADR-0026). Out of sample, the learned stage
buys 5.9 points of cells over GAME lead in-sample (dev) but 2.2 points on holdout3, and its edit-cost gain
shrinks to 4.8 cost/min, well inside the intervals.

Cost share by class:

| split | output | joined | chopped | length | missing | pitch | shift | extra |
|---|---|---|---|---|---|---|---|---|
| holdout | GAME lead | 39.7% | 23.6% | 11.1% | 13.8% | 4.4% | 3.3% | 3.6% |
| | stage (in-sample) | 43.2% | 30.6% | 10.4% | 6.5% | 4.0% | 3.8% | 0.6% |
| holdout2 | GAME lead | 36.0% | 13.7% | 12.6% | 25.7% | 5.8% | 2.8% | 3.0% |
| | stage (in-sample) | 38.9% | 22.4% | 10.8% | 18.2% | 5.0% | 3.2% | 1.2% |
| holdout3 | GAME lead | 32.0% | 12.4% | 11.4% | 25.3% | 11.7% | 2.7% | 4.2% |
| | stage (out of sample) | 34.6% | 16.8% | 11.7% | 12.5% | 15.7% | 3.2% | 5.3% |

The joined and chopped shares do differ between the stacks, in one direction: the stage's joined share is
1.3 points higher on dev and 2.6–3.5 on the spent splits, and its chopped share 4–9 points higher on every
split, because it removes missing cost (in-sample also extra and pitch) while inheriting GAME's boundaries and
adding cuts of its own. Joined is the largest class for both on every split.

Joined taxonomy (GAME lead; the stage within a few points):

| | holdout | holdout2 | holdout3 |
|---|---|---|---|
| hidden notes | 349 | 414 | 345 |
| same pitch as the previous note | 78.2% | 62.1% | 73.0% |
| … of which bare vowel / ん / consonant | 44.7 / 37.7 / 17.6% | 48.2 / 24.5 / 27.2% | 48.0 / 25.4 / 26.6% |
| same-pitch boundaries hidden: ん / vowel / consonant | 78.6 / 56.7 / 8.2% | 61.2 / 48.6 / 9.8% | 66.0 / 50.8 / 7.4% |
| different-pitch boundaries hidden | 3.3% | 5.7% | 3.9% |
| hidden-note duration, median | 109 ms | 108 ms | 125 ms |
| any source starts a note exactly there | 26.4% | 33.3% | 37.1% |
| Spearman, joined/min vs notes/s | +0.72 | +0.79 | +0.68 |

Chopped kinds (GAME lead): 1–2 semitone step 35.3 / 32.5 / 37.6%; same-pitch restart 29.0 / 31.8 / 33.1%;
one-cell head 28.0 / 27.4 / 25.6%; other step, octave or rest 7.7 / 8.3 / 3.8%. The stage adds octave flips on
holdout2 (15.5% of its pieces, in-sample).

Oracles (share of cost removed):

| fixed perfectly | holdout lead | holdout stage | holdout2 lead | holdout2 stage | holdout3 lead | holdout3 stage (OOS) |
|---|---|---|---|---|---|---|
| joined | 40.2% | 43.8% | 36.5% | 39.8% | 32.4% | 35.0% |
| … all-same-pitch joins only | 31.1% | 34.8% | 21.9% | 25.6% | 22.3% | 26.3% |
| … where a source proposed every hidden start | 10.2% | 11.7% | 11.1% | 13.3% | 10.0% | 13.1% |
| chopped | 24.1% | 31.5% | 14.0% | 23.1% | 12.7% | 17.1% |
| joined or chopped | 63.2% | 73.8% | 49.7% | 61.2% | 44.3% | 51.2% |
| every pair with an error | 19.1% | 18.6% | 21.0% | 18.7% | 25.7% | 30.4% |
| … length | 15.7% | 15.7% | 16.8% | 15.0% | 17.0% | 18.3% |
| … pitch | 6.2% | 5.8% | 7.7% | 6.8% | 15.7% | 20.5% |
| missing | 13.7% | 6.6% | 25.3% | 17.7% | 24.4% | 11.7% |
| extra | 3.6% | 0.6% | 3.0% | 1.2% | 4.2% | 5.3% |

Confidence calibration on holdout3 (the stage out of sample): 91% of notes score 0.60–0.65; flagging the lowest
30% by shipped confidence catches 32% of cost, by stage mean probability 45% (chopped 43%), longest notes first
49% (joined 72%). The same picture as dev, so the dev reading is not an in-sample artefact.

## What else these numbers could be measuring

- **The metric's own rounding.** UST notes lost or merged when rounded to cells and made monophonic: 0–22 per
  song (innocent-addict 22 of 679), 42 of 5 336 on dev; 12, 76 and 10 on the spent splits. Too few to make
  the joins (667 on dev).
- **Split-lead overlaps.** A hidden note from the other lead part: 1.6% on dev, 0.3–1.9% on the spent splits.
- **Phase.** The onset phase is within 0.06 cell of the UST's on 37 of 38 songs (run output), so rounding to
  the UST's cells cannot move a boundary by a cell. The exception is cinema-mitaina (holdout2, 0.41 cell),
  holdout2's top joined song; its cells score no better on the UST's own phase (79.6% against 81.8%), so the
  phase does not make its joins.
- **Two outlier songs.** amaten and atamaro are 40% of GAME lead's dev cost. Without them joined is still
  51% of the rest, and the mora-rate correlation holds on every split (+0.68 to +0.87). On holdout3,
  ironication (41% of cells right) carries two thirds of the missing cost; missing's rank there is one song.
- **A grazed note counted as joined.** A one-cell UST note covered by a neighbour whose boundary is one cell
  late counts as a join (4) where "shorten and draw" would cost 3. It is the same edit in spirit (the user has
  to make the note), so the ranking does not change.
- **The learned stage fitting its training songs.** Every stage number on dev, holdout and holdout2 is
  in-sample; GAME lead is the reference, and holdout3 shows the stage out of sample.
- **Candidate-boundary shares depend on rounding sources to the UST's cells.** The ±1-cell variant is reported
  beside the exact one; with one-cell notes ±1 is too lax (it reaches the neighbour's boundary).
- **The Python port.** It reproduces the Rust class counts on every song and output, so the taxonomy and
  oracles share the metric's definitions.
