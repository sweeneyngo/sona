# Rules for working on sona

What this project has learned, as rules. Where a rule can be checked, a check
enforces it; the list says which. The rest need judgment, so each one states the
failure it prevents.

## Enforced (CI: `.github/workflows/ci.yml`)

| Rule | Check |
|---|---|
| Never track corpus data: Pifuyuu's USTs (no redistribution), source songs, Vocadito. Only synthetic fixtures under `crates/*/fixtures/`. | `scripts/ci/check-no-corpus-data.sh` |
| Every segmenter emits one voice: sorted, non-overlapping, non-empty notes, MIDI pitch, scores in `0..=1`; grid-mode boundaries on one grid. Covers the note-HMM, pYIN grid mode, GAME (`snap_to_grid` after the octave check) and GAME + learned stage (`snap_notes`, `cell_features` → `choose_class` → `cells_to_notes` → `stage_segments`, on the grid phase `phase_of` gives). | `crates/sona-core/tests/output_invariants.rs`: random stems for sona's decoders; random raw GAME notes, per-cell choices and logits for GAME and the stage (no weights needed). The GAME tests sit in its `game` module, so CI's `--features game game::` run picks them up. |
| Grid note assembly and the review surface's snap keep the line monophonic and lose no note. | `verification/grid-notes/check.sh` (TLA+, mutants must be caught) and the exhaustive path test in `grid.rs` |
| The app's stateful UI keeps its promises: a typed lyric or pitch value lands on the note shown, a drag never overwrites a later edit, a job never runs without a usable tempo, a stale tempo answer never shows, an estimate never fills the BPM by itself, a job sent by hash names its own song and an upload sends its own file. | `verification/{lyric-editor,expr-bar,start-flow}/check.sh` (TLA+, mutants must be caught) |
| `sona serve`'s queue runs one job at a time, and a cancelled job never reports done. A panic or a hung tool fails only its job, and clean-up never deletes a queued or running job's folder. | `verification/serve-jobs/check.sh`, `crates/sona-cli/tests/serve.rs` |
| `web/lib/grid.ts` and its Rust port `crates/sona-eval/src/quantize.rs` agree. | shared `crates/sona-eval/fixtures/quantize/*.json`, asserted by both test suites |
| The editor does what a person expects: handles, the pen, lyrics, bar 1, undo/redo, exports, autosave, motion, the Help and the grid menus behave as `web/e2e/README.md` lists, with no console errors. | `scripts/ci/e2e.sh` (job `e2e`): the eleven editor drivers in Chromium against a Vite dev server, on the synthetic fixture. The three that need `sona serve` still run locally only. |
| No clippy warnings. | `cargo clippy --workspace --all-targets -- -D warnings` |
| A holdout read is deliberate and logged; a re-read of the same songs is refused without `--reread`. | `crates/sona-eval/src/holdout.rs`, ledger `crates/sona-eval/corpus/holdout-reads.jsonl` |

## Judgment (not checkable; each has bitten before)

- **Tune on `dev`, confirm on the holdout once.** Before reading the holdout, write the decision rule (what result ships, what doesn't) in the feature's `.scratch/` spec and commit it. The ledger makes the read visible; it can't make it honest. *Failure prevented:* a tuning loop that quietly fits the songs it is scored on.
- **Explain the number before trusting it.** For every eval result, list what else it could be measuring (leaked anchors or BPM, cached work, a matcher artefact, two outlier songs) and rule each out. State song-bootstrap intervals; with 9 songs, a ±0.01 change is noise. *Failure prevented:* shipping a gain that is an artefact.
- **Measure an oracle before building.** Before a mechanism, compute the best any version of it could do (e.g. "UST boundaries, pitch from our contour": 0.91). Lyric counts looked promising and were capped by placement; the oracle said so before the code did. *Failure prevented:* weeks on a lever that can't move the number.
- **Keep the default path byte-identical unless the change is the point.** New behaviour is opt-in (`Transcriber::grid`, `GridDecoder` fields) until a holdout read says otherwise. Check by re-running the default gate and comparing against the committed report. *Failure prevented:* a Vocadito regression hidden inside an in-domain gain.
- **Tests claim behaviour, not implementation** (pstack `principle-test-behavior-not-implementation`). Drive the public interface with concrete inputs and derive expectations from the requirement. Before keeping a new invariant or test, break the code once and watch it fail. *Failure prevented:* tests that pass whatever the code does.
- **Corpus-prep tools stay outside the workspace.** demucs and audio-separator live in their own venvs and never become dependencies of a crate (ADR-0005: no ML runtime in the product).
- **Record provenance for any stem.** The separation model and version go beside the stem (manifest `stem` field, `lead.provenance.json`). Different models leave different bleed, so unrecorded changes silently invalidate comparisons.

## Commands

The whole loop, from setup to shipping, is in [`../dev-workflow.md`](../dev-workflow.md).

```sh
scripts/ci/check-no-corpus-data.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --release
(cd web && npx tsc --noEmit && npx vitest run)
for m in verification/*/check.sh; do "$m" || break; done   # Java 11+, tla2tools.jar 1.7.4
scripts/ci/e2e.sh                                           # editor browser drivers, as CI runs them; see web/e2e/README.md
```
