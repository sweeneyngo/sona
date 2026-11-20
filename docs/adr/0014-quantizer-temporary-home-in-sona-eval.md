# Hard-snap quantizer duplicated into sona-eval (temporary)

**Status: Accepted, explicitly temporary.** The duplication ends when the quantizer is promoted (below). It must not become quietly permanent.

## Context

Hard-snap quantization ([ADR-0003](0003-hard-snap-quantization-for-vocaloid.md)) exists only in the review surface, `web/lib/grid.ts`. That's deliberate. Snapping is *interpretation*, and the core emits raw perception ([ADR-0004](0004-core-emits-raw-perception-ui-interprets.md)).

The UST corpus gate (`.scratch/ust-corpus-gate/`) scores sona's **deliverable**, which is snapped output, against a tick-quantized UST. The question it asks is how much of the raw offset gap quantization absorbs. Answering that needs quantization in Rust, inside the harness.

## Decision

Port `gridSpacingSeconds` / `snapTime` / `quantizeNote` to **`crates/sona-eval/src/quantize.rs`**, faithfully, including the confidence-aware escape hatch and JavaScript's `Math.round` half-rounding.

- **sona-eval, not sona-core.** The core's raw-perception contract stays intact. The duplication is confined to the harness, which never ships.
- **Drift guard.** One shared case file, `crates/sona-eval/fixtures/quantize/cases.json`, is asserted by both `quantize.rs` and `web/lib/grid.test.ts`. A change to either implementation that the other doesn't mirror fails a test.
- **Written to be promotable.** It is pure functions over plain numbers, `(onset, offset, onset_clarity)` plus `(spacing, anchor)`, depending on nothing in sona-eval (std only).

## Promotion path

Promote when a second Rust consumer needs snapped output, for example the headless snapped-MIDI path (out of v1 scope per CONTEXT.md), or when the UI is ready to take quantization over WASM:

1. Move `quantize.rs` into `sona-core` (still a separate interpretation module; `Transcriber` keeps emitting raw seconds) and re-export it from sona-eval.
2. If the UI adopts it via WASM, delete `grid.ts`'s copy and keep `cases.json` as the core's own regression set.

Either step removes the duplication. Until then, `cases.json` is the contract.

## Consequences

- Two implementations exist and must be changed together. The shared cases make forgetting one a test failure rather than silent drift.
- **Grid choice is per song, not built into the quantizer.** The UST audit (`reports/ust-format-audit.md`) found 15% of real note starts on triplet lines (ノイズゴト is 96%). Both implementations already take any `spacing`, so a triplet grid is just `division = 24` (or 12). Which grid a song uses is recorded per song in the corpus manifest, not hard-coded to the 16th default.
