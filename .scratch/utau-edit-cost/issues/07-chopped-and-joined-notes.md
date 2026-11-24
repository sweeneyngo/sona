# 07 — Same-pitch boundaries: split at moras, merge restarts

Status: resolved
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

## Answer

Report: `crates/sona-eval/reports/same-pitch-boundaries.md` (dev only; no holdout read, spent splits not
re-read). Baseline reproduced byte for byte: GAME lead 188.7 cost/min / 16.2% clean (out of sample), stage
180.2 / 23.8% (in-sample).

- **Oracles (GAME lead Δ cost/min; stage beside it).**
  - True mora count per note, split at the true onsets: −81.4 [−116.6, −50.3] (stage −84.3).
  - True count with each extra mora taking the note's last 2 cells (how the UST writes it): −51.9
    (stage −55.6). With equal division: −37.0 (−40.5). At the detector's peaks: −29.6 (−37.1).
  - Merging zero-mora same-pitch notes: −4.6. Merging restarts that have no UST onset: −8.2.
  - Only the phrase's count known, given to the longest notes: **+36.3**. Placement and allocation decide
    everything.
- **No-lyrics detector (2, 3): fails.**
  - MFCC/energy novelty reaches AUC 0.72 (hidden onsets are 8% of grid points); a leave-one-song-out
    classifier with f0 and position features reaches 0.73. Every split threshold raises the cost.
  - Merges: the best of 16 settings, −1.6 [−3.3, −0.1], becomes −1.8 [−3.8, +0.1] out of fold.
- **Whole-song lyric DP (1b, realistic): fails.** With wiki text read by fugashi: +2.3 … +15.0. With the
  UST's own kana: +0.9 … +13.1. A deficit mora is as often a note sona missed as a joined one.
- **Per-phrase lyric fit: passes the rule; ported (opt-in).**
  - What it does: the user pastes each phrase's kana onto that phrase's notes, a phrase being a run of
    touching notes. A DP uses kind-aware attach costs (open mora 0.55, consonant 0.15) and a duration term;
    then a note holding extra moras is split with a 2-cell tail, and a same-pitch note holding none is
    joined into the one before.
  - Cost: GAME lead **188.7 → 174.9, −13.9 [−30.2, −3.8]**; stage 180.2 → 162.4, −17.8 [−38.0, −4.8].
    With the design chosen leave-one-song-out: −11.5 [−28.4, −1.4] and −16.6 [−37.1, −3.1].
  - Clean measures: 16.2 → 15.9% and 23.8 → 25.0%.
  - **Phrases whose note count equals the mora count: 29.7% → 45.4% (lead), 43.0% → 71.1% (stage).**
  - amaten is 55% of the gain; 6/9 songs improve on GAME lead.
  - Code: `sona_core::lyric_fit` plus `sona-eval-cellstage --fit-phrase-lyrics`. The port reproduces the
    prototype's per-song class counts on 18/18 rows, and the default run is byte-identical. Behaviour tests
    pass, and three mutants are caught (equal division, joining across a pitch change, flat attach cost).
- **What changes the plan.** Joined notes, the largest cost class, are not fixable at transcription time
  with today's evidence. The lever is the lyric, pasted per phrase. That needs an editor feature (fit the
  selected notes to the pasted phrase); `web/lib/lyrics.ts` today assigns tokens in order with no fit. It is
  a product decision for the user, and a default needs holdout4. The prototype's numbers depend on
  phrase-level pasting: with phrases split only at rests of ≥ 4 cells, the gain falls to −7.0
  [−17.3, +0.1].

