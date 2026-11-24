# Bar one: where sona's tick 0 should sit (audit A03, dev split)

Date: 2026-11-24. Dev split only: no `--split`, no holdout read, and the ledger is unchanged. There are 9 aligned dev
songs. plus-space is `flagged` and has no lead stem, so it is skipped.

## Question

When sona exports, where should bar 1 (tick 0) sit against the audio? Two references disagreed:

- **Pifuyuu's UST**, placed with `alignment_offset_s`. Tick 0 is read as a bar line.
- **The music**: its drums and its harmony.

Utau-edit-cost 02 found that sona's tick 0 sits one cell after the UST's bar line on 9/9 songs. It also found
that on 6/9 songs the drums' beat sits a cell off the UST's beat lines. Two readings could explain this:

- **(a)** Pifuyuu's grid is a 16th off the music.
- **(b)** `alignment_offset_s` is biased about one cell early. The idea is that the onsets it used are consonant
  starts, while UTAU puts the note at the vowel.

The truth that matters to the user is the **music's** beat and bar.

## Answer

- **Reading (a) holds, in a stronger form. Reading (b) is rejected.**
  - Re-aligning every UST to **vowel** onsets moves it by −15…+11 ms (median +8 ms; −0.15…+0.27 cell) on
    9/9 songs.
  - Shifting by ±1 cell scores only 0.06–0.65 of the peak.
  - So `alignment_offset_s` sits within a fraction of a cell of the vowels. It needs no correction, and past
    cell-accuracy numbers stand.
- **Pifuyuu's grid is not the music's grid.** On every song, the UST's bar 5 sits 28–100 ms before audio 0 s.
  That is the mark of a 4-bar lead-in with the audio file dropped at bar 5. The grid is therefore anchored to
  the **file**, not to the music. Against the music:
  - the UST's **beat** lines are off on 7/9 songs: one 16th on 6, and a triplet 8th on atamaro (24-grid);
  - the UST's **bar** lines match the music's bar on 1/9 (azemichi). On the others they are off by 1 cell
    to 3¼ beats.
  - On the 7 songs whose beat is off, 71–100% of Pifuyuu's note starts fall on the music's 8ths, against
    0–31% on the UST's (atamaro: 93% on the music's beats and swung 8ths). That is how they can be "on odd
    16ths" in the UST and still be in time.
- **Neither audio-anchored rule finds the music.**
  - Today's rule, "first grid line at or after 0 s", lands on the music's beat on 5/9 songs and on its bar
    line on 2/9.
  - "Nearest grid line to 0 s" lands on the beat on 5/9 and on the bar line on 3/9.
  - Both hit only by accident: the music's beat nearest 0 s falls anywhere from −117 to +64 ms.
- **What does find the music.** sona's own `grid_phase` (ADR-0027), run on the **full mix** at **one-beat**
  spacing, finds the drums' beat on 9/9 songs (within 0.05 cell).
  - A harmonic-change downbeat on mix − vocals gives a bar line on every song. It is stable across song
    halves and in an 8-bar-block bootstrap.
  - That bar line has no independent ground truth: on 5/9 songs at least 3 of 4 independent drum and bass
    cues agree with it.
- **Recommended default** (opt-in until holdout4):
  1. Phase from the vocals' onsets (ADR-0027, unchanged).
  2. Beat from `grid_phase(mix, beat)`, snapped to the vocals' cell grid.
  3. Bar from the harmonic-change downbeat.
  4. Tick 0 is the music's bar line at or before 0 s.

  The frame metric must be re-anchored to the music, because the UST's grid is not a valid reference for
  bar 1.

## Method

Everything is at the UST's exact BPM. "Cells after X" is taken modulo a beat (or a bar).

| quantity | how |
|---|---|
| UST beat and bar lines | `alignment_offset_s + k·beat`, with bar lines every 4 beats from UST tick 0 |
| drums' beat (reference) | Drums stem, band-limited log-RMS rise (10 ms window, 1 ms hop, zero-phase filters). Kick (30–120 Hz) plus snare (150–2500 Hz) are folded at one-beat period. Pick the cell of the period with the most energy. Hat (5–8 kHz) and broadband are also reported. |
| drums' beat (sona's estimator) | `sona_core::tempo::grid_phase` at one-beat spacing on the drums stem and on the **mix** (`sona-eval-frame --bar-one`) |
| music bar (downbeat) | Chroma of the instrumental (mix − vocals − drums; mix − vocals gives the same votes on 9/9). Harmonic change across each music beat, with 1-, 2- and 4-beat windows on each side. Fold mod 4 beats and take the argmax of the summed normalised profiles. Cross-cues, each independent: bass-band onsets in the instrumental; the top 1% of drum hits; kick on beats 1 and 3; snare on beats 2 and 4. |
| vowel re-alignment, reading (b) | Lead stem, the same log-RMS rise in a **vowel band** (200–1500 Hz) and a **consonant band** (3–8 kHz). Sum the rise at every UST note start + lag, for lags in ±2 cells (1 ms steps); report the best lag and the score at −1, 0 and +1 cell over the peak. Also per note: the time of the largest vowel-band rise within ±1.5 cells of the UST start. |
| sona's tick 0 | `grid_phase(vocals, cell)`: today's export rule is the first line at or after 0 s; the alternative is the line nearest 0 s (`sona_eval::frame::{sona_tick0_s, nearest_tick0_s}`) |

The prototype is Python (librosa 0.11 and scipy in the separator venv) run on dumped 1 ms envelopes; it is
scratch and not committed. The Rust part is opt-in: `cargo run --release -p sona-eval --bin sona-eval-frame --
--bar-one`.

## Per-song table

Shifts are in cells: beat columns are cells after the UST's beat lines, bar columns are cells after the
UST's bar lines. On atamaro a cell is a 24th, so a beat is 6 cells.

| song | cell ms | UST bar line nearest 0 s | music beat: drums / `grid_phase` drums / mix | music bar (harmonic) | cues agreeing (of 4) | vowel re-align lag ms (score at −1/+1 cell) | consonant lag ms | sona tick 0 today | nearest rule | UST tick 0 on music beat / bar |
|---|---|---|---|---|---|---|---|---|---|---|
| innocent-addict | 75.8 | −64.5 ms | +1 / 1.00 / 1.05 | +13 | 4 | +10 (0.32 / 0.25) | −3 | +1: **beat** | +1: **beat** | no / no |
| kamaitachi-ni-yoru | 104.2 | −73.3 | +1 / 0.99 / 1.00 | +9 | 4 | −15 (0.19 / 0.13) | −24 | +1: **beat** | +1: **beat** | no / no |
| telescope-nevermore | 122.0 | −100.1 | +1 / 1.00 / 1.02 | +1 | 3 | +9 (0.16 / 0.20) | −1 | +1: **beat, bar** | +1: **beat, bar** | no / no |
| atamaro-deluxe (24) | 41.0 | −37.6 | +2 / 1.99 / 2.04 | +2 | 2 | +11 (0.36 / 0.65) | −3 | +1: off | +1: off | no / no |
| azemichi-ni-ochita-ginga | 76.5 | −36.0 | 0 / 3.97 / 3.99 | 0 | 3 | +2 (0.12 / 0.06) | −4 | +1: off | 0: **beat, bar** | **yes / yes** |
| yamineko | 60.0 | −57.0 | +3 / 3.00 / 3.02 | +3 | 0 | −3 (0.18 / 0.18) | −2 | +1: off | +1: off | no / no |
| my-axiom | 76.9 | −73.9 | 0 / 3.98 / 0.03 | +4 | 3 | +1 (0.11 / 0.16) | +18 | +1: off | +1: off | **yes** / no |
| amaten | 92.0 | −28.4 | +1 / 0.98 / 0.99 | +9 | 2 | +8 (0.62 / 0.58) | −1 | +1: **beat** | 0: off | no / no |
| fukagyakuteki-henyou | 63.6 | −58.2 | +1 / 0.98 / 1.00 | +1 | 1 | +11 (0.37 / 0.35) | −10 | +1: **beat, bar** | +1: **beat, bar** | no / no |
| **pooled (9)** | | | mix `grid_phase` = drums: 9/9 | | ≥3 agree: 5/9 | all within ±0.27 cell | | **beat 5/9, bar 2/9** | **beat 5/9, bar 3/9** | beat 2/9, bar 1/9 |

`sona-eval-frame --bar-one` gives sona's tick 0 exactly:

| song | today (ms) | nearest (ms) |
|---|---|---|
| innocent-addict | +12.6 | +12.6 |
| kamaitachi-ni-yoru | +34.7 | +34.7 |
| telescope-nevermore | +24.4 | +24.4 |
| atamaro-deluxe | +4.4 | +4.4 |
| azemichi-ni-ochita-ginga | +40.2 | −36.4 |
| yamineko | +5.2 | +5.2 |
| my-axiom | +4.2 | +4.2 |
| amaten | +67.9 | −24.2 |
| fukagyakuteki-henyou | +4.5 | +4.5 |

Today's rule puts tick 0 1.00–1.05 cells after the UST bar line on 9/9 songs. This reproduces utau-02's
+0.25-beat residual.

### Where the notes fall: UST frame vs music frame

This is the share of UST lead note starts on the 8th-note grid (on atamaro, the straight 8ths of the 24-grid)
and on the beat.

| song | on 8ths, UST frame → music frame | on the beat, UST → music |
|---|---|---|
| innocent-addict | 0.04 → **0.96** | 0.02 → 0.54 |
| kamaitachi-ni-yoru | 0.03 → **0.97** | 0.00 → 0.35 |
| telescope-nevermore | 0.15 → **0.85** | 0.10 → 0.34 |
| atamaro-deluxe | 0.31 → **0.62** (beat + swung 8th: 0.31 → 0.93) | 0.31 → 0.62 |
| azemichi-ni-ochita-ginga | 0.99 → 0.99 | 0.64 → 0.64 |
| yamineko | 0.00 → **1.00** | 0.00 → 0.75 |
| my-axiom | 0.90 → 0.90 | 0.49 → 0.49 |
| amaten | 0.29 → **0.71** | 0.14 → 0.37 |
| fukagyakuteki-henyou | 0.13 → **0.87** | 0.10 → 0.63 |

In the music's frame the melody sits on 8ths on every song. On atamaro it sits on beats and swung 8ths: the
24-grid shuffle pattern is 0.62 on the beat and 0.31 on the last triplet. In the UST's frame, 6 songs have
almost no note on a beat. yamineko has 0 of 597.

## Reading (b): is `alignment_offset_s` biased by a cell?

**No.**

- **Vowel re-alignment.** The best lag for the vowel band is −15…+11 ms on 9/9 songs. In cells that is
  −0.15…+0.27, with a median of +8 ms.
  - A one-cell bias would put the peak at +41…+122 ms. Instead the score at +1 cell is 0.06–0.65 of the peak,
    and at −1 cell it is 0.11–0.62.
  - The highest are atamaro (+1 cell: 0.65, on 41 ms cells) and amaten (0.62 and 0.58; its median note is
    1 cell long, so the neighbours are onsets too).
  - The longer 30 ms rise peaks at +13…+22 ms on 8/9 (kamaitachi: −7). On atamaro it peaks at +20 ms, which is
    half of its 41 ms cell, with +1 cell nearly tied (0.94). A window of 30 ms cannot separate 41 ms cells,
    but the 10 ms rise can, and it peaks at +11 ms.
- **The physics checks out.** The consonant band's lag is −24…+18 ms (median −3). It leads the vowel band on
  7/9 songs, by 6–21 ms. This is the expected order, but it is far smaller than a cell. The exceptions are
  yamineko (equal) and my-axiom (consonant 17 ms later).
  - Per note, the strongest vowel rise sits a median +11…+19 ms after the UST start (kamaitachi: −9).
  - So the offsets are at most 10–20 ms early against vowel energy.
- **Why a large bias couldn't happen anyway.** The fine step of `sona-eval-ust-align` snaps the UST's cell
  lines to the drums' onsets, within ±½ cell. Its residual is −0.5…+1.2 ms here, which reproduces the step.
  - So any bias in the offset is either about 0 (sub-cell, removed by the drums) or a whole number of cells.
  - The vowel test excludes a whole number.
- **Effect on past cell-accuracy numbers: none.**
  - A sub-cell bias of 10–20 ms does not change which cell a note covers, because scoring is on the
    UST-anchored grid.
  - A one-cell bias would have shown up. Scoring the UST's own cell labels against themselves shifted by one
    cell gives 74% agreement on sung cells (pooled; 61–84% per song), and two cells gives 52%. That is the
    ceiling a perfect transcriber could reach against a one-cell-biased reference.
  - The vowel test is the direct evidence. The ceiling argument is weaker for the learned cell stage, because
    it was trained on these alignments and could have absorbed a bias.
- **Proposal:** leave `alignment_offset_s` as it is.

## Reading (a): the UST's grid and the music

- **The beat.**
  - On 7 songs the drums' beat is 1 or 3 cells after the UST's beat lines (atamaro: 2 of 6).
  - Three estimators agree on all 9 songs: the kick + snare fold, `grid_phase` on the drums, and `grid_phase`
    on the mix.
  - **A beat or an off-beat 8th?** The 8th-note parity is not in doubt. In the drums' broadband within-beat
    profile, the best cell of the other parity reaches only 0.07–0.52 of the peak (telescope 0.52, amaten
    0.36, the rest ≤ 0.26), and the melody agrees.
  - Whether the chosen cell is the beat or the "and" was checked in two ways:
    - the snare and kick bands pick the same cell on 7/9;
    - harmonic change, at cell resolution, peaks within ±1 cell of the chosen beat on 8/9 and never at the
      other 8th. On amaten it peaks between the two.
- **The bar.** The harmonic downbeat puts the music's bar line at the UST bar line plus the shift in the table.
  - azemichi: 0 cells. telescope and fukagyakuteki: 1 cell. atamaro: 2 cells of 24. yamineko: 3 cells.
  - my-axiom: 4 cells (one beat). kamaitachi and amaten: 9 cells (2 beats + 1 cell). innocent-addict:
    13 cells (3 beats + 1 cell).
  - The vote is the same in each half of the song on 9/9, and in 200 resamples of 8-bar blocks on 9/9.
  - The other cues agree on 4 songs out of 4 cues (innocent, kamaitachi), 3/4 (telescope, azemichi, my-axiom),
    2/4 (atamaro, amaten), 1/4 (fukagyakuteki) and 0/4 (yamineko).
- **Why the cues disagree on some songs.** Snare-on-2-and-4 fails on the double-time songs, whose bars at the
  UST's 236–250 BPM are half a felt bar. This covers atamaro, yamineko and fukagyakuteki, plus telescope,
  which sona's tempo estimate reads at 2×. So those three bar calls are the weakest.
- **Mechanism.** The UST's bar 5 sits 28–100 ms before the audio's start on all 10 dev songs, plus-space
  included (issue A03's metadata table). This fits Pifuyuu dropping the audio at bar 5 and then transcribing on
  a grid tied to the file. The music's bars and beats fall wherever the song happens to sit in the file, and
  Pifuyuu's notes are in time with the singer whatever the bar lines say.

## Rules compared (against the music, dev 9)

| rule for tick 0 | on the music's beat | on the music's bar line | notes |
|---|---|---|---|
| UST tick 0 (Pifuyuu) | 2/9 | 1/9 | reference only; not a target |
| today: first grid line at or after 0 s | 5/9 | 2/9 | depends on where the file starts |
| nearest grid line to 0 s (A03 proposal) | 5/9 | 3/9 | A03 predicted 10/10 *against the UST*. It actually reaches the UST bar on 2/9 (azemichi, amaten), because the bar line sits 0.3–0.96 cell before 0 s and "nearest" takes it only within ½ cell |
| beat from `grid_phase(mix, beat)`, on the vocals' cell grid | **9/9** | 1 in 4 by chance | against the drums' kick + snare fold, a related but differently weighted estimate |
| + harmonic downbeat, bar line at or before 0 s | **9/9** | **9/9 by construction** | the bar reference *is* this estimator; on 5/9 songs at least 3 of 4 independent cues agree |

The last two rows are not evidence that the rules work. They show that the rules agree with the best
reference available without ears. Holdout4 needs a music reference labelled by ear.

## Alternative explanations ruled out

- **Offset biased by a cell (reading b):** rejected by the vowel re-alignment (±0.27 cell, low scores at ±1)
  and by the fine step's construction. See above.
- **Sub-cell bias hiding in the offset:** up to about +8…+19 ms against vowel energy. This is too small to move
  a cell label, and the cell scorer works on the UST-anchored grid.
- **The drums' "beat" is really the off-beat 8th (hats):** the kick and snare bands are folded separately, the
  harmonic change sits on the chosen beat, and the melody's parity agrees. One soft case remains (amaten:
  harmony between the beat and the "and"; 71% parity). See the section above.
- **Demucs latency or stem misalignment:** the stems match the mix sample for sample in length, and
  `grid_phase` on the **mix** agrees with the drums stem within 0.05 cell on 9/9.
- **A method bias in the envelopes:** the drums and the vocals use the same envelope definition, so lags
  between them cancel any filter or window delay. The consonant band leading the vowel band (7/9, by
  6–21 ms) is the physically expected order.
- **Tempo drift smearing the folds:** the BPM is exact (UST) and constant (asserted per song), and the folded
  profiles are sharp (the other parity's best cell is ≤ 0.52 of the peak).
- **The harmonic downbeat is an artefact of window length:** the 1-, 2- and 4-beat windows vote the same on
  8/9 (fukagyakuteki's 4-beat window ties beats 0 and 3). The vote is the same in each half of every song, and
  mix − vocals gives the same vote as mix − vocals − drums on 9/9.
- **"Today's 5/9" is a property of sona:** it is not. sona's tick 0 is the UST bar line + 1.00–1.05 cells on
  every song. That cell lands on the music's beat only when the file happens to start one 16th before it.

## Not measured, or not settled

- **No ground truth for the music's bar.** The harmonic estimator is the reference here, so its 9/9 is
  circular.
  - Click-overlay excerpts (UST grid vs music grid; 24 s from mid-song) were written for kamaitachi, my-axiom
    and yamineko in the study's scratch directory, for an ear check. Regenerate them with the scratch
    `instr_downbeat.py`.
  - yamineko (0/4 cues agree), fukagyakuteki (1/4), atamaro and amaten (2/4) are the calls to check by ear.
- **Felt tempo vs UST tempo:** on the 236–250 BPM songs a "bar" of 4 UST beats may be half a felt bar. The
  user types the BPM, so the bar is defined at that BPM. If they type the half tempo, the downbeat question
  changes.
- **Real users' files:** these are official uploads that start just after a bar line, give or take a cell.
  Files with intros, trims or encoder delay shift the music arbitrarily against 0 s. That is one more reason
  not to anchor bar 1 to the file's start.
- **Downbeat from what sona has at runtime:** the prototype used the residual (mix − vocals − drums) and
  mix − vocals. The app has the mix and the vocals stem, so mix − vocals is available. This was not ported to
  Rust.

## Recommendation

1. **Default bar-1 rule (opt-in; confirm on holdout4):**
   1. Cell phase from the vocals' onsets (ADR-0027, unchanged).
   2. Beat = the vocals' cell line nearest `grid_phase(mix, 60/BPM)`.
   3. Bar = the beat (mod 4) with the most harmonic change in the chroma of mix − vocals, using 1-, 2- and
      4-beat windows.
   4. Tick 0 = that bar line **at or before** 0 s. It may sit up to a bar before the audio, so the export must
      start the audio that far after tick 0. This is A02's export-frame work, not changed here.
   5. Fallback, if no harmonic estimate is possible: the music's beat at or before 0 s.
2. **Re-anchor the frame metric** (utau-edit-cost 02, 05, 08).
   - The UST's tick 0 is not a valid "bar 1" reference on 8/9 dev songs.
   - Propose two manifest fields, data and not code: `music_beat_shift_cells` (relative to UST tick 0) and
     `music_bar_shift_cells`.
     - Seed them from this study's dev values: beat +1, +1, +1, +2, 0, +3, 0, +1, +1; bar +13, +9, +1, +2,
       0, +3, +4, +9, +1, in table order.
     - Confirm each by ear before holdout4.
   - The `alignment_offset_s` values are **not** changed. Note accuracy against the UST stays valid, because
     the notes are in time with the vocals. Only the UST's bar and beat lines are wrong.
3. **Holdout4 decision rule (to commit in 08 before the read):**
   - Score tick 0 against **ear-labelled** music beat and bar lines.
   - Ship the rule if it beats today's rule on beat-correct songs, beyond the song-bootstrap noise.
   - Report bar-correct separately.

## Ear-labelling the music frame (utau-edit-cost 10)

The manifest holds the music's beat and bar per song: `music_beat_shift_cells` and `music_bar_shift_cells`
are whole cells after the UST's beat and bar lines, so the music's bar line is at
`alignment_offset_s + music_bar_shift_cells × cell`, mod one bar. `music_frame_status` says where the values
come from: `unverified-estimate` for the dev values seeded from this report's table (drums' beat, harmonic
downbeat), or `ear-checked`. `sona-eval-frame` scores bar offset and frame pass against these when present and
keeps the UST-anchored columns beside them.

To label a song by ear (holdout4, before any read; ticket 08):

1. Align the song first (`sona-eval-ust-align`), so `alignment_offset_s` and `bpm_from_ust` are in the
   manifest. Labelling needs only the mix and the UST's tempo and offset, and reads no sona output.
2. Find the beat. Run `--bar-one` only on dev. For a holdout song, use the drums: write overlays with
   `--bar-shift 0, 1, …, cells per beat − 1` and keep the one whose clicks sit on the kick and snare.
   That value is `music_beat_shift_cells` (cells per beat: 4 on a 16-grid, 6 on a 24-grid).
3. Find bar 1. With the beat fixed at `b`, try `--bar-shift b`, `b + c`, `b + 2c` and `b + 3c` (where
   `c` is cells per beat). The high click marks the bar line. Keep the one where the high click lands on
   the start of phrases and on chord changes. That value is `music_bar_shift_cells`.
4. Listen to at least two excerpts (for example `--from` at a quarter and at three quarters of the song).
   A song in 3/4 or 6/8 cannot be labelled in 4/4: flag it and leave the fields out.
5. Write both fields and `"music_frame_status": "ear-checked"`, and have the user confirm a sample of the
   songs before the read. The manifest test checks that the bar line falls on the beat
   (`bar % cells per beat == beat`).

```sh
cargo run --release -p sona-eval --bin sona-eval-frame -- --clicks SONG_ID --bar-shift CELLS [--from S] [--seconds S] [--out PATH]
```

- `--bar-shift` defaults to the manifest's `music_bar_shift_cells`, and to 0 (the UST's own lines) when
  there is none.
- The excerpt is 24 s from mid-song by default. `--seconds 0` writes the whole song.
- The output goes to the git-ignored `crates/sona-eval/data/click-overlays/SONG_ID-barCELLS.wav` by
  default. Never commit it.
- The WAV is the mix at 60% with a 30 ms click on every beat: 2 kHz on bar lines, 1 kHz on the other beats.
