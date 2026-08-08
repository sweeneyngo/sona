# Onset-deviation diagnostic (before spectral flux)

Run: `cargo run --release -p sona-eval --bin sona-eval-onset-deviation -- --corpus`
Corpus: Vocadito, 40 stems + 4 golden fixtures. Signed deviation = `detected − reference`, over a maximum-cardinality matching within ±100 ms (the metric's own matcher).

Purpose: before committing to spectral-flux onsets (which retires ADR-0006's no-FFT badge), check whether the onset-F1 dip (≈0.565) is a cheap systematic timing bias or genuinely hard attacks.

## Result

```
stems 44   reference onsets 2244   detected onsets 1114   matched pairs 993 (±100 ms window)

   deviation (ms)   count
      <   -60      58  ████████
     -30..  -25      53  ███████
     -25..  -20      79  ██████████
     -20..  -15     166  ██████████████████████
     -15..  -10     289  ████████████████████████████████████████   ← peak
     -10..   -5     153  █████████████████████
      -5..   +0      15  ██
       (positive side sparse; long +≥60 tail = 31)

mean -17.1 ms   median -11.6 ms   std 26.7 ms
matched 993 / 2244 reference onsets = 44%
```

## Verdict — two independent findings

1. **Recall-limited (the dominant lever).** Only **1114 detected vs 2244 reference** onsets; even at a generous ±100 ms only **44% match**. The energy-rise novelty simply misses ~half the attacks — soft/legato onsets with no sharp energy step. No constant correction recovers a *missing* onset. **This is the genuinely-hard case, and it justifies spectral-flux onsets** (a signal that sees timbral/harmonic change, not just energy). This is the substantive next move.

2. **A small systematic bias, but within tolerance (a minor, separate correctness win).** Detected onsets sit **median −11.6 ms (≈ 2 hops) early**, a tight peak at −15..−10 ms. This is a real frame-centering/convention offset (the energy-rise fires slightly before the annotated note onset), **but 11.6 ms is well inside the ±50 ms onset-F1 tolerance, so correcting it will not move the F1.** It is worth a constant ≈ +11.6 ms correction for onset *placement* accuracy (tighter note starts, marginally better COnP alignment), but it is not the lever for 0.565 — do not mistake it for one.

## Consequence

Proceed with **spectral-flux onsets** to attack the recall problem (finding 1). The timing bias (finding 2) is a cheap, optional placement fix that can ride along but should not be sold as an F1 improvement. Re-run this diagnostic after spectral flux: the match rate should climb, and the bias (if any) should be re-measured on the new detector.
