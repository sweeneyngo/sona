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

1. **Alignment (08).** `sona-eval-ust-align --split holdout4 --write` runs the shipped onset-delta check and
   flags a song for `|median delta| > 35 ms`, `|drift| > 30 ms` or fewer than 30 matched onsets. This step writes the read's
   ledger entry. Flagged songs are dropped before any score. **Fewer than 20 aligned songs stop the read**: nothing below
   is computed, and the split counts as spent.
2. **Bar 1:** `sona-eval-frame --estimate-bar-one --split holdout4 --music-frames LABELS --beat-this …` (rule 4).
3. **Stage D:** `sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost --split holdout4 --dump …`
4. **Stage C:** `sona-eval-cellstage --model ~/.cache/sona/cellstage-19C --edit-cost --settle-near-ties
   --lyric-placement --lyric-lines … --lyric-line-times … --split holdout4 --dump …`
5. **The lyric arm on C's notes** (L, below).
6. **Verdicts.** `read.py` prints each verdict and writes `verdicts.json`.

Steps 2 to 4 are logged as same-sitting re-reads, as the holdout2 and holdout3 reads were. Before any verdict, the
Python rows must equal the Rust reports per song and class (D, C, C+T, C+L14), and the Python copy of
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
  - S1 is variant `file-m3+adaptmora+abst3.0:split+start1`: the three CTCs' median, with per-class offsets frozen
    from dev (ticket 21) and abstention on a spread above 3 cells.

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
- Every arm's parameters were chosen on dev; this read is their first out-of-sample test.
- With about 20 songs, a ±2 cost/min change is inside the noise. 19's seed noise on 38 songs was −0.9
  [−6.2, +4.1].

### Holdout4 inputs: what is ready and what needs a decision (agent, 2026-11-24)

**Ready:**

- The beat overlays for every holdout4 song.
- The empty labels file.
- The lyric arm's audio-only inputs (below).

Making them read each song's mix or lead stem and the manifest's offset and BPM. Nothing else was read: no UST
note, onset or lyric, no sona output, and no score.

**Blocked on a decision: L's text inputs.** Every lyric input the dev L used is keyed to the UST's own mora
sequence:

- `--lyric-lines` holds the indices of the UST notes that start a line. `align_lines.py` gets them by aligning a
  lyric site's text to the UST's moras.
- `text/<id>.json` is the UST's moras (with the UST's times), grouped by those lines.
- The CTCs force-align exactly those moras.
- The `--lyric-line-times` files and S1's onset files are indexed by UST note.
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

**S1's onsets on holdout4** need three things:

1. 21's frozen offsets.
2. A non-scoring producer: today `sources.py` reads a `cellstage --dump` for the songs and for its lattice prior,
   and a dump holds edit costs.
3. sona's own notes for the lattice prior. These come either from inside the read (after step 4's C dump) or
   from a non-scoring transcription before it.

Under option B, the producer runs inside the read, after step 4.

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
