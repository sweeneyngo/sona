# 09 — Confidence sub-scores from HMM posteriors

Status: ready
Blocked by: 08
Phase: C (confidence)

Upgrade the confidence stage (ADR-0008) to use what the HMM now knows.

- Derive **pitch-stability** and **voicing** sub-scores from HMM state posteriors / emission likelihoods along a note's path, instead of recomputing heuristics from the contour.
- Keep onset-clarity (onset strength) and offset-clarity (energy-drop steepness) as-is.
- Re-check the **confidence-calibration** metric (02): the flag should predict the wrong notes better than before.

Done when: calibration improves (or holds) on the harness vs the 03 baseline; sub-score semantics documented; weakest-link soft-min (ADR-0008) unchanged.
