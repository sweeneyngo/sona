# 06 — Confidence scoring

Status: ready-for-agent
Blocked by: 05

**Spec priority #1 — highest leverage.** Compose per-note confidence.

- Four sub-scores, each 0–1: `pitch_stability` (from f0 variance over the note), `voicing` (pYIN voicing + **bleed guard**: penalize notes hovering near the noise floor), `onset_clarity` (transient sharpness), `offset_clarity` (from energy-drop steepness — sharp = high, slow tail = low).
- Combine by **weakest-link soft-min** into the scalar. Emit scalar *and* sub-scores.
- Exact per-sub-score formulas are empirical — tune against real Vocaloid stems.

Done when: notes carry a scalar + four sub-scores; a deliberately spurious bleed note and an ambiguous offset both score low.
