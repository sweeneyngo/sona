# 09 — Confidence sub-scores from HMM posteriors

Status: done
Blocked by: 08
Phase: C (confidence)

Upgrade the confidence stage (ADR-0008) to use what the HMM now knows.

- Derive **pitch-stability** and **voicing** sub-scores from HMM state posteriors / emission likelihoods along a note's path, instead of recomputing heuristics from the contour.
- Keep onset-clarity (onset strength) and offset-clarity (energy-drop steepness) as-is.
- Re-check the **confidence-calibration** metric (02): the flag should predict the wrong notes better than before.

Done when: calibration improves (or holds) on the harness vs the 03 baseline; sub-score semantics documented; weakest-link soft-min (ADR-0008) unchanged.

## Comments

**Done.** Confidence's `pitch_stability` and `voicing` now come from HMM-derived raw material carried on `NoteSegment`, not recomputed from the contour:

- **pitch-stability** ← `pitch_dispersion_semitones`: the RMS spread of the note's voiced frames about their mean pitch (over the frames the HMM *voiced*), through the existing tunable falloff. Faithful to ADR-0008's "how steady the f0 was" — spread, not off-tuneness (a note between semitones isn't penalised).
- **voicing** ← `voiced_confidence`: the mean of the model's per-frame voiced-vs-silent **emission evidence** (a sigmoid of a new `HmmModel::voiced_logodds` — a local log-odds, *not* a forward-backward posterior; the ticket's "posteriors / emission likelihoods" covers this). It folds pYIN voicing with pitch fit, so this dimension isn't strictly pitch-independent — kept because it holds/sharpens the flag.
- onset-clarity / offset-clarity unchanged; the weakest-link soft-min (ADR-0008) unchanged. The f0 contour is now dropped from `ConfidenceScorer::score`/`score_all` entirely (it was the whole point — "instead of recomputing from the contour").

**Calibration (the gate).** The literal "vs the 03 baseline" comparison isn't possible — baseline.md records that the ticket-03 pipeline's calibration signal is *uninformative* (every emitted note landed in the top bucket at error 0.000). So the honest comparison holds the segmenter fixed and varies only the confidence stage: an A/B of commit fc44630 (HMM + old contour-based confidence) vs now, over the 40 Vocadito stems. Error rate stays **monotone** in confidence and the result **holds**: trusted-note (confidence ≥ 0.6) error 0.140 → 0.142 and the full curve ties the old one within noise (e.g. 0.259/0.232/0.203/0.148 → 0.258/0.235/0.201/0.150). So it clears the "improves *or* holds" gate. (An earlier integer-pitch-referenced dispersion showed a larger apparent gain, but that was penalising off-tuneness, not wander — dropped as unprincipled.) The real win is architectural: the sub-scores now come from the HMM's own decoded pitch/voicing state (ADR-0010's "confidence gets better raw material"), not a second contour recompute, and calibration doesn't regress.

77 sona-core lib tests green; new unit tests for `voiced_logodds`, the HMM confidence fields, and a low-HMM-voicing case.
