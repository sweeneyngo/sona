# Spec: estimate the grid phase from onsets, for the learned cell stage

Status: **rule committed before any held-out song's phase was estimated with the new method.**

## Question

The learned cell stage loses 0.65 points of cells when it uses sona's estimated grid phase instead of
the UST's true one (ADR-0026), nearly all of it on one song. Can a better phase estimate close that gap?

## Method (fixed on dev)

`sona_core::tempo::grid_phase`:
- log spectral flux at a 256-sample hop;
- the grid offset in `[0, spacing)` (240 steps) whose grid points collect the most flux over the whole
  song;
- an onset taken at 0.772 of the analysis window, measured on synthetic clicks.

It runs on the **vocals stem**, which the CLI and the server both have; the full mix scored the same on
dev. sona's grid decode on the vocals then runs on that phase (`GridMode::anchor_s`), so every candidate
sits on one grid.

## Data and metric

- **Phase error:** the circular distance from the UST's alignment phase, as a fraction of a cell, with
  the UST's exact tempo.
- **Dev:** the 9 dev songs, which were used to build the method.
- **Test:** the 32 held-out songs (holdout, holdout2, holdout3), read **once**. No phase estimate with
  this method has been made on them. The current estimator (the phase of sona's grid notes) is scored
  alongside on the same songs.

## Pre-registered decision rule

Adopt the onset phase as the cell stage's phase if, on the 32 test songs:
1. it is within 0.1 cell of the true phase on **at least as many songs** as the current estimator; and
2. its **median absolute error is lower**.

Then, reported but not part of the rule: the cell stage's cells on holdout3 with the new phase. holdout3
is spent, so this is a re-read and confirms nothing new; the ADR-0026 gap to close is 78.18% against
78.83%.
