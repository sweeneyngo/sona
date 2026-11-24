# Beat and downbeat trackers for the BPM hint and bar 1 (utau-edit-cost 15)

Date: 2026-11-24. Dev split only: the 10 dev songs with a UST tempo for BPM, the 9 aligned ones with a music
frame for beat and bar 1. No holdout audio was opened (the runner refuses any non-dev id), and the ledger is
unchanged. Audio stayed on the machine; only public packages and weights were downloaded. The selection rule
was committed before any tracker ran (f17dad8, in the ticket).

## Answer

- **Bar 1: two downbeat trackers put bar 1 on the music's bar line on 9/9 dev songs** (today's rule: 5/9; +4
  [+1, +7] for each), 5/5 on the songs whose label is corroborated by drum and bass cues, and their beats on the
  music's beat cell on 9/9:
  - **Beat This!** (CPJKU, ISMIR 2024): beat residual median 9 ms. Code **and weights are MIT**, so it is the
    only measured candidate sona could link or ship.
  - **madmom** (RNN + DBN): beat residual median 1.8 ms. Its models are CC BY-NC-SA 4.0, and it is the slowest
    (206 s CPU per song, mostly one thread).
  - Essentia and librosa have no downbeat tracker. Essentia's beats hit 8/9 (telescope half a beat off, with a
    "low" confidence); librosa's hit 3/9.
- **Under the pre-registered rule, neither wins outright.** Both pass every accuracy criterion and **fail the
  speed gate as measured** (≤ 120 s wall per 4-minute song, default settings, this machine): Beat This median
  160 s (max 212 s) at load 40–62, madmom 510 s. The tie-break (a licence sona can ship) picks Beat This.
  - Beat This's failure is mostly the machine, not the model. The same two songs (kamaitachi, amaten), per
    4-minute song: default threads at load 23–32, 96–100 s; pinned to 4 threads, 43–72 s; pinned to 1 thread,
    71–97 s (57–60 s CPU per song). The beats and downbeats are identical at every thread count.
  - Thread count is a deployment setting, but the rule said "default configuration", so this is diagnosis.
    **Ticket 16 is filed as needs-triage**, not ready: adopting Beat This is the user's call (below), and its
    first step is a speed measurement on an unloaded machine.
- **Neither tracker beats sona's shipped opt-in harmonic estimator on dev: all three agree on 9/9.** The music
  labels *are* that estimator's output (ADR-0031), so this is the first independent check of them. Both
  trackers agree on the 4 songs whose label had weak or no corroboration (atamaro-deluxe, yamineko, amaten,
  fukagyakuteki-henyou). If a tracker picked the beat of the bar at random, agreeing on those 4 would happen
  with probability (1/4)^4 ≈ 0.4%. The dev labels stay `unverified-estimate`, since no ear has checked them.
  But "weak on 4/9" now reads "unconfirmed by ear, corroborated by two other methods on 9/9". What dev can't
  say is which source is better: they never disagree here.
- **BPM hint: no candidate wins; keep sona's estimator.**
  - Every tracker reports the *felt* tempo, which is **half the UST's notated tempo on 4–7 of the 10 dev songs**
    (195–250 BPM songs read at 98–125).
  - The hint shows "est" and "½× est", so a ½× estimate offers no right button. sona's estimator (140–280 BPM
    window, ADR-0022) offers the truth on 10/10. madmom offers it on 6/10, Beat This on 4/10 (3 more misses
    are its 20 ms frame quantisation), and librosa, Essentia, TempoCNN and Percival on 3–5/10.
  - A song notated at 250 and one notated at 125 sound the same to a tracker. The octave is a notation
    convention, which an acoustic tracker can't settle; sona's window encodes Pifuyuu's convention, tuned on
    these songs.
- **A lead, not pre-registered: trackers are more *precise* than sona's estimator.**
  - Keep sona's octave and take Beat This's tempo (60 / mean of its beat intervals near the median). Then the
    hint's whole-BPM buttons contain the exact UST BPM on **10/10** dev songs, against 6/10 for sona's estimate
    alone (madmom 10/10, TempoCNN, degara and librosa 8/10).
  - A user who clicks the hint instead of typing would then get no drift. Drift is what failed the frame with
    sona's own BPM (utau-edit-cost 02: 1/9).
  - The corpus has only integer tempos, which flatters rounding. This needs its own pre-registered test
    (holdout4-tempo), and ticket 16 lists it as an optional second use.

Status: resolved (2026-11-24).

## Candidates

| candidate | code licence | weights licence | how sona could use it | BPM vs UST, 10 songs (1× / 2× / ½× / other; truth offered) | beat cell hits /9 (median abs residual) | bar-1 hits /9 (corroborated /5) | CPU s per song, median | wall s per 4-min song, median (max) |
|---|---|---|---|---|---|---|---|---|
| **sona today** (`estimate_bpm`; tick 0 = first cell line at or after 0 s) | MIT | – | shipped | 8 / 2 / 0 / 0; **10** | – (`grid_phase(mix)`: 9/9, A03) | 5 [2, 8] (2) | 1.7 (tempo) | – |
| **sona harmonic bar** (ADR-0031, opt-in) | MIT | – | shipped, opt-in | – | 9/9 (within 0.05 cell) | 9 (5); circular | ≈ 8 (bar 1, both stems) | – |
| **Beat This!** `final0`, full mix | MIT | **MIT** | linked (Rust port, or ONNX through the `game` feature's `ort`), or a user-installed venv | 4 / 0 / 3 / 3; 4 (fitted tempo: 10/10 at its own level, see text) | **9** (9.3 ms) | **9** [9, 9], +4 [+1, +7] vs today (**5**) | 485 | 160 (212) |
| Beat This! `small0` (variant, not eligible) | MIT | MIT | as above | 3 / 0 / 3 / 4; 3 | 8 (8.8 ms; atamaro one 24th off) | 9 (5) | 344 | 123 (147) |
| madmom 0.17.dev `RNNDownBeatProcessor` + DBN (3 and 4 beats per bar), `RNNBeatProcessor` + DBN | BSD-2-Clause | **CC BY-NC-SA 4.0** | user-installed venv (non-commercial), or eval reference | 6 / 0 / 4 / 0; 6 | **9** (1.8 ms) | **9** [9, 9], +4 [+1, +7] vs today (**5**) | 206 | 510 (613) |
| Essentia 2.1b6 `RhythmExtractor2013` multifeature | **AGPL-3.0-only** (commercial licence on request) | – (no model) | user-installed venv as a separate process, or eval reference; never linked | 4 / 0 / 6 / 0; 4 | 8 (20.1 ms) | no downbeats | 10 | 29 (46) |
| Essentia `RhythmExtractor2013` degara | AGPL-3.0-only | – | as above | 4 / 0 / 5 / 1; 4 | 8 (1.8 ms) | no downbeats | 2.7 | 7 (10) |
| Essentia `PercivalBpmEstimator` | AGPL-3.0-only | – | as above | 3 / 0 / 7 / 0; 3 | tempo only | – | 4.0 | 7 (12) |
| Essentia `TempoCNN` deeptemp-k16-3 (essentia-tensorflow) | AGPL-3.0-only | MTG: CC BY-NC-SA 4.0 (models page); the TempoCNN folder's `LICENSE` is AGPL-3.0 | as Essentia; NC terms if CC applies | 4 / 0 / 6 / 0; 4 | tempo only | – | 7.3 | 21 (29) |
| librosa 0.11 `beat_track` | ISC | – | linkable in principle (port the DP tracker) | 5 / 0 / 5 / 0; 5 | 3 (36.6 ms) | no downbeats | 3.4 | 6 (33) |

"Truth offered": the start screen's hint ("est", "½× est", ADR-0028) contains the UST BPM within 2%, i.e. a 1×
or 2× estimate. CPU is `time.process_time()` of the tracker's process, model loading included (Beat This
includes its first-run checkpoint download on innocent-addict). Bootstrap: 10 000 song resamples, the same
xorshift as `sona_eval::frame::song_bootstrap` (ported; reproduces today's [2, 8] and ADR-0031's +4 [+1, +7]).

Not measured:
- **BeatNet** (Heydari et al. 2021; repository licence CC BY 4.0; offline inference uses madmom's DBN): an older
  joint beat/downbeat/tempo model, built for online tracking; with Beat This! and madmom measured, it was left out.
- **All-In-One** (`allin1`, Kim and Nam 2023; code MIT, no licence stated for the weights): runs demucs itself
  and needs NATTEN; a second separation per song is out of proportion for bar 1.
- **aubio** (GPL-3.0): beat tracking without downbeats, like librosa's.

## Method

- **Runner** (`crates/sona-eval/scripts/beat-trackers/run.py`): each candidate in its own venv under
  `~/.cache/sona/beat-trackers/` (Python 3.10.13), on the dev mix (`data/pifuyuu-audio/<id>.wav`), default
  settings, CPU. It writes beats, downbeats, tempo and timings as JSON to the scratchpad (times only, no audio).
  It refuses any song whose manifest split isn't `dev`.
  - Beat This!: `Audio2Beats(checkpoint_path="final0", device="cpu", dbn=False)` on the mono mix (the
    package's default model and post-processing). Tempo = 60 / median beat interval.
  - madmom: from the repository's `main` (the PyPI 0.16.1 doesn't build on Python ≥ 3.10 with numpy 2) with
    `madmom_models` copied in. `DBNBeatTrackingProcessor(fps=100)` on `RNNBeatProcessor`;
    `DBNDownBeatTrackingProcessor(beats_per_bar=[3, 4], fps=100)` on `RNNDownBeatProcessor` (the
    `DBNDownBeatTracker` program's defaults). Tempo = 60 / median beat interval.
  - Essentia: `RhythmExtractor2013(method=…)` on 44.1 kHz mono; `PercivalBpmEstimator()`;
    `TempoCNN(graphFilename=deeptemp-k16-3.pb)` on 11 025 Hz (the tutorial's model).
  - librosa: `beat.beat_track` at 22 050 Hz.
- **Scorer** (`score.py`), at the UST's exact BPM (a typed BPM), against the manifest's music frame:
  - **beat:** every tracker beat votes for its nearest cell after the UST's beat lines, mod one beat; a hit is
    the winning cell equal to `music_beat_shift_cells`;
  - **bar 1:** every downbeat votes for its whole-beat offset from the music's bar line (mod 4 beats); a hit is
    offset 0. This is how sona would use a tracker: the user's BPM, the beat from `grid_phase`, the bar from
    the tracker's vote. The vote share is a natural confidence.
- sona's own numbers come from `sona-eval-tempo` and `sona-eval-frame --estimate-bar-one` (built in this
  worktree), which reproduce ADR-0022's dev line and ADR-0031's table exactly.
- `hint_precision.py`: the whole-BPM diagnosis above.

## Per song (Beat This! `final0`)

| song | UST BPM | tempo (median / mean interval) | beat cell (ref) | vote share | bar vote | vote share | today | harmonic |
|---|---|---|---|---|---|---|---|---|
| innocent-addict | 198 | 200.0 / 198.0 | 1 (1) | 0.97 | 0 | 0.98 | 1 | 0 |
| kamaitachi-ni-yoru | 144 | 142.9 / 144.0 | 1 (1) | 1.00 | 0 | 1.00 | 2 | 0 |
| telescope-nevermore | 123 | 125.0 / 123.0 | 1 (1) | 1.00 | 0 | 0.97 | 0 | 0 |
| plus-space (no music frame) | 108 | 107.1 / 108.0 | – | – | – | – | – | – |
| atamaro-deluxe (24-grid) | 244 | 120.0 / 122.0 | 2 (2) | 0.73 | 0 | 1.00 | 0 | 0 |
| azemichi-ni-ochita-ginga | 196 | 100.0 / 98.1 | 0 (0) | 0.99 | 0 | **0.63** | 0 | 0 |
| yamineko | 250 | 125.0 / 125.0 | 3 (3) | 0.97 | 0 | 1.00 | 0 | 0 |
| my-axiom | 195 | 187.5 / 194.8 | 0 (0) | 1.00 | 0 | 0.96 | 3 | 0 |
| amaten | 163 | 166.7 / 163.1 | 1 (1) | 0.98 | 0 | 0.99 | 2 | 0 |
| fukagyakuteki-henyou | 236 | 120.0 / 118.0 | 1 (1) | 1.00 | 0 | 0.99 | 0 | 0 |

"today" and "harmonic": whole beats off the music's bar line (0 = hit). Beat This's tempo is quantised by its
50 fps frames: the median interval is a multiple of 20 ms, so 3 of its "other" BPM classes (azemichi 100,
my-axiom 187.5, amaten 166.7) are quantisation, not a wrong metrical level; the mean of the intervals near the
median is within 0.2 BPM of the UST's (or half of it) on 10/10.

madmom's downbeat votes equal Beat This's on all 9 songs (bar vote 0; shares 0.98–1.00, azemichi 0.67), and
its beats sit on the same cells (shares 0.95–1.00). Both trackers find azemichi's bar the hardest to vote on
(0.63 and 0.67); the bar they vote for is still the labelled one, and today's rule already gets it.

## What else the numbers could be measuring

- **The labels are the harmonic estimator's output (circularity).** True for "harmonic 9/9", not for the
  trackers: neither Beat This nor madmom sees the harmonic estimate. Agreement between independent methods
  is evidence for the labels; it is not ground truth, and a shared bias (both following the harmony's change
  points rather than the drums' accent) can't be excluded without ears. The ear check stays the holdout4
  prerequisite (ticket 08).
- **Training-set leakage.** Beat This!'s `final*` models were trained on public beat datasets (not GTZAN);
  Vocaloid songs by these producers are not in its listed datasets. madmom's models were trained on older
  Western datasets. Neither saw Pifuyuu's USTs or these labels.
- **The fold hides a drifting tracker.** A vote at the typed BPM ignores beats that drift; the vote shares
  (Beat This ≥ 0.96 on bar for 8/9, 0.63 on azemichi) say the trackers didn't drift, and the BPM is exact and
  constant on these songs (A03).
- **Half-tempo downbeats.** On the 236–250 BPM songs the trackers' bars are 8 UST beats long; their downbeats
  are still UST bar lines (vote shares 0.99–1.00), so mod 4 beats is well defined.
- **One outlier song.** The bar result is 9/9 with no song carrying it; azemichi is the only low-share vote
  and today's rule already gets it.
- **Telescope's beat for Essentia.** Its multifeature and degara beats sit half a beat off (cell 3 vs 1, share
  0.58) with a low confidence (1.25 on the 0–5.32 scale, "low": [1, 1.5]); A03 also found telescope the most
  ambiguous beat/off-beat song (other parity 0.52 of the peak). Beat This and madmom put it on cell 1.
- **Speed on a loaded machine.** Every run shared 16 cores with other sessions (load 40–62). Beat This's
  default thread pool over-subscribes a loaded machine: its CPU seconds (485 median) are mostly spinning
  threads. Pinned to one thread it uses 57–60 s CPU per song (kamaitachi, amaten; `small0` the same, so the
  model size isn't the cost). Controlled runs, per 4-minute song: default threads at load 23–32, 96–100 s wall;
  4 threads at load 15–26, 43–72 s; 1 thread at load 14–19, 71–97 s. Output identical in every run.

## Limits

- **4/9 dev labels are unverified by ear.** Before this study they were the harmonic estimator's own calls
  with 0–2 of 4 drum and bass cues agreeing; now two other methods (Beat This, madmom) agree on all four. That
  raises confidence in them but can't separate the trackers from the harmonic estimator: on dev they never
  disagree, so **which source is better is undecided** until holdout4's ear-labelled frames.
- Nine songs, one transcriber's corpus, all 4/4 at one integer tempo, files that start near a bar line.
  Real users' files (intros, trims, 3/4) aren't represented.
- BPM: sona's estimator was tuned on these 10 songs (ADR-0022), so its 10/10 "truth offered" is in-sample.

## Recommendation

1. **Ticket 16 (needs-triage): Beat This! downbeats as a second bar-1 source, opt-in until holdout4.** If the
   user takes it: measure its speed on an unloaded machine with a fixed thread count, then build the
   integration the user picks (below). In the app, the two sources' agreement is a useful confidence: on dev
   they agree on 9/9, and a disagreement on a user's song is the case to flag.
2. **Keep sona's BPM estimator for the octave.** No tracker can learn Pifuyuu's notation convention from audio.
3. **Holdout4** (ticket 08) should score three bar sources against ear labels: today, harmonic, Beat This.
   While labelling, the trackers' votes can point the ear at likely bar lines, but the label must come from the
   clicks (`sona-eval-frame --clicks`), or the read is circular again.
4. Keep madmom, Essentia and librosa as eval references only. madmom is as accurate as Beat This but slower and
   non-commercial; Essentia and librosa have no downbeats, and Essentia is AGPL.

## For the user to decide

- **Is about one CPU-minute per song worth it for bar 1?** That is 57–60 s single-threaded, and 43–100 s wall
  per 4-minute song on this machine when it isn't saturated. Separation already takes a few minutes (ADR-0030:
  demucs 101 s and UVR 107 s on one song). The harmonic estimator costs about 8 s and agrees with Beat This on
  every dev song, so on dev Beat This buys a second opinion, not a better answer.
- **How to run it:** (a) a user-installed venv like the separators (`scripts/setup-separators.sh` pattern, torch
  CPU, ~1 GB), the least code; (b) linked: Beat This's weights are MIT, so they can be downloaded at setup or
  even bundled, and run through ONNX with the `game` feature's `ort` (ADR-0017), or the MIT Rust port
  (`beat-this` crate, pure-Rust `rten` runtime; not checked against the reference). Linking puts an ML model in
  the default build unless it stays behind a feature, as GAME does.
- **Licence notes:** madmom's models (CC BY-NC-SA 4.0) and Essentia (AGPL-3.0; MTG models CC BY-NC-SA 4.0) could
  only ever be user-installed tools, and the NC terms would bind users' commercial use. Beat This! has no such
  restriction, though its README notes that some of its *training data* is copyrighted or under limited CC
  licences "and it is up to the user to assess whether this may impact their use case".

## Sources

- Essentia beat detection tutorial: https://essentia.upf.edu/tutorial_rhythm_beatdetection.html
- Essentia licensing: https://essentia.upf.edu/licensing_information.html; the wheel's `COPYING.txt`
  (`License-Expression: AGPL-3.0-only`); models: https://essentia.upf.edu/models.html and
  https://essentia.upf.edu/models/tempo/tempocnn/LICENSE
- `BeatTrackerMultiFeature` confidence scale: https://essentia.upf.edu/reference/std_BeatTrackerMultiFeature.html
- Beat This!: https://github.com/CPJKU/beat_this (`LICENSE`, README "License"); Foscarin, Schlüter and Widmer,
  ISMIR 2024, arXiv:2407.21658
- Rust port: https://docs.rs/crate/beat-this/1.0.0 (repository danigb/beat-this-rs)
- madmom: https://github.com/CPJKU/madmom (`LICENSE`: code BSD-2-Clause, models CC BY-NC-SA 4.0) and
  https://github.com/CPJKU/madmom_models (`LICENSE`)
- BeatNet: https://github.com/mjhydri/BeatNet; All-In-One: https://github.com/mir-aidj/all-in-one
