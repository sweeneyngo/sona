# Grid-decoder note assembly: TLA+ model

`GridNotes.tla` models how sona's grid mode ([ADR-0015](../../docs/adr/0015-grid-conditioned-cell-decoding.md))
turns a decoded cell path into notes, and how the review surface then snaps
them. TLC checks the rules on every path up to 5 cells.

| File | What |
|---|---|
| `GridNotes.tla` | The model. Each action names the function it stands for. |
| `GridNotes.cfg` | Decoder rules. These must hold. |
| `GridNotesSnap.cfg` | The review-surface snap on a dragged grid. Breaks today: a known finding. |
| `check.sh` | One command: rules, the known finding, and 6 mutants that must each be caught. |

```sh
verification/grid-notes/check.sh            # needs Java 11+ and tla2tools.jar 1.7.4 (TLA2TOOLS)
cargo test -p sona-core every_short_path    # the same rules on the real Rust reader
```

**Results (last run):**
- Decoder rules: no violation in 5,888,128 distinct states (22 s).
- Known finding: `SnappedMonophonic` breaks.
- Mutants: 6 of 6 caught.

**Covers:** `notes_from_path` and `fill_gaps` in `crates/sona-core/src/grid.rs`,
and `quantizeNote` in `web/lib/grid.ts` (= `quantize_note` in
`crates/sona-eval/src/quantize.rs`).

**Last confirmed at:** `b5391bb` (re-run after it; `notes_from_path` and `fill_gaps` unchanged since). Bump it when you re-confirm after changing a covered function.

**Known finding.** If the user drags the review grid ≥ ¼ cell off the decoded
phase, `quantizeNote`'s escape hatch can leave one note unsnapped next to a
snapped neighbour, and the two overlap. On the default anchor nothing moves.
