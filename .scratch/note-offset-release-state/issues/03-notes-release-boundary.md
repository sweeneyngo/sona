# 03 — Notes: derive the fall signal, treat Release as a boundary

Status: done
Blocked by: 02
Phase: implementation

Make the note-walk place offsets at the `Stable → Release` boundary ([ADR-0012](../../../docs/adr/0012-note-offset-release-state.md)).

- `NoteHmm.release_drop_threshold` (default 0.5); `release_flags()` derives per-frame flags by thresholding `drop_steepness(envelope, frame.time_s, drop_window_s)`, reusing the existing measurement.
- Feed `release` into `DecodeInput`.
- In `notes_from_path`, map `Sub::Release` → `None` in `voiced_midi` (a `Release` frame closes the note like `Silent`), so the offset lands at the musical note end and the decay tail sits in the gap.

Done when: golden fixtures unchanged; a new test shows a sharp energy fall under still-voiced pitch tightens the offset to the fall; a staccato offset is never pushed later.

## Comments

**Done** (`crates/sona-core/src/hmm/notes.rs`). Two new tests: `a_sharp_energy_fall_places_the_offset_at_the_decay_not_the_voicing_end` (offset snaps from ~0.6 voicing-end to ~0.35 energy-fall) and `a_staccato_cutoff_offset_is_tightened_never_pushed_later` (staccato offset moves earlier to the true cutoff, never later). 80 core lib tests + all integration/fixture tests green; clippy clean.
