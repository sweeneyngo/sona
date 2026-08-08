# 02 — Decode: Release column + energy-fall bonus

Status: done
Blocked by: 01
Phase: implementation

Wire the Release state into the Viterbi decode ([ADR-0012](../../../docs/adr/0012-note-offset-release-state.md)).

- `DecodeInput.release: Vec<bool>` — per-frame energy-fall flags, aligned to `obs`.
- Fold `release_offset_bonus` onto every `Release` state at flagged frames (mirror of the onset `Attack` bonus); generalise `emission_with_onset` → `emission_with_bonuses`.
- Add the `Release(q)` column: from `Stable(q)` (decay open) or its own self-loop; route `Silent(q)` from `Stable(q)` (sharp) **or** `Release(q)` (decay complete) or self. Note-change source set unchanged, so the dense `O(frames·bins²)` structure is untouched.
- Start prior: rule out `Release` at `t=0` (a mid-note phase, must be reached through an `Attack`).

Done when: the existing decode tests pass with the added `release` field; behaviour with an all-`false` `release` slice is identical to ADR-0010.

## Comments

**Done** (`crates/sona-core/src/hmm/decode.rs`). All-`false` `release` reproduces ADR-0010 exactly (verified by the unchanged decode tests). `Silent(q)` now takes the best of three predecessors.
