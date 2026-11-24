# How sona is developed

The loop this project runs on. The rules behind it, with the failure each one prevents, are in
[`agents/rules.md`](agents/rules.md); what to work on next is in [`roadmap.md`](roadmap.md).

## 1. Set up

```sh
cargo build --release                      # sona-core, sona-cli, sona-eval
(cd web && npm install && npm run build)   # the app that `sona serve` hosts
scripts/setup-separators.sh                # demucs + UVR karaoke environments (~4 GB, one-time)
```

Optional:
- **GAME notes.** Download a GAME ONNX release (CC BY-NC-SA: yours, never committed) and an ONNX Runtime
  library. Then build with `--features game`, set `ORT_DYLIB_PATH`, and pass `--game DIR` to
  `sona serve` or `sona transcribe`.
- **The UST corpus** (for measuring). See the headers of `crates/sona-eval/scripts/fetch-pifuyuu-*.sh`
  and `separate-*.sh`. Everything they fetch is git-ignored, and CI refuses it if it is ever tracked.
- **TLA+:** Java 11+ and `tla2tools.jar` 1.7.4 at `~/.cache/tla/tla2tools.jar` (CI pins its SHA-256).

## 2. Run it

```sh
cargo run --release -p sona-cli -- serve   # the app with one-click transcription, http://127.0.0.1:8977
(cd web && npm run dev)                    # the editor with hot reload, http://localhost:5173
```

## 3. Make something measurably better

Use this loop when the work is "make X better" (accuracy, tempo, curve error), not for routine edits.

1. **Pin the metric** with the user (for notes, cell accuracy against Pifuyuu's USTs). Measure the
   baseline, then an **oracle**: the best any version of the idea could do. Build only what the oracle
   says can move the number.
2. **Iterate on dev**:
   - `sona-eval-ust-dev`: it caches the f0 contour and GAME output, runs several `--cfg` configs in one
     go, and has `--per-song`, `--diag`, `--dump` and `--edit-cost`;
   - `sona-eval-cellstage --edit-cost` for the shipped stack (GAME + learned stage). The edit cost
     weights each error by how hard it is to fix in OpenUtau (`sona_eval::correction::edit_cost`);
   - `sona-eval-tempo` for tempo.
   - Prototype in Python on dumped intermediates, and port to Rust only once it wins. The port must
     reproduce the prototype's numbers.
3. **Pre-register**, before any held-out read: write the decision rule in `.scratch/<feature>/spec.md`
   and commit it. The rule says what ships, what doesn't, and the noise floor (song bootstrap).
4. **Read the held-out songs once.** The tools refuse to read a held-out split without `--reason`, log
   every read in `crates/sona-eval/corpus/holdout-reads.jsonl`, and refuse a second read without
   `--reread`. All of today's splits are spent for cell accuracy; see roadmap item 1.2.
5. **Know what produced it.** Every annotation carries its provenance (ADR-0024): the build, input and
   model hashes, settings and seed. Compare two results setting by setting before comparing scores.
6. **Explain the number.** List what else it could be measuring (leaked labels, a cache, one outlier
   song) and rule each one out. GAME is deterministic since ADR-0023; a model with random nodes of its own
   needs the same treatment before its numbers can be compared.
7. **Record the result, positive or negative**, with its numbers: a report in `crates/sona-eval/reports/`
   or an ADR in `docs/adr/`, plus the README. New behaviour stays opt-in until a held-out read confirms
   it.

## 4. Verify what matters

| Layer | Where | Notes |
|---|---|---|
| Unit and property tests | `cargo test --workspace --release`; `cd web && npx vitest run` | Test behaviour through the public interface; random sequences for invariants |
| Output invariants | `crates/sona-core/tests/output_invariants.rs` | One sorted, non-overlapping voice on random stems (sona's decoders) and random raw GAME notes and cell choices (GAME, GAME + learned stage; `--features game` for the GAME half) |
| Model checking (TLA+) | `verification/{grid-notes,lyric-editor,expr-bar,serve-jobs,start-flow}/check.sh` | Each runs its rules and **mutants that must be caught** |
| Server | `crates/sona-cli/tests/serve.rs` | Drives the real binary with stub separators |
| GAME determinism | `cargo test -p sona-core --features game -- --ignored same_notes`; `cargo test -p sona-cli --features game --test game_determinism -- --ignored` | Need `ORT_DYLIB_PATH`, `SONA_GAME_DIR` (and optionally `SONA_DETERMINISM_STEM`); not in CI |
| Browser | `web/e2e/run_all.sh` | Playwright drivers that use the app as a person does ([README](../web/e2e/README.md)) |
| Review passes | pstack: statechart, comment-sicko, typescript practices | Every bug a pass finds is replayed (a test or a driver) before it's fixed |

**A check isn't trusted until it has failed once on a deliberately broken version** (a mutant).

## 5. Ship

- CI (`.github/workflows/ci.yml`) runs the corpus guard, clippy with warnings as errors, the Rust and web
  tests and every TLA+ model. Push only when the local equivalents pass, then watch CI go green.
- Commit at each verified step, with the numbers in the message.
- Update the README after any change a user would notice, and add an ADR for a decision someone will
  later ask "why?" about.
- **Never commit:** Pifuyuu's USTs (the author forbids redistribution, and the repo is public), source
  audio, Vocadito, GAME or fine-tuned weights, or the published lyrics.
