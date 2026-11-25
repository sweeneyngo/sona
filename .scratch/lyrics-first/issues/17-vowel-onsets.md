# 17 — Vowel onsets precise enough to time the notes

Status: claimed
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
