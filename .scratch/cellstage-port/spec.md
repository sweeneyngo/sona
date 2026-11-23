# Spec: is the Rust cell stage the prototype that was confirmed on holdout3?

Status: **written before the Rust port has seen any holdout3 song.**

## What this read is (and isn't)

holdout3 is spent: it confirmed the prototype (78.78% cells against 76.62% for its GAME-lead column,
ADR-0017). This read **confirms nothing new**. It checks that the port behaves like the prototype on the
same songs, logged as a re-read.

Already shown, exactly:
- the ONNX model equals the PyTorch model on every holdout3 cell (17 623 of 17 623 decisions);
- the Rust features equal the prototype's on identical inputs (3 songs, 5 124 cells, max error 1e-8).

What can still differ:
- GAME's notes (the prototype's runs were unseeded; the port is seeded, ADR-0023);
- the chunking (Rust cuts can land one 50 ms frame from the Python ones);
- the grid phase. The prototype used the UST's true phase; what ships has to estimate it.

## Expectations (the port is accepted if all hold)

1. **Stage, UST phase:** within ±1.0 point of the prototype's 78.78%.
2. **Stage, estimated phase** (what ships): no more than 1.0 point below the stage on the UST phase.
3. **The stage beats its own GAME-lead column** on the same phase, by at least +1.0 point.

If (1) fails, the port or its inputs differ, and the cause gets found before anything ships. If (2) fails,
the prototype's gain leaned on the true phase, which is recorded as a finding; the stage then stays an
experiment.
