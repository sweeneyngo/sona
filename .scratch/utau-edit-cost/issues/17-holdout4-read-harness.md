# 17 — The harness for the one holdout4 read

Status: in progress
Type: task
Blocked by: lyrics-first 14 (its winner joins the bundle), ear labels for holdout4 (user)

## What

Everything the single pre-registered holdout4 read needs, built and tested on dev before the read:

1. **Alignment check first** (08): run the onset-delta check on every holdout4 song; drop the songs it flags
   (35 ms shift / 30 ms drift) before any score; at least 20 must remain or the read stops.
2. **`--split holdout4` through the ledger** for every scorer the bundle uses: `sona-eval-cellstage`
   (edit cost, lyric placement, the opt-in arms: 07/lyrics fit, 11, 12, 13's confidence curve) and
   `sona-eval-frame --estimate-bar-one` (05's arms A, B, C). Today `--estimate-bar-one` has no `--split` path.
3. **Ear labels**: the music's beat and bar 1 on each holdout4 song, from the click overlays
   (`sona-eval-frame --clicks`), with the user confirming at least 5 (05's rule).
4. **One script** that runs the whole read once, writes the ledger entry, and prints each pre-registered rule's
   verdict.

Rehearse the script end to end on dev (which can be re-read freely), then hand it over. Nothing touches
holdout4 until the user has labelled and the rules are final.

## Comments

### Pre-registered rules — DRAFT for the lead's review (agent, 2026-11-24; not final, nothing read)

Not yet approved. Once approved, the commit that marks them final is the read's `--rules-commit`, and
`read.py` refuses to run on holdout4 from any commit that does not descend from it. Holdout4 has not been
read: the ledger has no holdout4 entry, no holdout4 UST note or onset has been opened, no scorer has run on it.

**Data and order.** holdout4 (24 songs in the manifest), read once by `crates/sona-eval/scripts/holdout4-read/read.py`.
Its steps run in this order:

0. **Preflight, reading nothing.** Binaries, models, labels (`labels.py check`), every lyric input, a clean tree
   descending from the rules' commit, and no earlier read of the set. Then S1's member onsets (ticket 21):
   `s1_onsets.py check` and `produce` with the offsets frozen from dev. They read the aligners' output, the kana
   and the offsets: no UST time and no score.
1. **Alignment (08).** `sona-eval-ust-align --split holdout4 --write` runs the shipped onset-delta check and
   flags a song for `|median delta| > 35 ms`, `|drift| > 30 ms` or fewer than 30 matched onsets. This step writes the read's
   ledger entry. Flagged songs are dropped before any score. **Fewer than 20 aligned songs stop the read**: nothing below
   is computed, and the split counts as spent.
2. **Bar 1:** `sona-eval-frame --estimate-bar-one --split holdout4 --music-frames LABELS --beat-this …` (rule 4).
3. **Stage D:** `sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost --split holdout4 --dump …`
4. **Stage C, with T and L in the same run:** `sona-eval-cellstage --model ~/.cache/sona/cellstage-19C
   --edit-cost --settle-near-ties --lyric-placement --lyric-lines … --lyric-line-times … --lyric-onsets …
   --split holdout4 --dump …`. C+L is the report's row ending "split and started at the moras' onsets (S1)", and
   C+L14 is the row before it.
5. **Rows.** D, C and C+T come from the dumps.
6. **Verdicts.** `read.py` prints each verdict and writes `verdicts.json`.

Steps 2 to 4 are logged as same-sitting re-reads, as the holdout2 and holdout3 reads were. Before any verdict, the
Python rows must equal the Rust reports per song and class (D, C, C+T), and the Python copy of
`frame::song_bootstrap` must equal the interval the Rust report prints. If either differs, the read stops.

**Statistic for rules 1–3.** Pooled edit cost per minute over the aligned songs, with the revised weights (extra
1, pitch 3, octave 1, length 4, shift 4, moved 4, missing 2, chopped 4, joined 4). The stage runs on the onset
phase, which is what ships. The interval is a paired song bootstrap: 10 000 resamples, seed 1 (`lib01.paired`).
An arm **passes** when both of these hold:

- (a) the 95% upper bound of Δ cost/min (new − reference) is below 0;
- (b) the missing count does not rise, or, if it does, the upper bound of the same Δ with missing re-weighted to 4
  is still below 0 (19's rule 2).

Clean measures are reported with their interval but do not decide. **Question for the lead:** should 19's rule 3
(clean measures do not fall, point Δ ≥ 0) also decide?

- **Rule 1: C against today's shipped stage D.** `cellstage-19C` (switch cost 1.5, as in its `cellstage.json`)
  against `cellstage-D`, on the same songs. **If C passes, it becomes the app's default stage.** If not, D stays
  the default and C stays opt-in.
- **Rule 2: L on top of C, against C with no lyric** (17's rule). L is 14's line fit (`fit_lines`) with 17's S1
  start moves on top:
  - the fit uses the lyrics CTC's line and space times with energy ends (`lyrics-lead-energy`);
  - S1 is ticket 21's port (`fit_lines_with_onsets`, `OnsetRule::S1`, `consensus_onsets`): the three CTCs'
    median, with per-class offsets frozen from dev, the lattice prior from GAME lead's starts, and abstention on
    a spread above 3 cells. It runs inside the scorer.

  If rules 1 and 2 both pass, L ships on C as the default whenever a lyric is given. Its runtime (three CTCs,
  about 2 minutes per song on 4 cores) stays the user's call (lyrics-first map). If rule 1 fails, L's result is
  recorded but nothing ships from it, because it was measured on C and not on D.
- **Rule 3: T (`--settle-near-ties`, `NearTieRule::default()`) on top of C, against C.** It is judged only if
  rule 1 passes and rule 2 fails. If it passes, T ships on C. If L passes, T's row is reported, decides nothing,
  and T stays opt-in: 20 showed it earns nothing on top of L. M (`--merge-unheard-pieces`) and 12's trim are not
  read. They are deleted after the read (20).
- **Rule 4: bar 1 (05 and 16), unchanged.** The text is ticket 05's "Pre-registered decision rule for holdout4"
  with arm C as added by 16: A today, B `bar_one::estimate`, C Beat This on B's beat, with criteria 1–3 and the
  default choice as written there. Three details are fixed here:
  - **Reference:** the ear labels in `~/.cache/sona/holdout4/labels.json`. That file is local and never
    committed, and is read through `--music-frames`. The user confirms at least 5 songs, and songs not labelled
    4/4 are excluded before the read.
  - **Songs scored:** the songs that are both aligned (step 1) and labelled 4/4. At least 20 are needed;
    otherwise the frame rules decide nothing.
  - **"On the music's beat":** |residual| < ⅛ beat, from each arm's own bar offset.

**Reported, deciding nothing:**

- D → C+L, the bundle against today.
- C → C+L14 (14's fit alone) and C+L14 → C+L (S1 on top).
- Each judged Δ again without the song whose cost moved most (20's amaten check).
- Every song's cost/min under D, C, C+T and C+L.
- The dropped songs, with the reason for each.
- B's passes split by confidence (≥ 0.75 and below) and C's split by vote share; C − B with its interval; the
  songs where B and C disagree.

**What the numbers can't say:**

- L's lyric on dev was the UST's own moras, so it is an upper estimate for a real paste. On holdout4 that depends
  on the decision below.
- L's pass is not safe (lead, on 21). With 19C in sample on dev, L is −13.8 [−43.0, +10.7], and 14's fit under it
  costs +47.7 cost/min on my-axiom. The fold-model row (−20.0 [−41.1, −1.5]) is the forecast.
- Rule 1 on dev (both stages in sample) is −2.7 [−7.9, +6.8], with missing 213 → 231. C's extra missing notes
  (+147 in 19's CV) may fail criterion (b) on holdout4 even if the cost falls.
- Every arm's parameters were chosen on dev; this read is their first out-of-sample test.
- With about 20 songs, a ±2 cost/min change is inside the noise. 19's seed noise on 38 songs was −0.9
  [−6.2, +4.1].

### Holdout4 inputs: what is ready and what needs a decision (agent, 2026-11-24)

**Ready:**

- The beat overlays for every holdout4 song.
- The empty labels file.
- The lead RMS (for 14's energy line ends) is done. The three CTC emissions (lyrics, hira, jg) of every holdout4
  `lead.wav` are computing (`audio_inputs.py`, about 10 min per song under load). The job is detached, and it
  writes `~/.cache/sona/holdout4/l14/DONE` or `FAILED` when it ends; the log is `emissions.log` beside them.

  The emissions are the expensive, audio-only part of the forced alignment: `ctc_align.py` reuses them, and only
  the Viterbi through the moras depends on the text. Checked on a dev song, they reproduce 14's cached emissions:
  byte-identical for lyrics; for hira and jg one fp16 step apart at most, with the same argmax on every frame.
  The Viterbi on them gives the same mora starts and ends; only the aligner's own score differs, by < 0.007.

Making these inputs read each song's mix or lead stem and the manifest's offset and BPM. Nothing else was read:
no UST note, onset or lyric, no sona output, and no score.

**Blocked on a decision: L's text inputs.** Every lyric input the dev L used is keyed to the UST's own mora
sequence:

- `--lyric-lines` holds the indices of the UST notes that start a line. `align_lines.py` gets them by aligning a
  lyric site's text to the UST's moras.
- `text/<id>.json` is the UST's moras (with the UST's times), grouped by those lines.
- The CTCs force-align exactly those moras.
- The `--lyric-line-times` files are indexed by UST note, and S1's member files (`s1_onsets.py produce`) are one
  row per UST mora.
- `--lyric-placement` lands the UST's own lyric.

Holdout4 also has **no lyric-site texts** (`data/pifuyuu-lyrics` holds dev and the spent splits only). So
producing L's inputs needs the UST's lyrics, and by the brief that call belongs to the lead and the user. The
options:

- **A. As on dev, built before the read.** Fetch the 24 lyric texts (UST-free), then read only the UST's lyric
  field to make `lines` and `text`, Viterbi-align on the cached emissions, and write the times and S1 onsets.
  - The arm stays exactly the one 20 measured.
  - The inputs can be inspected before the read.
  - But the holdout's UST lyrics are opened before the read. They are not the scored notes, but they are UST
    content.
- **B. As on dev, built inside the read** (after step 1's ledger entry). The same pipeline, cheap at read time,
  since the emissions are already cached: about a minute per song for three Viterbis.
  - No UST content is opened before the read.
  - The lyric texts still have to be fetched first.
  - Alignment quality can't be eyeballed beforehand, and a crash after step 1 means `--resume`, which is logged.
- **C. An honest paste.** The text's own moras (fugashi readings), lines from the text, CTC on the text's moras.
  Nothing reads the UST.
  - But the arm is not the one 20 measured, so it needs a new landing format (pasted moras with times instead of
    UST indices) and a new dev measurement under a new rule before the read.
  - It gives the number a real user would get.
- **D. Leave L out of this read.** Read C, T and the frame now; L waits for C's dev measurement or a fresh split.

The agent's lean: **B**, then A. Both keep the arm 20 measured, and B opens no holdout UST content early. Either
way the lyric texts must be fetched first: 24 songs, the same sources as dev (`pifuyuu-lyrics/index.json`).

**S1 on holdout4, after 21.** `produce` needs only `text/<id>.json` (the kana) and the three aligners'
`align/<aligner>/<id>.json` under the split's L14 work dir. The lattice prior comes from GAME inside the scorer.
So S1 adds no scored run, and once A's inputs exist, `read.py` runs `check` and `produce` in step 0. Under B,
building the inputs and `produce` move to after step 1. That is a small change to `read.py`, not made yet.

### Ear labels: what the user does (agent, 2026-11-24)

All files are local and never committed. `labels.py` is in `crates/sona-eval/scripts/holdout4-read/`; run it with
`SONA_UST_AUDIO_DIR=…/pifuyuu-audio`.

1. **Beat.** Each song has four overlays per excerpt in `~/.cache/sona/holdout4/clicks/<song>/`:
   `beat0-q1.wav` to `beat3-q1.wav` (a quarter into the song) and `beat0-q3.wav` to `beat3-q3.wav` (three
   quarters in). Each is 24 s of the mix at 60% with a click on every beat; the high click marks a candidate bar
   line. Pick the shift whose clicks sit on the kick and snare, and write it as `beat_shift_cells` in
   `~/.cache/sona/holdout4/labels.json`.
2. **Bar.** `python3 -I labels.py bars --split holdout4 --labels ~/.cache/sona/holdout4/labels.json --clicks-dir
   ~/.cache/sona/holdout4/clicks` writes `bar<b>`, `bar<b+4>`, `bar<b+8>` and `bar<b+12>` (each as `-q1` and
   `-q3`) for every song with a beat. Pick the one whose high click lands where phrases start and chords change,
   and write it as `bar_shift_cells`.
3. **Meter.** `"meter": "4/4"`, or `"other"` for a song that isn't in 4/4 or can't be placed. Other songs are
   excluded before the read.
4. **Sure?** Set `"confirmed": true` when you heard both excerpts and are sure. The read needs at least 5.
5. **Check.** `python3 -I labels.py check --split holdout4 --labels ~/.cache/sona/holdout4/labels.json` says
   whether the labels are ready. A bar off the labelled beat is an error, and `sona-eval-frame` refuses it
   before it logs anything.

Don't run `--bar-one` or `--estimate-bar-one` on holdout4, and don't look at the UST while labelling (05).

## Answer

**Status stays in progress until the read.** The harness is built and rehearsed on dev. Holdout4 is unread: there
is no ledger entry, and no holdout4 UST note, onset or lyric was opened. Three things remain before the read: the
user's ear labels, the lead's approval of the draft rules above, and a decision on L's text inputs (above).

**Built:**

- **`sona-eval-frame --estimate-bar-one`**:
  - `--split NAME --reason … [--reread]` goes through the ledger.
  - `--music-frames FILE` reads the local ear labels (`sona_eval::frame::ear_labels`). A label whose bar is off
    its beat, or a 4/4 label without both shifts, is refused before anything is logged.
  - `--rows FILE` writes per-song JSON.
  - Mixes come from `$SONA_UST_AUDIO_DIR` (as the scripts already read it).
- **`sona-eval-ust-align`.** Checking a split whose songs are still `unverified` is the read's first ledger
  entry (`--split NAME --reason`). Without `--split`, the check now skips those songs. Before, a plain run would
  have checked holdout4 silently.
- **`crates/sona-eval/scripts/holdout4-read/`**:
  - `read.py` is the one read: steps 0–6, and it prints and writes every verdict.
  - `labels.py` is the ear-label kit: `kit`, `bars`, `check`, and `from-manifest` for rehearsal.
  - `audio_inputs.py` computes the CTC emissions and the lead RMS from audio only.
- **S1 is 21's path.** `s1_onsets.py check` and `produce` run in step 0, and `--lyric-onsets` goes into C's
  scorer run. C, T, 14's fit and L come from one scored run. To change how the onsets are made, change
  `read.py`'s `s1_onsets()`.

**Dev rehearsal** (`read.py --split dev`, end to end, 496 s):

- **Steps 0–1.** Step 0 passes; S1 `check` passes. One song is dropped: plus-space, flagged for a constant shift
  of +38 ms. The alignment check left dev's manifest byte-identical (`read.py` checks this).
- **Step 2.** Bar 1 on 9 songs: A 5, B 9, C 9. B − A and C − A are both +4 [+1, +7], equal to the Rust report.
  This reproduces 05 and 16, and on dev it is circular.
- **Steps 3–5.** Python D, C and C+T equal the Rust reports per song and class. D is 272.2, the shipped stage's
  known dev number.
- **Rules (dev: both stages in sample, so this decides nothing):**

  | rule | Δ cost/min [95%] | missing | rehearsal verdict |
  |---|---|---|---|
  | 1: C vs D | −2.7 [−7.9, +6.8] | 213 → 231 | fails (a) and (b) |
  | 2: L on C | −13.8 [−43.0, +10.7] | 231 → 228 | fails (a); equals 21's 19C row |
  | 3: T on C | −1.6 [−4.3, +1.1] | 231 → 233 | fails |
  | S1 on top of 14's fit (reported) | −11.2 [−20.4, −1.8] | | equals 21 |

- **Ledger:** no new entries. The rows without the song whose cost moved most are reported as well; for L that
  is amaten, −2.4 [−17.6, +16.2].
- **Holdout4 preflight**, which reads nothing, refuses today. 24 songs are unlabelled, 0 are confirmed, every
  lyric input is missing, and the tree has uncommitted changes. Nothing was logged.

**Checks:**

- Default runs are byte-identical to d7ccaf9 on dev, before and after merging 21: the frame table, the
  `--estimate-bar-one` table, and `ust-align --split dev`.
- `tests/holdout_read_tools.rs` (3 tests through the real binaries) and one `ear_labels` test.
- Mutants caught:
  - frame never authorizes;
  - ust-align never authorizes;
  - label check after the ledger entry (the first version had this bug, and the test caught it);
  - in `read.py`, a shifted bootstrap and B = A. Both stop the read at the Rust cross-check.
- clippy `-D warnings`, plain and with `--features sona-eval/game`; `cargo test --workspace --release` (414
  passed); the corpus guard.

**How many holdout4 songs pass the alignment check: unknown, by design.** The check compares UST onsets with
sona's onset detector, so it is the read's first step and its ledger entry. 08 found 3 of the 32 earlier songs
flagged, which suggests about 21–22 of the 24 remain. With 20 required, the margin is thin.

**Left:**

1. The user labels (procedure above; at least 5 confirmed).
2. The lead approves or edits the draft rules and commits them. That commit is `--rules-commit`.
3. L's text inputs: decide A, B, C or D. For A or B, fetch the 24 lyric texts.
4. The emissions job finishes (`DONE` marker).
5. Run `read.py --split holdout4 --reason "…" --rules-commit SHA …` once from a clean tree, then commit the ledger
   and the manifest's alignment statuses. Also update the manifest test that pins every holdout4 song as
   `unverified`.
