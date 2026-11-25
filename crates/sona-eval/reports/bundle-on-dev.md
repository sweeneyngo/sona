# The holdout4 bundle measured together on dev (utau-edit-cost 20)

Date: 2026-11-24. Songs: dev (9, 27.5 min). **holdout4 was not read; no spent split was read.** The stage is 19's C
scored out of sample (`--fold-models`: each dev song by the one fold model that never saw it); "today" is 19's
B under the same folds. Revised weights (`spec.md`): extra 1 · pitch 3 · octave 1 · length 4 · shift 4 · moved 4
· missing 2 · chopped 4 · joined 4. Intervals are paired song-bootstrap 95% (10 000 resamples, seed 1).

Selection rule (committed in the ticket before any scoring, 1430980): backward elimination from S = {T, M, L}.
An arm stays if S beats S without it with an upper bound < 0, and missing does not rise unless the bound
still holds with missing at 4. While an arm fails, the failing arm with the highest upper bound is dropped and
the rest re-tested.

Arms, applied to C's notes in the app's order:
- **T**: `--settle-near-ties` (18);
- **M**: `--merge-unheard-pieces` (11);
- **L**: 14's fit on the aligned lines and spaces, with lyrics-first 17's S1 start moves on top (three CTCs,
  abstain on spread > 3 cells; `split.py` variant `file-m3+adaptmora+abst3.0:split+start1`).

## Answer

- **The bundle holdout4 reads is C + L.** T and M fail on top of L and are dropped.
- **The lyric pays on top of C.** L alone is −19.8 [−41.0, −1.5] cost/min, and missing falls by 4. That
  passes 17's rule (against no lyric), which S1 failed on GAME lead (−10.5 [−31.4, +7.8]).
  - Most of the gain is S1's. 14's fit alone is flat on C (−5.0 [−26.3, +10.1]); S1 on top of it adds −14.9
    [−24.7, −6.3].
  - The pass leans on amaten (−96.5 cost/min there). Without amaten, L is −12.3 [−26.1, +2.7]. 7 of the 9
    songs get cheaper.
- **T and M interfere with L.** Each pays a little on C alone, and nothing once the lyric is there:

| row (9 dev songs) | cost/min [95%] | Δ [paired 95%] | clean | Δ clean | missing (Δ) | Δ upper, missing @4 |
|---|---|---|---|---|---|---|
| B, today (fold models) | 322.3 [212.7, 454.6] | | 16.0% | | 300 | |
| **C** (fold models), vs B | **291.7** [191.7, 410.5] | −30.6 [−57.1, −9.4] | 18.6% | +2.6 [+0.2, +5.9] | 362 (+62) | −4.3 |
| *alone on C, vs C* | | | | | | |
| C + T | 289.2 | −2.5 [−4.8, −0.3] | 18.6% | +0.0 | 362 (+0) | −0.4 |
| C + M | 290.1 | −1.6 [−4.1, +0.1] | 18.8% | +0.3 | 366 (+4) | +0.2 |
| **C + L** | **271.8** [180.6, 383.0] | **−19.8 [−41.0, −1.5]** | **20.3%** | **+1.8 [+0.6, +3.1]** | 358 (−4) | −1.9 |
| C + L14 (14's fit only; diagnostic) | 286.7 | −5.0 [−26.3, +10.1] | 18.7% | +0.2 | 363 (+1) | +10.4 |
| *pairs, vs C* | | | | | | |
| C + T + M | 287.8 | −3.9 [−7.7, −1.4] | 18.8% | +0.3 | 368 (+6) | −1.2 |
| C + T + L | 270.7 | −21.0 [−42.6, −1.8] | 20.3% | +1.7 | 359 (−3) | −2.1 |
| C + M + L | 271.4 | −20.2 [−41.5, −1.3] | 20.6% | +2.1 | 362 (+0) | −1.4 |
| **all together**, C + T + M + L, vs C | 270.6 [180.1, 381.4] | −21.1 [−43.1, −1.5] | 20.3% | +1.7 [+0.6, +3.0] | 364 (+2) | −1.4 |
| all together, vs B | | −51.7 [−92.8, −15.6] | | +4.3 [+1.6, +8.7] | (+64) | −13.0 |
| *leave one out of {T, M, L}* | | | | | | |
| − T (T's share: S vs S\{T}) | | −0.9 [−3.7, +1.6] | | −0.3 | +2 | +1.7 → **fails, dropped first** (highest bound) |
| − M | | −0.1 [−1.9, +1.3] | | +0.0 | +5 | +1.7 → fails |
| − L | | −17.2 [−38.6, +1.2] | | +1.4 | −4 | +0.9 → fails |
| *leave one out of {M, L}* | | | | | | |
| − M | | −0.4 [−2.0, +0.8] | | +0.3 | +4 | +1.1 → **fails, dropped** |
| − L | | −18.6 [−39.8, −0.3] | | +1.8 | −4 | −0.6 → passes |
| *{L}* | | | | | | |
| − L (= C + L vs C) | | −19.8 [−41.0, −1.5] | | +1.8 | −4 | −1.9 → **passes: bundle = C + L** |

- **Other pairings, not part of the rule.**
  - T on top of L: −1.1 [−3.6, +0.6]. M on top of L: −0.4 [−2.0, +0.8].
  - T on top of M: −2.3 [−4.1, −0.6]. M on top of T: −1.5 [−4.0, +0.6].
- **Interference.** T settles one-cell edges using GAME's rounding residual; S1 moves starts by one cell to
  the vowel onsets.
  - On C, T's gain is length and shift (−14 and −2 notes).
  - L removes 31 shift errors and 10 length errors, so the two probably fix the same starts. That is not
    checked note by note.
  - M's merges overlap the fit, which also joins same-pitch pieces.
- **Where C's cost sits after the bundle** (weighted units, share of cost):

| | cost/min | length | joined | chopped | shift | missing | pitch | extra | moved | octave |
|---|---|---|---|---|---|---|---|---|---|---|
| B | 322.3 | 2 864 (32%) | 2 444 (28%) | 1 200 (14%) | 972 (11%) | 600 (7%) | 489 (6%) | 138 (2%) | 96 (1%) | 51 (1%) |
| C | 291.7 | 2 444 (31%) | **2 664 (33%)** | 664 (8%) | 872 (11%) | 724 (9%) | 447 (6%) | 98 (1%) | 64 (1%) | 36 (0%) |
| **C + L** | 271.8 | **2 404 (32%)** | 1 916 (26%) | 860 (12%) | 748 (10%) | 716 (10%) | 579 (8%) | 107 (1%) | 92 (1%) | 46 (1%) |
| C + T + M + L | 270.6 | 2 384 (32%) | 1 912 (26%) | 832 (11%) | 748 (10%) | 728 (10%) | 585 (8%) | 106 (1%) | 92 (1%) | 46 (1%) |

  - **The lyric removes 28.1% of C's joined notes** (666 → 479). On top of T + M it removes 28.3% (667 → 478).
  - It adds 49 chopped notes (166 → 215), 44 near-miss pitches and 10 octave errors: splits that land on the
    wrong cell or on the next note's pitch.
  - After the bundle, **length is the largest class (32%)**, then joined (26%). Timing as a whole (length +
    shift + moved) is 43%.
- **Python-only arm: S1.** Everything else in the bundle is Rust: C's stage (`--model`, `cellstage.json`) and
  14's fit (`fit_lines`, `--lyric-placement --lyric-lines --lyric-line-times`). Port plans are below.

## The bundle as holdout4 will run it

The stage is `~/.cache/sona/cellstage-19C` (all 38 spent songs, switch 1.5).

1. **Rust**: C's notes, 14's fit (as a Rust row) and the dump S1 needs.

   ```sh
   sona-eval-cellstage --model ~/.cache/sona/cellstage-19C --edit-cost --split holdout4 --reason "…" \
     --lyric-placement --lyric-lines LINES --lyric-line-times TIMES --dump DUMP
   ```

2. **Python**: S1 onsets on the holdout4 lead stems, then S1 on C's notes.
   - The three CTCs come from 14's `align_lines.py` pipeline. `sources.py` builds the median, the lattice
     prior from GAME lead's starts, and the abstention.
   - **Before the read, `sources.py`'s per-class offsets must be frozen from dev.** Today `debias()` fits them
     leave-one-song-out on the songs being scored, which reads those songs' UST onsets. On holdout4 that would
     be a leak.
   - Then run:

     ```sh
     python3 crates/sona-eval/scripts/mora-split/split.py --variants none,tail,file-m3+adaptmora+abst3.0:split+start1
     ```

     The `stage` rows are C, C + 14's fit, and C + L.

The holdout4 rule should pre-register C + L against C, as it stands here. T and M stay out. The map
already put `--trim-releases` out. T can still be used for GAME-only output, where 18 measured it.

## Port plans (Python-only parts)

S1 has two halves. Porting the second half gives a one-command Rust bundle, given onset files.

1. **The fit with onsets (no model; port first).** `fit_timed` + `onset_cuts` + `move_starts` in `split.py`
   become `sona_core::lyric_fit::fit_lines_with_onsets(cells, kinds, spans, onsets: &[Option<i64>], OnsetRule {
   split, start_tolerance: 1, legato: true, min_gap: 1 })`, which extends `fit_lines`. The rule:
   - cut a multi-mora note at its moras' onset cells when they fall strictly inside it and increase;
   - otherwise use the tail rule;
   - then move each note's start to its mora's onset if that onset is within 1 cell and the note stays at
     least a cell long;
   - a touching previous note's end follows the start.

   The scorer takes `--lyric-onsets DIR` (per-mora onset seconds or null, as `$L17_WORK/onsets/<name>/<id>.json`
   holds them). Acceptance:
   - behaviour tests through `fit_lines_with_onsets`, with a mutant of each rule caught;
   - Rust = `split.py` on the 9 dev songs' per-class rows (`bundle.py --dump-rows` has them);
   - the default run stays byte-identical.
2. **The onset source (three CTC models).** Three steps:
   - the three CTC forced alignments of the song's moras on the lead stem (`mrfakename/w2v-bert-2.0-music-lyrics-ctc`,
     MIT, and the hiragana and jg CTCs, MIT/Apache; 14 and 17);
   - per-class consonant offsets: a constant table, frozen from dev;
   - median, adaptive sigma (2 × the members' median spread, ≥ 0.2 cell), the lattice prior from GAME lead's
     note starts, a snap to the best cell, and abstention when the spread is > 3 cells.

   The last two steps are small pure functions (`sona_core::onsets`). The aligners are the cost:
   - either export them to ONNX and run the forced alignment (a Viterbi over the mora CTC graph) in Rust
     through `ort`, which already runs GAME;
   - or keep them as a sidecar process, like the separator venv.

   They add about 2 minutes per song on 4 cores (17). Running them in the app is the user's call (lyrics-first
   map), only if L survives holdout4.

## What else these numbers could be measuring

- **Stage leakage.** None. C is scored out of sample (fold models), and my build reproduces C's and B's dev
  reports byte for byte. The shipped in-sample model (272.2) is not used.
- **The lyric is the UST's own moras.** L lands the UST's mora sequence, as 01/14/17 did. A user's pasted
  hiragana has 5.3% wrong units (lyrics-first 07). The line times come from an aligner forced through those
  moras. So L's dev gain is an upper estimate for a real paste. holdout4 has the same property, so its read
  compares like with like, but neither measures a user's typing.
- **Selection on dev.** S1's sigma, scale and abstention, T's 0.35 and M's rule were all looked at on dev. S1's
  per-class offsets are leave-one-song-out on dev. These Δ carry that selection, and holdout4 is the
  confirmation.
- **One song.** amaten carries L's interval. It has C's most joined notes (133) and most missing notes (175).
  The lyric removes 100 joined notes there and adds 28 chopped.
  - Without amaten: −12.3 [−26.1, +2.7].
  - Dropping any other song leaves L between −17.5 and −25.6.
  - So the sign is stable and the bound is not. On holdout4 the rule needs the bound, so a 20-song read could
    fail it.
- **Losses.** L costs more on my-axiom (+29.6: 20 more length errors, 9 more shifts) and on kamaitachi (+7.9).
- **The S1 onset files.** They are reused from 17's run. They depend on GAME lead's output (the lattice prior)
  and its grid phase, and those are identical between 17's dump and both of mine on all 9 songs.
- **The composition.** It is checked against Rust. The Python rows equal the Rust report per song and class
  for C, C + T, C + M, C + T + M and C + 14's fit, and equal the dump's own Rust edit costs. `split.py` run
  directly on C's dump also gives C + L 271.8 and GAME lead + S1 292.6 (Δ vs 14's fit −11.6 [−18.1, −5.5]),
  which is 17's number.

## Negative results

- T is out of the stage bundle. It pays on C alone (−2.5 [−4.8, −0.3], missing +0) but not on top of L
  (−1.1 [−3.6, +0.6]) or in the full bundle (−0.9 [−3.7, +1.6]).
- M is out. Alone on C: −1.6 [−4.1, +0.1], missing +4. On top of L: −0.4 [−2.0, +0.8].
- 14's fit without S1 is flat on C (−5.0 [−26.3, +10.1]), as it was on GAME lead (+1.2).

## Code

- `sona-eval-cellstage --dump DIR` adds outputs only when `--settle-near-ties` and/or `--merge-unheard-pieces`
  are given: `stage_settled`, `stage_merged` and `stage_settled_merged` (settled, then merged), each with its
  Rust edit cost.
  - Without the flags, both the report and the dump are byte-identical to 50cfdb4's (`cmp`, `diff -r`). With
    the arm flags, the report is also byte-identical.
  - One behaviour test (`stage_arms`). Two mutants were caught: merging before settling, and dropping the merge.
- `scripts/bundle/bundle.py` does the following:
  - every subset of {T, M, L} on the candidate stage;
  - the comparator's plain row;
  - the backward elimination;
  - the taxonomy;
  - per-song rows;
  - `--check` against the Rust report.

## Reproduce

```sh
export ORT_DYLIB_PATH=…/onnxruntime/capi/libonnxruntime.so.1.23.2 SONA_GAME_DIR=…/GAME-1.0.3-medium-onnx \
  SONA_DEV_CACHE=… SONA_UST_DIR=…/pifuyuu-ust SONA_UST_STEMS_DIR=…/pifuyuu-stems
F=<19's worktree>/target/stage19-cv/F; SP=<session scratchpad>   # l01/lines, l14/{text,align,times}, l17/onsets
cargo build --release -p sona-eval --features game --bin sona-eval-cellstage
ARMS="--merge-unheard-pieces --settle-near-ties --lyric-placement --lyric-lines $SP/l01/lines \
  --lyric-line-times $SP/l14/times/lyrics-lead-energy"
for m in C B; do
  target/release/sona-eval-cellstage --model $F/$m/models/fold0 --fold-models $F/$m/models --edit-cost $ARMS \
    --dump $SP/t20/dump-$m > $SP/t20/new-$m-arms.txt
done
L01_DUMP=$SP/t20/dump-C L01_WORK=$SP/l01 L14_WORK=$SP/l14 L17_WORK=$SP/l17 \
  python3 crates/sona-eval/scripts/bundle/bundle.py --base-dump $SP/t20/dump-B --check $SP/t20/new-C-arms.txt --per-song
```

The C and B rows without `$ARMS` reproduce `$F/{C,B}/rust_dev.txt` byte for byte (291.7 and 322.3).
