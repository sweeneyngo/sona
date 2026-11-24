# 07 — Delete the knobs and code paths that were measured and lost

Status: resolved
Type: task
Blocked by: 05

Project rule (`~/.claude/CLAUDE.md` §3, rules.md): "delete knobs that default off and earned nothing".
Each item below was measured and rejected, or is a placeholder, and is still compiled into the product
crates.

## Evidence

| Item | Where | Verdict on record | Size |
|---|---|---|---|
| Note-HMM **release sub-state** | `NoteHmm::release_drop_threshold = f32::INFINITY` (`crates/sona-core/src/hmm/notes.rs:84, 101`); `HmmParams::{sigma_release, release_self, stable_to_release_frac, release_offset_bonus}` (`hmm/model.rs:103-192`); `Sub::Release` edges (`hmm/decode.rs`) | ADR-0012:5 "shipped OFF … every setting neutral-to-harmful" (`reports/release-sweep.md`) | ~120 lines with "release" in core (model 62, decode 31, notes 27), plus `sona-eval/src/bin/release_sweep.rs` (259) |
| **Spectral-flux / SuperFlux onsets** | `OnsetNovelty::SpectralFlux` (`sona-core/src/onset.rs`), `onset/spectral.rs` (125) | ADR-0013:5: energy stays default, flux loses at the note level | Used only by `sona-eval` bins `onset_compare.rs` (199) and `note_onset_gate.rs` (240): core code reachable only from experiments |
| **`crepe` feature** | `pub mod crepe {}` (`sona-core/src/lib.rs:50-56`), forwarded in `sona-cli/Cargo.toml:26-27` | "Empty placeholder — the feature already pulls `ort` into the build" | small, but it is a public feature that does nothing |
| **`--salience` / `--spectral-cue`** | `sona-cli/src/main.rs:329-330` ("Accepted for compatibility: both are on by default now") | no-ops | small |
| `Estimate::from_reference` | `sona-eval/src/estimate.rs:42` | zero callers (`grep -rn from_reference crates web`) | ~20 lines |
| TS exports used only by tests | `gridLines` (`web/lib/grid.ts:51`), which `main.ts:935-948` re-implements; `pitchRange` (`view.ts:71`); `samplePitd` (`sungPitch.ts:149`); `curveErrorCents` (`fitexpr.ts:257`) | dead in the app | ~50 lines |
| `MIN_BPM`/`MAX_BPM` | exported by `web/lib/startFlow.ts:3-4` and used only inside it; `main.ts:642` (`scaleBpm`) hard-codes 20 and 400 again | duplicated constant | — |

Also checked: the ADR-0003 **escape hatch** (`grid.ts` `quantizeNote`). On a full dev song through pYIN grid
mode (innocent-addict, 735 notes), **0** notes have `onset_clarity ≥ 0.8`, so the hatch can't fire there.
GAME and cell-stage notes go through the same scorer (`sona-cli/src/main.rs:242, 272`); that was not
measured. The hatch still serves grid-free (note-HMM) annotations, so keep it, but say in ADR-0003 that it is inert
for grid producers.

## Proposal

1. Delete the spectral-flux onsets, the two eval bins that only compare them, the `crepe` feature,
   the no-op flags, `from_reference`, and the test-only TS exports (`main.ts` switches to `gridLines`, or
   `gridLines` moves into the seam of 08). Keep the reports and ADRs as the record; mark ADR-0012/0013
   "code removed in <commit>".
2. The release sub-state is **not byte-identical** to delete: `stable_to_release_frac = 0.1` still takes
   transition mass when the feature is off (`hmm/model.rs:184-192`). Either delete it and re-run the
   Vocadito gate (`reports/baseline.md` numbers must hold within the song-bootstrap interval), or leave it
   and record why.
3. Decide separately about the ablation switches `--no-salience`, `--no-spectral-cue` and
   `--no-octave-check`. They are opt-outs from confirmed defaults, useful for eval but user-facing. Moving
   them to `sona-eval --cfg` keys would shrink the CLI surface.

## Cost (S/M/L) and how to tell it worked

S, plus one Vocadito gate run if the release state goes. It worked when clippy, `cargo test --workspace
--release`, `cargo check -p sona-core --no-default-features`, vitest and every `verification/*/check.sh`
stay green, and the dev cell numbers for the default stack are byte-identical (`sona-eval-ust-dev` on dev,
compared with the committed report).

## Answer

Done on branch `audit-a07-delete-knobs` (from 718b543). Every candidate's evidence held when re-checked.
About 1,450 lines are deleted, most of them the code ADR-0012 and ADR-0013 added in bf1c64e. The
note-HMM and `onset.rs` are restored to their state before that commit: neither file changed after it.

| Candidate | Verdict | Evidence |
|---|---|---|
| Note-HMM `Release` sub-state (ADR-0012) and `sona-eval-release-sweep` | **removed** | Vocadito gate below: one stem of 40 changed, within noise |
| Spectral-flux / SuperFlux onsets (ADR-0013), `sona-eval-onset-compare`, `sona-eval-note-onset-gate` | **removed** | Only those two bins ever set `OnsetNovelty::SpectralFlux`. Default output byte-identical |
| `crepe` feature (`sona-core`, forwarded by `sona-cli`) | **removed** | Empty `pub mod crepe {}`. `Producer::Crepe` stays: it is part of the annotation contract |
| `--salience` / `--spectral-cue` | **removed** (now "unknown option") | No caller in the repo (scripts, e2e, serve, docs). See below |
| `Estimate::from_reference` | **removed** | Zero callers |
| `gridLines` in `web/lib/grid.ts` | **removed** with its tests | `main.ts` already draws from `frame.ts`'s `gridLines` (ADR-0029), so this one was test-only |
| `pitchRange` / `PitchWindow` (`view.ts`) | **removed** with its tests | Test-only |
| `samplePitd` (`sungPitch.ts`) | **moved** into `sungPitch.test.ts` | The test uses it as an oracle ("as OpenUtau samples it"). The app never calls it |
| `curveErrorCents` (`fitexpr.ts`) | **removed** | The one test now measures error against the known sung curve directly |
| `MIN_BPM` / `MAX_BPM` | **un-exported** | `main.ts` `scaleBpm` calls `usableBpm` instead of repeating 20 and 400. Same behaviour: `v` is always a number |

**Release sub-state: the Vocadito gate.** `sona-eval-report --corpus` (default `Transcriber`, note-HMM),
40 stems, before (718b543) and after:

| metric | before | after | Δ | paired 95% | baseline song-bootstrap 95% |
|---|---|---|---|---|---|
| RPA | 0.8576 | 0.8576 | 0 | — | — |
| onset F1 | 0.5648 | 0.5648 | 0 | — | — |
| COnP F1 | 0.6463 | 0.6466 | +0.0003 | [+0.0000, +0.0009] | [0.6125, 0.6806] |
| COnPOff F1 | 0.5077 | 0.5077 | −0.0000 | [−0.0002, +0.0000] | [0.4684, 0.5480] |

- One stem changed: vocadito_34 gained one matched note (COnP 0.457 → 0.469, COnPOff 0.299 → 0.297). The
  other 39 stems and all four golden fixtures are identical.
- The noise floor is the song-bootstrap interval of the mean, ±0.034 (COnP) and ±0.040 (COnPOff). The
  change is 100× smaller than that.
- The default needed one choice: `stable_to_silent_frac` goes back to ADR-0010's 0.5. The 0.1 that the
  decay path took was carved from the sharp-cutoff mass, so 0.5 restores ADR-0010's exact transitions and
  keeps rows normalised.
- The before numbers reproduce the recorded ADR-0010 / release-sweep baseline (0.646 / 0.508). The
  baseline was scored in this run, not copied from the report.

**Byte-identity.** `sona transcribe` before and after:
- stems: vocadito_1, 7 and 23 (note-HMM, and `--grid 16`); dev songs amaten and denpa (`--grid 16`,
  `--grid 24 --no-salience --no-spectral-cue`, note-HMM);
- result: every annotation is identical to the last note, except `provenance` (commit `-dirty` and
  timestamp), in 10 of 12 runs, including all 6 grid runs and the 3 Vocadito note-HMM runs;
- the 2 full-song note-HMM runs are the expected exception:
  - amaten: 2 of 158 notes changed;
  - denpa: 1 of 338 notes changed;
  - each changed note ends 35 ms earlier (6 frames), and its confidence rises (0.10 → 0.57, 0.21 → 0.47,
    0.25 → 0.52). These notes used to finish through the voiced `Release` tail. Now they end at
    `Silent`, where the energy falls.

`sona-eval-ust-dev --cfg base: --cfg grid:grid=est --edit-cost` on dev (9 songs):
- the `grid` config, the default pYIN grid stack, is byte-identical before and after: cell accuracy 79.5%,
  338.4 edit cost/min;
- `base` is the note-HMM: cell accuracy 53.3% both, COnP F1 0.341 → 0.342, edit cost 359.4 → 358.9/min.

One trap: the first run filled the f0 and salience cache, and later runs read from it. A cold-cache run
differs from a warm one by about a cell per song (grid COnPOff 0.387 vs 0.388), even with the same
binary, so compare warm runs only. GAME and the learned cell stage never call the note-HMM or the onset
novelty code. They take only `OnsetDetector::detect`'s energy path, which is byte-identical.

**The two flags.** I removed them rather than keep accepting them:
- they were opt-ins to what is now the default, so passing one changed nothing;
- nothing in the repo passes them;
- an unknown option fails loudly with `unknown option '--salience'`, which tells a stale script exactly
  what to delete. A silently accepted no-op would hide that.

The usage text never listed them. A new test (`the_retired_opt_in_flags_are_unknown_options`) covers
the error. The grid-only check now names only `--no-salience` / `--no-spectral-cue`.

**Kept, deliberately.**
- `--no-salience`, `--no-spectral-cue` and `--no-octave-check` (proposal item 3). They are live opt-outs,
  `jobs.rs` and `ust-dev` mirror them, and moving them is a CLI-surface decision outside this ticket.
- The ADR-0003 escape hatch. ADR-0003 now says the hatch is inert for grid producers on the measured
  song, and that GAME and the cell stage were not measured.

**Docs.**
- ADR-0012 and ADR-0013 now have the status "Rejected; code removed (audit A07)", with the numbers
  above. Their old status lines are kept below.
- ADR-0001 notes that the `crepe` placeholder is gone.
- `release-sweep.md`, `onset-compare.md` and `note-onset-gate.md` say their code was removed and how to
  restore it.
- The Cargo feature comments now say what `spectral` gates: the grid cue and tempo.
- README, rules.md and dev-workflow.md mentioned none of the removed things.

**Verification.**
- `cargo clippy --workspace --all-targets -D warnings`, plus `-p sona-core|sona-eval|sona-cli --features
  game` and `-p sona-core --no-default-features`: clean.
- `cargo check -p sona-core --no-default-features`: ok.
- `cargo test --workspace --release`: green.
- `tsc --noEmit`, and vitest 18 files, 153 tests: green.
- All five `verification/*/check.sh`: pass.
- Corpus guard: ok. `holdout-reads.jsonl` untouched.

## Comments
