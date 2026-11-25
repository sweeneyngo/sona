# Splitting and timing notes at the aligner's mora onsets (lyrics-first 16)

Date: 2026-11-24. **Dev only** (the 9 dev songs with a lyric text; 27.5 min). No holdout split was read.
New weights throughout (off-time start or end 4, near-miss pitch 3, octave 1, extra 1, missing 2, chopped and
joined 4; `.scratch/utau-edit-cost/spec.md`). GAME lead + octave is out of sample; the learned stage is in
sample on dev. The oracles were read first. The selection rule and the declared candidate grid were committed
before any aligned candidate was scored (4b95864).

Pifuyuu's lyrics, USTs and stems stayed on this machine. The dump, the alignments and the line times are in the
session scratchpad. This report carries pooled numbers and per-song rates only.

## Answer

- **Negative. No candidate passes the rule, and none beats today's aligned fit.** The aligner's per-mora onsets
  aren't precise enough at the grid's resolution. 57.6% of moras land on their UST note's start cell, 20.8% one
  cell early, and 21.6% further off. That is not good enough to split notes or move starts.
  - Splitting joined notes at the aligned onsets instead of the tail rule costs **+9.0 [+1.6, +21.3]** cost/min
    against today's aligned fit on GAME lead.
  - Moving starts to the aligned onsets costs +38 to +184, depending on tolerance, onset shift and score gate.
  - Nothing ships. No Rust was written, so the default and the opt-in paths are unchanged.
- **The oracle says the lever is real, and it is the starts, not the splits.** These use the UST's own mora
  onsets, with the same aligned line spans. GAME lead, against today's aligned fit (304.2):
  - splitting at the UST onsets: 290.6 (−13.7 [−24.6, −5.2]);
  - moving each note's start to its mora's onset when it is within one cell: 247.7 (−56.5);
  - both: **244.4 (−59.8 [−87.4, −34.9])**, with 19.6% clean measures and 294 missing notes (no lyric: 312);
  - with the UST's own line spans too: 241.9.
  - The stage gets the same kind of gain: 219.4 against 271.5.
- **How precise the onsets would have to be.** This is the oracle with each UST onset kept exact with
  probability *p*, and otherwise moved by one of the aligner's own wrong-cell errors. Split + start ±1 on GAME
  lead, against today's 304.2:

  | p (onsets on the right cell) | cost/min |
  |---|---|
  | 57.6% (the aligner's rate) | 448.7 |
  | 80% | 364.1 |
  | 90% | 305.4 (break-even) |
  | 95% | 265.7 |

  Splits alone break even sooner (80% gives −9.4), but they are worth at most −13.7. **Moving starts needs a
  vowel-onset source that is right on about 9 of 10 moras.** The CTC aligner is right on about 6 of 10.
- **Why ±1-cell moves lose.** Where the aligner and the fitted note disagree by one cell, the note is usually
  the one that's right. With no onset shift, 600 such moves would take a correct start off its cell, against
  174 that would fix one. With +20 ms the counts are 277 against 200. The aligner fires early (median −0.36
  cell, plausibly on the consonant), and a UST note starts on the vowel. A constant shift, gating by the
  aligner's score, or moving only 2-cell disagreements all reduce the damage, but none of them makes it a gain.

## Selection rule (declared before scoring; ticket 16's comments)

A candidate ships opt-in when, on dev:
1. GAME lead: the paired song-bootstrap 95% upper bound of Δ cost/min against no lyric (303.0) is < 0;
2. the stage is no worse: Δ against no lyric (272.2) ≤ 0;
3. missing notes are not up: ≤ 1.02 × the no-lyric count (lead ≤ 318, stage ≤ 217);
4. placement is kept: wrong moras/min ≤ today's aligned fit + 5 (≤ 98.1).

**Verdict: no candidate passes.** Every candidate fails 1 and 2. All 39 declared candidates and the 8 added later
fail the GAME lead point estimate against today's aligned fit as well.

## Table (dev, GAME lead unless noted; lines and spaces aligned by the lyrics CTC with energy ends; paired song bootstrap, 10 000 resamples)

"None": GAME lead with no lyric. "Today": 14's aligned fit, splits by the tail rule (`TAIL_CELLS`). Wrong/min
and placed are 01's lyric placement metric. Missing is the count of UST notes with no output note.

| fit | cost/min [95%] | Δ vs none [95%] | Δ vs today [95%] | clean | wrong/min | placed | missing | stage cost/min | stage Δ vs none [95%] | stage missing |
|---|---|---|---|---|---|---|---|---|---|---|
| none (no lyric) | 303.0 [206.6, 416.1] | 0 | −1.2 [−19.0, +22.0] | 16.2% | 192.7 | 0.0% | 312 | 272.2 | 0 | 213 |
| today: aligned fit, tail rule | 304.2 [211.3, 417.4] | +1.2 [−22.0, +19.0] | 0 | 16.3% | 93.1 | 51.7% | 318 | 271.5 | −0.7 [−19.3, +12.0] | 216 |
| *oracle:* split at UST onsets | 290.6 [201.1, 399.3] | −12.5 [−32.0, −0.5] | −13.7 [−24.6, −5.2] | 16.9% | 90.7 | 52.9% | 307 | 263.7 | −8.6 [−27.0, +2.7] | 213 |
| *oracle:* starts to UST onsets (±1 cell) | 247.7 [170.8, 338.6] | −55.4 [−80.1, −33.6] | −56.5 [−81.4, −34.0] | 19.4% | 89.7 | 53.4% | 293 | 221.1 | −51.1 [−77.0, −28.4] | 195 |
| *oracle:* split + starts (±1) | **244.4 [169.2, 333.4]** | −58.6 [−85.5, −34.9] | −59.8 [−87.4, −34.9] | 19.6% | 88.7 | 54.0% | 294 | 219.4 | −52.8 [−80.5, −28.8] | 197 |
| *oracle:* split + starts (±2) | 238.3 [168.3, 320.7] | −64.8 [−97.4, −36.8] | −66.0 [−100.7, −35.8] | 20.8% | 82.3 | 57.3% | 304 | 212.9 | −59.3 [−92.9, −31.6] | 207 |
| *oracle:* split + starts (any distance) | 246.1 [181.1, 322.0] | −57.0 [−97.7, −24.6] | −58.1 [−101.0, −22.8] | 20.2% | 62.5 | 67.6% | 307 | 219.1 | −53.1 [−94.3, −20.5] | 217 |
| *oracle:* split + starts (±1), no legato | 249.8 [173.9, 339.4] | −53.3 [−79.2, −30.6] | −54.5 [−79.8, −31.7] | 18.5% | 88.7 | 54.0% | 313 | 225.3 | −47.0 [−73.3, −24.5] | 221 |
| *oracle:* UST spans, tail rule (01's b+/r) | 306.6 [213.8, 419.5] | +3.6 [−17.5, +20.2] | +2.4 [−2.1, +7.4] | 16.4% | 83.4 | 56.7% | 318 | 271.8 | −0.4 [−20.0, +14.1] | 216 |
| *oracle:* UST spans, split + starts (±1) | 241.9 [167.1, 330.1] | −61.2 [−88.2, −37.3] | −62.3 [−91.4, −35.6] | 20.4% | 78.0 | 59.5% | 293 | 214.2 | −58.0 [−83.5, −34.5] | 196 |
| *oracle:* UST onsets + aligner's error (iid), split | 305.6 [210.3, 425.0] | +2.6 [−21.1, +19.9] | +1.4 [−9.4, +13.2] | 16.6% | 91.8 | 52.4% | 309 | 272.4 | +0.2 [−20.4, +14.7] | 213 |
| *oracle:* same, split + starts (±1) | 468.9 [364.0, 599.9] | +165.8 [+95.1, +240.5] | +164.7 [+107.7, +232.8] | 7.0% | 93.3 | 51.6% | 408 | 442.8 | +170.5 [+99.8, +242.6] | 309 |
| aligned: split | 313.2 [213.6, 438.5] | +10.1 [−16.4, +32.4] | +9.0 [+1.6, +21.3] | 16.3% | 94.1 | 51.2% | 314 | 279.2 | +7.0 [−15.3, +24.9] | 215 |
| aligned: split, +20 ms | 313.3 [214.2, 437.3] | +10.3 [−15.4, +30.1] | +9.1 [−0.3, +21.4] | 16.2% | 94.4 | 51.0% | 314 | 279.6 | +7.4 [−15.3, +25.2] | 217 |
| aligned: starts ±1 | 476.0 [369.9, 602.9] | +172.9 [+87.7, +256.7] | +171.7 [+97.8, +248.7] | 6.7% | 94.6 | 50.9% | 420 | 454.3 | +182.1 [+94.8, +265.7] | 300 |
| aligned: starts ±1, +20 ms, score ≥ median | 343.1 [248.1, 463.1] | +40.0 [+3.5, +73.0] | +38.8 [+17.4, +65.9] | 13.5% | 92.7 | 51.9% | 323 | 307.9 | +35.7 [−0.5, +67.5] | 219 |
| aligned: split + starts ±1, +20 ms, score ≥ median | 348.5 [249.1, 476.9] | +45.4 [+7.2, +81.6] | +44.3 [+18.4, +75.9] | 13.5% | 94.2 | 51.1% | 320 | 313.3 | +41.1 [+3.0, +77.3] | 221 |
| aligned: split + starts ±2, +20 ms, score ≥ median | 351.7 [253.4, 480.4] | +48.7 [+8.9, +85.9] | +47.5 [+20.5, +80.6] | 13.8% | 93.0 | 51.7% | 322 | 316.6 | +44.4 [+4.6, +81.0] | 225 |
| *added later:* aligned: starts only on 2-cell disagreements, +20 ms | 313.0 [219.2, 429.1] | +10.0 [−14.7, +27.7] | +8.8 [+3.5, +14.4] | 16.4% | 90.8 | 52.9% | 327 | 281.0 | +8.7 [−12.3, +22.0] | 225 |
| *added later:* aligned: split + starts on 2-cell disagreements, +20 ms, score ≥ 25th pct | 319.0 [220.6, 444.1] | +15.9 [−12.0, +38.0] | +14.7 [+3.4, +29.5] | 16.4% | 92.9 | 51.8% | 317 | 283.1 | +10.8 [−13.2, +29.9] | 221 |

The full declared grid has 39 aligned rows: split; starts ±1/±2; split + starts ±1/±2; onset shifts of 0, +20
and +40 ms; and gates at the song's 25th and 50th score percentile. The best start rule is the one in the table
(+38.8 against today); +40 ms is worse than +20 ms everywhere. The other added-later rows are 2-cell-only moves
with tolerance 2 or 3, shifts of 0 or +20 ms, and a 25th-percentile gate. They cost +8.8 to +38.9 against today,
and the UST oracle for the same rule is only −6.2 (split too: −17.4). A second jitter seed reproduces the iid
oracle (split + starts 468.1, split 306.5).

**Per song** (GAME lead cost/min; none → today → oracle split + starts ±1 → aligned split):

| song | none | today | oracle | aligned split |
|---|---|---|---|---|
| amaten | 495.4 | 403.5 | 387.1 | 402.2 |
| atamaro-deluxe | 626.7 | 633.0 | 493.9 | 687.3 |
| azemichi-ni-ochita-ginga | 136.9 | 141.1 | 117.4 | 144.6 |
| fukagyakuteki-henyou | 437.6 | 457.6 | 373.8 | 464.6 |
| innocent-addict | 285.2 | 283.0 | 204.7 | 287.8 |
| kamaitachi-ni-yoru | 177.1 | 183.9 | 149.2 | 182.5 |
| my-axiom | 265.3 | 316.9 | 227.0 | 322.4 |
| telescope-nevermore | 141.7 | 140.3 | 128.1 | 140.3 |
| yamineko | 225.9 | 230.0 | 173.3 | 236.4 |

The oracle gains on every song. The aligned split loses on six songs and is level on the other three.

## What the fit does

The prototype is `scripts/mora-split/split.py`. It follows 14's landing: an output note belongs to the aligned
piece its midpoint lies in, and `fit_span_moras` (the DP with rests) gives each note its block of moras. Then:
- **split:** a note holding k > 1 moras is cut at the onset cells of its 2nd … k-th moras, when every cut falls
  strictly inside the note and the cuts increase. Otherwise the tail rule cuts it, as today.
- **starts:** a note carrying a mora whose onset cell is within the tolerance of its start (and leaves it at
  least a cell long) starts there. If it touched the note before, that note's end follows (legato). A start
  that moves earlier into the previous note cuts that note, unless the cut would empty it.
- Notes with no mora merge as today. Onset cells are rounded on sona's grid phase.

The harness reproduces the Rust reference exactly. No lyric gives 303.0 and 272.2. Today's aligned fit gives
304.2 and 271.5, with every class count equal to `sona-eval-cellstage --lyric-line-times`'s.

## What else these numbers could be measuring

- **The oracle leaks the answer by construction.** UST onsets are the UST's note starts. "Start at the UST
  onset" is a reference-informed timing fix, so it is only a ceiling. The precision sweep turns it into the
  requirement above (about 90% of onsets on the right cell to break even).
- **The jitter is iid, but the aligner's errors are not.** amaten has drifting runs, and errors are correlated
  within a line. The real aligned rows are measured directly, though, and they agree with the jittered ones
  (start moves: +172 against +165; split: +9.0 against +1.4).
- **The grid phase.** Onsets are rounded on sona's onset phase, not the UST's anchor. Sona's phase is off by
  0.004–0.046 cell (dev run log), far below the 0.36-cell bias. The oracle uses the same rounding and still gains.
- **One song.** amaten carries most of the tail rule's gain, as in 14. The aligned rows lose on the other songs
  too (per-song table).
- **In-sample knobs.** The onset shift, the gate and the tolerance were all chosen on dev. They can only flatter
  a candidate, and none passes anyway.

## Next

Off-time starts are worth ~56 cost/min at the UST's precision, the largest lyric-side lever measured. That
needs a vowel-onset source at about 90% cell precision. One candidate is a singing phoneme aligner run inside
each placed line: SOFA is listed in 14, and it gives the vowel's own boundary rather than the CTC peak. Another
is the aligner's onset used as a prior on sona's own onset candidates, rather than as the onset itself. Either
needs the same precision sweep before any port. Ticket 18 (non-lyric timing) owns the non-lyric part of the
start and end errors.

## Reproduce

```sh
SP=<session scratchpad>   # w2/dump: `sona-eval-cellstage --edit-cost --dump`; l01/lines; l14/{text,align,times}
export L01_DUMP=$SP/w2/dump L01_WORK=$SP/l01 L14_WORK=$SP/l14
S=crates/sona-eval/scripts/mora-split/split.py
# oracles
python3 $S --variants none,tail,ust:split,ust:start1,ust:split+start1,ust:split+start2,ust:split+start9,ust:split+start1+nolegato,ustspan:tail,ustspan:split+start1,jit:split,jit:split+start1
# precision sweep
python3 $S --variants none,tail,ust:split+start1+exact57.6,ust:split+start1+exact80,ust:split+start1+exact90,ust:split+start1+exact95,ust:split+exact80,ust:split+exact90
# declared aligned grid (39 candidates)
v=none,tail,al:split+bias0,al:split+bias20,al:split+bias40
for sp in "" "split+"; do for st in 1 2; do for b in 0 20 40; do v="$v,al:${sp}start$st+bias$b"; for g in 25 50; do v="$v,al:${sp}start$st+bias$b+gate$g"; done; done; done; done
python3 $S --variants "$v"
# added later
python3 $S --variants none,tail,ust:start2+min2,ust:split+start2+min2,al:start2+min2+bias0,al:start2+min2+bias20,al:split+start2+min2+bias0,al:split+start2+min2+bias20,al:split+start3+min2+bias0,al:split+start3+min2+bias20,al:split+start2+min2+bias20+gate25,al:split+start2+min2+bias0+gate25
# per song, second jitter seed
python3 $S --variants none,tail,ust:split+start1,jit:split,jit:split+start1,al:split+bias0,al:split+start1+bias20+gate50 --jitter-seed 8 --per-song
```
