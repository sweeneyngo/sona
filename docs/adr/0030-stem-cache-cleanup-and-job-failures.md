# ADR-0030: `sona serve` keeps separated stems, cleans up after itself, and fails a job rather than the queue

Status: accepted, 2026-11-24. Amends [ADR-0020](0020-one-click-generation-sona-serve.md) (jobs folder,
failure handling) and [ADR-0024](0024-provenance.md) (the stem-cache key waits for the tool probe).
Tickets: `.scratch/audit-2026-11-24/issues/04-cheap-retranscribe.md`, `05-serve-worker-failure-modes.md`.

## Context

- **A BPM fix cost a full re-run.** Every job separated the song again, though the BPM, grid and engine
  are only used after separation. In the audit's measured run on a 158 s song, separation was 876 s of
  969 s. A wrong BPM is the costliest fix in OpenUtau, and in sona the fix is "transcribe again".
- **Jobs filled RAM.** The jobs folder was `/tmp/sona-jobs`, which is tmpfs on Fedora. Nothing was ever
  deleted: about 134 MB per 2.6-minute song.
- **One worker, no guard.** A panic in a job (core, ONNX Runtime, the cell stage) killed the only worker
  thread. That job stayed `transcribing` forever, and every later submit failed with "sending on a closed
  channel".
- **No deadline.** A hung demucs, UVR or ffmpeg blocked the queue until the user pressed Cancel, and the
  UI showed only "separating-vocals" and a clock while demucs's progress bar went to a file.

## Decision

1. **A stem cache** in `<cache-dir>/stems` (`crates/sona-cli/src/stems.rs`).
   - **Key:** the SHA-256 of a JSON map of everything that shapes the stems: the upload's SHA-256, the
     separation (`vocals` or `lead`), the demucs model, shift seed, demucs and torch versions, ffmpeg's
     version (demucs decodes compressed uploads with it), the demucs venv's path, and for the lead,
     audio-separator and its onnxruntime versions, the lead model's name and SHA-256, and the separator's
     path. A format number lets a later change invalidate every entry.
   - **Entry:** the stems exactly as a job uses them: `stem.wav` (and, for the lead, the mono `vocals.wav`
     the cell stage reads), plus `key.json`. A lead job now always writes the mono vocals so a later cell-
     stage job can reuse the entry. Files are hard-linked into the job folder (copied where links fail).
   - **On a hit** the job skips both separators. Its provenance gains `job.stems = {key, reused: true,
     separated_by: <job id>}`, its steps read `queued → reused-stems → transcribing`, and its log says
     "separation reused from job …". A miss records `reused: false`. The app's *Details* shows a *Stems*
     section.
   - **The key waits for the tool-version probe**, which ADR-0024 runs at start-up. The wait is bounded by
     the probe's own limits (20 s a tool) and a cancel ends it. If the versions still aren't known after
     90 s, the job runs uncached. A version the probe couldn't read keys as `unknown`.
   - **Transcription is unchanged:** a hit transcribes the same bytes the original job did.
2. **A bounded, on-disk jobs folder.**
   - `--cache-dir` defaults to `$XDG_CACHE_HOME/sona`, else `~/.cache/sona`. Jobs go to `<cache-dir>/jobs`
     (`--jobs-dir` still overrides).
   - **Policy:** between jobs, and once at start-up (old runs leave orphans), the worker keeps the
     `--keep-jobs` newest finished jobs (default 20, by the time of their last log line) and deletes the
     rest. It never deletes a queued job, and since it only runs when no job is running, never the running
     one. It deletes only folders that look like jobs (a hex id with a `job.log`).
   - The stem cache keeps the `--keep-stems` most recently used entries (default 10; 0 turns it off and
     empties it).
   - **Each job drops its separators' working folders** (`demucs/`, `uvr/`) when it ends, whatever the
     outcome. The logs and `job.json` stay.
3. **A panic fails its job, not the worker.** Each job runs under `catch_unwind`. A panic stops the job's
   tool, writes the message and a backtrace to `<job>/panic.log`, and fails the job with "sona hit an
   internal error (…); the details are in …/panic.log". The worker goes on to the next job. (A test hook,
   `SONA_TEST_PANIC=<upload name>`, panics where transcription would run.)
4. **Every tool has a deadline**: `--tool-timeout` seconds (default 600) or `--tool-timeout-ratio`
   seconds per second of audio (default 20), whichever is longer. The audio's length is exact for a WAV,
   read from ffmpeg otherwise, and estimated from the file size at 128 kbit/s failing both. A tool past its
   deadline is killed, and the job fails with what it last printed and the flags to raise. The defaults
   allow 80 minutes per separator for a 4-minute song, against about 5 s per second of audio measured on a
   heavily loaded machine: generous enough not to kill a slow run, but a hang no longer lasts forever.
5. **Smaller fixes from the audit.**
   - While a tool runs, the job's log ends with its last printed line, so demucs's progress bar shows in
     the app.
   - The app cancels the server's job when it gives up polling (`fetchJobOrCancel`), so a retry doesn't
     queue behind a ghost.
   - Every stem is 44.1 kHz: a WAV at another rate goes through ffmpeg like any other format. A 48 kHz
     "already a stem" upload no longer breaks GAME, and pYIN's sample-sized frames mean the same everywhere.
   - `sona serve --help` and `sona transcribe -h` print the usage. The 409 for a missing separator names
     `scripts/setup-separators.sh` and the venv flags.
   - `/api/capabilities` lists what separation still needs under `setup`, each with its fix (audit 12).
     ffmpeg is probed once at start-up (`ffmpeg -version`, 5 s limit). Only an ffmpeg that can't start or
     exits non-zero is missing. One still running at the limit is a busy machine, not a missing ffmpeg: it
     counts as present (stderr says it wasn't verified) and is never cached as missing (audit 17). The
     stem-cache key's version probe still reads a timed-out ffmpeg as `unknown`. The venvs, the htdemucs
     weights in torch's cache and the lead model are checked on each request with a file test, so a first
     download or a setup run shows without a restart. A missing ffmpeg or venv turns off the separations it
     blocks, and the 409 repeats its fix. Missing weights or a missing lead model only mean a download on
     the first job, so they don't. Without ffmpeg, an upload that isn't a 44.1 kHz WAV is refused before
     it is queued. The start screen lists the notes, and the tempo hint shows `/api/tempo`'s error.
   - `/api/*` requests whose `Origin` is present and not the server's own are refused, beside ADR-0020's
     Host check. The Vite dev proxy passes its own pages on as the server's.

## Evidence

- **Server** (`crates/sona-cli/tests/serve.rs`, stub separators that count their runs):
  - three lead jobs on one upload (BPM 120, 120 again, 60 on the triplet grid) ran demucs and UVR once;
    the repeat gave the same notes and the same stem bytes, and its provenance names the first job and
    the same key. A vocals-only job and another upload each separated again;
  - with `--keep-jobs 1 --keep-stems 1`, three jobs behind a slow separator all finished (the queued ones
    kept their folders), only the newest job folder was left, non-job folders were untouched, and the
    stem cache held one entry;
  - a panicking job failed with its message and `panic.log`, and the next two jobs finished;
  - a separator that prints a progress bar and hangs showed that line in the job's log, was stopped at its
    limit with that line in the error, and the next job finished;
  - a 48 kHz WAV became a 44.1 kHz stem with the right notes; `--help` works; foreign Origins get 403;
  - an ffmpeg that exits 1 or doesn't exist turns separation off; one whose `-version` outlasts the 5 s
    limit leaves it on, with no ffmpeg note, and a 48 kHz WAV is queued.
- **Mutants** (each applied to the code and caught by the test named): no `catch_unwind` (panic test),
  no deadline (hang test), no cache hit (cache test), clean-up deleting queued jobs or any folder (clean-up
  test), no resampling (48 kHz test), no tool line (hang test), no Origin check (guard test), a probe
  timeout cached as missing and a non-zero exit read as a timeout (ffmpeg probe test). Web: no
  cancel on a lost poll, and a `stems` record that doesn't parse, are each caught by vitest.
- **Model** (`verification/serve-jobs`, TLA+): a job may now panic, hang its tool or be cleaned up. New
  rules: `OnlyFinishedJobsAreRemoved`, `TheRunningJobKeepsItsFolder`, and the liveness property
  `EveryJobFinishes` (with only the worker's own steps fair). `CancelAndFailureStick` now says a job
  reports at most one outcome. All seven mutants are caught, among them `panic-kills-worker` and
  `no-deadline` (both by `EveryJobFinishes`).
- **Real run** (25 s sample, lead separation, pYIN grid 16, real venvs; machine load about 27 on 16 cores):
  - the first job at 123 BPM took 223.4 s (demucs 100.6 s, UVR 106.5 s, transcription 15.1 s);
  - the same song again took 17.3 s at 246 BPM and 23.1 s at 123 BPM, both from the cache;
  - the 123 BPM re-run gave the same 66 notes and the same stem hash;
  - the demucs and UVR progress bars showed in the job status while they ran.

## Consequences

- Re-transcribing a song the server has seen costs transcription time only. The app still uploads the
  file again; submitting by hash without an upload (ticket 04, item 2) and offering "transcribe again" from
  the editor are left for later.
- Disk use is bounded by counts, not bytes: roughly 20 jobs plus 10 songs' stems, each up to ~130 MB for a
  4–5 minute song (job folders and cache entries share their stems through hard links).
- A stem cached before a separator upgrade is never reused after it, as long as the probe can read the
  new version. If it reads `unknown` both times, a changed venv at the same path would still hit.
- ADR-0024's separator non-determinism (last-bit differences between runs) no longer shows between jobs on
  one song: they now share one separation.
