# Spec: grid-conditioned cell decoding (ADR-0015)

Status: done — holdout read once, all four criteria pass. See crates/sona-eval/reports/grid-cell-decoder.md

## Goal

Significantly improve how closely sona's output replicates a hand-made UST
(Pifuyuu's corpus) on grid-locked Vocaloid material, the declared target, acting
on the in-domain gate's finding that **recall** (0.25–0.39), not offsets, is the
dominant gap.

## What dev showed (9 dev songs; holdout untouched)

The frame-level note-HMM emits about 250 notes per song against 500–800 in the
UST. Misses on monophonic regions:
- right pitch, onset 50–150 ms off: 16%
- no estimated note at all: 15%
- overlapped by another-pitch note: 8%
- shorter than the 100 ms prune floor: 8%
- same-pitch merges: 6.5%

The root causes are frame-level boundary placement. Consonants unvoice note
starts, glides drag onsets, and the prune floor drops one-cell notes. 9,013 of
9,082 UST onsets sit exactly on the grid, so decoding per grid cell removes these
causes by construction.

Knob sweeps of the existing HMM gained at most +0.04 F1. The cell decoder
(Python prototype, then the Rust port at exact parity) gained +0.13 COnP F1 and
+0.12 COnPOff F1 on dev, with the phase estimated from the stem.

## Pre-registered decision rule (written before the holdout is read)

Measured once with `sona-eval-ust-dev --holdout-once --cfg base: --cfg grid:grid=est --per-song`
(same scoring as the gate) on the holdout's `vocaloid-original` songs:

**Ship** grid mode as the recommended opt-in for grid-locked material if all of:
1. Pooled quantized all-notes COnP F1 improves by ≥ 0.05 over the note-HMM.
2. Pooled quantized all-notes COnPOff F1 improves by ≥ 0.05.
3. The 95% song-bootstrap interval of the pooled COnP F1 difference excludes 0.
4. At most 2 holdout songs regress by more than 0.03 COnP F1.

Vocadito must be unchanged; grid mode is opt-in, so the default path is
byte-identical. If 1–3 hold but 4 fails, ship anyway and document the regressing
songs. If 1 or 3 fails, do not recommend it; report dev as overfit.

The noise floor is song resampling (sona is deterministic). Parameters are
frozen at the dev optimum in `GridDecoder::default()` as of this commit.

## Result (holdout, read once after commit b5391bb)

1. COnP F1 0.340 → 0.455 (+0.114 ≥ 0.05) ✔
2. COnPOff F1 0.158 → 0.303 (+0.145 ≥ 0.05) ✔
3. 95% song-bootstrap interval of the COnP difference [+0.064, +0.167] excludes 0 ✔
4. Songs regressing > 0.03 COnP F1: none ✔

Vocadito unchanged (0.646 / 0.508). Verdict: ship grid mode as the recommended opt-in.
