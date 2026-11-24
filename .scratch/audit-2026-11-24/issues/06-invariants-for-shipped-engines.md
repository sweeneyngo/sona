# 06 — The output invariants skip the engines sona recommends (GAME, GAME + learned stage)

Status: needs-triage
Type: task
Blocked by: —

## Evidence

- `docs/agents/rules.md` (Enforced table): "**Every segmenter** emits one voice: sorted, non-overlapping,
  non-empty notes, MIDI pitch, scores in 0..=1; grid-mode boundaries on one grid", checked by
  `crates/sona-core/tests/output_invariants.rs`.
- That test covers two segmenters only, the note-HMM and pYIN grid mode (`output_invariants.rs:1-7, 97-146`).
  The note assembly of the engines the README and start screen recommend is not covered:
  - `game::snap_to_grid` (`crates/sona-core/src/game.rs:562`), the GAME engine. It has 2 hand-made
    snapping cases (`game.rs:643, 666`).
  - `cellstage::snap_notes` (`cellstage.rs:73`) and `cellstage::cells_to_notes` (`cellstage.rs:246`), the
    "GAME + learned" engine, which is the app's default when offered (ADR-0026). Each has one or two
    example tests (`cellstage.rs:285-330`).
- All three are **pure**: they take note lists and logits and need no ONNX. CI can therefore fuzz them,
  even though GAME itself can't run there (its weights are user-downloaded, `ci.yml` runs only
  `game::` unit tests).
- They repeat the overlap rule of `web/lib/grid.ts` `resolveOverlaps`: `cellstage.rs:87-96` is the same
  overlap/collision walk. They are not asserted against the shared `crates/sona-eval/fixtures/quantize/
  sequence_cases.json` that pins the Rust and TS quantizers together (rules.md: "grid.ts and quantize.rs
  agree").
- `snap_notes` and `snap_to_grid` already had edge bugs of exactly this shape: ADR-0026 lists "a grid phase
  of exactly 0 could come out as a whole cell" and a chunk-edge read, found by review, not by a test.

Why this matters for the goal: an overlapping or zero-length note in the shipped stack becomes a corrupt UST
block or a chopped note in OpenUtau, the spec's tedious tier. The invariant everyone relies on is not
checked for the output users actually get.

## Proposal

1. Extend `output_invariants.rs` with random GAME-like note lists (voiced flags, sub-cell boundaries, octave
   blips, near-zero lengths, phases at 0 and just under one cell) through `snap_to_grid`, and random
   class/candidate/source arrays through `snap_notes` + `cells_to_notes`. Assert the same invariants: one
   voice, sorted, non-empty, on one grid at the requested spacing and phase.
2. Run the `sequence_cases.json` fixtures through `cellstage::snap_notes`'s overlap walk, or factor the walk
   into one Rust function that all three call, asserted by the fixture.
3. Mutants that must be caught: drop the collision branch in `snap_notes`; let `cells_to_notes` extend a
   note across a rest; return `spacing` instead of 0 for phase 0.
4. Update the rules.md row to name the engines it covers.

## Cost (S/M/L) and how to tell it worked

S. It worked when the new properties run in CI (`cargo test --workspace --release`, no features needed for
`cellstage`; `--features game` for `snap_to_grid`, which CI already builds) and each mutant fails them.

## Comments
