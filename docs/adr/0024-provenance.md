# ADR-0024: Every result says what produced it

Status: accepted (2026-11-23)

## Context

A transcription depends on more than the song:
- the sona build;
- the note engine and its settings;
- the exact model files;
- for `sona serve`, the separators and their versions.

Until now none of that was recorded. Comparing two results, or reproducing one, meant trusting memory.
Determinism (ADR-0023) only pays off if a result also says which inputs it is deterministic in.

## Decision

1. **Annotations carry `provenance`** (optional, so older files still load; the schema stays version 1):
   - `sona_version`, and `sona_commit` from `git describe --always --dirty` at build time;
   - `input`: name, size and SHA-256 of the stem;
   - `settings`: every setting that changes the notes. For GAME these include the presence threshold,
     diffusion steps, segmenter seed, deterministic kernels and the ONNX Runtime library;
   - `models`: name, size and SHA-256 of each GAME graph.

   SHA-256 (the `sha2` crate, pure Rust) matches `sha256sum`, so anyone can check a file by hand.
2. **`sona serve` jobs add a `job` record:**
   - the upload's name, size and SHA-256;
   - each step's duration;
   - the tool versions (demucs and its torch, audio-separator and its onnxruntime, the lead model's
     name, ffmpeg);
   - the lead model file's hash.

   The record is embedded in the annotation and written to `job.json` beside the results, also for a
   failed or cancelled job. It is served at `GET /api/jobs/{id}/provenance`. Tool versions are read once
   at server start in the background, with a 20 s limit per tool, so a hung tool never holds the queue;
   a tool that can't be read is recorded as `unknown`.
3. **In the app**, *File → Details…* shows it: the build, the input and upload, settings, models, step
   times and tools, with hashes shortened. *Download* saves the full JSON. Provenance survives sessions
   and autosaves.

4. **demucs's random time shift is seeded** (`random.seed(0)` before `demucs.separate.main`).
   - **Found by:** provenance itself. Two app runs of one song gave different stem hashes, and earlier
     app runs gave 67, 69 and 70 notes.
   - **Cause:** demucs's default applies one random shift of up to 0.5 s from Python's unseeded `random`.
   - **Why seed rather than disable:** the corpus stems were made with that same default, so seeding
     keeps the method and fixes only the draw. The seed is recorded as `demucs_shift_seed`.

## Evidence

- **Rust:** fingerprints match `sha256sum` (unit test, and on the real stem and GAME files); a tool that
  fails, hangs or is missing reads as `unknown` within its time limit.
- **Server** (`tests/serve.rs`):
  - a finished job's annotation names the exact stem and upload by hash, with steps in order (queued,
    separating vocals, separating the lead, transcribing) and non-negative times;
  - a stub without demucs records `unknown` rather than guessing;
  - `/provenance` returns the same record, and a failed job still returns its record, with the error and
    the steps it reached.
- **End to end** (`sona serve`, the same 25 s song twice, lead separation, GAME): 71 notes with
  identical hashes both times.
  - The stems still differ by last-bit rounding, from multi-threaded torch and ONNX Runtime in the
    separators: at most one 16-bit step (3e-5), on 0.2–1.8% of samples, about 100 dB below the signal.
  - Provenance shows this: the stem hashes differ while the notes match. Making separation bit-exact
    would need single-threaded separators, which is much slower, so it isn't done.
- **Regression found on the way:** the first version read tool versions on the job path. A slow tool
  then held the queue for its whole time limit; the existing cancel test caught it. Reading versions in
  the background at start-up fixed it.
- **Web:** the parser keeps only well-formed parts and returns nothing for older annotations; the Details
  layout is unit-tested; a real job's provenance shows in the browser (`web/e2e/song_to_notes.py`).

## Consequences

- An annotation can be checked against its inputs with `sha256sum`, and two annotations can be compared
  setting by setting.
- The `-dirty` flag is best-effort: it updates when a crate's sources, the git index or HEAD change.
- Exports (UST, USTX, MIDI) don't carry provenance; the session and *Details → Download* do.
