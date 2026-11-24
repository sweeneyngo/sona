# 16 — A session on the 1/16T grid reopens on another grid

Status: ready-for-agent
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
