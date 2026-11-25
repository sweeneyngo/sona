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
