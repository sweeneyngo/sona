# ADR-0037: The stage trained for edit cost is the default cell model

Status: accepted, 2026-11-25. Ticket: `.scratch/utau-edit-cost/issues/17-holdout4-read-harness.md` (the
holdout4 edit-cost read, rule 1). Amends [ADR-0026](0026-learned-cell-stage-in-rust.md): the model it names,
`cellstage-D`, is no longer the recommended one. Study: `crates/sona-eval/reports/stage-edit-cost.md`
(ticket 19).

## Context

sona's goal is a transcription that costs little to fix in OpenUtau, measured as edit cost per minute
(utau-edit-cost 01; weights: extra 1, pitch 3, octave 1, length 4, shift 4, moved 4, missing 2, chopped 4,
joined 4). ADR-0026's stage, `cellstage-D`, was trained to get grid cells right. Ticket 19 retrained it on
rows sona's own feature builder writes. The new stage, C, has two changes:

- it reads GAME lead's rounding residuals (`lead_residuals`);
- it decodes the cells with a cost per note edge (`switch_cost` 1.5).

In a song-level cross-validation on the 38 spent songs, C cut the edit cost from 275.8 to 254.1 per minute.

Holdout4 (24 songs, never read before) was read once on 2026-11-25 under rules pre-registered in ticket 17
(final at 6fc69dd). The alignment check kept 23 songs.

| | D (ADR-0026) | C (`cellstage-19C`) |
|---|---|---|
| edit cost / min | 186.8 | **176.8** (−9.9 [−19.5, −1.0]) |
| missing notes | 465 | 390 |
| clean measures | 22.0% | 24.5% |

Rule 1 needed the 95% upper bound below 0, and missing notes not rising (or the bound holding at missing weight
4, where it is −3.4). Both hold, so it passes.

- C is better on 15 of 23 songs.
- Without the song that moved most (ai), C's gain is −7.6 [−15.8, +0.5]. This was reported and decides nothing.
- The near-tie settling (`--settle-near-ties`) on top of C was +1.4 [−1.3, +4.0]. It stays opt-in.

## Decision

1. **The recommended `--cell-model` folder is the edit-cost stage**: `~/.cache/sona/cellstage-19C`, trained by
   ticket 19's `final19.sh` on all 38 spent songs and exported with `switch_cost` 1.5. `sona serve
   --cell-model DIR` and `sona … --cell-model DIR --vocals …` take it unchanged. The app and the scorer share
   `game::cell_stage_notes`, which reads `lead_residuals` and `switch_cost` from the folder's
   `cellstage.json`, so the scorer's holdout4 numbers are the app's.
2. **No code default changes.** sona has never shipped stage weights. They derive from Pifuyuu's USTs, whose
   author forbids redistribution, and from GAME's CC BY-NC-SA weights. The folder stays local, and the user
   names it.
3. **D is kept on disk** as the reference for the ADR-0017 cell-accuracy history. It is no longer the
   recommendation.
4. **The knobs the holdout4 read did not measure are deleted:** `--merge-unheard-pieces` (ticket 11) and
   `--trim-releases` (ticket 12). Each earned nothing on top of the bundle on dev (ticket 20).

## Consequences

- Fresh songs should cost about 10 edit-cost points per minute less to fix, with fewer missing notes. On
  holdout4, C's pooled cost (176.8) is under the "usable" target of 190 per minute. The other half of "usable",
  bar 1 on the music's bar line, is rule 4, read after the user's ear labels.
- The doubtful-note flags (ADR-0034) rank each song's notes by the stage's logits. Their default share (20%)
  was chosen on D's dev results, so it should be rechecked on C.
- The lyric step (ticket 14's line fit with ticket 17's S1) was measured on C. Its holdout4 read (rule 2) waits
  for lyric texts of all 23 aligned songs. If it passes, it ships on C whenever a lyric is given.
- Holdout4 is spent for stage decisions. The next stage change needs a fresh holdout.
