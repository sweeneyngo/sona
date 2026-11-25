# 17 — The harness for the one holdout4 read

Status: in progress
Type: task
Blocked by: lyrics-first 14 (its winner joins the bundle), ear labels for holdout4 (user)

## What

Everything the single pre-registered holdout4 read needs, built and tested on dev before the read:

1. **Alignment check first** (08): run the onset-delta check on every holdout4 song; drop the songs it flags
   (35 ms shift / 30 ms drift) before any score; at least 20 must remain or the read stops.
2. **`--split holdout4` through the ledger** for every scorer the bundle uses: `sona-eval-cellstage`
   (edit cost, lyric placement, the opt-in arms: 07/lyrics fit, 11, 12, 13's confidence curve) and
   `sona-eval-frame --estimate-bar-one` (05's arms A, B, C). Today `--estimate-bar-one` has no `--split` path.
3. **Ear labels**: the music's beat and bar 1 on each holdout4 song, from the click overlays
   (`sona-eval-frame --clicks`), with the user confirming at least 5 (05's rule).
4. **One script** that runs the whole read once, writes the ledger entry, and prints each pre-registered rule's
   verdict.

Rehearse the script end to end on dev (which can be re-read freely), then hand it over. Nothing touches
holdout4 until the user has labelled and the rules are final.

## Comments
