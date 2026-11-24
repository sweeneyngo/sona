# 04 — Re-transcribing costs a full re-run: reuse separated stems, parallelise pYIN, clean the jobs dir

Status: needs-triage
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
