# 07 — Same-pitch boundaries: split at moras, merge restarts

Status: claimed
Type: research
Blocked by: 03

## What

03 (`crates/sona-eval/reports/edit-cost-baseline.md`) found that joined notes are the largest cost class on
every split: 51.5% of GAME lead's cost on dev, 32–40% on the spent splits. Their cause is UTAU's
one-note-per-mora convention. 62–78% of the hidden UST notes have the same pitch as the note before, mostly
a bare vowel (さ>い) or ん (げ>ん), with no consonant to hear. No current source proposes most of these
boundaries: any source does so 26–37% of the time. So choosing better among today's sources fixes at most
10–16% of the cost.

The other side of the same question: GAME restarts a held note at the same pitch, and that is 29–41% of the
chopped cost. Splitting and merging pull opposite ways on the same kind of boundary (same pitch, does the
mora change?), so one ticket decides both.

## Steps (oracle first)

1. **Lyric oracle.** In the app, users paste lyrics (the lyric editor), so the mora count is often known.
   - (a) Ceiling: given each sung span's true mora count from the UST, split joined spans optimally. This
     says how much a perfect count is worth.
   - (b) Realistic: given the song's lyric text only (a mora stream with no note boundaries), align it to
     sona's notes (DP over mora vs note) and split where the alignment puts more moras than notes.
     Measure both on dev with the 03 dumps.
2. **No-lyrics detector**: find same-pitch vowel/ん changes from the spectrum (formant or MFCC change,
   energy dip), not from pitch. Oracle: split at the detector's peaks given the true count, then with a
   threshold.
3. **Merge same-pitch restarts** when the detector sees no mora change across them.
4. Port the winner to Rust behind an opt-in flag, reproduce the prototype's numbers exactly, and report
   dev with song-bootstrap intervals.

## Rules

- Dev only. The 03 dumps (`$SCRATCH/dump-dev`) contain UST lyrics, so keep them and anything derived from
  them out of the repo. The spent splits may be re-read only as logged diagnosis.
- The learned stage is in-sample on dev. Report GAME lead as the out-of-sample reference beside it.
- The Fog in `map.md` on lyrics applies: an extra note shifts every later pasted mora. Report the share of
  phrases whose note count equals their mora count, before and after.

## Acceptance

- Oracle table (a, b, detector) on dev: cost removed, by class.
- If anything ports, an opt-in flag with dev cost/min and clean measures, with intervals, and the default
  unchanged.

## Comments

- 2026-11-24 (claimed). **Selection rule, declared before any measurement.** A mechanism is worth porting
  if on dev it lowers GAME lead's pooled cost/min with a song-bootstrap 95% interval on the paired
  difference (mechanism minus baseline, 10 000 resamples of the 9 songs) that excludes 0, and it does not
  raise the learned stage's pooled cost/min. Oracles (1a, 2 with the true count) are ceilings and never
  port. Thresholds are chosen on dev only; spent splits are diagnosis only (logged re-reads) and confirm
  nothing; holdout4 is not read. Anything ported is opt-in and the default stays byte-identical.
