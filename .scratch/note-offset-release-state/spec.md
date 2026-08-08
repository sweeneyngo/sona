# Spec: Explicit note-offset Release state in the note-HMM

Status: done — shipped OFF by default (gate failed on Vocadito; see issue 04 / ADR-0012)

## Goal

Add a **Release** sub-state to the note-HMM ([ADR-0010](../../docs/adr/0010-note-hmm-segmentation.md)) so a note's offset is *modeled*, not inferred after the fact. Today the HMM has no concept of a decay tail: a note ends wherever the decode leaves `Stable` (to `Silent` or a pitch change), and `offset_kind()` merely *classifies* that boundary while `drop_steepness()` *measures* the energy fall from the envelope. Because pYIN's analysis window smears the voicing edge, the placed offset trails the true musical note end — which is exactly why offset precision is the measured bottleneck.

**Measured motivation:** the recorded note-HMM sweep gives corpus **COnPOff-F1 0.508 ≪ COnP-F1 0.646**. Onset+pitch land; offsets are where notes bleed. A note ending has no clear pitch signature (decay into breath/reverb), so the energy *fall* must drive it — the exact mirror of the energy *rise* that already drives the same-pitch onset splitter (ADR-0006).

## Design

A fourth left-to-right sub-state per pitch bin: **Attack → Stable → Release → Silent**.

- **Release** = a voiced, on-pitch **decay tail**. Emission is the `Stable` voiced mix at a tight σ (`sigma_release`, defaults to `sigma_stable`) — it is still the note's pitch, just fading.
- **Energy-fall signal drives entry.** A per-frame `release[t]` flag (mirror of the onset flag) marks frames where the RMS envelope drops steeply. At those frames every `Release` state gets an `release_offset_bonus`, pulling the decode into `Release` exactly at the energy fall. This is the offset analog of `onset_attack_bonus`.
- **Superset, not replacement.** `Stable → Silent` is **kept**, so a sharp staccato cutoff still goes straight to silence with no Release frame (zero change on the target material). `Stable → Release → Silent` is the *new* path, engaged only when the energy-fall signal fires. Note-changes (`Stable/Silent → Attack(q)`) are unchanged; `Release` is **not** a note-change source, so the dense `O(frames·bins²)` decode structure is untouched — Release adds only `O(1)` incoming edges.
- **Offset = the `Stable → Release` boundary.** In the path→notes walk, a `Release` frame is treated like `Silent` (not part of the note body), so the note ends at the musical note end and the decay tail sits in the gap before `Silent`. `drop_steepness` is still measured near that (now tightened) offset and continues to feed offset-clarity confidence unchanged.

### Parameter budget (transition-row normalisation preserved)

`Stable` leave-mass `(1 − stable_self)` splits three ways via `stable_to_silent_frac` + `stable_to_release_frac` (+ the remainder to note-change). `Release` self-loops with `release_self`, else completes to its own `Silent`. New `HmmParams` fields: `sigma_release`, `release_self`, `stable_to_release_frac`, `release_offset_bonus`. Defaults chosen conservative-but-active; the *shipped* values are set by the corpus sweep, not by eye (ADR-0010/0011 discipline).

## Non-goals

- **No FFT, no ML, no new dependency** — pure Rust, consistent with ADR-0005/0006. (Spectral-flux onsets are a separate, later move.)
- No change to the annotation JSON contract (ADR-0004) or the `Segmenter`/`NoteSegment` seam — internal to the HMM.
- No change to the confidence formula (ADR-0008); offset-clarity keeps reading `drop_steepness`, it just reads it at a better-placed offset.

## Done when

- The four golden fixtures still pass: staccato offsets stay sharp, legato still splits, vibrato stays one note, bleed still culled.
- New unit tests cover: the energy-fall signal routes `Stable → Release`; a sharp cutoff still takes `Stable → Silent`; the offset lands at the `Stable → Release` boundary; transition rows still normalise.
- **Gate (ran):** the Release sweep A/B'd the decay path on Vocadito. It showed **no COnPOff gain** (best setting neutral, eager settings harmful). Honest read: the mechanism only tightens offsets that end in a sharp RMS fall, which most singing doesn't — an intrinsic ceiling, not a distribution artifact. So the path ships **OFF by default** (`release_drop_threshold = f32::INFINITY` ⇒ exactly ADR-0010) because a pure-DSP energy-fall release isn't the offset fix; capability retained (harmless off). See `reports/release-sweep.md`.

## Sequencing

- **01** — model: `Sub::Release`, `SUBS = 4`, params, transitions, emission.
- **02** — decode: Release column, energy-fall signal + bonus, start prior.
- **03** — notes: derive the energy-fall signal from the envelope, treat Release as a boundary, feed the flags; tests.
- **04** — ADR-0012 (Proposed) + eval sweep dimension over the new knobs; run the corpus gate.
