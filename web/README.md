# web — sona review surface

The **interpretation** half of sona (ADR-0002): a static web piano-roll,
modeled as reference (not copy) on onlinesequencer.net. It loads the annotation
JSON from `sona-core`, plays stem-vs-synth, computes grid anchoring and
hard-snap 16th quantization live, lets the user correct flagged notes, and
exports MIDI.

The annotation JSON contract it consumes is the TypeScript in
[`lib/annotation.ts`](lib/annotation.ts), mirroring the Serde types in
`crates/sona-core/src/annotation.rs` (ADR-0004) — keep the two in sync. Because
the whole app is typed against it, a drifting field is a compile error, not a
silent bug.

Built with **Vite + TypeScript** (ADR-0009); tests run under **Vitest**.

```sh
cd web
npm install
npm run dev        # Vite dev server (HMR) at http://localhost:5173
npm test           # Vitest — the pure lib/*.test.ts
npm run typecheck  # tsc --noEmit
npm run build      # type-check + emit static site to dist/
```

Then open the dev URL, choose an annotation JSON and its stem, and press
**Play** to hear the stem against the synthesized notes. `npm run build` emits
a plain static bundle (relative asset paths), so a production deploy is still
just static files.

## Layout

- [`index.html`](index.html) — the page: toolbars (file inputs, transport, mute
  toggles, BPM/grid/flag controls, export), the `<canvas>` roll, and the legend.
- [`main.ts`](main.ts) — the browser layer: JSON/audio loading, canvas rendering
  (notes, grid + phase ruler, f0 contour, energy envelope, pitch grid,
  playhead), hover/select tooltips, note editing, synced Web Audio playback, and
  MIDI export.
- [`lib/*.ts`](lib/) — the pure DOM-free maths, unit-tested with Vitest:
  - [`annotation.ts`](lib/annotation.ts) — the core↔UI JSON contract types.
  - [`view.ts`](lib/view.ts) — pitch/colour formatting and the time/pitch scale
    (+ its inverses, for editing).
  - [`grid.ts`](lib/grid.ts) — grid spacing from BPM/division, snapping, grid
    lines, the hard-snap quantizer with the confidence-aware escape hatch, and
    flag state.
  - [`synth.ts`](lib/synth.ts) — MIDI→freq and the playback schedule.
  - [`midi.ts`](lib/midi.ts) — the format-0 Standard MIDI File encoder.

## What it ships (tickets 08–10)

- **Render (08).** Notes positioned by raw seconds, coloured by the confidence
  scalar (red → green heatmap), over the f0 contour and energy envelope.
  Hover — or click to pin — a note for its sub-score reason.
- **Grid + hard-snap (09).** A live grid from the toolbar BPM and division (1/16
  default), phase auto-anchored to the first onset and draggable via the top
  ruler. Onsets and offsets hard-snap to the grid, recomputed on any change
  (ADR-0003). A note that lands far off-grid *with* high onset confidence
  **escapes** snapping and is flagged (dashed cyan) — the tell of a real
  triplet/32nd run. The flag-threshold slider drives which notes are flagged
  (red outline).
- **Correct + export (10).** Drag a note to move it (time + pitch), drag its
  right edge to resize, double-click to add, Delete to remove. **Export MIDI**
  writes the authoritative file from the current snapped + corrected state
  (ADR-0004).
- **Playback (08).** The stem and the synthesized (snapped/corrected) notes play
  together via Web Audio, in sync.
