# 20 — Measure the holdout4 bundle together on dev

Status: resolved
Type: prototype
Blocked by: 19

## What

Four opt-in winners were each measured alone on dev, against different baselines and partly in sample:

- `--merge-unheard-pieces` (stage 272.2 → 267.2, in-sample stage);
- `--settle-near-ties` (18; GAME lead 303.0 → 297.8; nothing on the stage);
- 14's line fit (`--lyric-placement --lyric-lines --lyric-line-times`; the gain on GAME lead vanished under
  the revised weights: 304.2 vs 303.0) plus lyrics-17's S1 start moves (three CTCs, abstain on spread > 3;
  GAME lead 292.6, Python only, `scripts/mora-split/split.py`);
- 19's stage C (CV 275.8 → 254.1, out of sample; per-fold models in
  `.claude/worktrees/agent-a617324741adeda06/target/stage19-cv/F/C/models`, scored with `--fold-models`).

Winners can interfere. Before the one holdout4 read, measure the bundle as it would ship, on the dev songs
(9), with the stage scored out of sample (C's fold models). Answer:

1. Each arm's Δ on top of C (out of sample), one at a time, and all together, with paired song-bootstrap
   intervals, clean measures and missing counts (revised weights). Baseline B's fold models
   (`.../F/B/models`) give the "today" row under the same folds.
2. Does the lyric (14 fit + S1) still pay on top of C? S1 was built on GAME lead notes; apply it to C's notes.
   17's rule compares against **no lyric**: the lyric arm must beat C alone with the upper bound < 0.
3. Where does C's cost sit after the bundle (taxonomy by class, as `ec.py` does)? Joined notes are C's
   largest class; how much of it does the lyric remove?
4. The bundle that holdout4 will read: the arms that pass, in one command line (Rust where an arm is ported;
   name any arm that is Python-only and what porting it needs).

## Rules

- Commit the selection rule before scoring: an arm stays in the bundle if its Δ on top of the rest has an upper
  bound < 0 on dev and missing does not rise by more than its bound allows at missing weight 4 (19's rule 2).
- Dev only (dev may be re-read freely). Never touch holdout4. Spent splits only for diagnosis, through the
  ledger with `--reread --reason`.
- No UST, audio, stem, lyric or lyric-derived content, and no weights, in the repo.
- Build only into your worktree's own `target/`. Training is not part of this ticket.

## Acceptance

- Selection rule committed first; a table of arms (alone on C, all together, leave-one-out from the bundle).
- The bundle's command line and the list of Python-only arms with port plans.
- Report under `crates/sona-eval/reports/`, an `## Answer` on this ticket, a decision line in `map.md`.

## Comments

### Selection rule (committed before any scoring; agent, 2026-11-24)

**Data.** Dev (9 songs) only. The stage is C scored out of sample: `sona-eval-cellstage --edit-cost --model
C/fold0 --fold-models C/models` (19's fold models, `.claude/worktrees/agent-a617324741adeda06/target/stage19-cv/F`).
"Today" is B's fold models under the same folds. holdout4 is never read; no spent split is read.

**Arms**, applied to C's stage notes on the onset phase, always in this order (as they would run in the app):

- **T** `--settle-near-ties` (18, `NearTieRule::default()`, GAME's vocals run as the second listen);
- **M** `--merge-unheard-pieces` (11);
- **L** the lyric: 14's fit on aligned lines and spaces (`fit_lines`; lines `l01/lines`, times
  `l14/times/lyrics-lead-energy`) with 17's S1 start moves on top (`split.py` variant
  `file-m3+adaptmora+abst3.0:split+start1`). One arm. 14's fit alone (`tail`) is reported as a diagnostic,
  not a candidate.

T and M come from Rust (the scorer's `--dump` writes the stage with them applied); L runs in Python on those
notes. **Gate before any Δ is read:** my build reproduces C's dev report (291.7) and B's byte for byte, and
the Python composition reproduces, per song and class, the Rust rows it can be checked against (stage merged,
settled, settled + merged; 14's fit on the plain stage). If not, fix first.

**Statistic.** Pooled edit cost/min over the 9 songs under the revised weights (extra 1 · pitch 3 · octave 1 ·
length 4 · shift 4 · moved 4 · missing 2 · chopped 4 · joined 4); paired song bootstrap, 10 000 resamples,
seed 1 (`lib01.paired`). Missing counts and clean measures are reported beside every row.

**Rule (backward elimination from the full bundle).**

1. S = {T, M, L}.
2. For each arm a in S, compare S with S \ {a} (leave one out). a passes if
   - the 95% upper bound of Δ cost/min (S − S\{a}) is < 0, and
   - missing does not rise, or, if it rises, the upper bound of the same Δ with missing re-weighted to 4 is
     still < 0 (19's rule 2).
3. If every arm in S passes, S is the bundle holdout4 reads. Otherwise drop the failing arm with the highest
   upper bound (tie: larger point Δ) and go back to 2. S may end empty (C alone).
4. For L, S \ {L} has no lyric, so 2 is exactly 17's rule (against no lyric).
5. Reported, not criteria: each arm alone on C (vs C), all three vs C, C vs B, clean measures, the taxonomy by
   class (`ec.py`), and how much of C's joined cost L removes.

**What the dev numbers can't say.** Every arm's parameters were chosen on dev (T's 0.35 leave-one-song-out on
dev; M's rule on dev; S1's sigma, scale and abstention looked at on dev, its per-class offsets
leave-one-song-out on dev). So these Δ carry selection; only holdout4 confirms. With 9 songs the noise floor is
wide: 19's seed check on 38 songs was −0.9 [−6.2, +4.1].

## Answer

`crates/sona-eval/reports/bundle-on-dev.md`. Dev only (9 songs); C scored out of sample (fold models); holdout4
and the spent splits not read. My build reproduces C's and B's dev reports byte for byte (291.7, 322.3). The
Python composition equals the Rust rows per song and class.

| row | cost/min | Δ [95%] | clean | missing (Δ) |
|---|---|---|---|---|
| B (today) | 322.3 | | 16.0% | 300 |
| C | 291.7 | −30.6 [−57.1, −9.4] vs B | 18.6% | 362 |
| C + T (near ties) | 289.2 | −2.5 [−4.8, −0.3] | 18.6% | +0 |
| C + M (merge pieces) | 290.1 | −1.6 [−4.1, +0.1] | 18.8% | +4 |
| **C + L** (14's fit + S1) | **271.8** | **−19.8 [−41.0, −1.5]** | 20.3% | −4 |
| C + T + M + L | 270.6 | −21.1 [−43.1, −1.5] | 20.3% | +2 |
| leave out T / M / L from all three | | −0.9 [−3.7, +1.6] / −0.1 [−1.9, +1.3] / −17.2 [−38.6, +1.2] | | +2 / +5 / −4 |

1. **The rule gives C + L.**
   - T fails on top of the rest and is dropped first (highest bound).
   - M then fails on top of L (−0.4 [−2.0, +0.8]).
   - L passes alone and against no lyric (17's rule).
2. **The lyric pays on top of C.** Most of it is S1: 14's fit alone is −5.0 [−26.3, +10.1], and S1 on top adds
   −14.9 [−24.7, −6.3].
   - The bound leans on amaten (−96.5 there). Without it: −12.3 [−26.1, +2.7]. 7/9 songs improve.
   - The lyric is the UST's own moras, so this is an upper estimate for a real paste.
3. **Where C's cost sits after the bundle:** length 32%, joined 26%, chopped 12%, shift 10%, missing 10%, pitch 8%.
   - L removes **28% of C's joined notes** (666 → 479).
   - It adds 49 chopped, 44 near-miss pitch and 10 octave errors.
   - Length is now the largest class.
4. **The bundle holdout4 reads.**
   - Rust: `sona-eval-cellstage --model ~/.cache/sona/cellstage-19C --edit-cost --lyric-placement --lyric-lines
     LINES --lyric-line-times TIMES --dump DUMP`.
   - Then Python: S1 (`split.py --variants none,tail,file-m3+adaptmora+abst3.0:split+start1`; the `stage` rows).
   - **S1 is the only Python-only arm.** Port plan in the report: first `fit_lines_with_onsets` +
     `--lyric-onsets DIR`; the three CTC aligners via ONNX or a sidecar.
   - **Before the read, `sources.py`'s per-class offsets must be frozen from dev.** They are fitted
     leave-one-song-out on the scored songs' own UST, which would leak on holdout4.

Code: `--dump` writes the stage with T/M applied when those flags are on (default run and dump byte-identical;
1 test, 2 mutants caught). `scripts/bundle/bundle.py`.
