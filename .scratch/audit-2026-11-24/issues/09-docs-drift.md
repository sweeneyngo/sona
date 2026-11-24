# 09 — Docs that disagree with the code (export timing, grid phase, CLI help, stale ADR lines)

Status: resolved
Type: task
Blocked by: 01, 02, 04, 05, 07

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

## Answer

Resolved 2026-11-24 on branch `audit-a09-docs` (from 52d2851). Docs only: the Rust edits are doc comments
(`//!`), no behaviour changed. Checked against `cargo build --release` and `./target/release/sona --help`,
`sona serve --help`, `sona transcribe -h`; corpus guard passes.

### The ticket's rows

| Row | Outcome |
|---|---|
| README:95 / ADR-0018:15, MIDI = UST/USTX notes | Already fixed by A01/A02 (ADR-0029); checked, no edit |
| CONTEXT:54, phase drag for pickups | Fixed (below) |
| CONTEXT:34, time signature | Fixed (below) |
| CONTEXT:50 + README "the default" | Fixed (below) |
| ADR-0016:50 vs its status | Fixed (below) |
| `annotation.rs:9` names `web/annotation.d.ts` | Comment fixed; the TS fixture test is left for code (web is being refactored by another agent) |
| `sona serve --help`, `sona transcribe -h` | Already fixed in code: both print the usage and exit 0, so serve.rs's 409 hint is true |
| README usage block omits `--cell-model`, `--vocals` | Fixed; the error usage line in `main.rs` is left for code |
| Cargo.toml `spectral` comment | Already fixed (now "grid mode's spectral cue and tempo") |
| README:96, "one cell earlier" | Already true for all three formats since ADR-0029 |

### Fixes (file: before → after)

- README: usage block hand-written, missing `--cell-model`/`--vocals` → the real `OPTIONS (transcribe)` text from `sona --help`, saying `sona transcribe --help` and `sona serve --help` print it too
- README: "takes a **mono WAV vocal stem**" → "a WAV vocal stem (stereo is averaged to mono)" (`load_wav_mono`)
- README: serve options `--port, --demucs-venv, --separator-venv, --game` → adds `--web-dir`, `--cell-model`, `--jobs-dir`
- README: "GAME is greyed out: build with `--features game` + `ORT_DYLIB_PATH`" → also needs `sona serve --game DIR`; GAME + learned needs `--cell-model DIR` (`cellstage.onnx` + `.json`) and lead separation (`capabilities()` in serve.rs)
- README: annotation JSON lists four parts → also names the optional `provenance` and `bar_one`, and `web/lib/annotation.ts`
- README: "All numbers come from pre-registered rules, read once" (but dev rows are tuned) → "Every fresh-test number …; dev numbers are where the tuning happened"
- README: learned stage dev cell "—" → "— (trained on them)" (`.scratch/game-holdout3/spec.md`: trained on all 28 dev + spent-holdout songs)
- README: new bullet, edit cost on dev: 180.2/min, 23.8% clean measures; pYIN 338.4, 3.1%; joined = half; flagged as in-sample for the stack (source: `.scratch/utau-edit-cost/issues/01-edit-cost-metric.md`)
- README: "grid mode, pYIN (the default)" → "(what a fresh install runs)"; "now the default" → "now grid mode's default"
- README How it works: "the default is grid mode … the note-HMM serves grid-free material" → the app always runs grid mode; `sona transcribe` without `--grid` runs the note-HMM; learned stage named (ADR-0026)
- README Development: "The browser drivers run locally" → CI runs the ten editor drivers (`scripts/ci/e2e.sh`), the two serve drivers run locally (A13)
- CONTEXT Annotation JSON: "including basic-pitch via an adapter" → adds optional provenance/`bar_one` and both type files; drops the never-built adapter
- CONTEXT Comparison harness: pYIN, CREPE and basic-pitch auditioned by ear → only pYIN and GAME exist (CREPE and the adapter never built, ADR-0001); compared by metrics since ADR-0011
- CONTEXT Outperform: "judged by ear … _Avoid_ mir_eval/COnP/COnPOff" → measured since ADR-0011 (COnP on Vocadito, cells, now edit cost)
- CONTEXT Review surface: "BPM, grid, time signature … evaluation is by ear" → "bars (4/4 only; no time-signature setting)"; numbers come from the harness
- CONTEXT Tempo: "user-supplied for v1; _Avoid_ beat tracking" → typed by the user (ADR-0028), estimate a hint (ADR-0022), the mix's beat tracked only for bar 1, opt-in (ADR-0031)
- CONTEXT Grid mode: "An opt-in producer … The default path stays grid-free" → `sona serve` runs it on every job (`jobs.rs` `grid: Some(..)`); `sona transcribe` only with `--grid`
- CONTEXT Grid anchor: "drag the grid offset (needed for pickup notes / leading silence)" → a ruler drag pins bar 1 and carries it (`frame.ts` `drag`); before any drag or Shift-click only the sub-cell phase reaches the export; pickups are placed by bar 1
- roadmap: "Where sona stands on 2026-11-23" → 2026-11-24; Where things stand rewritten: learned stage in Rust and in the app where its weights exist (ADR-0026/0027), new Edit cost bullet (180.2 [115.9, 256.5] /min, 23.8% [13.8, 33.9] clean, joined 53%, in-sample caveat; utau 01), Tempo and bar 1 (BPM first, ADR-0029 frame, ADR-0031 opt-in), App (BPM first, undo, stem cache)
- roadmap 1.2: "a BowlRoll fetcher …, ≥ 10 songs" → Pifuyuu's OneDrive share (460 zips, 42 used; BowlRoll's terms forbid it), ≥ 20 songs, utau 08
- roadmap 2.3: open "grid phase from the mix (downbeats)" → done opt-in (ADR-0031: 5/9 → 9/9, circular for the bar); default waits for ear-labelled holdout4
- roadmap 3.4: "MIDI export is notes only" → notes, lyrics and 4/4 (ADR-0029), no pitch
- roadmap 6.3: open as written → A12's part done; left: GAME/cell-model greying, out of disk; path `tests/serve.rs` → `crates/sona-cli/tests/serve.rs`
- roadmap current effort: "the list below" → "above"; adds what is done (utau 01, 02, 04, 05, 10) and next (03, 07, 08)
- roadmap top three: "the app now runs the 78% stack" → "where its weights exist"
- dev-workflow: metric "cell accuracy" → the edit cost since 2026-11-24, cells alongside; adds `sona-eval-frame` (`--estimate-bar-one`)
- ADR-0002: no status → accepted; refined by ADR-0004, widened by ADR-0018
- ADR-0004: no status → amended by ADR-0015 (grid mode), used for every serve job (ADR-0020)
- ADR-0007: no status → superseded by ADR-0010; its knobs no longer exist
- ADR-0015: "opt-in only; default path unchanged" → `sona transcribe` without `--grid` unchanged; serve runs grid mode on every job (ADR-0020); salience and cue default (ADR-0016)
- ADR-0016: "Two opt-in evidence tracks", "No out-of-sample confirmation yet. That is why both stay opt-in", "Default output is unchanged … `None`/`VoicingEnergy` by default" → each framed as true when decided, with the holdout2 confirmation and today's `GridDecoder::default()` (`candidates: Some(..)`, `CueModel::Spectral`)
- ADR-0017: adds "Later: seeded (0023), ported (0026), onset phase (0027)"; "Learned cell stage (prototype, not yet ported)" → "ported in ADR-0026"; "holdout4 needs a BowlRoll fetcher" → OneDrive share (utau 08); run-to-run noise → "Seeded since ADR-0023"
- ADR-0018: status gains "Grid alignment amended by ADR-0029"; "Source separation is planned but not yet built" → "(since built: ADR-0020)"
- ADR-0020: status gains: ＋ From audio replaced by ADR-0022's start screen (BPM first, ADR-0028); `engine=cells` (0026), `bar_one=1` (0031), `POST /api/tempo` (0022)
- ADR-0026: status gains "grid phase amended by ADR-0027"
- ADR-0029: status gains "amended by ADR-0031: with `bar_one`, bar 1 starts there and Auto returns to it"
- ADR-0030: "(Numbered 0030 on its branch; the number may change at merge.)" → removed
- verification/expr-bar/README: `exprBarNote` (gone from `web/main.ts`) → `noteShownInExprBar` in the code, `exprBarNote` in the model
- reports `note-onset-gate.md`, `onset-compare.md`, `release-sweep.md`: Run lines name deleted bins → each notes the bin was removed in A07
- `crates/sona-core/src/annotation.rs` (doc comment): `web/annotation.d.ts` → `web/lib/annotation.ts`, "no test checks it yet"
- `crates/sona-cli/src/main.rs` (doc comment): "one subcommand and three options" → two subcommands (`serve` too) with about a dozen options each
- `crates/sona-eval/src/bin/ust_dev.rs` (doc comment): cue "needs `--features spectral`" → "the `spectral` feature (on by default)"

Checked and left alone: rules.md (every check, path and function it names exists), dev-workflow's tools and
flags, web/e2e/README (12 drivers, 10 + 2), the other verification READMEs (mutant counts match each
`check.sh`), README's UI labels, keys and numbers (each traced to its ADR or `web/index.html`), ADR history
sections that are dated as history.

### Left for code changes

1. `crates/sona-cli/src/main.rs`, `parse_transcribe_args`: the "transcribe needs a <STEM>" error prints a usage
   line without `--game`, `--no-octave-check`, `--cell-model`, `--vocals`, `--bar-one`; the doc comment above
   the function lists four flags. Print `USAGE` (or point at `sona --help`) instead.
2. `USAGE` footer: "the web review surface interprets it (… MIDI export)" leaves out UST/USTX.
3. No test pins `web/lib/annotation.ts` to the Rust struct. The ticket's proposal stands: a vitest that loads
   `crates/sona-core/tests/fixtures/annotation-example.json` as `Annotation`, shown failing on a renamed
   field. Not done here: web code is out of scope while `web/main.ts` is being split.
4. In-app Help (`web/index.html`, Workflow): "sona suggests its tempo; check it (½× if it sounds twice too
   fast)" predates ADR-0028 (you type the BPM; the estimate is a hint) and says nothing about bar 1
   (ADR-0029).
5. Heads-up, not a mismatch yet: `verification/{expr-bar,lyric-editor}/README.md` name functions in
   `web/main.ts` (`syncExprBar`, `applyExprBar`, `noteShownInExprBar`, `openLyricEditor`, …). The `web/lib/`
   split must update them.
6. Measurement note: utau-edit-cost 01's dev numbers for the learned stage (180.2/min, 23.8%, 92.7% cells)
   are in-sample: the stage was trained on all 28 dev + spent-holdout songs (`.scratch/game-holdout3/spec.md`).
   The roadmap and README now say so; the ticket's Answer does not.

