# Single-note errors: start, end and pitch (utau-edit-cost 12)

Date: 2026-11-24. Dev only for every decision (9 songs, 27.5 min). The spent splits were re-read from 03's dumps
**for diagnosis only** (no new run; ledger entries at 8fc6ae5); they confirm nothing. holdout4 was not touched.
Baseline, reproduced from the GAME cache byte for byte: **GAME lead + octave 188.7 cost/min, 16.2% clean
measures** (out of sample); learned stage 180.2 / 23.8% (in-sample on dev); pYIN 338.4 / 3.1%. 07 left the
default boundaries unchanged, so this is the default output.

Selection rule (committed in the ticket before measuring, d78a630): port only if dev GAME-lead pooled cost/min
drops with a paired song-bootstrap 95% interval excluding 0, the stage's cost does not rise and clean measures
do not fall; a port is opt-in, reproduces the prototype exactly, and has behaviour tests and a caught mutant.

A *pair* is one UST note matched to one output note (`edit_cost.rs`); its errors are **pitch**, **length** (cell
counts differ), **shift** (start one cell off) and **moved** (two or more), weight 1 each. On GAME lead, pairs
carry 1 155 of 5 185 cost units (22.3%): length 717, shift 218, pitch 203, moved 17.

## Answer

- **The largest single cause is the release: GAME holds a note through the rest the UST writes before the next
  note.** 263 GAME-lead notes start on time but run into a UST rest, and 211 of them fill the whole rest up to
  the next note. The UST's short rests are 2, 4 or 6 cells (an 8th-note multiple); 177 of the 263 are 2 cells.
  Fixing those ends alone is worth **−9.5 [−11.5, −7.5] cost/min (5.1%) and lifts clean measures 16.2% → 23.1%**,
  the largest clean-measure gain of any single-note cause. It holds on every spent split (−4.3, −8.1, −8.5).
  The learned stage has mostly learned it already (−2.0 there; 16 of its notes overrun by 2+ cells, against
  202 on GAME lead).
- **The vocals' energy predicts it; the pitch track does not.** Over the overrun the energy is a median 0.16
  and 0.28 of the note's level for the two rest cells, then 0.97 at the next note. Per cell, energy separates
  overrun cells from a right note's last cell with AUC 0.82 (0.93 against inner cells); pYIN voicing 0.68 (a
  right note's last cell is only 37% voiced, because UTAU puts the next mora's consonant at the end of the
  note before it), GAME on the vocals 0.57.
- **Ported, opt-in, for GAME notes only: `sona_core::release::trim_releases`** ends a note at the first cell
  under 0.3 of its loudest cell when every later cell stays under 0.8 and at least 2 cells go. GAME lead
  188.7 → **184.3 (−4.4 [−6.5, −2.5])**, clean measures 16.2% → 18.7% (+2.5 [−0.2, +5.3]); 7 of 9 songs improve.
  - The design chosen leave-one-song-out keeps most of it: −3.7 [−5.4, −1.9], 8 of 9 folds better, the same
    0.3 and 2 cells on every fold.
  - On the stage it raises the cost (+0.4 [−0.3, +1.4]) and lowers clean measures (−1.1 [−2.2, −0.1]), so it is
    **not applied to the stage**. That is why it serves `--game` output and not what ships.
  - Spent splits (diagnosis): GAME lead −1.2 [−2.1, −0.5], −2.5 [−4.1, −1.2] and −3.2 [−6.0, −1.0] cost/min,
    clean measures +1.0, +0.6 and +2.9 points; the stage −0.2, −0.5 and −0.2.
  - It reaches about half the oracle. Of the targets, 121 are cut exactly, while 55 right notes are cut. Those
    false cuts look the same in energy: a two-cell dip between two notes, which the UST writes legato. A
    perfect choice among the energy's candidates is worth −5.8; a logistic model on energy and voicing does
    worse than the rule (−0.5 [−3.2, +2.0] out of fold). One-cell trims always lose (+0.6 to +25).
- **Starts and the remaining ends are rounding jitter with no directional evidence.**
  - Every start fixed is worth −16.3 [−28.2, −7.3]: −8.5 late and −5.6 early at legato boundaries, mostly
    atamaro-deluxe.
  - A start error's raw GAME onset is a median 0.21 cell past the rounding point, in both directions
    (135 late, 83 early).
  - No global rounding bias beats the shipped rule (best −0.5 [−3.1, +2.0]); shifting GAME by ±10 ms loses.
  - The pitch track's transition is worse than GAME's at legato boundaries: it is right where GAME is wrong
    44 times, and wrong where GAME is right 663 times.
  - GAME's own onset phase (what the app's `snap_to_grid` uses) is worth −2.4 [−6.5, −0.0], all of it on
    atamaro: on its triplet grid, GAME's onsets sit 0.19 cell after the vocals' onset phase. It lowers clean
    measures (−0.4), so it fails the rule.
- **The snap rules are not a cause.** The one-cell minimum and the monophonic trim set the boundary of 2 of 747
  timing-error pairs.
- **Pitch.** Every pitch error fixed is worth −7.4: semitones −3.7 and octaves −2.2.
  - **Octave errors are one song**: amaten has 49 of 57. The vocals' track makes the same octave error 30 times
    in 48. The octave check's harmonic ratio ranks them well (AUC 0.90), but any threshold breaks more right
    notes than it fixes (24 against 278 at −0.5).
  - **Semitone errors are near the rounding point.** GAME's tuned pitch lies at least 0.3 semitone from the
    semitone's centre for 60% of ±1 errors, against 4.2% of right notes. But the track only says which way
    for a few: the best rule fixes 23 and breaks 11, about −0.4 cost/min. Not built.
- **What changes the plan.**
  - The shipped stage is already as good as this evidence allows on releases. Its remaining single-note cost is
    rounding jitter and semitone choices, and no evidence here says which way those go.
  - Both are well predicted as *doubtful*, though: a raw boundary within a quarter cell of the rounding point,
    or a pitch at least 0.3 semitone off centre. That makes them a case for ticket 14's flags (a cheap
    one-click fix), not for an automatic fix.
  - `trim_releases` is worth wiring into `sona transcribe --game` as an opt-in. As a default it needs holdout4.
  - The eval's "GAME lead" snaps on the vocals' onset phase, but the app's GAME-only path (`snap_to_grid`) uses
    GAME's own phase. They differ by atamaro's 17 cost/min, so the GAME-only numbers are not quite the app's.

## Taxonomy (dev, GAME lead)

Timing errors on pairs by (start Δ, end Δ) in cells, output minus UST; the cost counts timing classes only.

| start Δ | end Δ | pairs | cost | share of pair cost |
|---|---|---|---|---|
| 0 | +2 or more | 202 | 202 | 17.5% |
| 0 | +1 | 167 | 167 | 14.5% |
| +1 | 0 | 85 | 170 | 14.7% |
| −1 | 0 | 71 | 142 | 12.3% |
| 0 | −1 | 121 | 121 | 10.5% |
| +1 | +1 or more | 44 | 64 | 5.6% |
| other | | 58 | 86 | 7.4% |

End-only errors by the gap to the next note: running on through a UST rest while the output goes legato into
the next note is the largest group (191 at +2 or more, 31 at +1). Next come one-cell ends inside legato runs
(93 late, 58 early). Pitch errors: −12 57, ±1 102, ±2 18, other 26.

The stage's pairs carry 915 of 4 950 (18.5%). It has the same jitter (de ±1: 151/131; ds ±1: 99/71) but almost
none of the overruns (16 at +2 or more).

## Oracles

The output note of each matching pair takes the UST's start, end or pitch, and the real metric is re-run.
Cost/min Δ with a paired song bootstrap; the share is of the baseline; clean measures after the fix. The
spent-split columns are GAME lead, diagnosis only.

| fixed perfectly | GAME lead Δ [95%] | share | clean | stage Δ | holdout | holdout2 | holdout3 |
|---|---|---|---|---|---|---|---|
| **end late into a UST rest (release)** | **−9.5 [−11.5, −7.5]** | 5.1% | **23.1%** | −2.0 | −4.3 | −8.1 | −8.5 |
| end early before a UST rest | −1.5 [−2.6, −0.7] | 0.8% | 18.2% | −1.7 | | | |
| end late, UST legato | −3.0 [−4.9, −1.5] | 1.6% | 16.4% | −3.3 | | | |
| end early, UST legato | −0.9 [−1.4, −0.5] | 0.5% | 16.8% | −1.0 | | | |
| every end | −15.0 [−17.8, −12.3] | 8.0% | 26.5% | −8.0 | −7.7 | −14.6 | −11.9 |
| start late after a UST rest | −1.5 [−3.4, −0.2] | 0.8% | 16.4% | −1.4 | | | |
| start late, UST legato | −8.5 [−16.9, −2.1] | 4.5% | 17.2% | −9.0 | | | |
| start early, UST legato | −5.6 [−7.9, −3.3] | 3.0% | 16.5% | −5.8 | | | |
| every start | −16.3 [−28.2, −7.3] | 8.7% | 17.8% | −16.6 | −10.4 | −11.5 | −9.5 |
| every start and end | −33.6 [−50.3, −21.2] | 17.8% | 32.0% | −26.3 | | −27.7 | −22.6 |
| pitch: octave | −2.2 [−6.2, −0.1] | 1.2% | 16.4% | −1.0 | −1.3 | −5.5 | −13.3 |
| pitch: ±1 semitone | −3.7 [−7.1, −1.4] | 2.0% | 17.2% | −3.2 | −3.1 | −2.5 | −2.3 |
| pitch: ±2 / other | −0.7 / −0.8 | 0.7% | | −0.8 / −0.9 | | | |
| every pitch | −7.4 [−15.5, −2.8] | 3.9% | 17.6% | −5.9 | −6.1 | −10.6 | −18.9 |
| every pair error (03) | −41.0 [−58.7, −26.8] | 21.7% | 35.4% | −32.3 | −26.7 | −38.3 | −41.4 |

holdout3's octave row is two songs (anata, ironication), as 03 found.

## Mechanisms tried (dev)

| mechanism | evidence | GAME lead Δ [95%] | clean Δ | stage Δ | verdict |
|---|---|---|---|---|---|
| end at the last voiced pYIN frame (tail ≥ 0.5–2 cells) | pitch track | +13.7 … +29.1 | −10 … −14 | +17.7 … +35.3 | worse |
| drop quiet trailing cells, energy over the note's median, ≥ 2 cells | energy | −0.3 … −3.1 | ≤ +1.5 | −0.0 … +0.5 | weak |
| same, 1-cell trims allowed | energy | +2.2 … +24.9 | | | worse |
| **dip rule: first cell < 0.3 of the loudest, later cells < 0.8, ≥ 2 cells (ported)** | energy | **−4.4 [−6.5, −2.5]** | **+2.5** | +0.4 [−0.3, +1.4] | GAME only |
| … design chosen leave-one-song-out | | −3.7 [−5.4, −1.9] | +2.1 | | |
| logistic model on 13 energy/voicing features, leave-one-song-out | energy + voicing | −0.5 [−3.2, +2.0] | +0.2 | | worse |
| perfect choice among energy candidates (ceiling of the evidence) | labels | −5.8 [−7.6, −4.1] | +4.5 | | |
| global onset/offset rounding bias ±0.1–0.3 cell | none | −0.5 … +18.9 | ≤ −0.1 | | no gain |
| GAME times shifted ±10–30 ms | none | −1.5 … +47.1 | | | no gain |
| GAME's own onset phase (circular mean) | GAME onsets | −2.4 [−6.5, −0.0] | −0.4 | | fails (clean) |
| pitch-track transition for legato boundaries | pitch track | fixes 44, breaks 663 | | | worse |
| re-round near-boundary pitches by the track | pitch track | fixes 23, breaks 11 (≈ −0.4) | | | too small |
| octave check thresholds | harmonic ratio | fixes 24, breaks 278 | | | worse |

The dip rule per song (GAME lead, cost/min): amaten 394.1 → 392.8, atamaro 347.0 → 342.2, azemichi
75.1 → 70.0, fukagyakuteki 256.3 → 245.8, innocent 171.9 → 168.8, kamaitachi 100.5 → 92.5, my-axiom
190.1 → 184.6, telescope 93.8 → 91.2, yamineko 128.1 → 129.2. On some songs a trim also turns a joined
note (4) into a note and a missing one (2) where a rest splits it (atamaro −30 joined, +32 missing).

## What else these numbers could be measuring

- **The author's habit rather than the singing.** The UST's rests are 8th-note multiples, so part of the
  release cost is Pifuyuu's notation. But the energy really falls in those rests (median 0.16–0.28 of the
  note), so a user would draw the same rest. The 55 false cuts are where he did not.
- **One song.** No song carries the release gain: 7 of 9 improve and none worsens by more than 1.1 cost/min.
  atamaro (triplet grid) dominates the start rows and the own-phase result.
- **Selection on dev.** The rule was chosen from about 60 settings. The leave-one-song-out choice keeps
  −3.7 of −4.4 and picks the same quiet share and length on every fold. The spent splits move the same way,
  but they confirm nothing. A default needs holdout4.
- **The in-sample stage.** Stage numbers on dev are in-sample. The conclusion drawn from them (do not trim
  the stage) is conservative.
- **Metric artefacts.** A trim can turn a joined note into a note and a missing note. That is cheaper, and it
  is the edit a user would make: delete the held part, draw the next note.
- **The ports.** `lib12.snap_notes` reproduces the dumped GAME lead notes on every song, and `ec.py` the Rust
  counts. The Rust `trim_releases` gives the prototype's per-song class counts on all 18 song × output rows
  (`check_release_port.py`).

## Verification

- `sona-eval-cellstage --edit-cost` without the flag is **byte-identical** to the baseline run (`cmp`).
- `crates/sona-core/tests/release.rs`: 8 behaviour tests through `trim_releases`.
  - A breath ends the note.
  - Quiet is measured against the loudest cell.
  - A consonant lifting the last cell does not hide the release, but a loud cell after the dip does.
  - One-cell dips and fades are kept.
  - A random property: notes only get shorter, stay on the grid and keep a cell.
- Mutants caught: no tail check, cut one cell late, `min_cells` ignored, median instead of loudest.

## Reproduce

```sh
# Rust (dev): adds "GAME lead + octave rule, releases trimmed (opt-in)" and the stage's row.
ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
    --model ~/.cache/sona/cellstage-D --edit-cost --trim-releases
# Prototype, on a --dump of dev (UST content: keep it out of the repo)
export S12_DUMP=…
cd crates/sona-eval/scripts/single-note-errors
python3 -I check_port.py; python3 -I taxo12.py lead; python3 -I oracle12.py
python3 -I release.py; python3 -I release_auc.py; python3 -I release_energy.py sweep2|loso2|song 0.3 0.8 2
python3 -I release_learn.py; python3 -I causes.py; python3 -I bias_sweep.py; python3 -I phase_check.py
python3 -I boundary_check.py; python3 -I pitch_check.py; python3 -I octave_check.py
python3 -I check_release_port.py RUST_OUTPUT
```
