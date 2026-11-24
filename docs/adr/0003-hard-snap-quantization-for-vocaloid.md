# Hard-snap quantization (deviation from the spec's soft-snap)

The spec cautions against over-snapping and recommends a tolerance band that preserves off-grid notes as possible expressive timing. sona deliberately does the opposite: **hard-snap onsets and offsets to the grid (16th default) by default**, with a confidence-aware escape hatch — a note that lands far from the grid *and* has high onset confidence is left in place and flagged, rather than snapped.

This is correct for sona's target material (Vocaloid), which is sequenced from grid-aligned note data. There, off-grid deviation is almost always *tracking error*, so snapping is corrective; the spec's soft-snap would lovingly preserve sona's own onset jitter. The escape hatch covers genuine triplets/32nd runs, whose tell is "far from grid but confidently detected."

The hatch serves grid-free (note-HMM) annotations. Grid producers already place notes on the grid, so it can't fire for them: on a full dev song through pYIN grid mode (innocent-addict, 735 notes) no note reaches `onset_clarity ≥ 0.8` (audit A07). GAME and the learned cell stage were not measured.

Recorded so the deviation from the spec reads as deliberate, not accidental.
