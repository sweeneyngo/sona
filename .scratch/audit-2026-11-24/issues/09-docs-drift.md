# 09 — Docs that disagree with the code (export timing, grid phase, CLI help, stale ADR lines)

Status: needs-triage
Type: task
Blocked by: — (the export lines change again with 01/02; fix them in the same commits)

## Evidence

| Doc says | Code does |
|---|---|
| `README.md:95`, `docs/adr/0018-scope-utau-authoring.md:15`: UST/USTX hold "the same snapped notes as the MIDI export" / "exactly the notes the MIDI export uses" | MIDI is not shifted by the grid phase, so its timing differs (ticket 01) |
| `CONTEXT.md:54` (Grid anchor): "overridable by dragging the grid offset … (needed for pickup notes / leading silence)" | Export keeps only the anchor modulo one cell, so a drag by a beat or more changes nothing in UST/USTX (ticket 02) |
| `CONTEXT.md:34`: the review surface has "BPM, grid, time signature" | There is no time signature anywhere in `web/` (USTX hard-codes 4/4, `web/lib/ustx.ts:71, 78`) |
| `CONTEXT.md:50` (grid mode): "An opt-in producer … The default path stays grid-free" | `sona serve` always transcribes in grid mode (`crates/sona-cli/src/jobs.rs:356` passes `grid: Some(..)`). README:202, 252 call grid mode "the default". Only bare `sona transcribe` is grid-free. |
| `docs/adr/0016-…:50`: "No out-of-sample confirmation yet. That is why both stay opt-in." | Line 3 of the same ADR: confirmed on holdout2 and the default since 2026-11-21 |
| `crates/sona-core/src/annotation.rs:9`: "A matching TypeScript declaration lives at `web/annotation.d.ts`; keep the two in sync" | No such file. The type is `web/lib/annotation.ts`, and no test pins it to the Rust struct (only `crates/sona-core/tests/annotation_fixture.rs`, which reads `tests/fixtures/annotation-example.json` on the Rust side) |
| `crates/sona-cli/src/serve.rs:178` (409 message): "see `sona serve --help`" | `sona serve --help` prints `unknown serve option '--help'`; `sona transcribe -h` prints `unknown option '-h'`. Only `sona --help` works, and the usage claims `-h, --help` |
| README "Usage" block (`README.md:130`) and the error usage line `crates/sona-cli/src/main.rs:363` | Omit `--cell-model` and `--vocals`, which `sona --help` documents |
| `crates/sona-cli/Cargo.toml:28`: the `spectral` feature is "for `--spectral-cue`" | `--spectral-cue` is a no-op (`main.rs:329-330`); the feature now gates tempo and the default cue |
| `README.md:96`: "the vocal starts up to one grid cell earlier than the stem" | True for UST/USTX, not MIDI (01) |

## Proposal

Fix each line. Add one CI-cheap guard where drift has a mechanism: a test that the TS `Annotation` type
loads `crates/sona-core` fixture JSON (the same file `annotation_fixture.rs` uses), replacing the stale
`annotation.d.ts` sentence.

## Cost (S/M/L) and how to tell it worked

S. It worked when each row is gone and the new annotation-fixture test fails on a renamed field (mutant).

## Comments
