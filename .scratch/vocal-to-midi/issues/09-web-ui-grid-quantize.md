# 09 — Web UI: grid anchor + live hard-snap quantization

Status: ready-for-agent
Blocked by: 08

Add the grid and quantization in the UI (ADR-0003, ADR-0004 — snap lives UI-side).

- Draw the grid from BPM at a **16th** default (selectable). Auto-anchor phase to the first detected onset; let the user **drag the grid offset** to override (pickup notes / leading silence).
- **Hard-snap** onsets and offsets to the grid, recomputed live whenever BPM/grid/offset changes.
- **Escape hatch**: a note far from the grid *with* high onset confidence stays put and is flagged, not snapped.
- **Flag-threshold slider**: notes below it are visually flagged for review.

Done when: dragging the grid re-snaps instantly; the escape hatch spares a confident off-grid note; the slider changes what's flagged.
