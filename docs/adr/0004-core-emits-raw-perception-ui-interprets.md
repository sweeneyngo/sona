# Core emits raw perception; the UI does musical interpretation

**Status: Accepted.** Amended by [ADR-0015](0015-grid-conditioned-cell-decoding.md): grid mode decodes on a grid the core is given, so its notes are not invariant to the BPM and division. `sona serve` uses it for every job (ADR-0020); bare `sona transcribe` stays grid-free.

The Rust core is purely *perceptual*: it takes a stem (+ BPM) and emits JSON — raw note events (onset/offset in **seconds**, pitch, a confidence scalar + per-dimension sub-scores) plus display aids (f0 contour, energy/RMS envelope). All *musical interpretation* — grid anchoring, hard-snap quantization, manual correction, MIDI export — happens in the web UI, recomputed live as the user drags the grid.

Chosen because the manual grid-offset override (ADR-0002 / grilling Q7) requires quantization to re-run instantly on grid changes, which is only possible if snapping lives UI-side over raw timings. It also keeps the core a clean headless deep module whose output is invariant to musical settings — exactly what the backend benchmark needs (run the core once per backend, then re-grid/audition freely).

## Consequences

- **The JSON is the primary artifact; MIDI is a UI export.** The core may emit a raw unquantized MIDI for CLI convenience, but the authoritative, corrected MIDI comes out of the UI. This reframes the spec's "output = MIDI file."
- Quantization logic lives in the UI (JS/TS). A pure-headless "stem → snapped MIDI, no browser" path would require reimplementing snap in Rust. **Accepted as out of scope for v1** — browser-only quantization is fine to start.
- The JSON contract is now load-bearing and versioned; anything that can emit it (including basic-pitch via an adapter) becomes auditable in the same review surface.
