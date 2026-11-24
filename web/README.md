# web — sona review surface

The **interpretation** half of sona (ADR-0002): a static web piano-roll,
modeled as reference (not copy) on onlinesequencer.net. It loads the annotation
JSON from `sona-core`, plays stem-vs-synth, computes grid anchoring and
hard-snap 16th quantization live, lets the user correct flagged notes, and
exports MIDI.

The annotation JSON contract it consumes is the TypeScript in
[`lib/annotation.ts`](lib/annotation.ts), mirroring the Serde types in
`crates/sona-core/src/annotation.rs` (ADR-0004). `lib/annotation.test.ts` reads
an annotation the Rust core writes with every field
(`crates/sona-core/tests/fixtures/annotation-full.json`) into these types, so a
field renamed on either side fails a test.

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
  - [`grid.ts`](lib/grid.ts) — the grids the app offers (the grid menus, job
    options and session restore all read it), grid spacing from BPM/division,
    snapping, the hard-snap quantizer with the confidence-aware escape hatch, and
    flag state.
  - [`interpret.ts`](lib/interpret.ts) — the effective notes: each working note
    snapped to the frame's grid (or escaped), flagged and keyed, as the roll,
    the synth and every export read them.
  - [`noteEdits.ts`](lib/noteEdits.ts) — what a move, resize, add or delete does
    to a note.
  - [`session.ts`](lib/session.ts) — reading an annotation or a saved session
    into the editor's notes, baseline and settings, writing it back, and the
    undo snapshot.
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
- **Navigation.** Scroll to pan the timeline; ⌘/Ctrl-scroll to zoom horizontally
  (anchored at the cursor). Click the top ruler to **seek** — the playhead moves
  there and playback starts from that point (dragging the ruler still shifts the
  grid phase).
