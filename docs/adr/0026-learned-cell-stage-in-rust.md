# ADR-0026: The learned cell stage, ported to Rust

Status: accepted (2026-11-23). The grid phase (decision 3) amended by [ADR-0027](0027-grid-phase-from-onsets.md):
it comes from the vocals' onsets.

## Context

ADR-0017 confirmed a learned cell stage on holdout3: **78.78%** of grid cells against 76.62% for its own
GAME-lead column (+2.2, 95% interval +0.8 to +3.5). For every grid cell, a two-layer bidirectional GRU
chooses rest or one of five candidate pitches:
- GAME's notes on the lead;
- GAME's notes on the vocals;
- sona's grid notes;
- the lead an octave up;
- the lead an octave down.

It existed only as a Python prototype. Its inputs came from Python tooling: its own GAME runner, which
cached encoder features at half precision, and a Python octave check. So the best confirmed result was
not available in sona's app or CLI.

## Decision

1. **Features in Rust** (`sona_core::cellstage`), a line-for-line port of the prototype's builder. It
   reproduces Python's round-half-to-even, NumPy's median and left-side `searchsorted`, and the half
   precision of the cached encoder features.
2. **The model through ONNX Runtime** (`game::CellModel`), exported from the PyTorch checkpoint by
   `crates/sona-eval/scripts/cellstage/export_onnx.py`, with the feature normalisation inside the graph.
   The weights derive from Pifuyuu's USTs and from GAME (CC BY-NC-SA), so like GAME's weights they are
   built and kept locally and never committed.
3. **End to end** (`game::StageRuns`, `cell_stage_notes`): GAME on the lead (octave-corrected) and on the
   vocals, sona's grid mode on the vocals. **The grid phase is estimated** from sona's grid notes (since
   ADR-0027, from the vocals' onsets); the prototype used the UST's true phase, which a user can't have. Cells become notes: a note holds while
   the pitch does, and splits at a rest, a pitch change, or where the chosen source itself starts a new
   note on the same pitch.
4. **Wired in:**
   - CLI: `sona transcribe LEAD.wav --game DIR --cell-model DIR --vocals VOCALS.wav`.
   - Server: `sona serve --cell-model DIR` offers an engine `cells`, which needs lead separation, since
     that also yields the vocals.
   - App: when the server offers it, the start screen uses it by default ("GAME + learned").
   - Provenance records the engine, the model's and the vocals stem's hashes.

## Evidence

Pre-registered in `.scratch/cellstage-port/spec.md` before the port saw any holdout3 song.

- **Model:** ONNX Runtime equals PyTorch on every holdout3 cell (17 623 of 17 623 decisions; max logit
  difference 6e-6), and both equal the predictions recorded at the original read.
- **Features:** Rust equals the prototype on identical inputs (3 songs, 5 124 cells: every candidate
  exact, max feature error 1.25e-8).
  - Mutants for round-half-even, left `searchsorted`, the even-length median, beat phase and tuning
    are each caught, after the tolerance was tightened to 1e-6 and two edge-case unit tests were
    added.
- **holdout3 parity read** (spent, logged as a re-read; it confirms nothing new):

  | | GAME lead | learned stage | gain |
  |---|---|---|---|
  | prototype, true phase (ADR-0017) | 76.62% | 78.78% | +2.17 |
  | **port, true phase** | 76.63% | **78.83%** | +2.20 |
  | **port, estimated phase (ships)** | 76.17% | **78.18%** | +2.01 |

  All three expectations hold: the port is within ±1.0 of the prototype (+0.05); estimating the phase
  costs 0.65 (limit 1.0); the stage beats its GAME-lead column by 2.0 or more on either phase.
  - **Where the phase costs:** almost all of it is one song, maru-shite-kudasai (85.4% → 78.6%), where
    sona's phase estimate is off. Better phase estimation is now a roadmap item.
- **In the app:** a 25 s song takes 97 s with the stage against 81 s with GAME alone (one more GAME run
  and pYIN on the vocals). The browser flow defaults to it and loads without errors.
- **Review (comment-sicko pass) found three bugs, fixed after the parity read:**
  - a cell starting one floating-point step before a chunk's end read one frame past it (a crash);
  - the hand-written half-precision conversion was wrong at the very small and very large ends (now the
    `half` crate);
  - a grid phase of exactly 0 could come out as a whole cell, renumbering the cells and so shifting the
    beat-phase features.

  It also flagged a hazard: the server advertised the stage when only the `.onnx` file existed. Each
  bug has a regression test. Prototype parity is unchanged (exact), and a dev song scores identically
  before and after the fixes. The rare edge cases these fixes change didn't arise in the parity read.
- **Tests:** feature unit tests and cells-to-notes run in CI. The prototype-parity test and GAME tests
  need local data or models and are `#[ignore]`. The CLI rejects `--cell-model` without `--game` and
  `--vocals`, and the server refuses `engine=cells` without a model.

## Consequences

- sona's best confirmed configuration now runs in the app: about 78% of cells on fresh songs, against
  72.5% for the default pYIN grid mode.
- **Rebuilding the model** still goes through the prototype's training scripts. They are kept with the
  model in `~/.cache/sona/cellstage-D/`, and their feature builder depends on the Python GAME runner. A
  Rust feature dump for training is a follow-up (roadmap 1.7).
- The stage needs both the vocals and the lead stem, so it only runs with lead separation.
