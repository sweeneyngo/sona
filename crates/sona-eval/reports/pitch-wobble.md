# Wobble and scoop pieces: merge them inside one held note? (utau-edit-cost 11)

Date: 2026-11-24. Dev only (9 songs, 27.5 min). No holdout was read; holdout4 was not touched. The spent splits'
03 dumps were re-analysed once for diagnosis (no new run; logged in `corpus/holdout-reads.jsonl`). Baseline
reproduced from the GAME cache: pYIN 338.4, **GAME lead + octave 188.7 cost/min, 16.2% clean measures** (out
of sample), learned stage 180.2 / 23.8% (in-sample on dev).

Selection rule (committed in the ticket before measuring, 6423e43): port a merge rule only if, with its
thresholds chosen leave-one-song-out on dev, the out-of-fold GAME lead pooled cost/min drops with a paired
song-bootstrap 95% interval that excludes 0, and the stage's cost does not rise. Measure it with 07's
`--fit-phrase-lyrics` too.

## Answer

- **The ceiling is small.** Merging exactly the truly chopped 1–2 semitone steps and one-cell heads would
  remove **−11.3 [−17.0, −6.2] cost/min** on GAME lead (6% of its 188.7) and −17.5 on the stage. Same-pitch
  restarts (07's) are worth about as much again.
- **The pitch track can't find them.** Truly chopped cuts are 3.8% of GAME lead's touching 1–2 semitone
  cuts (6.0% on the stage), and a wrong merge hides a real note (joined, 4), as costly as the chop it fixes.
  Only one-cell pieces come near the needed ~50% precision (22–27% chopped). Inside that pool, sona's pitch
  track and GAME's unrounded pitch separate wobble from note at AUC 0.55–0.65, and the chop is not a rounding
  accident (GAME's own pitches differ by under half the step at only 4 of 76 chopped cuts). The ticket's rules
  as written lose:
  - "1–2 semitones, pitch track shows a glide": **+5.1 [+2.8, +7.6]** on GAME lead. It merges 4 chopped
    cuts and eats 50 true notes.
  - "absorb a one-cell head": +1.0 [+0.1, +2.5]. It merges 0 chopped cuts and eats 10 true notes; letting a
    one-cell head join a one-cell body gives +0.3.
- **A second listen finds a few.** GAME's run on the full vocals (one of the stage's sources, computed
  whenever the stage runs) gives the one usable signal: where it has no note edge at the cut, a one-cell
  piece is part of its neighbour 15 times out of 23 on GAME lead.
  - The rule: a one-cell note touching a neighbour 1–11 semitones away, with no vocals-run edge at the cut,
    joins that neighbour and takes its pitch. Of two touching one-cell notes, the first joins the second.
  - Every leave-one-song-out fold picks this rule from a grid of 336.
  - Out of fold: GAME lead **188.7 → 186.8, −1.9 [−3.2, −0.9]**, 7 of 9 songs better (amaten +0.4).
    Stage 180.2 → 176.3, **−3.9 [−7.8, −0.7]**.
  - Clean measures move 16.2 → 16.4% and 23.8 → 24.3%.
  - It merges 17 truly chopped cuts on GAME lead and **eats 10 true notes** (stage: 42 and 25).
  - This **passes the selection rule.**
- **Caveat: a second grid.** The first grid lacked the "one-cell head of a one-cell note" case. Out of fold
  it gave +0.3 [−1.7, +3.7], failing; with amaten held out, the training folds chose a different rule.
  - I added the case after seeing the head rule catch no chopped cut on GAME lead: all 5 truly chopped heads
    after a rest there have a one-cell body. Then I re-ran the same LOSO on the superset.
  - Both runs are reported. The second is the one that passes.
  - The spent splits (diagnosis only, below) point the same way.
- **With 07's fit** (merge, then fit each pasted phrase):
  - GAME lead: 188.7 → **173.6**, −15.1 [−31.1, −5.7]; against 07 alone (174.9), −1.3 [−2.8, +0.4].
  - Stage: 180.2 → 157.9; against 07 alone (162.4), **−4.5 [−7.5, −1.7]**.
  - The gains roughly add (alone: −1.9 and −3.9). 07 joins only same-pitch notes, and this rule merges only
    across a pitch step.
- **Ported, opt-in:** `sona_core::unheard_pieces::merge_unheard_pieces` and `sona-eval-cellstage
  --merge-unheard-pieces`.
  - The Rust run reproduces the prototype exactly: per-song class counts are equal on 36 of 36 rows
    (lead, stage, each with and without 07's fit).
  - The default run is byte-identical to the baseline.
  - Nine behaviour tests through the public interface, one of them a random-line property. Five mutants are
    caught: ignore the vocals edges, allow two-cell pieces, keep the first piece's pitch, prefer the larger
    step, merge octaves.
  - Nothing in `sona transcribe` or the app calls it yet.
- **What changes the plan.**
  - Chopped wobble and scoop pieces are not fixable from the pitch track; the remaining ~10 cost/min of
    their ceiling needs evidence about the note, not the pitch.
  - The cheapest lever left in chopped is a second transcription that agrees with the first.
  - The learned stage already picks each cell's pitch with the vocals run's notes among its inputs, yet it
    leaves more of these pieces than GAME lead.
  - Hypothesis, untested: a stage trained to weigh a chop's cost, or given "the other run has no edge here"
    directly, might do better than this post-hoc rule. That is a retrain, outside this ticket.
  - A default needs holdout4 (with ≥ 1 cost/min of expected gain, the bar should be a paired interval
    excluding 0 on GAME lead).

## Oracles: what each chopped subclass is worth (dev)

A cut is *truly chopped* when the output notes on both sides overlap the same UST note (the metric's chopped
count). Subclasses as in 03 (`taxo.py`'s `chop_kind`): a same-pitch restart (07's), a one-cell head (the first
piece of a UST note is one cell: a scoop or consonant), a 1–2 semitone step (vibrato, glide, drift), a larger
step, an octave flip, a rest. The oracle merges exactly the truly chopped cuts of a subclass and re-runs the
metric. The merged note takes the longer piece's pitch (a mechanism could do that) or the UST's pitch.

| merged exactly (truly chopped only) | GAME lead cuts | Δ cost/min [95%] | UST pitch | stage cuts | Δ cost/min [95%] | UST pitch |
|---|---|---|---|---|---|---|
| 1–2 semitone step | 55 | −7.4 [−11.0, −3.9] | −7.6 | 73 | −9.2 [−14.4, −4.5] | −9.8 |
| one-cell head | 28 | −4.0 [−6.9, −1.3] | −4.1 | 62 | −8.3 [−12.1, −4.5] | −8.7 |
| **step + head (this ticket)** | **83** | **−11.3 [−17.0, −6.2]** | −11.7 | **135** | **−17.5 [−25.1, −10.4]** | −18.5 |
| larger step | 14 | −1.6 [−3.1, −0.4] | −1.8 | 17 | −2.1 | −2.3 |
| octave flip | 0 | — | — | 5 | −0.6 | −0.7 |
| same-pitch restart (07's, for scale) | 69 | −9.6 [−12.9, −6.6] | | 77 | −10.9 [−14.6, −7.6] | |
| every chopped cut | 167 | −22.2 [−29.8, −14.9] | −22.9 | 240 | −30.8 [−41.2, −21.1] | −32.1 |

The subclass shares reproduce 03 exactly (lead 41.3 / 32.9 / 16.8 / 8.4%; stage 32.1 / 30.4 / 25.8 / 7.1 /
2.1 / 2.5%). The ceiling for this ticket is therefore −11.3 cost/min on GAME lead (6% of its cost) and −17.5 on
the stage. The UST's pitch adds at most 1 cost/min: these are cut errors, not pitch errors.

## Why a mechanism can't reach it: the truly chopped cuts are rare among their look-alikes

A rule sees only the output: two touching notes with a small step. On GAME lead there are 2 017 touching
1–2 semitone cuts and **76 of them (3.8%) are truly chopped**; on the stage 120 of 2 004 (6.0%). Merging a true
boundary hides a UST note (joined, 4), as expensive as the chop it would fix, so a rule needs about 50%
precision. The pool narrows only by length:

| touching 1–2 st cuts, shorter piece | lead: chopped / boundary | precision | stage: chopped / boundary | precision |
|---|---|---|---|---|
| 1 cell | 42 / 146 | 22% | 74 / 189 | 27% |
| 2 cells | 25 / 925 | 2.5% | 39 / 954 | 3.9% |
| 3 cells | 5 / 175 | 2.7% | 4 / 167 | 2.3% |
| 4+ cells | 4 / 651 | 0.6% | 3 / 546 | 0.5% |
| a 1-cell first piece after a rest (head), any step | 5 / 11 | 29% | 24 / 17 | 51% |

A vibrato-like return (A B A, B one step away) doesn't help: 21% precision with 1-cell pieces, 1% otherwise.

The evidence inside that pool, as the separation (AUC) of chopped from boundary cuts with a 1-cell piece:

| evidence (transcription time) | lead | stage |
|---|---|---|
| sona's pitch track over the short piece, as a fraction of the step | 0.65 | 0.56 |
| pitch-track medians of the two pieces, gap along the step | 0.62 | 0.55 |
| GAME's unrounded pitches of the two pieces, gap along the step | 0.57 | 0.57 |
| motion of the pitch track across the short piece toward its neighbour | 0.40 | 0.44 |
| **GAME's run on the full vocals has no cut there** | **0.65** | **0.69** |
| GAME vocals' pitches of the two pieces, gap along the step | 0.65 | 0.74 |

- **Drift is not the mechanism.** GAME's unrounded pitches differ by less than half the step at only 4 of 76
  truly chopped lead cuts: GAME hears two pitches, a semitone apart, not a rounding accident.
- **The pitch track can't tell a glide from a short note at this length.** A one-cell piece is 92–120 ms
  (16 to 20 frames of sona's track, often unvoiced at a consonant: 13 of 42 chopped lead pieces have no voiced
  frame). A real one-cell UST note (a passing or ornament note) looks the same.
- **The only useful evidence is a second opinion**: GAME's run on the vocals stem (the stage's other source).
  Where it doesn't cut, a 1-cell piece is chopped 15 times out of 23 on lead.

## Mechanisms (dev; Δ against the unmerged output, paired song bootstrap)

"Eats" counts the true UST boundaries a rule merges over: each hides a real note (joined, weight 4).

| rule | GAME lead Δ [95%] | merges: chopped / eats true notes / other | stage Δ [95%] | merges | + 07 fit, lead Δ vs 07 alone |
|---|---|---|---|---|---|
| ticket head: absorb a 1-cell head after a rest into the note | +1.0 [+0.1, +2.5] | 0 / 10 / 0 | −0.2 [−3.5, +2.6] | 11 / 14 / 5 | +0.6 [−0.7, +2.3] |
| … a 1-cell body counts as the note | +0.3 [−0.8, +2.0] | 5 / 10 / 1 | −1.3 [−4.4, +1.8] | 21 / 16 / 6 | −0.2 [−1.5, +1.6] |
| ticket step: ≤ 2 cells, 1–2 st, pitch track under half the step | **+5.1 [+2.8, +7.6]** | 4 / 50 / 3 | +4.4 [+1.8, +7.5] | 11 / 56 / 3 | +5.3 [+2.0, +9.5] |
| … 1-cell pieces only | +0.1 [−0.7, +1.4] | 3 / 10 / 1 | −0.3 [−1.8, +1.8] | 7 / 14 / 2 | −0.2 [−0.6, +0.1] |
| 1-cell piece, GAME vocals has no cut there (first grid's fold majority) | −1.3 [−2.0, −0.5] | 11 / 8 / 2 | −2.4 [−6.1, +0.4] | 27 / 24 / 3 | −0.9 [−2.3, +0.8] |
| first grid (168 rules), chosen leave-one-song-out | +0.3 [−1.7, +3.7] | | +0.5 [−5.7, +9.3] | | |
| **… and a 1-cell head joins a 1-cell note (ported)** | **−1.9 [−3.2, −0.9]** | **17 / 10 / 3** | **−3.9 [−7.8, −0.7]** | **42 / 25 / 4** | −1.3 [−2.8, +0.4] (stage −4.5 [−7.5, −1.7]) |
| **second grid (336 rules), chosen leave-one-song-out: every fold picks the row above** | **−1.9 [−3.2, −0.9]** | | **−3.9 [−7.8, −0.7]** | | |
| logistic model on all the evidence, LOSO, pieces ≤ 2 cells | −1.0 [−1.9, +0.2] | 12 / 13 / 3 | −0.4 [−5.6, +5.3] | 30 / 63 / 5 | |
| … pieces of 1 cell | +3.3 [−1.2, +9.4] | 14 / 52 / 3 | +0.3 [−4.9, +5.8] | 28 / 64 / 6 | |

The rule grid: shorter piece ≤ 1 or 2 cells; step ≤ 1, 2 or 11 semitones (never an octave); GAME vocals has no
cut there, or any; the pitch track over the short piece at most 25/50/75% of the way to its own pitch, or no
pitch-track test (a piece with no voiced frame passing or not); heads only or any position; equal-length head
pieces allowed or not. A short piece joins at most one neighbour (the smaller step) and the merged note takes the
neighbour's pitch.

Combined with 07's fit (merge first, then fit each pasted phrase): 07 alone is 174.9 on GAME lead and 162.4 on
the stage. The ported rule on top of it: GAME lead 173.6, −1.3 [−2.8, +0.4] against 07 alone; stage 157.9,
−4.5 [−7.5, −1.7]. The gains roughly add (alone: −1.9 and −3.9). 07's fit splits notes where a pasted phrase
has extra moras and joins only same-pitch notes, and these rules merge only across a pitch step.

## Spent splits: diagnostic re-analysis (confirms nothing)

This is a re-analysis of 03's dumps for holdout (9 songs), holdout2 (10) and holdout3 (10), with no new run,
logged in the ledger at ae3fd4e. The rules are unchanged from dev. The stage is in-sample on holdout and
holdout2 and out of sample on holdout3.

| split | output | baseline | oracle step + head | first grid's rule | **ported rule** | merges: chopped / eats / other |
|---|---|---|---|---|---|---|
| holdout | GAME lead | 139.8 | −19.3 | −2.3 [−3.1, −1.3] | **−3.5 [−4.9, −2.2]** | 32 / 16 / 1 |
| | stage | 132.9 | −23.5 | −2.7 [−4.1, −1.2] | **−4.6 [−6.4, −2.6]** | 44 / 29 / 4 |
| holdout2 | GAME lead | 182.3 | −13.7 | −0.6 [−2.1, +1.0] | **−1.2 [−3.3, +1.4]** | 25 / 31 / 11 |
| | stage | 178.9 | −18.8 | −2.5 [−4.8, −0.5] | **−4.4 [−7.5, −1.3]** | 45 / 33 / 4 |
| holdout3 | GAME lead | 161.1 | −11.4 | −1.1 [−2.4, +0.6] | **−2.8 [−5.4, −0.4]** | 27 / 23 / 12 |
| | stage (out of sample) | 156.3 | −14.5 | −2.5 [−4.1, −0.6] | **−5.0 [−7.4, −2.2]** | 41 / 17 / 25 |

The sign holds on every split and both outputs. The size is 1–3% of the cost, about a fifth of the
step-and-head oracle. The one-cell-head case adds a gain on every split, so the second grid did not pick up
a dev artefact. This confirms nothing; a default still needs holdout4.

## What else these numbers could be measuring

- **The truth label.** "Chopped" uses the metric's own matching: a piece shares more than half its cells with
  the UST note. The counts equal `taxo.py`'s and the Rust metric's on every song.
- **A boundary merge is not always a full +4.** Merging next to an already broken component can cost less, so a
  rule with a 50/50 merge mix can still lower the cost a little (the fold-majority rule on the stage: 27 / 24,
  −2.4). That is why the selection is by the real metric, not by precision.
- **Fold instability.** In the first grid, with amaten held out, the training folds prefer a different rule.
  amaten (joined-heavy, 69% one-cell notes) is where every rule hurts most, and its one bad fold is what fails
  that grid. In the second grid every fold picks the same rule, amaten's too (+0.4 on amaten).
- **One song.** Out of fold on GAME lead the gain is spread: telescope −5.8, kamaitachi −3.3, my-axiom −2.4,
  four more songs −1.1 to −1.9, atamaro 0, amaten +0.4.
- **The second listen is not independent.** GAME on the vocals stem hears the same singer through the
  backing vocals. Where both runs cut, the cut is more often real, but a shared mistake passes. That is part
  of why the precision is only about 60%.
- **The stage is in-sample on dev**; GAME lead is the reference.
- **The Python port.** Scores come from `ec.py`, which reproduces the Rust counts exactly (03); 07's fit,
  re-implemented on arbitrary notes here (`fit07.py`), reproduces 07's −13.9 / −17.8 exactly.

## Reproduce

```sh
# Rust (dev): adds "… unheard pieces merged (opt-in)" reports (and merged + fitted with --fit-phrase-lyrics);
# the default run is byte-identical.
ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
    --model ~/.cache/sona/cellstage-D --edit-cost --merge-unheard-pieces --fit-phrase-lyrics > rust-merge.out
# (flag deleted 2026-11-25, utau-edit-cost 17)
# Prototype
export S11_DUMP=…/dump-dev            # `sona-eval-cellstage --dump` of dev (UST content: keep it out of the repo)
cd crates/sona-eval/scripts/pitch-wobble
python3 oracle.py      # per-subclass oracles
python3 cands.py       # candidate pools by truth
python3 wfeats.py      # evidence AUCs in each pool
python3 gaps.py        # pitch gaps (GAME unrounded, GAME vocals, pitch track)
python3 context.py     # A-B-A returns
python3 rules.py       # the rule grid, in-sample and leave-one-song-out
python3 learned.py 2   # logistic model, LOSO (pieces <= 2 cells; `1` for 1 cell)
python3 named.py       # the ticket's rules, alone and with 07's fit, per song
python3 fit07.py       # 07's fit reproduced on these notes
S11_WORK=… python3 proto_counts.py rust-merge.out   # the port against the prototype, per song
# (script deleted 2026-11-25, utau-edit-cost 17, with the flag it checked)
S11_DUMP=…/dump-holdout python3 spent.py   # diagnosis only, logged
```
