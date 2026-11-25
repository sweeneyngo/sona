# 17 — Vowel onsets precise enough to time the notes

Status: resolved
Type: research
Blocked by: 16

## What

16 showed that moving note starts to each mora's onset, within one cell, would cut GAME lead's cost from
303.0 to 244.4 cost/min (stage 272.2 → 219.4) with the UST's onsets. The gain needs an onset on the
right cell about 90% of the time (break-even near 90%; 95% gives 265.7 with splits). 14's CTC aligner
gets 57.6% and fires a median 0.36 cell early, likely on the consonant, while a UST note starts on the
vowel.

Find a vowel-onset source inside each placed line that reaches about 90% cell precision on dev.
Candidates include a singing phoneme aligner (SOFA, MIT; it needs line spans, which 14 now gives), the
CTC onset as a prior on sona's own onset candidates (energy, GAME boundaries, pitch-track voicing), and
a consonant-to-vowel offset per consonant class. Measure cell precision first, and score with
`scripts/mora-split/split.py` only once a source passes 85%.

## Acceptance

- Precision per source (exact cell, within one cell, and the median signed error), by consonant class.
- The 16 precision sweep run on the best source; a selection rule committed before that.
- Local only; licences recorded for code and weights.

## Comments

### 2026-11-24 — precision first, then the selection rule (committed before any onset source is scored by `split.py`)

Precision on dev (cell exact against the UST note start, on sona's grid phase; `scripts/vowel-onsets/precision.py`).
No source reaches 85% on every mora: the best full-coverage sources are the 5-member median with the lattice
prior (83.3%) and the adaptive-sigma version (82.9%). 14's line failures (amaten's drift) put ~10% of moras two or
more cells off for every aligner, which caps any full-coverage source near 89%. Sources that abstain where their
members disagree do pass 85% on the moras they cover:

| source | exact | coverage | weights |
|---|---|---|---|
| S1 `m3+adaptmora+abst3.0` (lyrics, hira, jg CTCs) | 90.5% | 78.4% | MIT, Apache-2.0, Apache-2.0 |
| S2 `m3+adaptmora+abst1.5` | 92.8% | 70.8% | same |
| S3 `m5+adaptmora+abst3.0` (+ MMS, SOFA JPN) | 95.1% | 65.9% | + CC-BY-NC, "commercial use not approved" |
| S4 `m5+adaptmora+abst1.5` | 96.7% | 56.4% | same |

**Gate:** a source goes to `split.py` when its exact-cell precision on the moras it gives an onset for is ≥ 85% and
it covers ≥ 50% of moras. S1–S4 pass; nothing else is scored.

**Declared grid** (16 candidates): each of S1–S4 with `split`, `start1`, `split+start1`, `split+start2`
(`file-<source>:<rules>`). Baselines in the same run: none, tail (today), `ust:split+start1` (oracle).

**Selection rule** (16's, unchanged). A candidate ships opt-in when, on dev:
1. GAME lead: the paired song-bootstrap 95% upper bound of Δ cost/min against no lyric (303.0) is < 0;
2. the stage is no worse: Δ against no lyric (272.2) ≤ 0;
3. missing notes are not up: lead ≤ 318, stage ≤ 217 (1.02 × no lyric);
4. placement is kept: wrong moras/min ≤ 98.1.

Also required: the GAME lead point estimate beats today's aligned fit (304.2). **Tie-break:** the lowest GAME lead
cost/min; a licence-clean source (S1, S2) is preferred over S3/S4 when its paired difference to the best one has a
95% interval containing 0. Anything else (other gates, thresholds, tolerances) is reported as added later.

## Answer

**No single source reaches 85%. Three CTC aligners combined do, where they agree. Their start moves beat today's
aligned fit but fail 16's rule against no lyric, so nothing ships and no Rust was written.**
`crates/sona-eval/reports/vowel-onsets.md`.

- **Precision** (dev, exact UST start cell):
  - single sources: lyrics CTC 57.6%, hiragana CTC 68.8%, Japanese CTC 63.9%, MMS 50.8%, SOFA JPN vowel phoneme
    64.1% (unbiased, +0.03 cell, but with local jumps);
  - with a per-consonant-class offset (leave one song out): 68–73% per CTC. The CTC fires 0.26–0.42 cell early
    on every consonant class;
  - median of the three CTCs 77.4%; with a beat-position prior from sona's own output starts, 82.1%; with 5
    members, 83.3%. About 10% of moras are two or more cells off even for the median, 40% of them in amaten;
  - abstaining where the members span more than 3 cells: **S1 (3 CTCs; MIT and Apache weights) 90.5% [85.6,
    94.8] on 78.4% of moras**; S3 (5 members, with MMS and SOFA, non-commercial weights) 95.1% on 65.9%;
  - vowel-initial moras and ん are the weak class everywhere: S1 75.7% and 64.6% on the moras it covers;
  - sona's own onset candidates (GAME's raw starts, the onset segmenter, the output starts) add 0.6–1.3 points.
    Voicing onsets lose 4 points.
- **split.py, the declared 16 candidates** (S1–S4 × split / start1 / split + start1 / split + start2):
  - S3 split + start1: GAME lead **287.6**, Δ against today's aligned fit **−16.6 [−23.4, −10.0]**, against no
    lyric −15.4 [−38.3, +4.5]; stage 256.7; missing 311. It gains on all 9 songs;
  - S1 split + start1: 292.6, −11.6 [−18.1, −5.5] against today, −10.5 [−31.4, +7.8] against no lyric. It gains
    on 8 of 9 songs;
  - the oracle is 244.4. The best source reaches 28% of its gain.
- **Verdict under the declared rule: no candidate passes.** All 16 fail rule 1 (upper bound against no lyric
  < 0). That interval carries 14's landing noise against no lyric (±20 cost/min).
- **For the user:**
  - whether start moves layered on 14's fit should be judged against that fit (S1 passes on dev) or against no
    lyric (16's rule);
  - whether two more CTC models (about 2 min per song on 4 cores) are worth about −12 cost/min.
