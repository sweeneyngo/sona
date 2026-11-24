# 15 — Code-side leftovers from the docs pass (A09)

Status: resolved
Type: task
Blocked by: 08 (it is moving web/main.ts code; do this after)

## Evidence (A09's Answer)

1. `crates/sona-cli/src/main.rs`: the "transcribe needs a <STEM>" error prints a usage line without `--game`,
   `--no-octave-check`, `--cell-model`, `--vocals`, `--bar-one`; the doc comment on `parse_transcribe_args`
   has the same gap.
2. The `USAGE` footer says the web surface does "MIDI export" and omits UST/USTX.
3. No test ties `web/lib/annotation.ts` to the Rust `Annotation` struct.
4. The in-app Help (`web/index.html`) still says "sona suggests its tempo; check it" (pre-ADR-0028) and doesn't
   mention bar 1 (ADR-0029/0031) or the edit/export workflow changes.

## Proposal

Generate the short usage line from the same table as `--help` (one source); fix the footer; add a vitest that
parses a Rust-written annotation fixture (`crates/sona-core/tests/fixtures/annotation-example.json` or a new
one written by a Rust test) with `web/lib/annotation.ts` and fails on a renamed field (show the mutant);
rewrite the Help copy for BPM-first, bar 1 and the setup notes, short, in the existing voice.

## How to tell

CLI test asserting the error's usage line names every flag; the annotation contract test fails on a renamed
field; a browser driver or snapshot check of the Help text.

## Comments

## Answer

Done on branch `audit-a15-a16` (from 228fecd).

1. **One usage source.** `crates/sona-cli/src/main.rs` has a `transcribe_synopsis!` literal naming every
   transcribe option; `USAGE` (`--help`) and the "transcribe needs a <STEM> path" error are both built from
   it with `concat!`, and the doc comment on `parse_transcribe_args` points at it.
   Test `the_missing_stem_error_names_every_transcribe_option_as_help_does` (`tests/cli_transcribe.rs`):
   `--help`'s OPTIONS (transcribe) lists exactly the ten options the parser takes, the error's usage names
   each one, and the error's usage appears verbatim in `--help`.
   Mutants: the old hand-written error line fails it ("the usage misses --game"); dropping
   `[--bar-one <MIX>]` from the synopsis fails it ("the usage misses --bar-one").
2. **Footer:** "…interprets it (grid, quantization, correction) and exports UST, USTX and MIDI". The
   README's quoted OPTIONS block is unchanged (that part of `--help` didn't change).
3. **Annotation contract.** `full_annotation_matches_the_contract_fixture` (`sona-core`, `annotation.rs`)
   builds an annotation with every field (provenance with job and stem source, `bar_one`, notes with
   sub-scores, f0, energy) and checks it against `crates/sona-core/tests/fixtures/annotation-full.json`
   byte for byte (`UPDATE_GOLDEN=1` rewrites it, as `export.test.ts` does). `web/lib/annotation.test.ts`
   reads that file with the app's loader (`normalizeAnnotation`) and compares it to the same annotation
   written as a literal of `annotation.ts`'s `Annotation` type.
   Mutants:
   - Rust `#[serde(rename = "level")]` on `EnergyFrame::rms`: the Rust test fails until the fixture is
     regenerated; regenerated, the vitest fails (`- "rms": 0.12 / + "level": 0.12`). `rms` is passed
     through untouched by the loader, so this is the case a typed-only check would miss.
   - Rust rename of `StemSource::separated_by`: vitest fails (`separated_by: "j0"` read as `""`).
   - TS rename of `EnergyFrame.rms` in `annotation.ts`: `tsc` fails on the test's literal (TS2353).
4. **Help** (`web/index.html`) rewritten in the same voice: type the BPM (sona's guess is a hint, ADR-0028);
   find bar 1 by Shift-clicking the ruler; a "Tempo, grid and bar 1" section (♩ → Bar 1, At playhead,
   Auto, bars numbered as OpenUtau numbers them); "From a song" covers the setup notes (A12) and the
   **Bar 1 from the music** option (ADR-0031); the export set is USTX, USTX with the singer's pitch, UST and
   MIDI; stale labels fixed (`あ Lyrics` → Edit → Lyrics…, `〰 Fit pitch` → Edit → Fit pitch, "More
   options" → Options); the pitch pen and its `P` key added. Checked by a new browser driver,
   `web/e2e/help_and_grids.py` (in `run_all.sh`, so CI's `e2e` job runs it): the Help names every
   Export-menu item and says "type its BPM", "Shift-click", "At playhead", "Bar 1 from the music" and the
   setup line, and no longer "suggests its tempo". Mutant: the driver run against 228fecd's `index.html`
   fails six of those checks.

Also: the module docs in `annotation.rs`/`annotation.ts`, `web/README.md` and the README point at the
contract test; `web/README.md`'s module list gains `interpret.ts`, `noteEdits.ts` and `session.ts` (A08).
The editor-driver count is now eleven in README, rules.md, dev-workflow.md and roadmap 6.4.

**Verified:** clippy `-D warnings` clean; `cargo test --workspace --release` 309 passed, 0 failed; `tsc --noEmit` clean, vitest 183/183 (22 files); `scripts/ci/e2e.sh` 11/11 editor drivers; the five TLA+ checks pass; corpus guard ok.
