# 08 — Web UI: piano-roll render + playback

Status: ready-for-agent
Blocked by: 07

Static web piano-roll (onlinesequencer.net as reference, not a copy). Load an annotation JSON + the stem audio. Render notes on a `<canvas>` positioned by raw seconds; draw the f0 contour + energy envelope underneath as guides. Color notes by confidence scalar (heatmap); show sub-score reason on hover/select. Play stem and synthesized MIDI via Web Audio, synced.

No grid/quantization/editing yet — that's tickets 09–10.

Done when: loading a JSON + stem shows the notes over the audio and plays both in sync, confidence-colored.
