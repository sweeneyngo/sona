# One-click generation: `sona serve`

**Status: Accepted (2026-11-22).** This is the third milestone of ADR-0018. Amended by ADR-0030 (stem
cache, jobs folder and clean-up, panics and tool deadlines).

## Context

Getting from a song to an editable project took four separate tools:
1. demucs for the vocals;
2. the UVR karaoke model for the lead;
3. `sona transcribe` for the annotation;
4. the review surface, where the user loaded both files by hand.

The separators are Python, with ~2 GB of venvs. Transcription is Rust; GAME additionally needs ONNX
Runtime and user-downloaded weights. None of that can run inside a browser.

## Decision

1. **A `sona serve` subcommand** (`crates/sona-cli/src/{serve,jobs,http}.rs`) serves the built review
   surface and a small job API. It listens on 127.0.0.1 only:
   - `GET /api/capabilities`: which separations and note engines this machine has.
   - `POST /api/jobs?bpm&grid&separation&engine&name`, with the audio file as the request body.
   - `GET /api/jobs/{id}`: phase, elapsed time, log tail, error.
   - `DELETE /api/jobs/{id}`: cancel.
   - `GET /api/jobs/{id}/annotation` and `/stem`: results, only for a job that finished cleanly.
2. **Pipeline.**
   1. Separate: demucs `htdemucs --two-stems vocals`, then for the lead `audio-separator` with
      `UVR_MDXNET_KARA_2.onnx`. These are the same tools and models as the corpus scripts, from the same
      venvs (`--demucs-venv`, `--separator-venv`).
   2. Downmix the stem to mono in-process (`hound`). ffmpeg is needed only for a compressed upload with no
      separation.
   3. Transcribe in-process with the CLI's own `transcribe` (pYIN grid mode, or GAME with `--game DIR`).
   4. Run one job at a time on a single worker. Separation is CPU-heavy, and a queue keeps the machine usable.
3. **Cancellation** kills the running tool's process. A job's phase is absorbing once final (done, failed or
   cancelled), so a step that finishes after a cancel can't overwrite it.
4. **Safety.**
   - It binds to 127.0.0.1.
   - It answers only requests whose `Host` is its own address, which blocks DNS-rebinding pages.
   - The HTTP layer is std-only and hand-rolled, in keeping with the repo's minimal-dependency choices. It
     accepts `Content-Length` bodies only (chunked is refused), with a 16 KB head and a 500 MB upload limit.
   - Static files come only from the web build, and any `..` or backslash path is refused.
   - Job ids name server-made folders, never paths.
5. **The review surface** gains **＋ From audio**. It shows the server's capabilities, uploads the file,
   shows the phase and log live, can cancel, and on completion loads the annotation and stem itself, setting
   the BPM and grid. It asks before replacing unsaved edits. In development, Vite proxies `/api` to
   `sona serve`.

## Evidence

- **Real run.**
  - **Setup:** a 25 s MP3 excerpt of telescope-nevermore's original mix, at 123 BPM. Lead separation, GAME,
    CPU only.
  - **Timing:** demucs 14 s, UVR 16 s, GAME 12 s. Results were loaded and playable 46 s after Start, with no
    console errors.
- **Tests:**
  - HTTP parsing and limits: unit tests.
  - `crates/sona-cli/tests/serve.rs` runs the real binary over TCP, with stub separators that emulate demucs
    and audio-separator's command lines and output layout. It covers:
    - the lead path to an A4/B4 annotation and a mono stem;
    - a separator failure surfacing its own message;
    - a cancel that stops a 30 s separator, frees the worker for the next job, and never reports done;
    - refusal of foreign hosts, bad parameters, empty uploads and path traversal.
  - Each guard is shown to matter by a mutant the tests catch: no host check, no path sanitising, a cancel
    that doesn't kill. The last one first survived the tests; they were strengthened to observe the freed
    worker.
- **Model:** `verification/serve-jobs` (TLA+, in CI). A cancelled or failed job never reports done, results
  only come from done jobs, and the worker never runs a finished job. Three checked mutants.

## Consequences

- Separation still needs the Python venvs on the machine. The dialog shows what is missing, and
  `none` (an existing stem) always works.
- The BPM is still typed by the user: tempo estimation is a separate, future piece.
- Uploads and results stay in a temp folder (`--jobs-dir`) for the server's lifetime; nothing is sent anywhere.
- GAME and the separator models keep their own licences (GAME weights CC BY-NC-SA). `sona serve` only runs
  what the user installed.
