# 08 — holdout4: fresh songs from BowlRoll

Status: needs-info
Type: task
Blocked by: —

## What

Every split is spent. Nothing in 05–07 can become a default without a fresh set. Build a fetcher for
Pifuyuu's USTs posted only on BowlRoll, add the next ≥ 20 aligned songs as split `holdout4` (20, not 10,
so a +2 change is detectable), with the manifest's skip-and-backfill rules.

## Needs from the user

- Does BowlRoll need an account, and is automated downloading acceptable under its terms?
- Should holdout4 keep the "one integer tempo" rule, or include songs with tempo changes so the frame
  metrics see them?

## Comments
