# sona

A **monophonic vocal-to-MIDI transcriber**.
Turn a single isolated vocal stem — ripped from a finished mix — into a clean MIDI melody, specialized for staccato, pitch-precise, grid-aligned singing (Vocaloid and the like).

sona splits the problem in two: a headless Rust **perception** core reads the audio and emits raw note events, per-note confidence, an f0 contour, and an energy envelope as a single JSON annotation; a web **interpretation** surface renders that JSON as a piano-roll, plays the stem against a synthesized version, snaps notes to a live grid and key, and exports the corrected MIDI. The core stays invariant to musical settings, so the UI can re-grid and re-audition freely without re-running any DSP.

The pipeline is pure Rust with no ML runtime and no C dependency, and no FFT on the default (note-HMM) path: hand-rolled pYIN pitch tracking, energy-novelty onset detection, a Tony-style note-HMM (Viterbi) for segmentation, and a weakest-link confidence score that tells you *where to listen* instead of asking you to re-check the whole song. For grid-locked songs, an opt-in **grid mode** decodes notes per grid cell instead of per frame (see *In-domain* below).

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

### In-domain: how close to a hand-made UST?

Vocadito is general singing. sona's target is grid-locked Vocaloid songs, so it is also gated against **Pifuyuu's UTAU USTs**: 20 songs, 9 dev, 9 holdout, 2 watched or excluded. The USTs are fetched locally and never committed. Two findings changed how sona should be run on that material.

1. **Decode on the grid.** Most frame-level misses come from consonants unvoicing a note's start, from pitch glides, and from the 100 ms prune floor. Grid mode decodes one state per grid cell instead (`--grid`, [ADR-0015](docs/adr/0015-grid-conditioned-cell-decoding.md)) and estimates the grid phase from the stem. It was confirmed on the holdout, read once against a pre-registered rule: COnP F1 0.340 → **0.455**, COnPOff F1 0.158 → **0.303**, with no song regressing.
2. **Follow the lead through other voices.** Two thirds of a UST's notes sit where harmony or doubling also sings, and two voices break pYIN.
   - Harmonic-salience pitch evidence (on by default in grid mode) finds the lead's f0 in 63% of those frames, against pYIN's 32%.
   - A lead-vocal stem from a UVR karaoke model helps too, and the two combine.
3. **Split held pitches at syllable changes.** A spectral model (also on by default) separates a new syllable on a held pitch from a sustained note far better than voicing or energy (AUC 0.80 against 0.69).

Items 2 and 3 ([ADR-0016](docs/adr/0016-grid-evidence-salience-and-spectral-cue.md)) were found by a 4-worker parallel search on the dev songs. A **fresh holdout** of 10 songs nothing was tuned on confirmed them: cell accuracy 52.8% → **70.0%** and note F1 0.330 → 0.514, with every song improving. Both are now grid mode's defaults. The holdout also showed that lead-vocal stems add nothing once salience is on, so a plain demucs vocal stem is enough.

| dev split (9 songs), quantized, all notes | COnP F1 | COnPOff F1 | grid cells matching the UST |
|---|---|---|---|
| note-HMM, vocal stem | 0.341 | 0.133 | 53.3% |
| grid mode, vocal stem | 0.474 | 0.256 | 67.0% |
| grid mode, lead-vocal stem | 0.518 | 0.285 | 76.0% |
| **grid mode (with salience + spectral cue, now the default), vocal stem** | **0.611** | **0.388** | **79.5%** |
| grid mode (with salience + spectral cue), lead-vocal stem | 0.596 | 0.369 | 80.7% |

**Where the rest goes.** With perfect note boundaries, sona's pitch contour would score 0.91 monophonic COnP F1, so the pitch information is there. What's left:
- **Same-pitch repeated syllables:** no acoustic cue tried separates them well.
- **Separation failures:** two songs whose stems are mostly bass or an octave double.

**GAME note extraction (experimental, opt-in).** openvpi's GAME singing-transcription model, run through ONNX Runtime (`--features game`, [ADR-0017](docs/adr/0017-game-note-extraction.md)), is the biggest single gain found.
- **Dev:** cells 79.5% → 87.4% on lead stems.
- **Fresh holdout (holdout3, 10 songs, read once):** 72.5% → 76.3% (+3.8, 9 of 10 songs better). That is short of the pre-registered +5 bar, because one song where every system fails (ironication) loses 10 points. So it stays experimental.
- **Two additions confirmed on that holdout:**
  - a subharmonic octave check, +0.3 with no song worse;
  - a learned per-cell stage trained on Pifuyuu's USTs, +2.2 (95% interval +0.8 to +3.5).
- **Full stack on holdout3:** **78.8%** against 72.5% for today's default.
- **Licence:** GAME's weights are CC BY-NC-SA 4.0 (non-commercial), so you download them yourself; they are never bundled. The learned stage's weights stay local.

Three uses of lyrics (note counts, count-constrained decoding, vowel alignment) were tried and measured, and none beat grid mode yet. Forced alignment of published lyrics with openvpi TiFA (Vocal2Midi's lyric path) was also measured and rejected. All findings, ceilings and negative results: [`crates/sona-eval/reports/grid-cell-decoder.md`](crates/sona-eval/reports/grid-cell-decoder.md).

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

`--grid` is the mode to use for UST/Vocaloid-style material. Without it, sona stays on the grid-free perception path. Grid mode includes harmonic-salience pitch evidence and the spectral re-articulation cue by default (`--no-salience` and `--no-spectral-cue` turn them off). Grid cells matching the UST:
- about 80% on the dev songs;
- 70% on a fresh holdout, against 53% for plain grid mode.

A plain demucs vocal stem is enough: on the holdout, a separate lead-vocal stem added nothing.

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

# against the UST corpus (fetch USTs + audio, then separate; see the scripts' headers)
cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --grid        # the gate, dev split
cargo run --release -p sona-eval --bin sona-eval-ust-dev -- --stem lead --cfg base:grid=est --cfg try:grid=est,g.change_cost=6 --per-song
```

The holdout split is read only on purpose: `--include-holdout --reason "…"` on the gate, or `--holdout-once --reason "…"` on the dev harness. Every read is logged in `crates/sona-eval/corpus/holdout-reads.jsonl`, and a second read of the same songs is refused without `--reread`.

### Checks

CI (`.github/workflows/ci.yml`) encodes what the project has learned, so no one can regress it by forgetting:
- **No corpus data:** nothing from the UST corpus, audio or Vocadito is ever tracked.
- **Output invariants:** on random stems, both decoders always emit one sorted, non-overlapping voice with valid scores.
- **TLA+ model check:** grid note assembly plus the review surface's snap, including mutants that must be caught.
- **Shared cases:** the quantizer's TypeScript and Rust ports agree.
- **Clippy** with warnings as errors.

The rules that need judgment (dev and holdout discipline, measure an oracle first, opt-in defaults, tests that claim behaviour) are in [`docs/agents/rules.md`](docs/agents/rules.md).

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
├── verification/    # TLA+ models (grid note assembly + snap), run by CI
├── scripts/ci/      # repo guards run by CI
├── docs/adr/        # architecture decision records (the load-bearing "why")
├── docs/agents/     # rules and conventions for contributors and coding agents
└── CONTEXT.md       # domain glossary
```
