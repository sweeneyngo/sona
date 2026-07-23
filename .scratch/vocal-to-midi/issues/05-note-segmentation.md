# 05 — Note segmentation (offsets + plateau splitting)

Status: ready-for-agent
Blocked by: 04

Turn f0 contour + onsets + energy envelope into note events. **Highest-error area (spec priority #2) — put the effort here.**

- **Offsets — hybrid**: next-onset is the primary offset when the next onset arrives within a musical threshold; otherwise energy-decay fallback closes the note into a rest.
- **Pitch-plateau splitting**: split on sustained pitch changes without a new onset (legato/slides).
- Record, per offset, the **energy-drop steepness** — feeds offset confidence in ticket 06.

Notes come out as raw seconds (no quantization here — that's the UI).

Done when: segmentation yields musically plausible notes on the test stem; each note carries the raw material confidence needs (drop steepness, plateau info).
