# 04 — Re-transcribing costs a full re-run: reuse separated stems, parallelise pYIN, clean the jobs dir

Status: resolved (items 1, 4, 5; items 2 and 3 remain, see the answer)
Type: task
Blocked by: — (pairs with utau-edit-cost 04, "ask for the BPM first")

Sharper scope, with numbers, for roadmap 5.2 ("cache by file hash").

## Evidence

**A new BPM re-runs separation.**
- Every `POST /api/jobs` makes a new job dir and runs the whole pipeline (`crates/sona-cli/src/jobs.rs:
  168-188, 313-350`). The BPM is used only in the transcribe step (`jobs.rs:351-356`), so the separated
  stems are valid for any BPM, grid or engine that uses the same separation.
- The upload's SHA-256 is already computed, for provenance (`jobs.rs:239-241`), but nothing uses it as a
  key.
- In the app, ½×/2× exist only before Transcribe (`web/lib/startFlow.ts:48-51`). After results load,
  "start over" goes back to `idle` (`startFlow.ts:74-75`), so the user picks the file again. It is uploaded
  again for tempo and again for the job (`web/main.ts:2446, 2466`), and separated again. A retry after a
  failure redoes everything too.
- Changing the BPM in the editor afterwards (`main.ts:611-618`) only re-snaps notes decoded on the old
  grid onto the new one. Grid-mode notes are decoded per cell at the job's BPM (ADR-0015), so at ½× every
  note that started on an odd old cell now falls on a half cell and rounds. The editor gives no hint that a
  re-transcription would do better.

**Measured times (this machine, 16 cores; dev songs; CPU).**
- `sona transcribe <281 s vocals stem> --bpm 198 --grid 16` (pYIN grid, the default engine): 155 s wall,
  146 s user, **94% CPU**, so a single core. With `--no-salience`: 147 s. Note-HMM path (no `--grid`):
  172 s. The shared front end therefore dominates, not the grid decoder. The likely cost is pYIN's
  difference function, O(w·τmax) ≈ 1024 × 679 multiply-adds per frame over about 48k frames
  (`crates/sona-core/src/pitch/pyin.rs:213-223`); nothing in `sona-core` is parallel (no rayon, no
  threads). Profile before acting.
- Full `sona serve` job: amaten (dev, 158 s of audio), lead separation, pYIN, jobs dir in a scratch folder.
  Job log: `separating-vocals 0 s → separating-lead 545.9 s → transcribing 876.1 s → 507 notes 969.3 s`.
  That is **demucs 546 s, UVR 330 s, transcription 93 s: 16 minutes for a 2.6-minute song**, with 90% of
  it spent on separation. Caveat: the machine was shared (load average about 24 on 16 cores; another
  agent's `sona-eval-cells` was running). demucs's own progress bar shows its best rate at about 1.4 s per
  second of audio, and about 5 s per second under load. Even uncontended, separation is several minutes per
  song, and it is repeated on every BPM change.
- That progress bar goes only to `<job>/demucs.log`. The UI shows "separating-vocals" and a clock for
  9 minutes (see 05).
- README says "A 25 s clip takes about 80 s". Users drop whole songs, so the realistic first wait is
  several minutes, and every BPM correction costs the same again.

**Job dirs pile up in RAM.**
- The default jobs dir is `std::env::temp_dir()/sona-jobs` (`crates/sona-cli/src/serve.rs:28`). On Fedora
  (this machine) `/tmp` is **tmpfs** (`df /tmp`), so it counts against RAM.
- Nothing ever deletes a job dir. The only `remove_file` calls in `sona-cli/src` are tempo scratch files
  (`jobs.rs:490-491`) and a provenance temp file. The job map is memory-only, so after a restart old dirs
  are orphaned.
- Each job keeps the upload, demucs vocals + no_vocals, the UVR output, `stem.wav` (32-bit float for WAV
  input, `jobs.rs:446-447`) and, for the cells engine, `vocals.wav`. For the amaten job: **134 MB** (five 27 MB WAVs: input, demucs vocals and
  no_vocals, UVR vocals, stem), so about 250 MB for a typical 4–5 minute song, every time.

Why this matters for the goal: the spec ranks a wrong BPM as the hardest fix in OpenUtau. In sona the fix
is "transcribe again at the right BPM". That is only cheap if it doesn't redo separation. utau-04 makes
"I know my BPM" the main path, and the ½× hint stays for when the estimate is used.

## Proposal

1. **Stem cache keyed by (input SHA-256, separation, separator model hashes)** in a persistent dir (e.g.
   `$XDG_CACHE_HOME/sona/stems`), checked before the separation phases. Record a hit in provenance and the
   job log ("separation reused from <hash>").
2. **Re-transcribe in the app**: after results load, "Transcribe again at … BPM" (and a new engine/grid)
   submits by hash with no upload when the server has the stems. A BPM change in the editor offers it.
3. **pYIN across frames in parallel** (frames are independent, so the output is byte-identical; check it
   with the dev gate and the output invariants). Profile first. If the difference function dominates, an
   FFT-based one is faster still, but not byte-identical, so it stays opt-in.
4. **Bound the jobs dir**: default to a cache dir, not `/tmp`. Keep the last N jobs or N GB; delete a job's
   intermediates (demucs `no_vocals`, UVR output) once `stem.wav` is written; delete half-written dirs on
   failure.
5. Serve tests: a second job with the same upload and a new BPM skips the stub separators (assert they ran
   once); eviction keeps the newest.

## Cost (S/M/L) and how to tell it worked

M. It worked when re-transcribing a dropped song at a new BPM takes transcription time only (wall time
recorded for the same dev song), the default output stays byte-identical, and the jobs dir stays under its
bound across 20 jobs.

## Comments

## Answer

Items 1, 4 and 5 are done with 05 on branch `audit-a04-a05-serve`:
[ADR-0030](../../../docs/adr/0030-stem-cache-cleanup-and-job-failures.md) (numbered 0030 on the branch; the
number may change at merge). Items 2 and 3 remain.

**1. Stem cache** (`crates/sona-cli/src/stems.rs`, in `<cache-dir>/stems`).
- **Key:** SHA-256 of a JSON map of the upload's SHA-256, the separation (`vocals`/`lead`), the demucs
  model, shift seed, demucs, torch and ffmpeg versions, and the demucs venv path. A lead job adds
  audio-separator and onnxruntime versions, the lead model's name and SHA-256, and the separator path.
  A `format` field lets a later change retire every entry.
- **Entry:** `stem.wav` (plus the mono `vocals.wav` for the lead, so a later cell-stage job can reuse it),
  hard-linked into job folders.
- **Provenance:** `job.stems = {key, reused, separated_by}`. On a hit the steps read `queued →
  reused-stems → transcribing`, and the log says "separation reused from job …". *Details* in the app has
  a *Stems* section.
- **The key waits for ADR-0024's version probe:** bounded by the probe's limits, cancellable, and after
  90 s the job runs uncached.

**4. Jobs folder.**
- The default is `$XDG_CACHE_HOME/sona/jobs` (else `~/.cache/sona/jobs`), not `/tmp`.
- Between jobs and at start-up, the worker keeps the `--keep-jobs` (20) newest finished jobs. It never
  deletes a queued job, and it never runs while a job does. It deletes only hex-named folders with a
  `job.log`.
- The cache keeps the `--keep-stems` (10) most recently used songs; 0 turns it off.
- `demucs/` and `uvr/` go when a job ends, whatever the outcome. A failed job keeps its logs.

**5. Tests** (`tests/serve.rs`, stub separators that count their runs):
- three lead jobs on one upload (120, 120, 60 BPM on the triplet grid) ran demucs and UVR once. The
  repeat gave the same notes and stem bytes;
- a vocals-only job and a different upload each separated again;
- with `--keep-jobs 1 --keep-stems 1`, three jobs behind a slow separator all finished, and only the
  newest job folder and one cache entry were left. A pre-made orphan job folder was deleted, while a
  non-job folder and a hex folder without a log were kept.
- Mutants caught: no cache hit; clean-up that deletes queued jobs; clean-up that deletes any folder.

**Real run** (the 25 s `telescope-clip.mp3`, lead separation, pYIN grid 16, real venvs; machine load
about 27 on 16 cores):

| Job | Wall | Steps |
|---|---|---|
| 123 BPM, first | 223.4 s | demucs 100.6 s, UVR 106.5 s, transcribe 15.1 s |
| 246 BPM, re-run | 17.3 s | stems reused, transcribe 14.3 s |
| 123 BPM, re-run | 23.1 s | stems reused, transcribe 20.9 s |

- The 123 BPM re-run gave the same 66 notes and the same stem hash as the first run.
- The demucs and UVR progress bars showed in the job status while they ran.
- Three job folders took 12 MB in all, with the stems hard-linked four ways (three jobs and the cache).

**Transcription is unchanged.** `crate::transcribe` and the stem it reads are untouched for 44.1 kHz input.
The lead path now also writes the mono vocals, but only the cell stage reads them, as before. The one
change is that a non-44.1 kHz WAV is now resampled (05, item 5).

**Remaining:**
- **2. Re-transcribe in the app:** submit by hash without an upload, a "transcribe again at … BPM"
  button, and a hint after a BPM change in the editor. The server side is now cheap; the app still
  uploads again, which is fast on localhost. It needs `startFlow`/`main.ts` work, which another agent
  is changing.
- **3. Parallel pYIN:** not started. Profile first, as the ticket says.
- **The bound is by count, not bytes.**