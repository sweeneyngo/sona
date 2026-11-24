# sona

**Turn a song into notes you can sing with.** Drop a song: sona separates the singer, writes down their
notes on the song's grid, reads their glides and vibrato, and exports the result to **OpenUtau (USTX)**,
**UTAU (UST)** or **MIDI**, all on your computer. It is built for grid-locked, pitch-precise singing such
as Vocaloid songs, and its accuracy is measured against hand-made USTs rather than eyeballed.

## Run it on your computer

sona runs locally: `sona serve` hosts the app at http://127.0.0.1:8977 and does the separation and
transcription on your machine. Nothing is uploaded. It is developed on Linux; macOS should work the same
way, and on Windows use WSL2 (neither is tested).

**You need:**
- [Rust](https://rustup.rs) via rustup. The pinned toolchain (1.88) installs itself on the first build.
- [Node.js](https://nodejs.org) 22 or newer.
- Python 3.10, 3.11 or 3.12, for the vocal separators.
- [ffmpeg](https://ffmpeg.org) on your `PATH`, to read MP3, FLAC and other non-WAV files.
- About 5 GB of disk and an internet connection for the first run (the separators and their models
  download once).

**Set up (once):**

```bash
git clone https://github.com/sweeneyngo/sona.git && cd sona
cargo build --release                       # the `sona` binary
(cd web && npm install && npm run build)    # the app that `sona serve` hosts
scripts/setup-separators.sh                 # demucs + the UVR lead-vocal separator (~4 GB, CPU)
```

**Run:**

```bash
./target/release/sona serve                 # then open http://127.0.0.1:8977
```

Drop a song on the page and type its BPM (decimals are fine). If you don't know it, click the detected
tempo, or its **½×**, beside the field. **Transcribe** does the rest. A 25 s clip takes about 80 s on a
laptop CPU; the first job is slower while the separator models download. Then edit the notes and use **Export → USTX** (OpenUtau), **UST** (UTAU) or
**MIDI**. Got the BPM wrong? Drop the same song again with the right one: the server keeps the separated
stems of recent songs, so a second run (another BPM, ½×, grid or engine) skips separation and takes only
the transcription time ([ADR-0030](docs/adr/0030-stem-cache-cleanup-and-job-failures.md)).

### Better notes with GAME (optional)

openvpi's [GAME](https://github.com/openvpi/GAME) singing-transcription model gives better notes than the
default (76.3% vs 72.5% of grid cells on fresh songs; see *How well it works*). Its weights are
CC BY-NC-SA 4.0 (non-commercial), so sona never bundles them: you download them yourself.

1. Download `GAME-1.0.3-medium-onnx.zip` (release v1.0.3) from [GAME's releases](https://github.com/openvpi/GAME/releases)
   and unzip it: `unzip GAME-1.0.3-medium-onnx.zip -d ~/models` creates `~/models/GAME-1.0.3-medium-onnx`.
2. Point `ORT_DYLIB_PATH` at an ONNX Runtime library. `setup-separators.sh` already installed one:

   ```bash
   export ORT_DYLIB_PATH="$(ls ~/.local/share/sona/separator-venv/lib/python3.*/site-packages/onnxruntime/capi/libonnxruntime.so.* | head -1)"
   # macOS: libonnxruntime.*.dylib in the same folder
   ```
3. Build with GAME and serve with it:

   ```bash
   cargo build --release -p sona-cli --features game
   ./target/release/sona serve --game ~/models/GAME-1.0.3-medium-onnx
   ```

The app picks the best engine the server has, so nothing changes on the page.

### If something goes wrong

- **The page says "Start sona serve to transcribe":** the page isn't talking to `sona serve`. Open it
  from http://127.0.0.1:8977, not from a file.
- **Under Options, only "Already a stem" can be chosen:** `scripts/setup-separators.sh` didn't
  finish. Rerun it; if your Python isn't found, set `SONA_PYTHON=/path/to/python3.12`. Custom
  locations go to `sona serve --demucs-venv DIR --separator-venv DIR`.
- **"ffmpeg could not read the file":** install ffmpeg, or drop a WAV.
- **GAME is greyed out under Options:** the binary must be built with `--features game`, and `ORT_DYLIB_PATH` must be
  set in the shell that runs `sona serve`.
- **Port 8977 is taken:** `sona serve --port 9000`.
- **"… was stopped after N s, its time limit":** a separator or ffmpeg ran far longer than the song
  should take and was stopped. If the machine is only slow, raise `--tool-timeout` (seconds) or
  `--tool-timeout-ratio` (seconds per second of audio). `sona serve --help` lists every option.
- **"sona hit an internal error":** a bug. The job's `panic.log` (path in the message) says where; the
  server keeps running.

## Using the app

- **Hear and fix.** ▶ or Space plays the singer against the notes. Drag a note to move it, drag its right
  edge to resize it, double-click to add one, Delete to remove one. **Ctrl+Z** undoes and
  **Ctrl+Shift+Z** redoes any edit: a drag, a lyric, a pen stroke or a tempo change is one step each
  ([ADR-0025](docs/adr/0025-undo-redo.md)). Colours flag the notes worth
  checking (Help explains them).
- **Tempo, grid and key** are under the **♩** button. **View** holds the pitch curve, the pitch pen and
  the mutes.
- **Bars and beat 1** ([ADR-0029](docs/adr/0029-export-frame-bar-one.md)). Bright lines are bar lines,
  numbered in the ruler as OpenUtau will number them; middle ones are beats. **Shift-click the ruler** on
  the beat you hear as beat 1 (or seek there and use **Bar 1 → At playhead** under **♩**), and that line
  becomes a bar line in the editor and in every export. Dragging the ruler shifts the grid and the bars
  move with it; **Auto** puts bar 1 back on the first grid line. Bars are 4/4.
- **Export → USTX / UST / MIDI.** The page autosaves; **File → Save session** keeps a copy.
  **File → Details…** shows what produced the notes: the sona build, the input's and models' SHA-256,
  every setting, and, for a song, each step's time and the separator versions
  ([ADR-0024](docs/adr/0024-provenance.md)).
  **File → Open files** loads an annotation from `sona transcribe` and its stem.

**Lyrics and UTAU/OpenUtau export** ([ADR-0018](docs/adr/0018-scope-utau-authoring.md)):
- **Paste lyrics.** **Edit → Lyrics…** takes a whole text and fills notes in time order, one mora per note for kana: きゃ is one mora, and ー, っ and ん are their own. Romaji and English split on spaces.
- **Type lyrics.** Select a note and press **L** to type its lyric. Enter or Tab saves it and moves to the next note.
- **Export.** **Export → UST** (UTAU; Shift-JIS, or UTF-8 when a lyric needs it), **Export → USTX** (OpenUtau) and **Export → MIDI** are written from one sequence, so every note lands on the same tick in all three, with its lyric (MIDI carries it as a lyric event, UTF-8, and a 4/4 time signature). A note without a lyric sings あ.
- **Grid alignment.** Tick 0 of every export is a bar line of the editor. By default that is the first grid line of the stem, so the vocal starts up to one grid cell earlier than the stem, as before. Once you choose bar 1 (or drag the ruler), tick 0 is the first bar line of the stem (or the bar before it, when a pickup is sung before it), and OpenUtau's beats and bars fall on the editor's. The status line says where tick 0 is in the stem.
- **Sessions.** Lyrics are saved in the session JSON.

**Pitch: portamento, vibrato and the singer's own pitch** ([ADR-0019](docs/adr/0019-pitch-expression-portamento-vibrato.md), [ADR-0021](docs/adr/0021-direct-pitch-editing-and-sung-pitch.md)):
- **Fit.** After a transcription, and on **Edit → Fit pitch**, sona reads every note's glide and vibrato from the singer's pitch. The orange **curve** is what OpenUtau will sing, drawn over the singer's pitch (white).
- **Edit on the roll.** Select a note and its pitch handles appear on the orange curve, as in OpenUtau. Circles are portamento points: drag to move, double-click the curve to add, Alt-click (or hover + Delete) to remove. Diamonds shape the vibrato: drag its start, the top corners for depth and fade-in/out, the lower one for its period. **V** toggles a note's vibrato; **Values** under the roll takes exact numbers.
- **The singer's own pitch.** **Export → USTX with the singer's pitch** adds the white line as OpenUtau's drawn pitch curve (`pitd`), measured against the notes exactly as exported, so OpenUtau sings the singer's inflections and you can keep editing them with its pitch pen. Unvoiced frames, rests and octave slips (> 600 cents off) fall back to the notes. UST has no free curve, so the plain exports keep points and vibrato.
- **Pitch pen.** Press **P** (or **View → Pitch pen**) and drag on the roll to redraw the white line; Alt-drag erases it back to the notes. Edits are saved in the session.
- **Export.** UST and USTX carry points and vibrato in OpenUtau's own form: USTX pitch points and vibrato, UST `PBS/PBW/PBY/PBM` and `VBR`.
- **How close it gets.** On songs the fitter was never tuned on, fitted curves sit 9–30% closer to the singer's pitch than flat notes; the per-song numbers are in the ADR.

## Command line

`sona transcribe` takes a **mono WAV vocal stem** and a **BPM**, and prints the annotation JSON:

```bash
# print the annotation JSON
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 128

# also write a raw, unquantized MIDI, capturing the JSON
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 128 --midi out.mid > annotation.json

# grid-locked material (Vocaloid): decode notes on a 16th grid (24 = 16th-triplets)
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 198 --grid 16 > annotation.json

# experimental: notes from openvpi GAME (ADR-0017). Download a GAME ONNX release
# (e.g. GAME-1.0.3-medium-onnx; weights are CC BY-NC-SA, non-commercial) and an
# ONNX Runtime library; a 44.1 kHz lead-vocal stem works best.
export ORT_DYLIB_PATH=/path/to/libonnxruntime.so
cargo run --release -p sona-cli --features game -- transcribe lead.wav --bpm 198 --grid 16 \
    --game ~/models/GAME-1.0.3-medium-onnx > annotation.json
```

```
Usage: sona transcribe <STEM.wav> --bpm <BPM> [--midi <PATH>] [--grid <16|24> [--no-salience] [--no-spectral-cue] [--game <DIR> [--no-octave-check]]]

Transcribes an isolated mono vocal stem to an annotation JSON (stdout).

Arguments:
  <STEM.wav>        Isolated mono vocal stem (extract with UVR or similar first).

Options:
  --bpm <BPM>       Tempo, required (finite, > 0). Carried into the JSON as a UI grid seed only.
  --midi <PATH>     Also write a raw, unquantized MIDI file. The authoritative,
                    snapped MIDI is exported from the web review surface, not here.
  --grid <16|24>    Decode notes per grid cell at --bpm (ADR-0015) instead of frame by
                    frame; the phase is estimated. Needs an exact, single tempo.
  --no-salience     With --grid: turn off harmonic-salience pitch evidence, which
                    follows the lead where other voices sing (on by default, ADR-0016).
  --no-spectral-cue With --grid: turn off the spectral cue that splits held pitches
                    at syllable changes (on by default, ADR-0016).
  --game <DIR>      With --grid: take the notes from openvpi GAME (experimental, ADR-0017;
                    build with --features game, set ORT_DYLIB_PATH, 44.1 kHz stem).
  --no-octave-check With --game: skip the subharmonic octave check (on by default).
```

`--grid` is the mode for UST/Vocaloid-style material. It includes harmonic-salience pitch evidence and
the spectral re-articulation cue by default. A plain demucs vocal stem is enough; on the holdout, a
separate lead-vocal stem added nothing (it does help GAME).

`sona serve` hosts the app and runs jobs: demucs, then the UVR karaoke lead, then pYIN or GAME notes. It
listens on 127.0.0.1 only and refuses requests for other host names ([ADR-0020](docs/adr/0020-one-click-generation-sona-serve.md)).
Options: `--port`, `--demucs-venv`, `--separator-venv`, `--game DIR`, and for disk and time:
- `--cache-dir` (default `$XDG_CACHE_HOME/sona`, else `~/.cache/sona`, never RAM-backed `/tmp`): jobs and
  the stem cache;
- `--keep-jobs N` (20) finished jobs and `--keep-stems N` (10) separated songs are kept; older ones are
  deleted between jobs, never a queued or running one;
- `--tool-timeout S` (600) / `--tool-timeout-ratio R` (20 s per second of audio): a hung tool fails its
  job instead of blocking the queue. A panic fails only its job, too ([ADR-0030](docs/adr/0030-stem-cache-cleanup-and-job-failures.md)).

**GAME** (openvpi's singing-transcription model; setup above) gives better notes. **GAME plus the
learned cell stage** is the best measured (`--cell-model DIR --vocals VOCALS.wav`, or
`sona serve --cell-model DIR`; [ADR-0026](docs/adr/0026-learned-cell-stage-in-rust.md)), but the stage
is **not distributed**: it is trained on Pifuyuu's USTs, whose author forbids redistribution, so its
weights exist only on the machine that trained them. A fresh install runs pYIN grid mode, or GAME if you
add it.

The annotation JSON is the contract between the core and the app: raw notes in seconds, a confidence
score with four sub-scores per note, the f0 contour and the energy envelope.

```jsonc
{
  "version": 1,
  "sample_rate": 44100,
  "bpm": 128.0,
  "duration_s": 12.34,
  "producer": "pyin",
  "notes": [
    {
      "onset_s": 0.50, "offset_s": 0.73, "midi_pitch": 69,
      "confidence": 0.82,
      "sub_scores": {
        "pitch_stability": 0.95, "voicing": 0.88,
        "onset_clarity": 0.79, "offset_clarity": 0.60
      }
    }
  ],
  "f0_contour":      [ { "time_s": 0.0, "frequency_hz": 440.0, "voicing": 0.9 } ],
  "energy_envelope": [ { "time_s": 0.0, "rms": 0.12 } ]
}
```

## How well it works

All numbers come from pre-registered rules, read once on songs nothing was tuned on. The process is in
[`docs/dev-workflow.md`](docs/dev-workflow.md).

**Notes against Pifuyuu's hand-made USTs** (42 Vocaloid songs; the USTs stay local, never committed).
The metric is the share of grid cells that match the UST.

| | dev (9 songs, tuned on) | fresh test |
|---|---|---|
| grid mode, pYIN (the default) | 79.5% | 70.0% (holdout2), 72.5% (holdout3) |
| GAME + octave check | 87.4% | 76.3% (holdout3) |
| + learned per-cell stage (not distributed; [ADR-0026](docs/adr/0026-learned-cell-stage-in-rust.md)) | — | **78.8%** (holdout3, with the grid phase sona estimates from onsets: [ADR-0027](docs/adr/0027-grid-phase-from-onsets.md)) |

- **Ceilings** (ADR-0017): perfect sung/rest decisions would give 91.4%, and the best available pitch per
  cell 95.6%.
- **Negative results:** GAME fine-tuned on 30 songs gave +0.8 (95% CI −0.5 to +1.9), so it is not
  adopted ([report](crates/sona-eval/reports/game-finetune.md)). Lyric-count decoding, vowel alignment
  and TiFA forced alignment were also measured and rejected
  ([report](crates/sona-eval/reports/grid-cell-decoder.md)).
- **Reproducible:** GAME's random boundary sampling is seeded and its kernels are deterministic, so
  the same song always gives the same notes; seeded dev is 87.1%
  ([ADR-0023](docs/adr/0023-deterministic-game.md)).

How grid mode got there, on the dev songs:

| dev split (9 songs), quantized, all notes | COnP F1 | COnPOff F1 | grid cells matching the UST |
|---|---|---|---|
| note-HMM, vocal stem | 0.341 | 0.133 | 53.3% |
| grid mode, vocal stem | 0.474 | 0.256 | 67.0% |
| grid mode, lead-vocal stem | 0.518 | 0.285 | 76.0% |
| **grid mode (with salience + spectral cue, now the default), vocal stem** | **0.611** | **0.388** | **79.5%** |
| grid mode (with salience + spectral cue), lead-vocal stem | 0.596 | 0.369 | 80.7% |

**Tempo** from the full mix ([ADR-0022](docs/adr/0022-tempo-estimation-and-one-step-start.md)): exact on
24/32 fresh songs (75%), within an octave on 31/32, and never at half tempo. Slow songs read at double, so
the app asks for the song's BPM and offers the estimate and its ½× only as hints
([ADR-0028](docs/adr/0028-ask-for-the-bpm-first.md)).

**Pitch:** fitted glides and vibrato sit 9–30% closer to the singer's pitch than flat notes
([ADR-0019](docs/adr/0019-pitch-expression-portamento-vibrato.md)).

**General singing (Vocadito)**, for the grid-free path. Replacing the heuristic segmenter with the
note-HMM was gated on these numbers:

| Metric (Vocadito corpus)        | Heuristic baseline | Note-HMM | Δ     |
|---------------------------------|--------------------|----------|-------|
| Note F1 — COnP (onset + pitch)  | 0.515              | 0.646    | +25%  |
| Note F1 — COnPOff (+ offset)    | 0.348              | 0.508    | +46%  |
| Onset F1 (±50 ms)               | 0.565              | —        | —     |
| Raw Pitch Accuracy              | 0.858              | —        | —     |

COnP means a correct onset (±50 ms) and pitch (±50 cents); COnPOff also requires the offset. The metrics
follow `mir_eval` and are reimplemented in Rust.

## How it works

1. **Separation** (`sona serve`): demucs, then the UVR karaoke model for the lead. Both run in their own
   Python environments, never inside the crates.
2. **Tempo:** spectral flux, then autocorrelation, then a comb searched in 140–280 BPM (ADR-0022).
3. **Notes**, in the Rust core:
   - the default is grid mode, which decodes one state per grid cell with pYIN pitch, harmonic salience
     and a spectral syllable cue (ADR-0015, ADR-0016);
   - optionally GAME notes with an octave check (ADR-0017);
   - the original note-HMM path serves grid-free material (ADR-0010).
4. **Interpretation**, in the browser: snapping to a live grid and key, per-note confidence, lyrics,
   pitch fitting, handles and the pen (ADR-0003/0004, ADR-0018 to ADR-0021).
5. **Export:**
   - UST in Shift-JIS, or UTF-8 when a lyric needs it;
   - USTX 0.7, optionally with a `pitd` curve;
   - MIDI, on the same ticks (one export seam, ADR-0029).

Every decision has an ADR in [`docs/adr/`](docs/adr/). The domain glossary is in
[`CONTEXT.md`](CONTEXT.md), and what's next is in [`docs/roadmap.md`](docs/roadmap.md).

## Development

[`docs/dev-workflow.md`](docs/dev-workflow.md) covers setup, the measure-and-confirm loop and every
check. The judgment rules are in [`docs/agents/rules.md`](docs/agents/rules.md). CI runs:
- the corpus guard (no USTs, audio or Vocadito ever tracked);
- clippy with warnings as errors;
- the Rust and web tests, including output invariants on random stems and the shared quantizer cases;
- the TLA+ models with their mutants: grid notes, the lyric editor, the pitch bar, the job queue and the
  start screen.

The browser drivers (`web/e2e/`) run locally.

```
sona/
├── crates/
│   ├── sona-core/      # perception: audio → notes, pitch, tempo (Rust)
│   ├── sona-cli/       # the `sona` binary: transcribe, serve
│   └── sona-eval/      # measurement: Vocadito, the UST corpus, tempo; holdout ledger; reports
├── web/                # the app (Vite + TypeScript); web/e2e = browser drivers
├── verification/       # TLA+ models, each with mutants, run by CI
├── scripts/            # setup-separators.sh; ci/ guards
├── docs/               # adr/, roadmap.md, dev-workflow.md, agents/
└── CONTEXT.md          # domain glossary
```
