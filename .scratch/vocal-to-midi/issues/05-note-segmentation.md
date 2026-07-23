# 05 — Note segmentation (offsets + plateau splitting)

Status: done
Blocked by: 04

Turn f0 contour + onsets + energy envelope into note events. **Highest-error area (spec priority #2) — put the effort here.**

- **Offsets — hybrid**: next-onset is the primary offset when the next onset arrives within a musical threshold; otherwise energy-decay fallback closes the note into a rest.
- **Pitch-plateau splitting**: split on sustained pitch changes without a new onset (legato/slides).
- Record, per offset, the **energy-drop steepness** — feeds offset confidence in ticket 06.

Notes come out as raw seconds (no quantization here — that's the UI).

Done when: segmentation yields musically plausible notes on the test stem; each note carries the raw material confidence needs (drop steepness, plateau info).

## Comments

**Done.** `sona-core::segment` (`Segmenter` → `Vec<NoteSegment>`). Hybrid offset is *earliest of two candidates*: next-onset (eligible only within `max_offset_gap_s`) vs. energy-decay to a peak-relative rest floor — so back-to-back notes close on the next onset, staccato notes close on the drop and leave a rest. Pitch-plateau splitting cuts each onset span into MIDI-pitch runs, absorbing sub-`min_plateau_s` excursions so vibrato/slides don't over-split. Each `NoteSegment` carries `onset_strength`, an `OffsetKind`, and an `OffsetInfo::drop_steepness` (0–1, rate of the RMS fall) — the raw material ticket 06 scores. Segmentation stops at `NoteSegment`, not annotation `Note`; confidence composes the scalar/sub-scores downstream. Design recorded in ADR-0007. Verified end-to-end through the real pYIN + onset pipeline in `tests/note_segmentation.rs` (staccato phrase + legato slur).
