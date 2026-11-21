# sona

A **monophonic vocal-to-MIDI transcriber**.
Turn a single isolated vocal stem — ripped from a finished mix — into a clean MIDI melody, specialized for staccato, pitch-precise, grid-aligned singing (Vocaloid and the like).

sona splits the problem in two: a headless Rust **perception** core reads the audio and emits raw note events, per-note confidence, an f0 contour, and an energy envelope as a single JSON annotation; a web **interpretation** surface renders that JSON as a piano-roll, plays the stem against a synthesized version, snaps notes to a live grid and key, and exports the corrected MIDI. The core stays invariant to musical settings, so the UI can re-grid and re-audition freely without re-running any DSP.

The pipeline is pure Rust with no ML runtime, no FFT, and no C dependency on the default path: hand-rolled pYIN pitch tracking, energy-novelty onset detection, a Tony-style note-HMM (Viterbi) for segmentation, and a weakest-link confidence score that tells you *where to listen* instead of asking you to re-check the whole song.

Accuracy is measured, not eyeballed. A separate `sona-eval` crate scores the pipeline against labeled vocal data (the Vocadito corpus + committed golden fixtures) with per-stage, `mir_eval`-style metrics. Replacing the original heuristic segmenter with the note-HMM was gated on these numbers:

| Metric (Vocadito corpus)        | Heuristic baseline | Note-HMM | Δ     |
|---------------------------------|--------------------|----------|-------|
| Note F1 — COnP (onset + pitch)  | 0.515              | 0.646    | +25%  |
| Note F1 — COnPOff (+ offset)    | 0.348              | 0.508    | +46%  |
| Onset F1 (±50 ms)               | 0.565              | —        | —     |
| Raw Pitch Accuracy              | 0.858              | —        | —     |

**Notes:**

- **COnP** = correct onset (±50 ms) and pitch (±50 cents); **COnPOff** additionally requires the offset within `max(20% of duration, 50 ms)`.
- Metrics mirror `mir_eval` conventions and are reimplemented in pure Rust (cross-checked once, offline, against `mir_eval` 0.8.2).
- Fixtures run in the normal `cargo test`; the corpus-backed run is opt-in (fetch script + `--corpus` flag).

---

## Installation

Clone the repo and build both halves — the Rust workspace and the web review surface:

```bash
git clone https://github.com/sweeneyngo/sona.git
cd sona
cargo build                       # sona-core, sona-cli, sona-eval
(cd web && npm install)           # the review surface
```

## Usage

### Transcribe a stem (CLI)

The `sona` binary takes a **mono WAV vocal stem** and a **BPM**, and prints the annotation JSON to stdout:

```bash
# print the annotation JSON
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 128

# also write a raw, unquantized MIDI, capturing the JSON
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 128 --midi out.mid > annotation.json

# grid-locked material (Vocaloid): decode notes on a 16th grid (24 = 16th-triplets)
cargo run -p sona-cli -- transcribe path/to/stem.wav --bpm 198 --grid 16 > annotation.json
```

```
Usage: sona transcribe <STEM.wav> --bpm <BPM> [--midi <PATH>] [--grid <16|24>]

Transcribes an isolated mono vocal stem to an annotation JSON (stdout).

Arguments:
  <STEM.wav>        Isolated mono vocal stem (extract with UVR or similar first).

Options:
  --bpm <BPM>       Tempo, required (finite, > 0). Carried into the JSON as a UI grid seed only.
  --midi <PATH>     Also write a raw, unquantized MIDI file. The authoritative,
                    snapped MIDI is exported from the web review surface, not here.
  --grid <16|24>    Decode notes per grid cell at --bpm (ADR-0015) instead of frame by
                    frame; the phase is estimated. Needs an exact, single tempo.
```

`--grid` is the mode to use for UST/Vocaloid-style material. On the in-domain UST corpus it nearly doubles note recall and lifts COnPOff F1 from 0.14 to 0.28 ([report](crates/sona-eval/reports/grid-cell-decoder.md)). Without it, sona stays on the grid-free perception path.

The annotation JSON is the core↔UI contract — raw note events in *seconds*, a confidence scalar plus four sub-scores per note, and display aids:

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

### Review, correct, and export (web)

Run the Vite dev server, then load the annotation JSON alongside its stem:

```bash
cd web
npm run dev        # http://localhost:5173 (HMR)
```

Open the app, use **Settings (⚙)** to choose the annotation JSON and its stem audio, set the BPM and grid division, and press **Play** to hear the stem against the synthesized notes. Notes are colored by a confidence heatmap (red → green); low-confidence and off-grid notes are outlined and flagged. Drag to move, resize, double-click to add, Delete to remove, then **Export MIDI** to write the authoritative file. Use **Save** to keep a session JSON if you want to resume editing later.

### Evaluate (optional)

```bash
# fast, offline: the committed golden fixtures
cargo run -p sona-eval --bin sona-eval-report

# against the real Vocadito corpus
crates/sona-eval/scripts/fetch-vocadito.sh
cargo run -p sona-eval --bin sona-eval-report -- --corpus
cargo run --release -p sona-eval --bin sona-eval-sweep -- --corpus
```

## Building

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable, edition 2021). No C toolchain or model download needed for the default path.
- [Node 22](https://nodejs.org/) or later — for the web review surface only.

```bash
git clone https://github.com/sweeneyngo/sona.git
cd sona
cargo build --release                        # workspace binaries (target/release/sona)
(cd web && npm install && npm run build)     # static review surface in web/dist
cargo test                                   # fast, offline test suite
```

## Layout

```
sona/
├── crates/
│   ├── sona-core/   # perception: stem → annotation JSON (pYIN, onsets, note-HMM, confidence)
│   ├── sona-cli/    # the `sona` binary
│   └── sona-eval/   # evaluation harness: per-stage metrics vs labeled data
├── web/             # interpretation: Vite + TypeScript piano-roll review surface
├── docs/adr/        # architecture decision records (the load-bearing "why")
└── CONTEXT.md       # domain glossary
```
