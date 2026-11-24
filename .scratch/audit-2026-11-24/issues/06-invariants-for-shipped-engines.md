# 06 — The output invariants skip the engines sona recommends (GAME, GAME + learned stage)

Status: resolved
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

## Answer

Done on branch `audit-a06-invariants` (from a3f9d13). `crates/sona-core/tests/output_invariants.rs` now
drives the note assembly of both recommended engines with random inputs and asserts the existing
invariants (one voice: sorted, non-overlapping, non-empty, finite; MIDI pitch; scores in `0..=1`; on one
grid):

- **GAME + learned stage, no feature needed** (runs in `cargo test --workspace --release`):
  - `snap_notes` on 3000 random raw GAME note lists: one voice, every voiced note kept, every boundary on
    `anchor + k × spacing`.
  - `cells_to_notes` on 3000 random per-cell candidates, sources and classes (classes pointing at a missing
    candidate included): one voice, on the grid, and each cell lies inside a note exactly when it chose a
    pitch, and that note has the chosen pitch.
  - The whole stage minus the model on 300 random cases: `cell_features` (lead and vocals GAME notes with
    octave-check ratios, sona grid notes, an encoder chunk, f0, RMS) → random logits → `choose_class` →
    `cells_to_notes`. Output: one voice, on the grid, no rest emitted as a note.
- **GAME, `--features game`** (in a `game` module, so CI's existing
  `cargo test -p sona-core --release --features game game::` runs it; no `ci.yml` change):
  - `snap_to_grid` on 3000 random raw lists, half of them after `correct_octaves_with` with random ratios:
    one voice, every voiced positive-length note kept, on one grid, scores in `0..=1` from
    `ConfidenceScorer::score_all`.
  - The stage's real last step on 300 cases: `phase_of` on sona's notes is in `[0, spacing)` and they sit
    on it; `cells_covering` starts at or before 0 (within one cell) and reaches the end; the stage's notes
    pass through `stage_segments` and the scorer and are valid on that phase.

The generator mixes what the ticket listed: GAME-like back-to-back segments, rests, zero-length and
sub-millisecond notes, sub-cell notes, 10 ms frame boundaries, octave blips, a song-wide tuning offset,
some overlaps, stray pitches up to 127, and phases at exactly 0, one ulp under a cell and 1e-7 under a cell.

**Seam.** The CLI's cell-stage step dropped notes whose pitch isn't MIDI (a lead an octave up can pass 127)
inline before `grid_segments`. That one line is now `game::stage_segments`, and the CLI calls it. Behaviour
is unchanged. Without the seam, nothing in sona-core asserted the stage's MIDI range.

**Violations found: none.** No case broke an invariant in any of the five properties.

**Mutants** (each applied alone, test run, file restored):

| Mutant | Caught by |
|---|---|
| `snap_notes` without the collision branch | snapping property: "out of order" at case 0 |
| `cells_to_notes` keeps `prev` across a rest (a note extends over the rest) | cells → notes property: "cell 3 chose -1 but sits in (…, 60)" |
| `phase_of` returns `spacing` instead of 0 | stage property: "phase 0.0663… is outside one cell" at case 3 |
| `snap_to_grid` without the trim of the earlier note | `snap_to_grid` property: overlap at case 0 |
| `snap_to_grid` lets a note snap to zero length | `snap_to_grid` property: empty note at case 0 |
| `stage_segments` without the `<= 127` filter | stage property: `midi_pitch: 130` at case 8 |
| `cells_to_notes` ignores the anchor | cells → notes and whole-stage properties: off the grid |
| `snap_to_grid` without its sort | **survives, and is equivalent for these invariants.** After the walk handles note *i*, its onset is at or past note *i−1*'s offset, and a trim only shortens *i−1*, so the walk alone yields one voice in any input order. The sort only decides which of two colliding notes yields. Pinning that would be a behaviour test of the snapping rule, not an invariant. |

**Not done: item 2** (running `sequence_cases.json` through these walks). The fixtures pin `resolveOverlaps`
on notes that are already quantized and may be off the grid (the escape hatch, e.g. onset 0.25 at spacing
0.1), with input order kept. `snap_to_grid` and `snap_notes` snap first and then sort, so most cases
can't be fed to them as written. The fuzz checks the walk's postconditions directly instead. Factoring the
three walks into one function would change code on the shipped path for no invariant gain.

Verification: `cargo clippy --workspace --all-targets -D warnings` and the `--features game` clippy are
clean. The corpus guard passes. `cargo test --workspace --release` passes (284 passed, 0 failed, 4 ignored), and so does CI's game-filtered
run (7 `game::` tests including the 2 new ones). `rules.md` and `dev-workflow.md` now name the engines
covered.
