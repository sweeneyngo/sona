# Grid-decoder note assembly: TLA+ model

`GridNotes.tla` models how sona's grid mode ([ADR-0015](../../docs/adr/0015-grid-conditioned-cell-decoding.md))
turns a decoded cell path into notes, and how the review surface then snaps
them. TLC checks the rules on every path up to 5 cells.

| File | What |
|---|---|
| `GridNotes.tla` | The model. Each action names the function it stands for. |
| `GridNotes.cfg` | Every rule. All must hold. |
| `check.sh` | One command: the rules, plus 7 mutants that must each be caught. |

```sh
verification/grid-notes/check.sh            # needs Java 11+ and tla2tools.jar 1.7.4 (TLA2TOOLS)
cargo test -p sona-core every_short_path    # the same rules on the real Rust reader
```

**Results (last run):**
- All 8 rules: no violation (NCells = 5).
- Mutants: 7 of 7 caught.
- The `resolveOverlaps` collision branch is unreachable from grid-decoded input, so `web/lib/grid.test.ts` covers it.

**Covers:** `notes_from_path` and `fill_gaps` in `crates/sona-core/src/grid.rs`,
`quantizeNote` and `resolveOverlaps` in `web/lib/grid.ts`
(= `quantize_note` and `resolve_overlaps` in `crates/sona-eval/src/quantize.rs`).

**Last confirmed at:** `b5391bb` (re-run after it; `notes_from_path` and `fill_gaps` unchanged since). Bump it when you re-confirm after changing a covered function.

**History.** The first run found that snapping a dragged grid could overlap two
neighbours: one note snapped while its confident neighbour escaped. That was
fixed by `resolveOverlaps`, and the rule `SnappedMonophonic` now holds.
