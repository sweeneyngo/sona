# web — sona review surface

The **interpretation** half of sona (ADR-0002): a static web piano-roll,
modeled as reference (not copy) on onlinesequencer.net. It loads the annotation
JSON from `sona-core`, plays stem-vs-synth, computes grid anchoring and
hard-snap 16th quantization live, lets the user correct flagged notes, and
exports MIDI.

The annotation JSON contract it consumes is declared in
[`annotation.d.ts`](annotation.d.ts), mirroring the Serde types in
`crates/sona-core/src/annotation.rs` (ADR-0004) — keep the two in sync.

Static assets only — no build tooling. Serve the directory with any static
file server, e.g.:

```sh
python3 -m http.server --directory web 8000
```

Then open <http://localhost:8000>, choose an annotation JSON and its stem, and
press **Play** to hear the stem against the synthesized notes.

## Layout

- [`index.html`](index.html) — the page: toolbar (file inputs, transport, mute
  toggles), the `<canvas>` roll, and the confidence legend.
- [`main.js`](main.js) — the browser layer: JSON/audio loading, canvas
  rendering (notes, f0 contour, energy envelope, pitch grid, playhead), hover
  tooltips, and synced Web Audio playback.
- [`lib/view.js`](lib/view.js), [`lib/synth.js`](lib/synth.js) — the pure
  geometry / colour / schedule maths, kept free of the DOM so they can be
  unit-tested with `node --test web/lib/*.test.js`.

## What ticket 08 ships

Load an annotation JSON + the stem and the roll draws the notes positioned by
raw seconds, coloured by the confidence scalar (red → green heatmap), over the
f0 contour and energy envelope. Hover a note for its sub-score reason. Play
streams the stem and the synthesized notes together via Web Audio, in sync.

Grid anchoring, hard-snap quantization, manual correction, and MIDI export land
in tickets 09–10.
