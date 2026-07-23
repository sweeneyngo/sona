# 03 — PitchTracker trait + pYIN backend

Status: ready-for-agent
Blocked by: 02

Define the `PitchTracker` trait (ADR-0001) and implement pYIN as the default backend. Output per-frame f0 + voicing confidence. Median-filter the f0 contour to kill jitter/octave errors; smooth vibrato at the contour stage so it isn't later read as multiple notes.

The trait is the seam that makes CREPE (ticket 09) and comparison possible — design it so a backend is `audio → (f0 contour, voicing)` with nothing musical baked in.

Done when: pYIN produces a clean f0 contour + voicing on a test vocal stem; contour is exposed for the JSON's `f0_contour`.
