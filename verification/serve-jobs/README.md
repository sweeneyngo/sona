# Serve job queue model (TLA+)

`ServeJobs.tla` models `sona serve`'s job queue (`crates/sona-cli/src/jobs.rs`).
One worker runs jobs in order through their steps (separate, transcribe). A
cancel can arrive from the API at any moment, and any step can fail, panic or
hang its tool. Between jobs the worker deletes old job folders (ADR-0030).

Rules, checked on every path with 2 jobs of 2 steps:
- **CancelAndFailureStick:** a job reports at most one outcome: a cancelled or failed job never reports done or another failure. `Job::set_phase` refuses to leave a final phase, so a step that finishes after a cancel can't overwrite it.
- **ResultsOnlyWhenDone:** the annotation and stem are only served for a job that finished cleanly (and whose folder is still there).
- **WorkerNeverRunsADoneOrFailedJob:** a failed step frees the worker for the next job. (A cancelled job may stay on the worker until its current step notices the cancel; the runtime test checks the separator is killed promptly.)
- **OnlyFinishedJobsAreRemoved** and **TheRunningJobKeepsItsFolder:** clean-up never deletes a queued job's folder, nor the folder of the job on the worker, cancelled or not.
- **EveryJobFinishes** (liveness, with the worker's own steps fair and nothing asked of the user): every submitted job reaches a final phase and leaves the worker. A panic fails that job and the worker goes on; a hung tool is stopped at its deadline.

`./check.sh` runs TLC on the rules and on seven mutants, each of which must be
caught:

| Mutant | Caught by |
|---|---|
| `set-phase-not-absorbing` | CancelAndFailureStick |
| `results-before-done` | ResultsOnlyWhenDone |
| `failure-keeps-worker` | WorkerNeverRunsADoneOrFailedJob |
| `panic-kills-worker` (no `catch_unwind`) | EveryJobFinishes |
| `no-deadline` (a hung tool waits for a cancel) | EveryJobFinishes |
| `cleaner-takes-queued` | OnlyFinishedJobsAreRemoved |
| `cleaner-during-a-job` | TheRunningJobKeepsItsFolder |

The runtime side of the same guarantees is covered by
`crates/sona-cli/tests/serve.rs`: a cancel that kills the running separator, a
panic followed by a job that finishes, a hung separator stopped at its limit,
and clean-up that keeps queued jobs. CI runs `check.sh` in the `model-check` job.
