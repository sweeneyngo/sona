# 11 — Merge wobble and scoop pieces inside one note

Status: resolved
Type: research
Blocked by: 03

## What

03: chopped is 12–24% of GAME lead's cost. Besides same-pitch restarts (07), the extra pieces are 1–2
semitone steps from vibrato, glide or drift (33–38%), and a one-cell piece at the note's start from a
scoop or consonant (17–28%). The learned stage chops more than GAME on every split and adds octave flips.

## Steps

- Oracle per subclass: merge exactly the pieces that are chopped in truth, then measure the cost removed.
- Mechanism: merge a short piece into its neighbour when it is within 1–2 semitones and the pitch track
  shows a glide or vibrato rather than a step; absorb a one-cell piece at a note's start into the note.
  Check how many true short notes each rule eats (they become joined, weight 4).
- Opt-in flag; dev numbers with intervals.

## Rules

Same as 07. Measure the combination with 07 before claiming either.

## Comments

- 2026-11-24 (claimed). **Selection rule, declared before any measurement.** A mechanism is worth porting
  if on dev it lowers GAME lead's pooled cost/min with a song-bootstrap 95% interval on the paired
  difference (mechanism minus baseline, 10 000 resamples of the 9 songs) that excludes 0, and it does not
  raise the learned stage's pooled cost/min (point estimate). Its thresholds are chosen leave-one-song-out
  on dev, and the out-of-fold numbers are the ones the rule is applied to. It is also measured combined
  with 07's opt-in `--fit-phrase-lyrics`, and the report states whether the gains add. Oracles (merging
  exactly the truly chopped pieces of a subclass) are ceilings and never port. For each rule, the number of
  true short notes it eats (turned into joined notes, weight 4) is reported. Dev only; spent splits are
  diagnosis only (logged re-reads) and confirm nothing; holdout4 is not read. Anything ported is opt-in
  and the default output stays byte-identical. Same-pitch restarts belong to 07 and are not redone here.

## Answer

Report: `crates/sona-eval/reports/pitch-wobble.md`. Dev only. No holdout was read. The spent splits' 03 dumps
were re-analysed once for diagnosis (ledger, ae3fd4e). Baseline reproduced: GAME lead 188.7 / 16.2%, stage
180.2 / 23.8%, pYIN 338.4.

- **Oracles (GAME lead Δ cost/min; stage beside it).** Each merges exactly the truly chopped cuts.
  - 1–2 semitone steps: −7.4 [−11.0, −3.9] (stage −9.2).
  - One-cell heads: −4.0 [−6.9, −1.3] (−8.3).
  - Both: **−11.3 [−17.0, −6.2]** (−17.5).
  - Larger steps: −1.6. Same-pitch restarts (07's): −9.6. The UST's pitch adds ≤ 1 cost/min.
- **The ticket's pitch-track mechanism fails.**
  - Truly chopped cuts are 3.8% of GAME lead's touching 1–2 semitone cuts (22% among one-cell pieces). A
    wrong merge hides a real note (joined, 4).
  - The pitch track and GAME's unrounded pitch separate them at AUC 0.55–0.65. Rounding drift is not the
    cause: 4 of 76 chopped cuts.
  - "≤ 2 cells, 1–2 st, pitch track shows a glide": +5.1 [+2.8, +7.6]. It merges 4 chopped cuts and eats
    50 true notes.
  - "Absorb a one-cell head": +1.0 [+0.1, +2.5]. It merges 0 chopped cuts and eats 10 true notes.
- **What works, a little: a second listen.** A one-cell note touching a neighbour 1–11 st away joins it
  where GAME's run on the full vocals has no note edge at the cut; of two touching one-cell notes, the
  first joins the second.
  - Every leave-one-song-out fold of a 336-rule grid picks it.
  - Out of fold: GAME lead **188.7 → 186.8, −1.9 [−3.2, −0.9]** (7/9 songs better). Stage **180.2 → 176.3,
    −3.9 [−7.8, −0.7]**.
  - It merges 17 chopped cuts and eats 10 true notes (stage: 42 and 25).
  - It passes the selection rule.
  - Caveat: the first grid lacked the one-cell-head-of-a-one-cell-note case and failed out of fold
    (+0.3 [−1.7, +3.7]). I added the case after seeing the head rule catch no chopped cut, then re-ran the
    same LOSO. The report gives both.
- **With 07's fit.**
  - GAME lead 173.6 (−15.1 [−31.1, −5.7]; −1.3 [−2.8, +0.4] against 07 alone).
  - Stage 157.9 (−4.5 [−7.5, −1.7] against 07 alone).
  - The gains roughly add.
- **Spent splits (diagnosis, confirms nothing).**
  - GAME lead −3.5 / −1.2 / −2.8 (holdout / holdout2 / holdout3).
  - Stage −4.6 / −4.4 / −5.0; holdout3 is the stage out of sample.
  - Same sign everywhere.
- **Ported, opt-in.** `sona_core::unheard_pieces::merge_unheard_pieces` and `sona-eval-cellstage
  --merge-unheard-pieces`, which combines with `--fit-phrase-lyrics`.
  - The Rust run equals the prototype on 36/36 song × output rows. The default run is byte-identical.
  - 9 behaviour tests (one a random-line property). 5 mutants caught.
  - Not wired into `sona transcribe`; a default needs holdout4.
- **What changes the plan.**
  - Chopped wobble and scoop pieces are not fixable from pitch evidence. The rest of their ceiling (~9 of
    11.3 on GAME lead) needs a note-level signal.
  - The vocals run's agreement is that kind of signal.
  - The learned stage already picks each cell's pitch with the vocals run's notes among its inputs, yet it
    leaves more of these pieces than GAME lead.
  - Hypothesis, untested: a stage trained to weigh a chop's cost, or given "the other run has no edge
    here" directly, could do better than this post-hoc rule. That is a retrain and needs a new holdout.
