# 12 — Capabilities should check ffmpeg, the UVR model and demucs weights; surface tempo errors

Status: resolved
Type: task
Blocked by: —

## Evidence

A05 left these (`05-serve-worker-failure-modes.md` Answer): `/api/capabilities` reports separation as
available without checking that ffmpeg runs or that the UVR model file and demucs weights exist, so a first
job fails late; the client swallows a 422 from `/api/tempo`, so a bad file shows no reason.

## Proposal

Probe ffmpeg and the model files at start-up (cheap, cached), report missing pieces in capabilities with the
fix (`scripts/setup-separators.sh`), show them on the start screen; show the `/api/tempo` error text.

## How to tell

serve tests with a missing ffmpeg / model path; a vitest for the 422 path.

## Comments

## Answer

Done on branch `audit-a12-capabilities`. Recorded in [ADR-0030](../../../docs/adr/0030-stem-cache-cleanup-and-job-failures.md)
(item 5) and in the README's troubleshooting.

**What each piece needs, checked in the installed packages:**
- **ffmpeg:** audio-separator 0.48 refuses to start without it (`check_ffmpeg_installed` raises). demucs
  falls back to torchaudio for a WAV. sona needs it for anything that isn't a 44.1 kHz WAV. So without
  ffmpeg, vocals and lead are off.
- **htdemucs weights:** demucs 4.0.1 downloads `955717e8-8726e21a.th` (84 MB) on first use through
  `torch.hub` to `$TORCH_HOME/hub/checkpoints` (default `$XDG_CACHE_HOME/torch`, else `~/.cache/torch`).
- **UVR model:** audio-separator downloads `UVR_MDXNET_KARA_2.onnx` (53 MB) on first use into
  `--model_file_dir`, which is `<separator-venv>/models`.
- Both models download on first use, so a missing one is a **first-use** note that turns nothing off. A
  missing ffmpeg or venv is **missing**, and turns off the separations it blocks.

**Server** (`serve.rs`):
- `/api/capabilities` gains `setup`: a list of `{piece, status: "missing" | "first-use", blocks, message}`.
  Each message says what is wrong and how to fix it. Missing pieces come first.
- **Probes and their cost:** ffmpeg is probed once at start-up with `ffmpeg -version` (5 s limit, about
  20 ms) and the result is cached. The venvs, weights and model are file tests on each request (five
  `stat`s), so running `scripts/setup-separators.sh` or a first download shows without a restart. No
  Python runs for any of this.
- A job whose separation is blocked gets 409 with the fixes of the pieces that block it. Without ffmpeg,
  an upload that isn't a 44.1 kHz WAV gets 409 before it is queued ("install ffmpeg and restart sona serve,
  or drop a 44.1 kHz WAV"). `/api/tempo` gets 422 for a non-WAV with the same fix. Its ffmpeg error is now
  ffmpeg's last line, with the scratch path replaced by "the file". ffmpeg 7 prints several lines and
  names the path inside them.
- `sona serve` prints the same notes on stderr after its "serving" line.
- `SONA_FFMPEG` names the ffmpeg to use; the default is `ffmpeg` on `PATH`. Its folder is put first on
  the separators' `PATH`, so they use the same ffmpeg. `main.rs` is unchanged.

**App:** the start screen lists the notes under the drop zone. Missing pieces show in the ink colour
with a red "!", and first-use notes are muted. They show while idle or with a song chosen. The tempo hint
reads `no tempo: <the server's reason>` when `/api/tempo` fails. `serveClient.estimateTempo` returns
`{bpm}` or `{error}`, and the reducer has a new `tempo-failed` event with the same guard as `tempo`
(right song, still pending, the BPM untouched). The TLA+ model's states don't change: a failed estimate is
`NoTempo` (noted in `verification/start-flow/README.md`), and all four of its mutants are still caught.

**Setup script:** it checks for ffmpeg first, and then downloads the htdemucs weights
(`get_model('htdemucs')`) and the lead model (`audio-separator --download_model_only`). Both steps do
nothing when the files are present. Run on this machine with both files present, the script exited 0,
left both files unchanged, and ran nothing from pip. Without ffmpeg on `PATH` it stops at once with the
install hint.

**Tests:**
- `serve.rs::without_ffmpeg_separation_is_off_and_a_file_that_needs_it_is_refused_with_the_fix`:
  - the capabilities show `none` only, with an ffmpeg note marked `missing` that blocks vocals and lead;
  - vocals and lead get 409, and so do a 48 kHz WAV and an "mp3", each with "install ffmpeg";
  - a 44.1 kHz WAV still ends `done`;
  - tempo gets 422 for the mp3 and 200 for a WAV.
- `serve.rs::a_missing_lead_separator_or_model_is_reported_with_its_fix`:
  - `--separator-venv` points at a missing folder: vocals stays on, lead is off, and the note (listed
    first) and the 409 both name `setup-separators.sh lead` and `--separator-venv`;
  - with an empty `TORCH_HOME`, a weights note is `first-use` and blocks nothing;
  - once the file is written, the note is gone without a restart.
- The lead test asserts the stub venv's missing model is `first-use` with lead still on. Its demucs
  stub fails if the `SONA_FFMPEG` folder isn't on its `PATH`.
- The tempo test asserts the 422 text is one line, without the scratch path.
- The stub servers use an `ffmpeg-or-stub` wrapper. It is the real ffmpeg where there is one, and
  otherwise answers only `-version`, so CI without ffmpeg passes the probe as before. With `PATH` cut down
  to `sh mkdir cp sleep cat rm` (no ffmpeg), all 13 serve tests pass.
- Vitest:
  - `estimateTempo` on a 422 returns the server's text, and an unreachable server gives the
    "isn't reachable" text;
  - `parseCapabilities` keeps good setup notes and drops malformed ones;
  - the reducer shows the reason for the song on screen only;
  - the 3 000 random event orders now include `tempo-failed`, with the property that a shown error
    belongs to the shown song.

**Mutants (each caught):**
- serve: ffmpeg probe always passes; no ffmpeg check on upload; separator venv not checked; weights not
  checked; lead model not checked; a first-use note that blocks; no ffmpeg check on `/api/tempo`;
  `SONA_FFMPEG` not put on the separators' `PATH`; the tempo error keeps the raw ffmpeg text.
- web: `estimateTempo` swallows the error as "no beat" (the old behaviour); `tempo-failed` treated as
  no beat; a stale song's error shown; setup notes dropped by the parser.

**Not done:** the GAME and cell-model checks are unchanged (the engine options still grey out with no
note). The pre-existing `--no-default-features` clippy warning (`Queue::root` unused) is not touched;
CI doesn't run that configuration.
