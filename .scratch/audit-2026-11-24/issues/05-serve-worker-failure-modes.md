# 05 — `sona serve`: a panic kills the only worker silently; hung children block the queue

Status: ready-for-agent
Type: task
Blocked by: —

Sharper scope for roadmap 6.3 ("errors a user can act on"). These are the failure modes that leave the
app stuck rather than showing an error.

## Evidence (crates/sona-cli/src unless noted)

- **One worker thread with no panic guard.** `Queue::start` spawns `for job in rx { run(&job, &tools); }`
  (`jobs.rs:160-164`). There is no `catch_unwind` anywhere in the workspace. Transcription runs
  **in-process** on that thread (`jobs.rs:351-360` builds `TranscribeArgs` and calls the CLI's transcribe
  path). If anything in core, `ort` or the cell stage panics:
  - that job stays `transcribing` forever, and the client polls it forever (`web/main.ts:2490`);
  - every later submit writes its upload and registers a job (`jobs.rs:173-186`), then `send` fails, so the
    user gets a 500 ("sending on a closed channel") and a job directory that is never used.

  ADR-0026's review already found one crash of this kind in the cell stage (a read one frame past a chunk),
  so the class is real.
- **No deadline on children.** `run_child` polls `try_wait` every 100 ms with no timeout
  (`jobs.rs:398-411`). A demucs/UVR that hangs, for example on a first-run model download with a stalled
  network, blocks the single queue until the user presses Cancel. The UI log shows only phase lines while a
  child runs (`jobs.rs:133-144`), so the user can't tell a hang from progress. In a
  real run (04), demucs printed a percentage progress bar to `<job>/demucs.log` for 9 minutes while the
  job's log said only `separating-vocals`.
- **Ghost jobs.** If one `fetchJob` poll throws, the UI marks the run failed (`web/main.ts:2492-2494`) but
  never cancels it on the server. A retry then queues behind the ghost.
- **Capabilities say ready when they aren't.** Only the existence of `bin/python` / `bin/audio-separator`
  is checked (`serve.rs:107-108`); ffmpeg, the UVR model file and the demucs weights are not. A failed
  tempo request (422) is swallowed by the client (`web/main.ts:2447-2449`).
- **Sample rate depends on the container.** A WAV hound can read keeps its own rate (`jobs.rs:437-439,
  446-447`); anything else is resampled to 44.1 kHz by ffmpeg (`jobs.rs:441-442`). With "Already a stem"
  and a 48 kHz WAV, the GAME engines fail with CLI advice to run ffmpeg by hand (`main.rs:256-261`), even
  though serve has ffmpeg. pYIN's frame sizes are in samples, so the default engine also behaves
  differently at 48 kHz.
- **A 409 points at help that doesn't exist.** The message says "see `sona serve --help`"
  (`serve.rs:178`), but `sona serve --help` prints `sona: unknown serve option '--help'`, and
  `sona transcribe -h` prints `unknown option '-h'`. Only top-level `sona --help` works.
- **No Origin check.** The Host check stops DNS rebinding (ADR-0020), but the server ignores Content-Type
  and Origin. Any web page can send a cross-origin "simple" POST to `127.0.0.1:8977/api/jobs` (blind, no
  preflight), which fills the jobs directory and burns CPU. Low severity on a local tool, and a two-line
  fix.
- `tests/serve.rs` covers a failing separator, cancel and the Host check. It has no test for a panic, a
  hang, missing ffmpeg/venv, non-44.1 kHz input or a lost poll.

## Proposal

1. Wrap each job in `catch_unwind`: a panic marks that job `failed` with "internal error, see
   <job>/panic.log" and the worker keeps going. Test: a debug-only stub engine that panics (or an env hook
   in the test binary). Mutant: remove the guard, and the next job must then fail the test.
2. An inactivity deadline for children (no new stderr bytes for N minutes means failed, with the log
   tail), and show the child's last stderr line in the UI log while it runs.
3. The client cancels the server job when it gives up polling.
4. `to_mono_wav` resamples every input to 44.1 kHz, the WAV path included.
5. Accept `--help`/`-h` on both subcommands. Reject `/api/*` POSTs whose `Origin` is present and not the
   server's own.
6. Fault-injection tests in `tests/serve.rs` for each item, each shown failing on its mutant first
   (rules.md).

## Cost (S/M/L) and how to tell it worked

S–M. It worked when each new serve test passes and fails on its mutant, and when a job that panics, then a
normal job, gives "failed" followed by "done".

## Comments

- `verification/serve-jobs` models the queue. Adding "a job's run may crash" as an action would show the
  stuck state in TLA+ too. Worth it only if the model is cheap to extend.
