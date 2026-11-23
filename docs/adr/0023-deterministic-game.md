# ADR-0023: GAME gives the same notes for the same song, every time

Status: accepted (2026-11-23)

## Context

The same stem gave different GAME notes from run to run: 68, 69 and 70 notes on three runs of a 25 s clip.
That cost about 0.3–0.7 points of cell accuracy between runs and made every GAME measurement noisy
(`crates/sona-eval/reports/game-finetune.md`). Two causes turned up, and only the first was known
before this work:

1. **An unseeded random node.** GAME's D3PM segmenter samples which boundaries to drop with an unseeded
   `RandomUniformLike`, and ONNX Runtime seeds it differently in each process.
2. **Thread-dependent arithmetic.** With the seed fixed, runs on an idle machine agreed. Under CPU load
   they did not (68 or 69 notes on four runs): the default kernels split work across threads
   differently, the last bits of the floats changed, and boundary decisions near the threshold
   flipped.

## Decision

- **Seed the segmenter at load, in memory.** `sona_core::onnx_seed::seed_random_nodes` walks the ONNX
  protobuf (model, graph, node) and gives each random node `seed = 1.0` (`GameParams::segmenter_seed`).
  The value was fixed before any seeded result was looked at. Your downloaded files are not modified,
  and no ONNX dependency is added.
- **Start every transcription from the seed.** ONNX Runtime advances a seeded generator across runs of
  one session. So the segmenter session is rebuilt from the seeded bytes at the start of each
  `transcribe`, and a song's notes don't depend on what ran before it in `sona serve`, the harness or
  the library.
- **Deterministic kernels** (`SetDeterministicCompute`) on every GAME session.
- **The harness's GAME cache key includes the seed**, so stale unseeded results are never reused.

## Evidence

- **Patcher:**
  - unit tests (in CI) cover seeding, replacing an existing seed, leaving a model without random nodes
    byte-identical, and refusing a truncated file;
  - on the real GAME 1.0.3 segmenter, the output passes `onnx.checker`, has exactly one seed (1.0),
    every other node and every initializer unchanged, and is the same graph as the Python-seeded copy
    used in the fine-tune evaluation.
- **Same notes:**
  - CLI, idle: 71 notes, identical over 3 runs (before: 68, 69 and 70);
  - CLI under load: 71 notes, identical over 4 runs, and the same as idle;
  - in one process, song A, then song B, then song A again gives identical notes, and matches a freshly
    loaded model;
  - several `sona transcribe` processes at once all agree (`crates/sona-cli/tests/game_determinism.rs`).
- **Mutants:**
  - dropping the per-transcription restart fails the in-process test;
  - dropping the seed fails the cross-process test;
  - removing deterministic kernels is **not caught by an automated test**. Three concurrent processes,
    even with every core busy-looping, didn't flip a boundary. The evidence is the natural experiment
    above: the same binary under the dev evaluation's load gave four different results without the
    flag (68/69 notes), and four identical results matching idle with it (71 notes).
- **Dev baseline, seeded and deterministic:** cells **87.1%**, COnP F1 0.770 (9 songs, lead stems,
  `game=0.8,oct=1`). This is inside the 87.0–87.4 range of the five seeds measured for the fine-tune,
  so the fix is accuracy-neutral and the 87% dev figure stands.

The two determinism tests need ONNX Runtime and a downloaded GAME release, so they are `#[ignore]` in CI.
Run them with `ORT_DYLIB_PATH` and `SONA_GAME_DIR` set (commands are in their headers).

## Consequences

- GAME results are now exactly reproducible, so A/B comparisons no longer need several seeds per arm.
- Rebuilding the segmenter session costs a moment per transcription, negligible beside inference.
  Deterministic kernels cost nothing measurable: the 9 dev songs took 19 min, as the unseeded runs
  did.
- The holdout results recorded in ADR-0017 were unseeded single runs. They stand, with the noise noted
  there.
