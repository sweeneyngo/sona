# Spec: confirm ADR-0016 on the fresh holdout (holdout2)

Status: pre-registered (holdout2 not yet read)

## Question

Do harmonic-salience pitch evidence and the spectral re-articulation cue
(ADR-0016, both tuned on the 9 dev songs only) improve grid mode on 10 songs
nothing was tuned on? If so, make both the grid-mode defaults.

## Measurement (one read; the ledger logs it)

`sona-eval-ust-dev --split holdout2 --reason "…" --stem vocals --cfg base:grid=est --cfg adr16:grid=est,cands=1,g.cue=spectral --per-song`
(built with `--features spectral`), plus the same with `--stem lead`, in the
same sitting. All decoder parameters are frozen at this commit.

## Pre-registered decision rule

Primary input: **vocal stems** (demucs `htdemucs`), what a user has without
extra separation. Primary metric: **cell accuracy, all cells, quantized** (the
user's chosen target), with note COnP F1 alongside.

Make both features the grid-mode defaults if, on holdout2 vocal stems, all of:
1. Cell accuracy improves by at least 3 points over plain grid mode (dev: +12.5).
2. Pooled quantized all-notes COnP F1 improves by at least 0.05 (dev: +0.137).
3. The 95% song-bootstrap interval of the cell-accuracy difference excludes 0.
4. At most 2 of the 10 songs lose more than 2 points of cell accuracy.

- **If 1–3 hold but 4 fails:** make them defaults anyway, and name the regressing songs.
- **If 1 or 3 fails:** keep both opt-in and report dev as overfit.

**Secondary input (lead stems):** recommend the extra UVR karaoke step only if
lead stems with both features beat vocal stems with both features by at least
1 point of cell accuracy on holdout2.

## Noise

sona is deterministic, so song resampling is the noise. With 10 songs, a
difference under about 2 points of cell accuracy is not distinguishable.
