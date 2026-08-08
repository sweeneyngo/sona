# 01 — Model: the Release sub-state, transitions, emission

Status: done
Phase: implementation

Add the fourth sub-state to the note-HMM model ([ADR-0012](../../../docs/adr/0012-note-offset-release-state.md)).

- `Sub::Release` between `Stable` and `Silent`; `SUBS = 4`; `split()` maps the new index.
- `HmmParams`: `sigma_release`, `release_self`, `stable_to_release_frac`, `release_offset_bonus`; repartition `Stable`'s leave-mass three ways (sharp / decay / note-change) with rows still normalised.
- `transition()` + `trans_consts()`: `Stable → Release`, `Release → Release`, `Release → Silent`; `Release` is not a note-change source.
- `emission_logs()`: `Release` emits the voiced mix at `sigma_release`.

Done when: `transition_rows_are_normalized` still holds; `state_indexing_round_trips` covers all four subs; a new test asserts both offset paths are legal and `Release` never fans out to another pitch.

## Comments

**Done** (`crates/sona-core/src/hmm/model.rs`). Four-way `Stable` leave-mass split with clamped `change_frac` so an over-specified fraction pair floors note-change at zero instead of going negative. New test `a_note_may_end_sharp_or_decay`. Row normalisation preserved (defaults sum: `0.4 + 0.1 + 0.5 = 1`).
