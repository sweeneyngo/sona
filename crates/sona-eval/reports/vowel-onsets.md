# Vowel onsets precise enough to time the notes (lyrics-first 17)

Date: 2026-11-24. **Dev only** (the 9 dev songs with a lyric text; 27.5 min, 5,336 moras). No holdout split was
read. New weights throughout (off-time start or end 4, near-miss pitch 3, octave 1, extra 1, missing 2, chopped
and joined 4; `.scratch/utau-edit-cost/spec.md`). GAME lead + octave is out of sample; the learned stage is in
sample on dev. Precision was measured first. The gate, the declared grid and the selection rule were committed
before any onset source was scored by `split.py` (16b6f54).

Pifuyuu's lyrics, USTs and stems stayed on this machine. Onsets, alignments and logs are in the session
scratchpad. This report carries pooled numbers and per-song rates only. The only downloads were public code and
models.

## Answer

- **No single source reaches 85% of moras on the right cell. A combination of three CTC aligners does, on the
  moras where they agree.** Pooled exact-cell precision against the UST note starts:
  - 14's lyrics CTC 57.6%; the hiragana speech CTC 68.8%; SOFA's Japanese singing model, inside 14's lines,
    64.1% on the vowel phoneme.
  - Subtracting a per-consonant-class offset (fitted leave-one-song-out) gives 68–73% per CTC aligner.
  - The median of the three CTCs is 77.4%. With a beat-position prior taken from sona's own output, it is
    82.1%; with 5 members, 83.3%.
  - Where the members span at most 3 cells: **90.5% [85.6, 94.8] on 78.4% of moras** (3 CTCs, licence-clean),
    and 95.1% on 65.9% (5 members, two with non-commercial weights).
  - About 10% of moras sit two or more cells off even for the median of the aligners. 40% of them are in amaten,
    where 14's lines drift; another 23% are in atamaro-deluxe (41 ms cells). They cap any source that gives every mora an onset near 89%.
- **The gated sources cut the cost against today's aligned fit, but they don't pass the declared rule.** Moving
  starts to the onsets (split + starts within one cell), GAME lead:
  - 5 members, span ≤ 3 cells: **287.6 cost/min, −16.6 [−23.4, −10.0] against today's aligned fit** (304.2). It
    gains on all 9 songs. The stage gives 256.7 (−14.9 [−23.1, −7.4]). Missing notes 311, against 318 today.
  - 3 CTCs, span ≤ 3 cells: 292.6, −11.6 [−18.1, −5.5] against today, with gains on 8 of 9 songs.
  - 16's rule asks for the 95% upper bound against no lyric (303.0) to be below 0. The best candidate gets
    −15.4 [−38.3, +4.5]. That interval inherits the noise of 14's landing against no lyric (+1.2 [−22.0, +19.0]).
    **No candidate passes rule 1. Nothing ships, and no Rust was written.**
  - The oracle with the UST's onsets is 244.4. The best real source gets 28% of the oracle's gain.
- **For the user to decide:**
  1. Whether start moves on top of 14's aligned fit should be judged against that fit (they pass) or against no
     lyric (16's rule; they fail).
  2. Whether running two more CTC models is worth −12 cost/min. Today's pipeline runs one CTC model, about
     2.5 min per song on 4 cores. The two speech models add about 2 min.

## Precision per source (dev, 5,336 moras)

A mora's onset is right when it rounds, on sona's grid phase, to the cell its UST note starts on. "±1" means
within one cell. "Median" is the signed error in cells. The interval is a song bootstrap (10 000 resamples).
"Moves" counts what start moves within one cell would do to today's aligned fit: a fix lands a start on the UST
cell, a break takes it off. The oracle (UST onsets) fixes 345 and breaks none. On placed moras, today's fitted
note start is on the UST cell 54.3% of the time.

| source | exact [95%] | ±1 | median | coverage | moves: fix / break |
|---|---|---|---|---|---|
| CTC lyrics (14; w2v-bert music lyrics) | 57.6% [46.9, 69.0] | 80.4% | −0.36 | 100% | 174 / 600 |
| CTC hiragana (speech) | 68.8% [57.9, 80.0] | 84.3% | +0.16 | 100% | 176 / 271 |
| CTC Japanese kana+kanji (speech) | 63.9% [54.9, 74.1] | 83.1% | −0.17 | 100% | 211 / 437 |
| CTC MMS, first letter (romaji) | 50.8% [41.1, 62.0] | 82.4% | −0.34 | 100% | 179 / 858 |
| CTC MMS, vowel letter | 34.2% [21.6, 48.4] | 81.3% | +0.60 | 100% | 151 / 1393 |
| SOFA JPN, vowel phoneme (inside 14's lines) | 64.1% [52.7, 76.0] | 75.6% | +0.03 | 100% | 177 / 207 |
| SOFA JPN, first phoneme | 34.2% | 69.1% | −0.51 | 100% | 159 / 1065 |
| Whisper + stable-ts (14) | 2.2% | 13.6% | −3.22 | 100% | 7 / 270 |
| lyrics CTC − class offset (LOSO) | 68.1% [55.9, 80.7] | 83.1% | +0.00 | 100% | 196 / 314 |
| hiragana CTC − class offset | 72.0% [62.8, 81.6] | 84.0% | +0.00 | 100% | 192 / 172 |
| Japanese CTC − class offset | 70.2% | 83.5% | −0.00 | 100% | 201 / 216 |
| MMS − class offset | 73.3% | 82.9% | −0.00 | 100% | 207 / 160 |
| SOFA vowel − class offset | 64.2% | 75.6% | +0.00 | 100% | 180 / 213 |
| median of 3 CTCs (lyrics, hira, jg; offsets) | 77.4% [68.0, 87.6] | 89.7% | +0.00 | 100% | 223 / 160 |
| … snapped to GAME's raw note starts (≤ 0.6 cell) | 78.0% | 89.6% | +0.04 | 100% | 167 / 107 |
| … to sona's onset segmenter | 78.3% | 89.6% | +0.02 | 100% | 224 / 148 |
| … to pitch-track voicing onsets | 73.3% | 89.6% | +0.08 | 100% | 207 / 296 |
| … to the output notes' starts | 78.7% | 89.6% | +0.02 | 100% | 211 / 127 |
| … lattice prior (σ 0.5 cell) | 82.1% [74.0, 90.2] | 88.8% | +0.02 | 100% | 226 / 54 |
| median of 5 (+ MMS, SOFA), lattice prior | 83.3% [73.3, 92.0] | 89.3% | +0.02 | 100% | 233 / 47 |
| 3 CTCs, adaptive prior | 81.5% [72.2, 90.7] | 88.0% | +0.02 | 100% | 246 / 59 |
| 5 members, adaptive prior | 82.9% [71.6, 92.8] | 87.9% | +0.02 | 100% | 251 / 33 |
| **S1: 3 CTCs, adaptive, span ≤ 3 cells** | **90.5% [85.6, 94.8]** | 96.9% | +0.02 | 78.4% | 209 / 50 |
| S2: 3 CTCs, adaptive, span ≤ 1.5 cells | 92.8% [88.2, 96.5] | 98.1% | +0.02 | 70.8% | 174 / 39 |
| **S3: 5 members, adaptive, span ≤ 3 cells** | **95.1% [92.2, 97.3]** | 99.0% | +0.02 | 65.9% | 167 / 20 |
| S4: 5 members, adaptive, span ≤ 1.5 cells | 96.7% [94.5, 98.3] | 99.4% | +0.02 | 56.4% | 137 / 12 |

**By consonant class** (exact / ±1 / median signed error in cells):

| class (moras) | CTC lyrics | SOFA vowel | lyrics − offset | 5-median + prior | S1 (covered) | S3 (covered) |
|---|---|---|---|---|---|---|
| vowel (833) | 40.8 / 61.1 / −0.40 | 46.2 / 64.6 / +0.04 | 34.6 / 65.2 / −0.01 | 70.3 / 83.8 / +0.02 | 75.7 / 90.7 (571) | 85.5 / 95.9 (434) |
| ん (253) | 3.6 / 15.8 / −2.37 | 32.8 / 45.1 / +0.03 | 18.2 / 46.2 / −0.02 | 36.8 / 57.3 / +0.02 | 64.6 / 90.3 (113) | 77.5 / 95.5 (89) |
| unvoiced stop (1418) | 64.2 / 85.5 / −0.34 | 65.2 / 76.4 / +0.01 | 79.5 / 86.0 / +0.00 | 86.7 / 88.7 / +0.02 | 94.7 / 98.2 (1138) | 97.9 / 99.7 (960) |
| voiced stop (544) | 71.7 / 89.0 / −0.26 | 72.2 / 78.7 / +0.04 | 81.8 / 89.7 / +0.00 | 88.6 / 91.4 / +0.02 | 94.7 / 97.3 (452) | 97.6 / 99.2 (381) |
| fricative (691) | 60.5 / 87.0 / −0.34 | 73.1 / 84.7 / +0.14 | 73.2 / 87.8 / −0.01 | 83.5 / 94.2 / +0.02 | 87.4 / 98.6 (571) | 91.9 / 99.2 (508) |
| nasal (902) | 72.3 / 90.7 / −0.27 | 69.3 / 80.2 / −0.03 | 79.8 / 91.4 / +0.00 | 91.7 / 93.3 / +0.02 | 95.2 / 98.1 (751) | 98.7 / 99.8 (636) |
| approximant (695) | 51.1 / 90.2 / −0.42 | 72.5 / 81.3 / +0.02 | 71.8 / 91.4 / −0.00 | 93.5 / 96.7 / +0.02 | 95.7 / 98.3 (585) | 97.8 / 99.4 (507) |

What the table says:
- **The CTC fires early on every consonant class.** The median is −0.26 to −0.42 cell, consistent with 16's
  consonant hypothesis. A constant offset per class (fitted on the other songs) removes it and gives about +10
  points. A vowel-initial mora (and ん) is the hard class for every source: there is no consonant to mark the
  boundary, and the change is often a legato vowel-to-vowel move.
- **SOFA finds the vowel, unbiased (median +0.03), but it makes local jumps.** Within a line it is exact on most
  moras. Then a run of 2–6 moras slips several cells, often at the line's first or last moras, where the margin
  lets it put phonemes in the gap. Its ±1 rate (75.6%) is the lowest of the aligners. Tighter margins (0.1 s)
  give 67.0% exact, but more breaks (237).
- **Sona's own onset candidates add little.** Snapping to GAME's raw starts, the onset segmenter or the output
  starts gains 0.6–1.3 points. Voicing onsets lose 4 points: they mark the consonant's voicing, not the vowel.
- **The lattice prior is the largest single step** (+4.7 points on the 3-median, breaks 160 → 54). It uses no
  lyric and no UST: the positions in the beat where sona's own output notes start. In most songs the UST's
  onsets sit on 2–3 of the 4 (or 6) cell positions. A Gaussian around the estimate times this prior picks the
  cell. The adaptive version takes σ from the members' own spread on that mora: 2 × their median distance from
  the median, at least 0.2 cell.
- **Agreement is the abstention signal.** Where the members span more than K cells, the source gives no onset
  and the note isn't moved. Precision rises fast, and the cost is coverage.

## Selection rule (declared before scoring; ticket 17's comments, 16b6f54)

Gate: precision ≥ 85% on the moras a source covers, and coverage ≥ 50%. S1–S4 pass. The grid was each of
S1–S4 × {split, start1, split+start1, split+start2}: 16 candidates. The rule is 16's, unchanged. On dev:
1. GAME lead: the paired song-bootstrap 95% upper bound of Δ cost/min against no lyric (303.0) is < 0;
2. stage Δ against no lyric (272.2) ≤ 0;
3. missing: lead ≤ 318, stage ≤ 217;
4. wrong moras/min ≤ 98.1.

Also required: the point estimate beats today's aligned fit (304.2). Tie-break: the lowest lead cost/min. A
licence-clean source wins when its paired difference to the best one has a 95% interval containing 0.

**Verdict: no candidate passes.** All 16 fail rule 1. The upper bounds against no lyric are +4.5 to +25.8.
Rules 2 and 4 pass for all 16. Rule 3 fails for S1 and S2 with split + start2 (stage missing 223 and 220; S1's
lead missing is 319). Only S1 split + start2 misses the check against today (+0.3).

## Table (dev; lines and spaces from 14's lyrics CTC with energy ends; paired song bootstrap, 10 000 resamples)

"Today" is 14's aligned fit with the tail rule. Start moves are within 1 (or 2) cells, legato, as in 16.

| fit | lead cost/min [95%] | Δ vs none [95%] | Δ vs today [95%] | clean | wrong/min | placed | missing | stage cost/min | stage Δ vs none [95%] | stage missing |
|---|---|---|---|---|---|---|---|---|---|---|
| none (no lyric) | 303.0 [206.6, 416.1] | 0 | −1.2 [−19.0, +22.0] | 16.2% | 192.7 | 0.0% | 312 | 272.2 | 0 | 213 |
| today: aligned fit, tail rule | 304.2 [211.3, 417.4] | +1.2 [−22.0, +19.0] | 0 | 16.3% | 93.1 | 51.7% | 318 | 271.5 | −0.7 [−19.3, +12.0] | 216 |
| *oracle:* UST onsets, split + start1 | 244.4 [169.2, 333.4] | −58.6 [−85.5, −34.9] | −59.8 [−87.4, −34.9] | 19.6% | 88.7 | 54.0% | 294 | 219.4 | −52.8 [−80.5, −28.8] | 197 |
| S1 split | 303.0 [210.4, 416.2] | −0.1 [−22.4, +15.9] | −1.2 [−4.3, +1.5] | 16.6% | 92.0 | 52.3% | 314 | 271.1 | −1.1 [−18.3, +10.3] | 216 |
| S1 start1 | 292.8 [199.5, 406.5] | −10.2 [−31.5, +8.8] | −11.4 [−17.3, −5.7] | 17.3% | 92.0 | 52.2% | 309 | 261.8 | −10.4 [−31.1, +6.8] | 213 |
| **S1 split + start1** | **292.6 [199.8, 406.5]** | −10.5 [−31.4, +7.8] | **−11.6 [−18.1, −5.5]** | 17.5% | 91.3 | 52.6% | 309 | 261.8 | −10.4 [−31.6, +7.1] | 214 |
| S1 split + start2 | 304.5 [210.9, 416.5] | +1.4 [−23.3, +25.8] | +0.3 [−9.3, +10.8] | 17.6% | 86.6 | 55.1% | 319 | 270.3 | −1.9 [−25.8, +18.3] | 223 |
| S2 split | 303.0 [210.4, 416.5] | −0.0 [−22.8, +16.8] | −1.2 [−3.8, +1.4] | 16.6% | 92.2 | 52.2% | 316 | 271.4 | −0.8 [−18.4, +11.3] | 216 |
| S2 start1 | 295.8 [199.6, 412.6] | −7.2 [−29.7, +13.2] | −8.4 [−15.0, −2.5] | 17.3% | 92.0 | 52.3% | 313 | 263.3 | −8.9 [−30.2, +8.4] | 213 |
| S2 split + start1 | 295.4 [200.0, 412.1] | −7.6 [−29.6, +12.2] | −8.8 [−15.9, −2.7] | 17.4% | 91.3 | 52.6% | 313 | 263.4 | −8.8 [−30.8, +8.7] | 214 |
| S2 split + start2 | 302.5 [207.5, 417.5] | −0.6 [−24.6, +23.4] | −1.7 [−10.9, +7.5] | 17.7% | 87.5 | 54.6% | 318 | 266.6 | −5.6 [−28.8, +14.0] | 220 |
| S3 split | 301.5 [209.0, 413.3] | −1.6 [−24.2, +16.0] | −2.7 [−5.2, −0.4] | 16.6% | 92.1 | 52.2% | 314 | 269.2 | −3.1 [−21.4, +9.7] | 215 |
| S3 start1 | 289.0 [195.2, 402.1] | −14.1 [−37.2, +5.9] | −15.3 [−21.6, −9.4] | 17.4% | 92.1 | 52.2% | 310 | 257.4 | −14.8 [−37.4, +2.6] | 209 |
| **S3 split + start1** | **287.6 [195.2, 399.6]** | −15.4 [−38.3, +4.5] | **−16.6 [−23.4, −10.0]** | 17.6% | 91.3 | 52.6% | 311 | 256.7 | −15.5 [−38.3, +2.3] | 211 |
| S3 split + start2 | 293.1 [200.9, 404.4] | −9.9 [−36.1, +15.6] | −11.1 [−21.5, −0.0] | 17.9% | 86.8 | 55.0% | 314 | 257.4 | −14.8 [−40.0, +6.9] | 215 |
| S4 split | 303.0 [210.1, 415.7] | −0.1 [−23.3, +17.5] | −1.2 [−2.3, −0.3] | 16.3% | 92.8 | 51.9% | 317 | 270.6 | −1.6 [−20.4, +11.3] | 216 |
| S4 start1 | 292.6 [198.2, 406.0] | −10.5 [−32.8, +8.0] | −11.6 [−17.0, −7.3] | 17.2% | 92.2 | 52.1% | 312 | 261.3 | −10.9 [−32.3, +5.1] | 210 |
| S4 split + start1 | 292.4 [198.3, 405.6] | −10.7 [−32.9, +7.7] | −11.8 [−17.2, −7.4] | 17.2% | 92.0 | 52.3% | 313 | 261.2 | −11.0 [−32.8, +5.1] | 212 |
| S4 split + start2 | 292.8 [198.6, 406.0] | −10.2 [−33.5, +10.3] | −11.4 [−19.0, −5.0] | 17.5% | 89.2 | 53.7% | 315 | 260.6 | −11.6 [−33.6, +6.5] | 214 |

Almost all the gain is in the starts. Splits alone are worth −1 to −3. The class counts move where they should.
For S3 split + start1 against today: starts one cell off (`shift`) 248 → 201, wrong lengths (`length`)
779 → 721, missing 318 → 311. Near-miss pitch is flat (188 → 192).

**Per song** (GAME lead cost/min; none → today → oracle → S1 → S3, split + start1):

| song | none | today | oracle | S1 | S3 |
|---|---|---|---|---|---|
| amaten | 495.4 | 403.5 | 387.1 | 405.5 | 393.7 |
| atamaro-deluxe | 626.7 | 633.0 | 493.9 | 622.8 | 608.5 |
| azemichi-ni-ochita-ginga | 136.9 | 141.1 | 117.4 | 129.1 | 128.6 |
| fukagyakuteki-henyou | 437.6 | 457.6 | 373.8 | 442.5 | 453.1 |
| innocent-addict | 285.2 | 283.0 | 204.7 | 257.4 | 256.9 |
| kamaitachi-ni-yoru | 177.1 | 183.9 | 149.2 | 179.6 | 160.8 |
| my-axiom | 265.3 | 316.9 | 227.0 | 309.2 | 305.4 |
| telescope-nevermore | 141.7 | 140.3 | 128.1 | 139.4 | 138.2 |
| yamineko | 225.9 | 230.0 | 173.3 | 202.4 | 194.9 |

S3 beats today on all 9 songs, and S1 on 8 (amaten +2.0). Against no lyric, both still lose on
fukagyakuteki-henyou and my-axiom, and S1 on kamaitachi-ni-yoru too. 14's landing costs more than it saves on
those songs, and the start moves don't make up for it. That is why rule 1 fails.

## What else these numbers could be measuring

- **Leaked labels.** Only the per-class offsets read the UST, and they are fitted leave one song out. The lattice
  prior counts sona's own output note starts; the abstention compares the members with each other. Neither sees
  the UST.
- **In-sample choices.** The prior's σ (0.5 cell for the fixed version; scale 2 and floor 0.2 for the adaptive
  one), the members, the span thresholds and SOFA's margins were all looked at on dev precision. The fixed σ is
  sensitive (0.3: 81.1%, 0.5: 82.1%, 0.7: 80.0% on the 3-median). The adaptive σ is flat across scale 1.5–3
  (net moves +205 to +218 for 5 members), which is why S1–S4 use it. The split.py grid was declared and small
  (16 rows), and the rule fails anyway.
- **Abstention picks the easy moras.** It does, and the start-move counts show the cost. Gated S3 makes 167
  fixes and 20 breaks; ungated, the same source makes 251 and 33. The ungated sources failed the 85% gate and
  were not scored in `split.py`, so whether the extra moves pay is open (Next).
- **The non-lyric part of the gain.** The prior pulls onsets toward the song's common beat positions. A move
  still needs a lyric onset within one cell of the note's start, so it is not a pure lattice snap. Ticket 18
  owns pure non-lyric timing; this result should be measured again on top of its fixes, because the two may
  overlap.
- **One song.** The gain against today is spread over all 9 songs (−2 to −35). It is not carried by amaten.
- **The comparator.** Rule 1 compares with no lyric. Every lyric fit carries 14's landing noise against no lyric,
  ±20 cost/min. The paired test against today isolates what this ticket changes, and it passes.

## Licences and speed

| component | code | weights | notes |
|---|---|---|---|
| lyrics CTC (14) | transformers Apache-2.0 | MIT | `mrfakename/w2v-bert-2.0-music-lyrics-ctc`; training lyrics not named |
| hiragana CTC (14) | Apache-2.0 | Apache-2.0 | `vumichien/wav2vec2-large-xlsr-japanese-hiragana` |
| Japanese CTC (14) | Apache-2.0 | Apache-2.0 | `jonatasgrosman/wav2vec2-large-xlsr-53-japanese`; JSUT audio is research / personal use |
| MMS forced aligner (14) | Apache-2.0 | CC-BY-NC-4.0 | user-installed tool only |
| SOFA (qiuqiao/SOFA, main) | MIT | — | read before running; the checkpoint is loaded with `torch.load(weights_only=True)` (tensors and OrderedDict only) |
| SOFA JPN_Test2_Plus (Greenleaf2001/SOFA_Models) | — | release note: "Commercial Use: Not Approved"; no licence file | singing; trained on Namine Ritsu, Tohoku Kiritan, Oniku Kurumi, Natsume Singing, HTS (+ GTSinger unlabelled) |

S1 uses only the first three, which are MIT and Apache-2.0. S3 and S4 add MMS and SOFA, which can't ship
commercially.

On 4 threads of this CPU, SOFA takes about 2 s per song. The re-run Viterbi for the MMS vowel letter and all the
combining take seconds. S1's cost is running the two speech CTCs next to the lyrics model: about 1 min per song
each (14's Speed).

## Next

- A decision on the comparator (Answer). If start moves are judged against today's aligned fit, S1 (licence-clean)
  passes on dev with −11.6 [−18.1, −5.5]. It would go to holdout4 with 14's port, bundled under the spec's rule.
- The ungated adaptive sources move more starts right (fix 251, break 33 for 5 members; 246 / 59 for 3 CTCs).
  They need their own declared run, with the gate rewritten around the moves rather than whole-song precision.
- Vowel-initial moras and ん are the weak class (S1: 75.7% and 64.6% on the moras it covers). A pitch-change
  detector inside held vowels is the obvious candidate. GAME's boundaries alone add nothing.

## Reproduce

```sh
SP=<session scratchpad>   # w2/dump: `sona-eval-cellstage --edit-cost --dump`; l01/lines; l14/{text,align,times,emis}
export L01_DUMP=$SP/w2/dump L01_WORK=$SP/l01 L14_WORK=$SP/l14 L17_WORK=$SP/l17
export SONA_STEMS=crates/sona-eval/data/pifuyuu-stems
D=crates/sona-eval/scripts/vowel-onsets
# MMS vowel letters from 14's cached emissions (14's venv)
~/.cache/sona/line-align/venv/bin/python -I $D/mms_vowels.py
# SOFA inside 14's lines (own venv: torch CPU, lightning, einops, librosa, numba, soundfile, soxr, matplotlib)
SOFA_DIR=~/.cache/sona/vowel-onsets/dl-sofa/SOFA-main SOFA_CKPT=~/.cache/sona/vowel-onsets/dl-sofa-jpn/step.100000.ckpt \
  OMP_NUM_THREADS=4 ~/.cache/sona/vowel-onsets/venv/bin/python -I $D/sofa_align.py
# derived sources and the precision table
python3 $D/sources.py
python3 $D/precision.py ctc-lyrics-lead ctc-hira-lead ctc-jg-lead ctc-mms-lead mms-vowel sofa-vowel sofa-first \
  ctc-whisper-large-v3-lead deb-ctc-lyrics-lead deb-ctc-hira-lead deb-ctc-jg-lead deb-ctc-mms-lead deb-sofa-vowel \
  ens3 ens5 ens3+gamesnap ens3+sonasnap ens3+voicesnap ens3+outsnap ens3+prior ens5+prior m3+adaptmora m5+adaptmora \
  m3+adaptmora+abst3.0 m3+adaptmora+abst1.5 m5+adaptmora+abst3.0 m5+adaptmora+abst1.5 --per-song
# the declared grid
v=none,tail,ust:split+start1
for s in m3+adaptmora+abst3.0 m3+adaptmora+abst1.5 m5+adaptmora+abst3.0 m5+adaptmora+abst1.5; do
  for r in split start1 split+start1 split+start2; do v="$v,file-$s:$r"; done; done
python3 crates/sona-eval/scripts/mora-split/split.py --variants "$v"
python3 crates/sona-eval/scripts/mora-split/split.py --per-song \
  --variants "none,tail,ust:split+start1,file-m3+adaptmora+abst3.0:split+start1,file-m5+adaptmora+abst3.0:split+start1"
```

SOFA source: `https://codeload.github.com/qiuqiao/SOFA/tar.gz/refs/heads/main` (fetched 2026-11-24). Model:
release `JPN_Test2_Plus` of Greenleaf2001/SOFA_Models (`step.100000.ckpt`, 53 MB, `japanese-extension-sofa.txt`).
