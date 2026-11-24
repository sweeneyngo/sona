# ADR-0027: The learned cell stage takes its grid phase from the vocals' onsets

Status: accepted, 2026-11-23. Spec: `.scratch/grid-phase/spec.md` (pre-registered in c90befd).

## Context

The learned cell stage (ADR-0026) needs the song's grid phase: where the 16th-note cells start.
- The prototype was given the UST's true phase.
- The Rust port estimated it from sona's grid notes on the vocals stem (the first note's onset modulo
  a cell).
- That cost 0.65 points of cells on holdout3, nearly all on one song where the estimate was half a
  cell off.

## Decision

`sona_core::tempo::grid_phase` estimates the phase from onsets:
- **Onset strength:** log spectral flux at a 256-sample hop (the tempo estimator's, ADR-0022, at a
  finer hop).
- **The phase:** the offset in `[0, spacing)`, in 240 steps, whose grid points collect the most flux
  over the whole song.
- **Timing:** an attack registers in the flux at 0.772 of the analysis window rather than its centre.
  This was measured on click tracks and is the same at 22.05 and 44.1 kHz. A unit test holds the
  estimate within 2 ms on clicks.

The cell stage runs it on the **vocals stem**, which the CLI and the server already have. sona's vocals
notes are then decoded on that phase (`GridMode::anchor_s`), so every candidate sits on one grid. When
there is too little audio to hear onsets, it falls back to the old estimate.

## Evidence

Phase error is the circular distance from the UST's alignment phase, as a fraction of a cell, at the
UST's exact tempo (`sona-eval-phase`).

| | dev (9): median | dev: within 0.1 cell | test (30): median | test: within 0.1 cell |
|---|---|---|---|---|
| onset phase, vocals (adopted) | 0.021 | 9/9 | **0.023** | **29/30** |
| onset phase, full mix | 0.021 | 9/9 | 0.017 | 29/30 |
| sona grid notes (before) | 0.081 | 5/9 | 0.102 | 14/30 |

- **The test read:** one pre-registered read of phase error, logged in the ledger, on the held-out
  songs that have an aligned UST. These songs were spent for cell accuracy, but no phase estimate with
  this method had been made on them. Both conditions of the rule held: more songs within 0.1 cell, and
  a lower median.
- **The one miss:** cinema-mitaina is 0.4 cell off for every estimator, the full mix included. That
  points at the song's UST grid, not the estimator.

Cells correct with the learned stage. holdout3 was spent before this read, so it confirms nothing new;
it shows the size of the effect.

| | dev | holdout3 |
|---|---|---|
| sona's grid-note phase (ADR-0026) | 92.48% | 78.18% |
| **onset phase** | 92.69% | **78.84%** |
| the UST's true phase (an oracle the app can't have) | 92.80% | 78.83% |

maru-shite-kudasai, the song that caused the gap, goes from 78.6% to 85.9%.

## Consequences

- The cell stage now matches what it scores with the true phase, so phase is no longer a frontier for
  it.
- The rest of sona's grid mode still estimates its own phase inside the grid decoder. Whether onsets
  would help there too is untested.
