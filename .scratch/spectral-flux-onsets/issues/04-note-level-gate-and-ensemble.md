# 04 — Note-level (COnP) gate + energy⊕SuperFlux ensemble

Status: done
Blocked by: 03
Phase: gate

SuperFlux beats energy on recall and on the staccato fixtures but not on Vocadito *onset-F1* (0.542 vs 0.565). Onset-F1 is a proxy — the metric that matters is note-level COnP through the full pipeline. Two things to try before concluding pure-DSP onsets have plateaued:

1. **Note-level gate (the real test).** Run the full pipeline (pYIN + onsets + note-HMM + confidence) with energy vs SuperFlux onsets and score **note-F1 (COnP / COnPOff)** on Vocadito, reusing the `sweep`/`release-sweep` stem-caching. Rationale: onsets feed the HMM same-pitch splitter — a *missed* onset merges two notes (a lost boundary, likely a COnP hit), while a *false* onset is an over-split the HMM's interval prior + `merge_same_pitch` can absorb. So SuperFlux's +0.19 recall at −0.023 onset-F1 may still *raise* COnP. If it does, that flips the verdict and justifies promotion (guard the staccato/fixture COnP).

2. **Energy⊕SuperFlux ensemble.** Energy has precision (0.854), SuperFlux has recall (0.629). Union the two onset sets (energy's onsets ∪ SuperFlux onsets that energy missed, within a dedup window), and gate that — on onset-F1 *and* COnP. Likely captures both strengths cheaply.

Done when: the note-level gate reports COnP for energy / SuperFlux / ensemble onsets on Vocadito (+ fixtures as the regression wall). Promote whichever wins end-to-end COnP without regressing the staccato fixtures; record it in `reports/`. If none beats energy on COnP, that is the honest signal that pure-DSP onsets have plateaued and an ML onset model (the `crepe`/basic-pitch seam) is the next lever.

## Comments

**Done — decisive, energy wins (`reports/note-onset-gate.md`).** `sona-eval-note-onset-gate` runs the full pipeline with energy / two SuperFlux thresholds / an energy⊕SuperFlux ensemble and scores note COnP/COnPOff.

Result: **energy COnP-F1 0.646 beats every alternative** (superflux δ=0.10 → 0.619, δ=0.02 → 0.519, ensemble → 0.520). More onsets make *notes* worse — the "HMM absorbs false onsets" hypothesis is falsified; they over-split into spurious notes.

**The insight:** energy's onset recall is 0.44 but its note COnP is 0.65 — COnP > onset recall because the note-HMM already recovers most boundaries from **pitch** (the interval-prior legato split), so the onset stage and pitch-splitting are partly redundant. **Onset recall was a proxy-metric red herring; it does not bottleneck the product.** SuperFlux stays feature-gated/off (isolated, no default-build cost). If onsets are ever revisited, only an ML model remains — but the payoff looks small. The real levers stay: offset precision (COnPOff 0.508) and the target-domain (UVR) eval gap.
