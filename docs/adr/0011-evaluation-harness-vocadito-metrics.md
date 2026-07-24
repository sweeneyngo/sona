# Transcription quality is measured against labeled vocal data, not judged by ear

**Reverses a spec decision.** [`.scratch/vocal-to-midi/spec.md`](../../.scratch/vocal-to-midi/spec.md) states, under *What "outperform basic-pitch" means*: "judged by ear in the review surface. **No mir_eval, no public datasets.**" This ADR overturns that. The by-ear stance was right for bootstrapping a v1; it is wrong now.

## Context

Every tuning decision to date — the onset/segment default relaxations, the UI slope filter, the MAD outlier trim — was made by eyeballing the piano roll. That approach has run out of road:

- It **can't attribute error to a stage.** "Severe lack of notes" is ambiguous between onset recall (stage 2) and over-merging in segmentation (stage 3); the fixes are opposite, and the eye can't tell which is at fault.
- It **can't catch regressions.** Relaxing defaults for normal vocals may have silently degraded the staccato Vocaloid case the spec targeted; nobody would know.
- It **can't tune the note-HMM.** ADR-0010 introduces a Viterbi self-transition probability whose failure mode (real notes vanishing) is invisible without a recall number. The 85 existing tests are all synthetic pure-sine tones — they prove the code runs, not that it transcribes vocals.

The AMT literature (Benetos et al., 2019) places sona squarely at **note-level** transcription and points to the standard MIREX / `mir_eval` metric families as the sanctioned way to measure it.

## Decision

Adopt an **offline evaluation harness** with labeled vocal data and per-stage metrics.

**Data.**
- **Vocadito** (Zenodo record 5578807) — a small, vocal-specific, permissively-licensed set with note- and f0-level annotations. Primary corpus.
- **3–5 committed golden fixtures** — short WAV clips + expected annotation JSON, covering clean legato, staccato/Vocaloid, vibrato, and separation bleed. Small enough to live in git and run as ordinary tests; they are the fast regression signal.
- The Vocadito corpus is **not** committed (size + licensing): a manifest + download script fetch and cache it locally; the corpus-backed metrics are gated so they don't run in the normal `cargo test` fast path.

**Metrics — reported per stage**, so a number points at the stage that owns it (mirroring `mir_eval` conventions so results are comparable to the literature):
- **Pitch / f0:** Raw Pitch Accuracy, **Raw Chroma Accuracy** (octave-insensitive — isolates the octave errors ADR-0010's interval prior targets), Voicing Recall / False Alarm. ±50-cent tolerance.
- **Onsets:** Precision / Recall / F1 at **±50 ms**.
- **Notes:** Precision / Recall / F1 as **COnP** (correct onset + pitch) and the stricter **COnPOff** (+ offset). ±50 ms onset, ±50-cent pitch, offset within 50 ms or 20 % of duration.
- **Confidence calibration:** bucket notes by their confidence scalar and measure the note error rate per bucket. sona's whole thesis (ADR-0004/0008) is "emit raw, flag the uncertain bits," so *whether the flag predicts the wrong notes* is a first-class metric — arguably the most important one, and one raw F1 doesn't capture.

Benetos's caveat is recorded and heeded: these metrics don't fully match perceptual quality (an extra note may be over-penalized vs a missed one). The confidence-calibration metric and the by-ear review surface remain complementary checks, not casualties — this ADR adds measurement, it doesn't remove listening.

**Where the code lives.** A separate **`sona-eval`** crate (or `xtask` target), so dataset/metric code and its dependencies never leak into the headless `sona-core` library. The handful of metrics are reimplemented in pure Rust (matching the no-heavy-dep stance) and validated against `mir_eval` on a couple of fixtures once, offline.

## Consequences

- Establishes a **baseline number for the current ADR-0007 heuristic segmenter** before any change — the gate ADR-0010 must beat.
- Turns tuning mechanical: sweep `Default` params (and the HMM self-transition knob) against the corpus instead of by eye; named presets (legato / staccato), if any, are then backed by F1, not vibes.
- Golden fixtures give a cheap regression wall in the normal test suite; the Vocadito-backed run is opt-in (feature flag / ignored-by-default test / `xtask`) so unit tests stay fast and offline.
- The spec's *Comparison harness* section (single-load, by-ear audition of pYIN vs CREPE vs basic-pitch) is unaffected and complementary — this ADR measures a single producer's accuracy against ground truth; that one compares producers by ear.
- Adds a `sona-eval` crate to the workspace and a local (git-ignored) data cache; CI either skips the corpus run or restores it from cache.
