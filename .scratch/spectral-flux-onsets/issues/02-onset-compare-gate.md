# 02 — The gate: energy vs spectral onset comparison on the corpus

Status: done
Blocked by: 01
Phase: gate

Prove (or disprove) that spectral flux beats energy on real onsets before promoting it.

- `sona-eval-onset-compare` (feature `spectral`): run both detectors over Vocadito + fixtures, report onset P/R/F1 at ±50 ms. Sweep the spectral `threshold_delta` (its novelty scale differs from energy's, so the picker threshold can't be inherited).
- Gate: spectral lifts onset F1 on Vocadito without regressing the staccato fixtures → promote toward default. Else: keep off, record why.

## Comments

**Done — partial win, not promoted (`reports/onset-compare.md`).** Spectral flux **recovers the missing onsets**: at δ=0.02 recall 0.872 vs energy 0.441 — the diagnostic's hypothesis holds, the information is there. But raw log-magnitude flux over-fires on continuous spectral motion (vibrato/formant/breath), so precision collapses and **no threshold beats energy F1** (best 0.515 at δ=0.10 vs energy 0.565). Fixtures echo it (best spectral 0.676 vs energy 0.750).

**Decision:** ship spectral off by default (energy stays default); not promoted. The recall proves the signal is recoverable; the missing piece is a **vibrato-robust novelty** (ticket 03), not a bigger threshold.
