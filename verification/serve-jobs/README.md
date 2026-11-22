# Serve job queue model (TLA+)

`ServeJobs.tla` models `sona serve`'s job queue (`crates/sona-cli/src/jobs.rs`).
One worker runs jobs in order through their steps (separate, transcribe). A
cancel can arrive from the API at any moment, and any step can fail.

Rules, checked on every path with 2 jobs of 2 steps:
- **CancelAndFailureStick:** a cancelled or failed job never reports done. `Job::set_phase` refuses to leave a final phase, so a step that finishes after a cancel can't overwrite it.
- **ResultsOnlyWhenDone:** the annotation and stem are only served for a job that finished cleanly.
- **WorkerNeverRunsADoneOrFailedJob:** a failed step frees the worker for the next job. (A cancelled job may stay on the worker until its current step notices the cancel; the runtime test checks the separator is killed promptly.)

`./check.sh` runs TLC on the rules and on three mutants, each of which must be
caught. The runtime side of the same guarantees, including a cancel that kills
the running separator, is covered by `crates/sona-cli/tests/serve.rs`. CI runs
`check.sh` in the `model-check` job.
