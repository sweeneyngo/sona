# Lean toward a note when unsure (utau-edit-cost 06)

Date: 2026-11-24. Dev only (9 songs, 27.5 min); no holdout was read. Ticket:
`.scratch/utau-edit-cost/issues/06-lean-toward-sung.md` (selection rule committed at 499821a, before the Rust
sweep). The learned stage is **in-sample on dev**, so its rows are an optimistic view; GAME lead is out of sample.

## Answer

**Negative. No setting passes the rule, and the knob is deleted.** Leaning toward sung trades each missing note
(2) for an extra, a chop or a join (1 or 4), and on dev the trade never comes out ahead.

- **Rest bias on the stage's choice** (rest's logit lowered by b): cost rises with b, +1.2 [+0.2, +2.4] at
  b = 0.5. The bias mostly lengthens notes into the rests the UST writes, rather than drawing missing notes.
  The unbiased stage is already at the minimum. The best bias chosen per song with hindsight gains only −2.1.
- **Drawing cells the model believes sung but no source pitches** (P(rest) < τ, no candidate): this removes
  up to 100 of the stage's 213 missing notes. The best setting still costs +0.9 [−3.3, +4.1].
  - Nearly every such run touches a note, so drawing it is a split-or-extend decision. As its own note it is
    chopped wherever the UST holds one note there (+24 chops at τ = 0.2). Extending the neighbour joins it
    wherever the UST has two notes, as in げ+ん (+68 joins).
  - A perfect per-run choice between skip, own note and extend, made with the UST, is worth only −3.9 to −4.4.
- **GAME's note presence is where the missing notes are dropped.** 86% of GAME lead's missing notes lie under a
  GAME note it marked unvoiced, but that note has the UST's pitch only 18% of the time.
  - Promoting the best possible subset of unvoiced notes, chosen with the UST, is worth at most −2.9 on GAME
    lead. A presence threshold can only select such a subset, so GAME was not re-run.
  - Promoting every unvoiced note at least 1 s long costs +4.2 [+2.3, +6.6].
- **Lyrics (the Fog):** with the lyric placed by lines and spaces, rests holding moras (stage 167.4), extras do
  not steal moras. Fills place slightly more moras (wrong moras −1.0 to −2.9 per minute), but cost does not
  fall: best +0.1 [−3.6, +2.8].
- **Why:** the missing notes are quiet moras next to a sung note: ん is 25–34% of them against 1% of matched
  notes, and the pitch track is unvoiced over most of them (median voiced share 0.00, against 0.57). The model
  knows they are sung: its 1 − P(rest) separates them from other rest cells with AUC 0.90 on the stage's rests
  and 0.96 on GAME lead's. But no source gives their pitch or their boundary with the neighbour. The missing
  oracle's −21.5 (`edit-cost-baseline.md`) needs both; perfect timing with the neighbour's pitch already drops
  to −12.9 (GAME lead) and −9.9 (stage).

Where that leaves the lever: most missing notes here are moras the singer runs into a neighbouring note. That
is the joined-notes problem of 07: it needs evidence of where the mora starts (lyrics, a spectral detector),
not a looser sung/rest threshold.

## Rule (from the ticket) and verdict

Ships opt-in only if one setting has, on the stage without lyrics, a pooled cost/min Δ whose paired song-bootstrap
95% upper bound is < 0 and clean measures not below the unmodified stage's; and, stacked with lyric placement
(lines and spaces, rests hold moras), a cost/min Δ ≤ 0 and wrong moras/min not up.

**Verdict: fail.** The smallest upper bound without lyrics is +2.4 for the bias and +3.6 for the fills. Every
point estimate is ≥ +0.9 and clean measures never rise by more than 0.1 point. The knob
(`choose_class_leaning`, `fill_unproposed`, `--lean-sung`) lived at 43d9b8d and was deleted after it.

## Sweep (Rust, `sona-eval-cellstage --edit-cost --lean-sung`)

Learned cell stage, onset phase (what ships; in-sample). Δ against the unmodified stage, paired song-bootstrap 95%
(10 000 resamples). Counts are UST notes missing, output notes extra, hidden notes joined, extra pieces chopped.

| setting | cost/min | Δ [95%] | clean | Δclean | cells | missing | extra | joined | chopped |
|---|---|---|---|---|---|---|---|---|---|
| stage (unmodified) | 180.2 | — | 23.8% | — | 92.69% | 213 | 33 | 654 | 240 |
| rest bias 0.5 | 181.4 | +1.2 [+0.2, +2.4] | 23.8% | +0.0 | 92.59% | 204 | 40 | 660 | 244 |
| rest bias 1 | 182.5 | +2.3 [+0.9, +4.0] | 23.9% | +0.1 | 92.49% | 200 | 46 | 662 | 249 |
| rest bias 2 | 185.7 | +5.5 [+3.7, +7.6] | 21.4% | −2.4 | 92.06% | 182 | 68 | 672 | 247 |
| fill P(rest) < 0.1, own note | 181.5 | +1.3 [−1.5, +3.6] | 23.6% | −0.2 | 92.76% | 162 | 51 | 658 | 264 |
| fill P(rest) < 0.2, own note | 181.1 | +0.9 [−3.3, +4.1] | 23.4% | −0.4 | 92.74% | 138 | 62 | 661 | 264 |
| fill P(rest) < 0.3, own note | 181.5 | +1.3 [−3.6, +5.1] | 23.1% | −0.7 | 92.71% | 125 | 74 | 663 | 269 |
| fill P(rest) < 0.5, own note | 183.0 | +2.8 [−2.2, +7.0] | 22.6% | −1.2 | 92.53% | 113 | 103 | 664 | 273 |
| fill P(rest) < 0.1, extend | 181.8 | +1.6 [−0.2, +4.1] | 23.8% | −0.0 | 92.76% | 163 | 39 | 700 | 235 |
| fill P(rest) < 0.2, extend | 183.1 | +2.9 [−0.0, +7.3] | 23.5% | −0.3 | 92.74% | 141 | 38 | 722 | 235 |
| fill P(rest) < 0.3, extend | 184.2 | +4.0 [+0.5, +9.4] | 23.1% | −0.7 | 92.71% | 129 | 36 | 734 | 236 |
| fill P(rest) < 0.5, extend | 185.8 | +5.6 [+1.1, +12.6] | 22.5% | −1.3 | 92.53% | 118 | 42 | 743 | 236 |

Stacked with the lyric placed by its lines and spaces, rests holding moras (`--lyric-placement --lyric-lines`):

| setting | cost/min | Δ [95%] | clean | wrong moras/min | Δwrong [95%] |
|---|---|---|---|---|---|
| stage (unmodified) | 167.4 | — | 23.9% | 86.2 | — |
| rest bias 0.5 | 168.7 | +1.3 [−0.0, +3.1] | 24.0% | 85.9 | −0.3 [−1.3, +0.7] |
| rest bias 1 | 170.0 | +2.6 [+0.7, +4.9] | 24.1% | 84.8 | −1.4 [−2.3, −0.4] |
| rest bias 2 | 172.6 | +5.2 [+3.1, +7.6] | 21.6% | 84.3 | −1.9 [−3.6, −0.1] |
| fill P(rest) < 0.1, own note | 167.9 | +0.5 [−1.8, +2.7] | 23.7% | 84.7 | −1.5 [−3.4, −0.0] |
| fill P(rest) < 0.2, own note | 167.5 | +0.1 [−3.6, +2.8] | 23.5% | 85.2 | −1.0 [−2.6, +0.0] |
| fill P(rest) < 0.3, own note | 168.0 | +0.6 [−4.3, +4.6] | 23.2% | 83.3 | −2.9 [−6.6, −0.3] |
| fill P(rest) < 0.5, own note | 169.1 | +1.7 [−3.8, +6.5] | 22.7% | 83.5 | −2.7 [−6.3, −0.2] |
| fill P(rest) < 0.1, extend | 169.1 | +1.7 [−0.7, +5.0] | 23.7% | 85.8 | −0.4 [−1.4, +0.5] |
| fill P(rest) < 0.2, extend | 170.5 | +3.1 [−0.1, +7.1] | 23.4% | 86.0 | −0.2 [−0.9, +0.5] |
| fill P(rest) < 0.3, extend | 171.3 | +3.9 [+0.4, +8.5] | 23.0% | 85.3 | −0.9 [−2.9, +0.3] |
| fill P(rest) < 0.5, extend | 173.4 | +6.0 [+1.6, +11.9] | 22.3% | 86.0 | −0.2 [−2.0, +1.1] |

GAME lead with the same lyric landing is 175.2 (`lyric-placement.md`); no GAME-side knob was ported, because
every GAME-side ceiling below is under 3 cost/min.

The Rust port equals the prototype on all 11 settings: pooled cost, class counts and cells. The default
`--edit-cost` run is byte-identical to the one before the change. Two mutants failed the knob's tests: the bias
sign flipped, and the no-candidate condition dropped.

## Where the missing notes come from (dev)

`missing_taxo.py`, `premise.py` on a `--dump`. A missing note is a UST note no output note shares a majority of
cells with.

| | GAME lead | learned stage (in-sample) |
|---|---|---|
| missing UST notes | 312 | 213 |
| … on amaten and atamaro | 63% | 68% |
| one / two / three or more cells | 35 / 48 / 17% | 47 / 43 / 10% |
| touch an output note / one of the same pitch | 45 / 10% | 62 / 31% |
| mora: consonant / ん / bare vowel (matched notes: 87 / 1 / 11–12%) | 57 / 25 / 18% | 48 / 34 / 17% |
| pitch-track voiced share, median (matched notes: 0.57) | 0.00 | 0.00 |
| vocals' energy, median, log10 against the song's median (matched: +0.14) | −0.08 | −0.18 |
| under a GAME lead note marked unvoiced / with the UST's pitch | 86 / 18% | 71 / 11% |
| under a voiced GAME lead note (lost when snapped) | 14% | 29% |
| under a voiced GAME vocals note | 28% | 30% |
| under a sona pYIN note / with the UST's pitch | 44 / 28% | 22 / 13% |
| the stage has any candidate in its cells / the UST's pitch among them | 62 / 43% | 44 / 28% |
| the stage's mean P(rest) over it < 0.5 | 89% | 72% |

So GAME drops them at presence, sona's pitch track does not hear them, and the stage believes them sung but
has nothing to sing them with.

## Ceilings (oracles read the UST; dev)

| what could be done | GAME lead | stage (in-sample) |
|---|---|---|
| missing fixed perfectly (`edit-cost-baseline.md`) | −21.5 | −14.4 |
| each missing note drawn on its UST cells, pitch of the nearest output note | −12.9 | −9.9 |
| best subset of GAME's unvoiced lead notes promoted (any presence threshold is such a subset) | −2.9 | — |
| per run of unproposed sung cells, best of skip / own note / extend (τ 0.2 · 0.5 · 0.9) | −2.1 · −2.5 · −1.9 | −3.9 · −4.4 · −3.6 |
| new notes in the rests from the stage at rest bias 2 / 6 / 20, best subset | −2.6 (stage's notes in GAME lead's rests) | −0.5 |
| rest bias chosen per song from −2 … +2 | — | −2.1 |

Separability over the cells the output leaves at rest (`detector.py`), "a missing UST note covers this cell"
against all other rest cells:

| signal | GAME lead's rests (542 / 5 621 cells) | stage's rests (285 / 6 417) |
|---|---|---|
| stage 1 − P(rest) | 0.961 | 0.900 |
| vocals' energy | 0.861 | 0.751 |
| pitch-track voicing | 0.598 | 0.470 |

## Other prototype variants (Python, dev, no lyrics; Δ cost/min [95%])

- GAME lead, every unvoiced GAME note kept: +20.6 [+16.5, +25.4] (cells 86.8 → 63.5%). Kept only if
  ≥ 0.3 s: +8.6; if ≥ 1 s: +4.2 [+2.3, +6.6].
- GAME lead, rests filled with sona's pYIN notes ≥ 1 / 2 / 4 cells: +5.4 / +1.8 / +0.8. The stage the same way:
  +7.2 / +3.6 / +1.5.
- The stage plus only the biased stage's notes that lie wholly in its rests: ≥ 1 cell +1.1 … +5.1; ≥ 2 cells
  +0.1 … +2.0.
- Fill runs touching no note (the only ones without a split-or-extend choice): +0.1 to +0.4, because there are
  almost none.
- Fill pitch from the pitch track or GAME's unvoiced note instead of the neighbour: the same or worse (+0.9 …
  +1.8 at τ 0.2).
- GAME lead plus the stage's unproposed-cell fills (needs the stage, so not a GAME-only option): +0.9 … +12.7.

## What else these numbers could be measuring

- **The stage fitting dev.** It is in-sample, so its P(rest) is likely sharper here than on new songs. That
  makes the stage rows optimistic, and they still fail.
- **Two songs.** amaten and atamaro hold 63–68% of the missing notes, so the pooled missing counts mostly
  describe those two fast one-note-per-mora songs. Their missing and joined notes are the same mechanism, a
  mora run into its neighbour, which is why a fill there turns into a split decision.
- **The metric's matching.** A drawn note that covers a missing UST note but is a cell off counts as length or
  shift (1) plus pitch (1), so it gains nothing. That is the real cost in OpenUtau (move it, transpose it), so it
  is not an artefact.
- **Cached runs.** The cache was keyed and read back byte-identical (03). The unmodified row reproduces 180.2 /
  23.8% / 92.69% exactly, and the lyric row reproduces 167.4 / 86.2.

## Reproduce

```sh
# Rust sweep (the knob lives at 43d9b8d; deleted after it)
git checkout 43d9b8d
cargo build --release -p sona-eval --features game --bin sona-eval-cellstage
ORT_DYLIB_PATH=… SONA_GAME_DIR=…/GAME-1.0.3-medium-onnx SONA_DEV_CACHE=… ./target/release/sona-eval-cellstage \
    --model ~/.cache/sona/cellstage-D --edit-cost --lean-sung --lyric-placement --lyric-lines "$L01_WORK/lines"
# Prototype on a dev dump (holds UST content: keep it out of the repo)
./target/release/sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost --dump "$DUMP"
cd crates/sona-eval/scripts/lean-sung
python3 -I check_port.py "$DUMP"; python3 -I missing_taxo.py "$DUMP"; python3 -I premise.py "$DUMP" stage
python3 -I sweep.py "$DUMP"; NOTE_LEVEL=1 python3 -I sweep.py "$DUMP"; python3 -I oracle.py "$DUMP"
python3 -I detector.py "$DUMP"; python3 -I fill.py "$DUMP"; TOUCH=1 python3 -I fill.py "$DUMP"; python3 -I run_oracle.py "$DUMP"
```
