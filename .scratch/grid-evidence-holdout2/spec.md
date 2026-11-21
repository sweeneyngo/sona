# Spec: confirm ADR-0016 on the fresh holdout (holdout2)

Status: done — holdout2 read once (0ebadc3); all four criteria pass; both features are now grid-mode defaults

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

## Comments

**2026-11-21, before the read.** Alignment (05) flagged 2 of the first 10 songs:
- kieru-mama: constant shift −49 ms;
- teni-suru-bug: drift +69 ms, so a single BPM doesn't hold.

Both stay in the manifest, flagged, and are excluded from scoring. The next two uploads in channel order were backfilled (dreamcore, mc-tokyo); both aligned. **10 aligned songs are scored**, as the rule assumes. No holdout2 score had been computed when the backfill was made.

## Result (holdout2, 10 aligned songs, read once at 0ebadc3)

Vocal stems (primary), quantized, all notes:

| | plain grid mode | + salience + spectral cue |
|---|---|---|
| cell accuracy, all | 52.8% | **70.0%** |
| note COnP F1 | 0.330 | **0.514** |
| COnPOff F1 | 0.206 | 0.335 |

1. Cell accuracy +17.2 points, against a bar of +3. ✔
2. COnP +0.184, against a bar of +0.05. ✔
3. 95% song-bootstrap interval of the cell difference: [+12.0, +22.3] points. ✔
4. Songs losing more than 2 points: none; every song improves. ✔

**Secondary:** lead stems with both features score 69.5% against 70.0% on vocal stems, so they miss the +1 point bar. The extra UVR step isn't recommended.

The lead-stem read is the second ledger entry (`--reread`, same sitting, as pre-registered).

**Verdict:** salience and the spectral cue become grid mode's defaults. The default note-HMM path is unchanged.

**Absolute level against dev:** 70.0% cells against dev's 79.5%, and 77.8% against 86.8% in monophonic regions. holdout2 is harder: cinema-mitaina scores 38% cells, from a 0.07 COnP baseline. That is worth a data check (key, alignment) before it is read as decoder weakness.
