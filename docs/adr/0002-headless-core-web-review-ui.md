# Headless Rust core, thin web review-and-correct UI

**Status: Accepted.** Refined by [ADR-0004](0004-core-emits-raw-perception-ui-interprets.md) (the JSON is the primary artifact; the core's MIDI is a raw convenience) and widened by [ADR-0018](0018-scope-utau-authoring.md) (UST/USTX export).

The transcription engine is a headless Rust library/CLI that takes a vocal stem (+ BPM) and emits a MIDI file plus a JSON annotation (notes, per-note confidence, f0 contour). Evaluation and manual correction happen in a separate thin web UI — a piano-roll modeled on onlinesequencer.net (as a reference, not a copy) that renders the JSON on a `<canvas>`, color-codes notes by confidence, plays stem-vs-synthesized-MIDI via Web Audio, lets the user fix flagged notes, and exports the corrected MIDI.

Chosen over a native Rust GUI because canvas + Web Audio make piano-roll rendering and synced dual-source playback nearly free, letting effort go into the transcription rather than GUI plumbing — while keeping the engine "solely Rust" where it matters. The browser is just a viewer/editor of the core's output, the way a DAW would be, except it can render sona's own confidence data (which a DAW cannot).

## Consequences

- Not a single double-click native binary; it's a Rust process that emits files plus a small local web page.
- The JSON annotation is the seam between core and UI and must be designed as a stable contract.
- The web UI carries editing + MIDI export, since it's where the remaining ~20% manual correction lands.
