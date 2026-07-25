# 05 — Note-HMM state, emission, and transition model

Status: done
Blocked by: 04
Phase: B (note-HMM)

Define the model (no decoding yet — pure, unit-testable construction). Tony-style (ADR-0010):

- **States:** vocal-range pitches at **3 bins/semitone** + a **silence** state; each pitch has **Attack / Stable / Silent** sub-states.
- **Emission:** Gaussian on semitone deviation from the state center, trust exponent τ≈0.1; **σ Attack≈5, Stable≈0.9**; voicing prior v≈0.5 splitting mass to silence, driven by pYIN voicing.
- **Transitions:** left-to-right within a note (self-transitions ≈ **0.9 / 0.99 / 0.9999**); Silent fans out to other Attack states via an **interval prior** favouring small jumps, octaves expensive (this replaces the UI slope filter + MAD trim).

Starting parameters are the literature values above; final values come from 08's sweep.

Done when: the model builds for a given pitch range; emission/transition matrices are unit-tested (mass normalization, σ ordering, interval-penalty monotonicity).
