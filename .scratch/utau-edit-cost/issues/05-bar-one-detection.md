# 05 — Find beat 1 of the measure

Status: resolved
Type: prototype
Blocked by: audit A02 (export frame), 10

## What

Only if 03 shows bar offsets are common. Estimate which beat starts the measure (downbeat) from the mix
and/or the vocal, and shift sona's tick 0 to it (roadmap 2.3).

- Oracle first: frame pass rate if the bar offset were always right.
- Prototype on dumped intermediates (dev), then port; the port must reproduce the prototype exactly.
- Opt-in until holdout4 confirms it, under a rule committed before the read.

## Comments

- 2026-11-24 (after A03, `crates/sona-eval/reports/bar-one.md`): scope sharpened. (1) **Beat:** the vocals'
  cell line nearest `grid_phase(mix, 60/BPM)`; matches the drums' beat on 9/9 dev songs. (2) **Bar:** the
  beat (mod 4) with the most harmonic change in chroma(mix − vocals); a prototype only, no ground truth,
  weak on yamineko, fukagyakuteki, atamaro, amaten and the 236–250 BPM songs. (3) Tick 0 = that bar line
  at or before 0 s; the export starts the audio that much after tick 0 (A02's frame). Opt-in; confirm on
  holdout4 against **ear-labelled** beat and bar 1, not the UST's grid.

## Pre-registered decision rule for holdout4 (committed before any holdout4 read)

Written 2026-11-24, before holdout4 exists. Nothing here has been read on it; the ledger is unchanged.

- **Reference.** Every holdout4 song in 4/4 gets `music_beat_shift_cells`, `music_bar_shift_cells` and
  `music_frame_status: ear-checked`, labelled by the procedure in `reports/bar-one.md` ("Ear-labelling the
  music frame") from the mix and click overlays only. The labeller runs neither `--bar-one` nor
  `--estimate-bar-one` on holdout4. The user confirms a sample of at least 5 songs. Songs that cannot be
  labelled in 4/4 are listed and excluded **before** the read.
- **Rules compared**, per song, at the UST's exact BPM (a typed BPM, so tempo always passes):
  - **A (today):** tick 0 = the first grid line at or after 0 s, from `grid_phase(vocals, cell)`.
  - **B (this ticket):** bar 1 = `sona_core::bar_one::estimate` exactly as committed here
    (`MIN_CONFIDENCE` 0.5, beat fallback). No change to it between now and the read.
- **Metric.** Music-anchored frame pass = bar offset 0 whole beats against the ear-labelled bar line
  (`sona_eval::frame::music_bar_offset`, `beats == 0`).
- **Read.** Once, logged in the ledger, with a reason naming this rule; holdout4 must hold at least 20
  labelled songs (ticket 08).
- **B becomes the default** (the app requests it and the default bar 1 is the estimate) only if **all** hold:
  1. B passes on at least **3 more songs** than A;
  2. the 95% song-bootstrap interval of the paired difference B − A (`song_bootstrap`, 10 000 resamples,
     fixed seed) has a lower bound of at least **+1**;
  3. B's beat is on the music's beat (|residual| < ⅛ beat) on at least as many songs as A's.
- **Otherwise** B stays opt-in. If B passes on fewer songs than A, record the negative result with its
  numbers and remove the option unless a per-song look shows a named, fixable cause.
- **Reported beside it, not deciding:** B's pass rate split by confidence (≥ 0.75 vs below), the beat-only
  rule, and every song's B − A.

## Answer

Resolved 2026-11-24 (opt-in shipped). Dev only: no `--split`, `--holdout-once` or `--reread`; the ledger
and holdout manifest entries are unchanged. ADR-0031.

**Oracle** (dev 9, typed UST BPM, music frame from the manifest; `sona-eval-frame --estimate-bar-one`):

- Today: frame pass **5/9** [2, 8] (95% song bootstrap).
- Perfect bar 1: **9/9** by construction. The gap is 4 songs (innocent 1 beat off, kamaitachi 2, amaten 2,
  my-axiom 3).
- **The beat alone closes none of it.** Using the music's own (manifest) beat as bar 1: the first beat at
  or after 0 s gives **5/9** (+0 [−3, +3]); the first at or before gives 2/9. The beat fixes the residual
  but not the bar: it gains my-axiom and loses azemichi.

**Estimator** (`crates/sona-core/src/bar_one.rs`, beside `tempo`): beat = the vocals' cell line nearest
`grid_phase(mix, 60/BPM)`; bar = the beat mod 4 with the most chroma change in mix − vocals, summed over
1-, 2- and 4-beat windows; confidence = share of 8-bar blocks whose own vote agrees; below 0.5 (chosen
before the first dev run) it falls back to the beat. No Python prototype was rebuilt (A03's was scratch and
is gone). The port was checked against the prototype's recorded outputs, the manifest's seeded values: its
bar call equals A03's on **9/9**, and its beat is within 0.05 cell of the drums' on 9/9.

**Dev table** (whole beats off the music's bar line + residual in beats; pass = 0 beats):

| song | ref bar / est bar (cells) | beat Δ (cells) | today | beat alone (est) | beat + bar (shipped) | confidence |
|---|---|---|---|---|---|---|
| innocent-addict | 13 / 13 | +0.02 | 1 +0.00 | 1 +0.00 | **0** +0.00 | 0.61 |
| kamaitachi-ni-yoru | 9 / 9 | +0.04 | 2 +0.01 | 2 +0.01 | **0** +0.01 | 1.00 |
| telescope-nevermore | 1 / 1 | +0.02 | 0 +0.01 | 0 +0.01 | **0** +0.01 | 1.00 |
| atamaro-deluxe (24) | 2 / 2 | +0.03 | 0 −0.16 | 0 +0.00 | **0** +0.00 | 0.92 |
| azemichi-ni-ochita-ginga | 0 / 0 | −0.00 | 0 +0.25 | 1 −0.00 | **0** −0.00 | 0.70 |
| yamineko | 3 / 3 | +0.04 | 0 −0.49 | 0 +0.01 | **0** +0.01 | 1.00 |
| my-axiom | 4 / 4 | +0.02 | 3 +0.25 | 0 +0.00 | **0** +0.00 | 0.60 |
| amaten | 9 / 9 | +0.05 | 2 +0.01 | 2 +0.01 | **0** +0.01 | 0.85 |
| fukagyakuteki-henyou | 1 / 1 | −0.01 | 0 −0.00 | 0 −0.00 | **0** −0.00 | 0.57 |
| **pass** | 9/9 agree | 9/9 < 0.05 | **5/9** [2, 8] | 5/9 [2, 8] | **9/9** [9, 9], +4 [+1, +7] vs today | none fell back |

- **The bar column is circular.** The music frame's bar *is* A03's harmonic estimate, so 9/9 shows that the
  Rust port re-derives the prototype faithfully, not that the bar is right. Only the beat has a partly
  independent reference (the drums' kick + snare fold vs the mix's flux). Proof needs ear labels.
- **Confidence on the weak songs.** A03's weak calls were yamineko (0/4 cues), fukagyakuteki (1/4),
  atamaro and amaten (2/4). Confidence: fukagyakuteki 0.57 (the lowest), amaten 0.85, atamaro 0.92,
  yamineko 1.00. It measures how consistently the harmony votes through the song, not agreement with the
  drum and bass cues, so it flags only fukagyakuteki. my-axiom (0.60) and innocent (0.61) are also low.
  Nothing fell below 0.5.
- **Other explanations.** Cached work: none, every run decodes and computes from the WAVs. Leaked labels: the
  estimator reads only the mix, the vocals stem and the BPM, never the UST or the manifest's shifts.
  Typed-BPM leak: intended, as the user types the BPM (04). Sample-rate or length mismatch: the stems match
  the mix sample for sample, and the harness checks the rate.

**Wiring (opt-in; default byte-identical)**

- `Annotation.bar_one: Option<BarOneEstimate { time_s, confidence, harmonic }>`, skipped when `None`
  (mirrored in `web/lib/annotation.ts`).
- `sona transcribe --bar-one MIX.wav` takes the vocals from `--vocals`, else the stem. It records `bar_one: mix`
  and the mix's fingerprint in the provenance only when used. `sona serve` takes `bar_one=1` (400 with
  `separation=none` or any other value) and writes `mix.wav` in the job folder.
- **The app.** The start screen's Options has **Bar 1 from the music**, unticked and disabled for "Already a
  stem". The query gains `bar_one=1` only when ticked. A fresh annotation with `bar_one` starts the frame's
  bar 1 there; without it, `barOne = null` as before. The ♩ popover reads "0.260 s · from the music (85%)"
  (or "the music's beat (bar unsure, N%)"), and **Auto** returns to the estimate. The status line after a
  job names it.
- Tick 0 is ADR-0029's rule, unchanged: the first bar line at or after 0 s, stepped back for pickups. Mod
  a bar it is the same bar line as the comment's "at or before 0 s", so the metric is the same.
- **Byte identity.** `web/lib/frame.test.ts`'s A02 golden (420 sessions, UST and USTX SHA-256) passes. The
  CLI and server tests assert no `bar_one` key and no `bar_one` provenance setting by default, and the
  same notes with the option on. The default job query string is asserted verbatim.

**Mutants** (each caught, then restored):

| mutant | caught by |
|---|---|
| no vocals subtraction (chroma of the mix) | 2 `bar_one` core tests |
| argmin instead of argmax | `finds_the_beat_and_the_chords_bar_line_not_the_singer_s` |
| confidence ignored (always the harmonic bar) | `falls_back_to_the_beat_…` |
| beat not snapped to the vocals' cells | 3 core tests |
| `music_bar_offset` against the UST's tick 0 | `a_tick_zero_whole_beats_after_…` |
| bootstrap quantiles 0 / 1 | `a_song_bootstrap_brackets_…` |
| serve: bar 1 estimated for every job | `bar_one_is_estimated_only_for_a_job_that_asks…` |
| annotation: `bar_one: null` serialised by default | annotation unit test + CLI test |
| app: Auto ignores the estimate | frame.test property |
| app: `bar_one` without `harmonic` accepted | frame.test parse |
| app: readout never names the estimate | frame.test label |
| app: `bar_one=1` always sent | serveClient.test |

**Verification:** see the commit message for clippy, `cargo test --workspace --release`, tsc and vitest,
`verification/*/check.sh` and the corpus guard.

**Left for later:** the holdout4 read (needs ticket 08's split with ear labels, and a `--split` path in
`--estimate-bar-one` that goes through the ledger). Ear-check the dev music frames: the click overlays for
yamineko, fukagyakuteki, atamaro and amaten come first. Browser e2e for the option and the popover (not run here).
3/4, 6/8 and tempo changes are out of scope.
