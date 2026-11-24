# 16 — A session on the 1/16T grid reopens on another grid

Status: resolved
Type: task
Blocked by: —

## Evidence

A08: session restore (`web/lib/session.ts` `settingsOnLoad`, from the old `restoreSession`) accepts only grid
divisions 4/8/16/32, but the grid menu and `sona serve` jobs offer 24 (16th triplets). A saved or autosaved
1/16T session reopens on the previous or default grid, so the notes snap and export differently after a
reload. A user loses their work's timing silently.

## Proposal

Accept every division the app offers (one shared list, used by the menu, the job options and restore); a
regression test that saves and reopens a division-24 session and gets the same UST bytes; extend A08's
save → reopen property test to every offered division (show the old 4/8/16/32 list fails it).

## How to tell

The property test over all divisions passes; the mutant (old list) fails.

## Comments

## Answer

Done on branch `audit-a15-a16` (from 228fecd).

**One list.** `web/lib/grid.ts` `GRIDS` holds the grids the app offers (1/4, 1/8, 1/16, 1/16T, 1/32), each
with its label and whether a `sona serve` job can decode on it (16 and 24, as `--grid` and the server
accept). Everything reads it:
- the grid menu and the start screen's job grid menu are filled from it by `main.ts` (`index.html`'s
  hard-coded `<option>`s are gone);
- the job request: `JobRequest.grid` is `JobDivision`, and `jobDivision()` replaces
  `value === "24" ? 24 : 16` (same result);
- loading a job's result takes its grid when `isDivision`;
- session restore (`settingsOnLoad`) accepts any `isDivision` value instead of `[4, 8, 16, 32]`; that is
  the fix: a 1/16T session now reopens on 1/16T;
- the export test helpers' `DIVISIONS` (`exportCases.ts`) is now `grid.ts`'s (same values, same order, so
  the seeded cases are unchanged).

**Tests** (`web/lib/session.test.ts`):
- the save → reopen property test draws its division from `DIVISIONS` (all five) instead of
  `[4, 8, 16, 32]`;
- new: for every offered division, a saved session reopened over a 1/16 editor comes back on that grid
  and exports the same UST bytes; the 1/16T UST differs from the 1/16 one, so the check is not vacuous.
- `grid.test.ts`: the list, `isDivision`, the job grids and `jobDivision`.
- browser: `web/e2e/help_and_grids.py` checks both menus' options and that a session saved on 1/16T
  reopens on 1/16T (tempo chip "125 · 1/16T") with an identical UST.

**Mutant** (the old `[4, 8, 16, 32]` in `settingsOnLoad`): the property test fails, the new test fails
("expected 16 to be 24"; with that assertion removed, the UST bytes differ, 463 vs 548 bytes), and the
browser driver fails both its reopen checks ("125 · 1/16").

**Verified:** clippy `-D warnings` clean; `cargo test --workspace --release` 309 passed, 0 failed; `tsc --noEmit` clean, vitest 183/183 (22 files); `scripts/ci/e2e.sh` 11/11 editor drivers; the five TLA+ checks pass; corpus guard ok.
