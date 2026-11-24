# 15 — Code-side leftovers from the docs pass (A09)

Status: ready-for-agent
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
