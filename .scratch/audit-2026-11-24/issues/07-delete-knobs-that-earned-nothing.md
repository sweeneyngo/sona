# 07 — Delete the knobs and code paths that were measured and lost

Status: ready-for-agent
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

## Comments
