# 06 — Viterbi decode + energy same-pitch split

Status: done
Blocked by: 05
Phase: B (note-HMM)

Decode the model and integrate energy.

- Standard **Viterbi** DP over the state lattice (`score[t][j] = emit[t][j] + max_i(score[t-1][i] + trans[i][j])`, backpointers, backtrack). Pure Rust, `O(frames × states²)`.
- **Energy same-pitch splitter:** a detected onset (existing `onset.rs`, ADR-0006) forces an Attack/Silence transition so re-articulated notes at one pitch split (Tony's RMS-rise splitter — the piece pitch-only methods lack). Feed onsets as a per-frame prior/constraint into the decode.

Done when: decoding a synthetic contour recovers the expected note path; the legato-slur and repeated-same-pitch cases from `tests/note_segmentation.rs` pass through the new decoder; octave-jump artifact test confirms the interval prior rejects it.
