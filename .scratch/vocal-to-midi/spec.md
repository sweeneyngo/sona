# Spec: sona — Vocal-to-MIDI (v1)

Status: ready-for-agent

## Goal

Transcribe a single isolated vocal stem into MIDI, specialized for **Vocaloid** material (staccato, pitch-precise, grid-aligned) ripped from a finished mix. Eliminate ~80% of manual transcription work; flag the rest for by-ear review. Output stops at MIDI — notation, UST/VSQX, and source separation are out of scope.

See [CONTEXT.md](../../CONTEXT.md) for the glossary and [docs/adr/](../../docs/adr/) for the load-bearing decisions. This spec assumes them.

## What "outperform basic-pitch" means

basic-pitch is a polyphonic generalist; sona is a **monophonic vocal specialist** — one voice at a time, which is the entire edge. "Outperform" = cleaner note boundaries and fewer spurious notes on isolated vocal stems, **judged by ear** in the review surface. No mir_eval, no public datasets.

## Architecture (locked)

- **Core = perception (Rust, headless).** stem (+ BPM) → **annotation JSON**: raw note events (onset/offset in *seconds*, pitch), per-note confidence (scalar + sub-scores), f0 contour, energy/RMS envelope. Invariant to musical settings. (ADR-0004)
- **UI = interpretation (web).** Piano-roll modeled on onlinesequencer.net: renders the JSON, plays stem-vs-synth, grid anchoring + **hard-snap** quantization computed live, manual correction, MIDI export. (ADR-0002)
- The **annotation JSON is the primary artifact and the core↔UI contract.** MIDI is derived from it (raw MIDI optionally from the core; authoritative corrected MIDI from the UI).
- **Pluggable `PitchTracker`**: pYIN (DSP, default) + CREPE (ONNX via `ort`). basic-pitch joins the comparison via an adapter that emits the same JSON. (ADR-0001)

## Pipeline (core)

1. **Pitch tracking** — `PitchTracker` trait; pYIN default. Per-frame f0 + voicing confidence. Median-filter the contour; smooth vibrato so it isn't read as multiple notes.
2. **Onset detection** — energy/spectral transient detection.
3. **Segmentation** — offsets via **hybrid**: next-onset primary; energy-decay fallback closes notes into rests. Pitch-plateau splitting for legato/slides. Offset confidence from **energy-drop steepness** (sharp = high, slow tail = low).
4. **Confidence** — per-dimension sub-scores (pitch stability, voicing/bleed, onset clarity, offset clarity), each 0–1, combined by **weakest-link soft-min** into a scalar. Bleed guard: penalize notes near the noise floor. Both scalar and sub-scores emitted.
5. **Emit** — annotation JSON (+ optional raw unquantized MIDI).

## Musical interpretation (UI)

- **Tempo**: single fixed BPM, user-supplied. No beat-tracking in v1.
- **Grid anchor**: auto-anchor to first detected onset; manual grid-offset drag override.
- **Quantization**: hard-snap onsets *and* offsets to a **16th** grid (default), recomputed live on grid change. Escape hatch: a note far from grid *with* high onset confidence is left + flagged, not snapped. (ADR-0003)
- **Confidence UI**: scalar drives a heatmap over the roll; sub-scores surface the *reason* ("⚠ uncertain offset"); **flag threshold is a slider**, calibrated by ear per song.
- **Correction + export**: fix flagged notes; export MIDI from current snapped state.

## Comparison harness

Every producer emits the annotation JSON — pYIN, CREPE, and basic-pitch (via adapter, mapping its note probabilities to sona's confidence). All auditioned in the same review surface over the same stem. **v1 ships single-load** (load one, listen, load the next). Side-by-side colored overlay is a fast-follow.

## Expected accuracy (target material)

Staccato Vocaloid is the easy case: clean f0, sharp offsets, grid-locked timing. The hard residue is UVR separation bleed (→ spurious notes, handled by the bleed guard + flagging) and the occasional real triplet/32nd run (→ escape hatch).

## Out of scope (v1)

Source separation (UVR, external) · notation / UST / VSQX (external site) · lyric/phoneme transcription · polyphony · beat-by-beat tempo · headless snapped-MIDI path (browser-only quantization) · overlay comparison.

## Suggested stack

pYIN via `pitch-detection`/`aubio-rs` · MIDI via `midly` · CREPE via `ort` (ONNX) · web UI: canvas + Web Audio.
