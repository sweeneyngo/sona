# 20 — Measure the holdout4 bundle together on dev

Status: ready-for-agent
Type: prototype
Blocked by: 19

## What

Four opt-in winners were each measured alone on dev, against different baselines and partly in sample:

- `--merge-unheard-pieces` (stage 272.2 → 267.2, in-sample stage);
- `--settle-near-ties` (18; GAME lead 303.0 → 297.8; nothing on the stage);
- 14's line fit (`--lyric-placement --lyric-lines --lyric-line-times`; the gain on GAME lead vanished under
  the revised weights: 304.2 vs 303.0) plus lyrics-17's S1 start moves (three CTCs, abstain on spread > 3;
  GAME lead 292.6, Python only, `scripts/mora-split/split.py`);
- 19's stage C (CV 275.8 → 254.1, out of sample; per-fold models in
  `.claude/worktrees/agent-a617324741adeda06/target/stage19-cv/F/C/models`, scored with `--fold-models`).

Winners can interfere. Before the one holdout4 read, measure the bundle as it would ship, on the dev songs
(9), with the stage scored out of sample (C's fold models). Answer:

1. Each arm's Δ on top of C (out of sample), one at a time, and all together, with paired song-bootstrap
   intervals, clean measures and missing counts (revised weights). Baseline B's fold models
   (`.../F/B/models`) give the "today" row under the same folds.
2. Does the lyric (14 fit + S1) still pay on top of C? S1 was built on GAME lead notes; apply it to C's notes.
   17's rule compares against **no lyric**: the lyric arm must beat C alone with the upper bound < 0.
3. Where does C's cost sit after the bundle (taxonomy by class, as `ec.py` does)? Joined notes are C's
   largest class; how much of it does the lyric remove?
4. The bundle that holdout4 will read: the arms that pass, in one command line (Rust where an arm is ported;
   name any arm that is Python-only and what porting it needs).

## Rules

- Commit the selection rule before scoring: an arm stays in the bundle if its Δ on top of the rest has an upper
  bound < 0 on dev and missing does not rise by more than its bound allows at missing weight 4 (19's rule 2).
- Dev only (dev may be re-read freely). Never touch holdout4. Spent splits only for diagnosis, through the
  ledger with `--reread --reason`.
- No UST, audio, stem, lyric or lyric-derived content, and no weights, in the repo.
- Build only into your worktree's own `target/`. Training is not part of this ticket.

## Acceptance

- Selection rule committed first; a table of arms (alone on C, all together, leave-one-out from the bundle).
- The bundle's command line and the list of Python-only arms with port plans.
- Report under `crates/sona-eval/reports/`, an `## Answer` on this ticket, a decision line in `map.md`.

## Comments
