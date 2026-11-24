# 05 — `sona serve`: a panic kills the only worker silently; hung children block the queue

Status: resolved
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

## Answer

Done together with 04 on branch `audit-a04-a05-serve`:
[ADR-0030](../../../docs/adr/0030-stem-cache-cleanup-and-job-failures.md) (numbered 0030 on the branch; the
number may change at merge). Item by item:

1. **Panic guard.** Each job runs under `catch_unwind` (`jobs.rs`, `Queue::start`). A panic stops the job's
   tool and writes the message and a backtrace to `<job>/panic.log` (a panic hook keeps them per thread).
   It fails the job with "sona hit an internal error (…); the details are in …/panic.log" and writes its
   `job.json`. The worker then takes the next job. Test hook: `SONA_TEST_PANIC=<upload name>` panics where
   transcription runs. In `tests/serve.rs::a_panic_fails_that_job_and_the_worker_goes_on`, the panicking
   job ends `failed` and the next two end `done`. Mutant (no `catch_unwind`): the panicking job stays
   `transcribing`, and the test fails.
2. **Deadline.** It is a total deadline proportional to the audio, not an inactivity one, because ffmpeg
   with `-loglevel error` prints nothing during a long decode. The limit is `--tool-timeout S` (default
   600) or `--tool-timeout-ratio R` (default 20 s per second of audio), whichever is longer. The length is
   exact for a WAV, ffmpeg's `Duration:` otherwise, and the file size at 128 kbit/s failing both. Past the
   limit the tool is killed, and the job fails with the tool's last printed line and the flags to raise.
   **Progress:** while a tool runs, the job's log ends with `<tool>: <its last line>` (`\r` counts as a line
   end), so demucs's bar reaches the UI with no UI change. Test: a stub demucs prints a tqdm-like bar and
   sleeps 30 s. The status showed `demucs: 40%|####      | 4/10 …`, the job failed at its 2 s limit with
   that line in the error, and the next job finished. Both mutants (no deadline, no tool line) are caught.
3. **Ghost jobs.** `serveClient.fetchJobOrCancel`: when a poll fails, the client cancels the job on the
   server (best effort) and rethrows. `main.ts` changes only in its import and the poll call. Vitest with a
   stubbed `fetch`: a failing poll sends `DELETE`, a good one doesn't. The mutant (no cancel) is caught.
4. **Capabilities that say ready when they aren't: not done.** ffmpeg, the UVR model file and the demucs
   weights still aren't checked, and the client still swallows a 422 from `/api/tempo`. Checking the model
   file is cheap. The demucs weights live in torch's cache, and the first run downloads them.
5. **Sample rate.** `to_mono_wav` sends every WAV that isn't 44.1 kHz through ffmpeg, as it does any other
   format. With no ffmpeg the error reads "resampling the 48000 Hz WAV to 44100 Hz: could not start
   ffmpeg …". Test: a 48 kHz upload with no separation gives a 44.1 kHz mono stem with A4 and B4. The
   mutant is caught. A 44.1 kHz WAV takes the old in-process path, byte for byte.
6. **409 and `--help`.** `sona serve --help` and `sona transcribe -h|--help` print the usage, which now
   lists the new serve options. The 409 says "run scripts/setup-separators.sh, or start sona serve with
   --demucs-venv and --separator-venv (see `sona serve --help`)". Both are tested.
7. **Origin.** `/api/*` requests whose `Origin` is neither `http://127.0.0.1:<port>` nor
   `http://localhost:<port>` get 403. The Vite dev proxy rewrites its own `localhost:5173` origin to the
   server's. Tests: foreign and `null` origins get 403, the server's own pass. The mutant is caught.

**Model** (`verification/serve-jobs`):
- A running job may now panic, hang its tool (failed at the deadline), or be cleaned up between jobs.
- New liveness property `EveryJobFinishes`, with only the worker's steps fair. New invariants
  `OnlyFinishedJobsAreRemoved` and `TheRunningJobKeepsItsFolder`.
- `CancelAndFailureStick` now says a job reports at most one outcome. The model now skips a job cancelled
  while queued (as `run` does), which closed the path the old `set-phase-not-absorbing` mutant was caught
  through; the stronger rule catches it again.
- 7/7 mutants caught, including `panic-kills-worker` and `no-deadline` (both by `EveryJobFinishes`).

**Not tested:** missing ffmpeg or venvs beyond the 409 and the resample message, and a real hung
separator.
