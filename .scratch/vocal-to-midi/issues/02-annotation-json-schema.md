# 02 — Annotation JSON schema (the core↔UI contract)

Status: ready-for-agent
Blocked by: 01

Define and version the annotation JSON — the primary artifact (ADR-0004). Everything downstream depends on it, so nail it first.

Fields:
- `version`
- `sample_rate`, `bpm` (user-supplied), `duration_s`
- `notes[]`: `onset_s`, `offset_s`, `midi_pitch`, `confidence` (scalar 0–1), `sub_scores` { `pitch_stability`, `voicing`, `onset_clarity`, `offset_clarity` }
- `f0_contour[]`: time/frequency/voicing per frame (display aid)
- `energy_envelope[]`: time/rms (display aid)
- `producer`: which backend/adapter emitted it (pyin | crepe | basic-pitch)

Serde types in `sona-core`; a matching TS type for the UI. Timing is in **seconds** (raw, unquantized) — grid/snap is a UI concern.

Done when: types serialize/deserialize round-trip; a hand-written example JSON validates.
