# Map: UTAU edit cost

Spec: `spec.md`. Tickets: `issues/NN-*.md`. A ticket is on the frontier when it is open, unblocked (every
`Blocked by:` ticket is resolved) and unclaimed; the lowest number goes first. Claim by setting
`Status: claimed` before any work. Resolve by appending `## Answer`, setting `Status: resolved`, and adding
a line below.

## Notes

- Order: metric and frame measurement (01, 02) in parallel, then the baseline (03). The baseline decides
  which of 05–07 is worth doing. 04 (BPM first) needs no measurement and can run any time.
- Parallel agents work in separate git worktrees off the same green commit and never touch a holdout.

## Decisions so far

- 2026-11-24, **targets (user: "yes the intent behind the targets are correct"; "try to get it down to the
  floor")**. Edit cost/min against drawing every note from scratch (~190 UST notes/min × missing 2 ≈ 380):
  - **usable**: ≤ 190 (half of scratch), *and* the frame (bar 1, tempo) right on nearly every song;
  - **reliable**: ≤ ~95 (a quarter), ≥ 50% clean measures, no song above ~2× the pooled cost, doubt marks
    on most of the cost, all confirmed on a holdout;
  - **floor**: the disagreement of two careful transcriptions of the same audio (the UST is one person's
    arrangement), not measured yet; it needs a second human transcription. Oracle reference: joined or
    chopped fixed perfectly gives 67.3 (GAME lead) / 50.0 (stage). Aim for the floor.
- 2026-11-24: the target is cheap fixes in OpenUtau, not cell accuracy (user). Tiers and weights in
  `spec.md`.
- 2026-11-24: users usually know their BPM, so ask for it first (user).
- 2026-11-24, **02 resolved** (merged 903352d; `sona-eval-frame`, verified). Dev, sona's own BPM: frame
  pass 1/9. **Drift fails the frame** (0.01–0.42% tempo error → 0.07–3.8 beats over a song), which a
  typed BPM (04) removes. Bar offset is 0 on 9/9, but only because every corpus file starts about 4 bars
  after the UST's bar 1, so **this corpus can't test bar-1 detection (05)**. sona's tick 0 sits **one cell
  after the UST's** on 9/9; on 6/9 the UST's own beat grid is a 16th off the drums, so the residual
  measures agreement with Pifuyuu's grid, not with the music. 03 decides whether the residual counts.
- 2026-11-24, **04 resolved** (merged af495ae, ADR-0028, verified): the BPM field starts empty and
  focused; the estimate and its ½× are whole-BPM hint buttons; decimals accepted; TLA+ adds `BpmIsYours`.
  Removes drift whenever the user types the true BPM.
- 2026-11-24, **audit merged** (`../audit-2026-11-24/summary.md`): 9 tickets, all triaged ready. Two hit
  this effort directly: MIDI export skips the grid shift (A01), and export tick 0 is the anchor mod one
  cell, so the editor's beats match OpenUtau's 1 time in 4 and **05 has nowhere to put beat 1** until A02
  lands. **05 is now blocked by A02 and A03.** A03 settles what "bar 1" means first: Pifuyuu's grid or the
  music's beat (they disagree by a cell on 6/9 dev songs, possibly an alignment bias).
- 2026-11-24, **A03 resolved** (merged 825ee0d; `reports/bar-one.md`, verified): the alignment is **not**
  biased (vowel re-alignment moves it −15..+11 ms), so past cell-accuracy numbers stand. **Pifuyuu's grid
  is tied to the file, not the music**: his beat lines are off the music's beat on 7/9 dev songs and his
  bar lines match the music's bar on 1/9. So the UST is the right reference for *notes* (cells line up)
  but the wrong one for *where beat and bar 1 are*. `grid_phase(mix, one beat)` matches the drums' beat on
  9/9 (verified). The music's downbeat is only a Python prototype with no ground truth (weak on 4/9).
  Consequences: 05 becomes "beat from the mix (strong), bar from harmony (weak), opt-in"; the frame
  metric must be re-anchored to the music (ticket 10); holdout4 needs ear-labelled beat/bar 1 to score 05.
- 2026-11-24, **A01+A02 and A04+A05 merged** (full suite green on the combination: 275 Rust tests, 156
  web, 5 TLA+ models, clippy, corpus guard). 05's beat 1 now has somewhere to go (ADR-0029's frame); a
  BPM fix re-runs in seconds on the server (ADR-0030), the app side is audit 10.
- 2026-11-24, **10 resolved** (merged; verified): `sona-eval-frame` scores against the music too (manifest
  `music_*` fields, dev only, `unverified-estimate`), and `--clicks` writes click overlays for ear-labelling.
  Dev, music-anchored: with a typed BPM the frame passes when bar offset is 0, which is **5/9**; sona's bar 1
  is whole beats off the music on 4/9 (the file doesn't start on the music's bar). That gap is 05's target.
- 2026-11-24, **01 resolved** (merged; verified: tests, a joined mutant, amaten reproduced exactly). Dev,
  shipped stack (learned stage; **in-sample**: trained on dev + spent holdouts, corrected after A09): **180.2 cost/min [115.9, 256.5], clean measures 23.8% [13.8, 33.9]**, while
  its cell accuracy is 92.7%. GAME lead 188.7 / 16.2%; pYIN default 338.4 / 3.1%. Cost by class: **joined
  53%**, chopped 19%, length 11%, missing 9%, shift 4%, pitch 3%, extra 1%. So **07 outranks 06**
  (missing is only 9%). Unverified: whether joined notes are mostly same-pitch re-articulations (03).
- 2026-11-24, **09 resolved** (merged; verified): `web/e2e/run_all.sh` passes on a clean checkout from a
  generated fixture (`web/e2e/fixture.py`); drivers find targets on the canvas instead of fixed pixels.
- 2026-11-24, **05 resolved as opt-in** (merged 1a6f839, ADR-0031; verified): beat from `grid_phase(mix)`
  (within 0.05 cell of the drums on 9/9), bar from harmonic change; dev music-anchored frame pass 5/9 → 9/9
  (+4 [+1, +7]) **but circular for the bar** (the reference bar came from the same method). Confidence does
  not flag most of A03's weak songs. Pre-registered default rule in the ticket: holdout4, ear-labelled
  music frames (≥ 20 songs, user confirms ≥ 5), B ≥ A + 3 songs, paired-bootstrap lower bound ≥ +1, beat
  no worse. Default unchanged (A02 byte-identity passes).
- 2026-11-24, **correction (A09):** the learned stage's dev numbers are in-sample. The out-of-sample dev
  baseline is GAME lead + octave, 188.7 cost/min and 16.2% clean measures. Any claim about the shipped stack
  needs holdout4.
- 2026-11-24, **03 resolved** (merged; verified: dev 180.2/188.7/338.4 reproduced from the new GAME
  cache in 48 s, a trailing-bytes cache mutant caught). `reports/edit-cost-baseline.md`. The learned stage
  is in-sample on dev, holdout and holdout2, **out of sample only on holdout3**: there it costs 156.3/min vs
  GAME lead's 161.1, inside the noise. **Joined is the largest class on every split** (dev 51.5%, spent
  splits 32–40%), caused by one note per mora: hidden notes are mostly same-pitch vowel or ん moras that no
  source proposes. Oracles (GAME lead, dev · holdout · holdout2 · holdout3): joined 52/40/37/32%, chopped
  14/24/14/13%, single-note 22/19/21/26%, missing 11/14/25/24%. The confidence score is flat (no better than
  random). So: **07 re-scoped** to same-pitch boundaries (split at moras via lyrics or a spectral detector,
  merge restarts), **11** wobble/scoop pieces, **12** single-note errors (after 07), **13** a confidence score
  that works; 06 last (its ceiling on the stage is 7–18%).
- 2026-11-24, **07 resolved** (`reports/same-pitch-boundaries.md`; verified: Rust port = prototype on 18/18
  song rows, default run byte-identical, 3 mutants caught). Without lyrics nothing pays: the spectral detector
  (AUC 0.72–0.73) can't reach the ~50% precision a split needs; merges −1.8 [−3.8, +0.1] out of fold. A whole
  song's lyric aligned by DP makes it worse (+0.9 … +15). **A lyric pasted per phrase and fitted to its notes**
  (2-cell tail split, same-pitch join) lowers GAME lead 188.7 → 174.9 (−13.9 [−30.2, −3.8]; stage −17.8), and
  phrases with the right note count go 29.7% → 45.4%. Opt-in `sona_core::lyric_fit` /
  `--fit-phrase-lyrics`. The lever is now an editor feature (fit selected notes to a pasted phrase), the user's
  call; a default needs holdout4.
  - **Correction (lyrics-first 01):** the −13.9 leaked. The simulated paste gave each run of sona's notes only
    the moras sung inside it, so moras sona missed were never pasted. On honest phrases (the UST's) the shipped
    `fit_phrase` costs **+16.5** [+3.4, +29.6]; with rests holding moras (`fit_span_moras`) it is −14.1
    [−31.1, −3.5]. Lines work as phrases only if placed in time, and sona's notes can't place them.

- 2026-11-24, **13 resolved** (ADR-0033; verified: Rust reproduces the prototype on all 4 700 dev notes, notes
  and cells unchanged, mutants caught): the learned stage's note confidence is now `1 − σ(2.27 − 4.44·class
  prob + 0.58·ln cells)`. Dev, lowest 30% per song: **47.5% of cost caught** (old 30.9%, class prob 43.1%,
  random 30%), +4.4 [+1.8, +6.0] over the class prob; holdout3 (diagnosis) +4.4 [+3.4, +5.5]. Only 2% of
  notes fall under the editor's default 0.50 flag; the threshold and the inspector's "why" are open UI
  questions. GAME-only is flat too (length catches 46.9%), unchanged pending a measurement.
- 2026-11-24, **11 resolved** (`reports/pitch-wobble.md`; verified: Rust port = prototype on 36/36 song rows,
  default run byte-identical, 5 mutants caught).
  - Oracle: merging exactly the chopped 1–2 st steps and one-cell heads is worth −11.3 on GAME lead
    (stage −17.5).
  - The pitch track can't find them (AUC 0.55–0.65; they are 4% of small-step cuts). The ticket's rules lose
    (+5.1 and +1.0) and eat true notes.
  - **A second listen pays a little.** A one-cell piece merges where GAME's run on the full vocals has no
    edge: −1.9 [−3.2, −0.9] out of fold on GAME lead, −3.9 [−7.8, −0.7] on the stage. It eats 10 true notes
    against 17 chops fixed.
  - Caveat: only the second rule grid passes (the first gave +0.3). The spent splits show the same sign
    (diagnosis).
  - It adds to 07's fit (stage −4.5 [−7.5, −1.7] on top).
  - Opt-in `sona_core::unheard_pieces` / `--merge-unheard-pieces`; a default needs holdout4.
  - The rest of chopped needs a note-level signal, not pitch.
- 2026-11-24, **12 resolved** (`reports/single-note-errors.md`; verified: Rust port = prototype on 18/18 song rows,
  default run byte-identical, 4 mutants caught). Single-note errors are 22% of GAME lead's cost. Largest cause:
  **the release**: GAME holds a note through the 2/4/6-cell rest the UST writes before the next note. Fixing it
  is worth −9.5 [−11.5, −7.5], and clean measures go 16.2 → 23.1%; the stage has mostly learned it (−2.0). The
  vocals' energy predicts it (AUC 0.82); pYIN voicing does not.
  - Opt-in `sona_core::release::trim_releases` / `--trim-releases`: GAME lead 188.7 → 184.3 (−4.4 [−6.5, −2.5],
    leave-one-song-out −3.7), clean +2.5. It raises the stage (+0.4, clean −1.1), so it is for GAME notes only.
  - Starts and the remaining ends are symmetric rounding jitter: no bias, phase or pitch-track rule pays.
    Octaves are amaten's. Semitone errors sit near the rounding point but the track can't say which way.
  - Those are doubt signals for 14, not fixes. A default needs holdout4.
- 2026-11-24, **14 resolved** (ADR-0034, [ticket](issues/14-flag-doubtful-notes.md); verified: web + e2e +
  Rust tests, 15 mutants caught, pYIN and GAME-only pixel-identical to 1f6d5fa): a stage result outlines each
  song's least sure **20%** (Settings slider) instead of the threshold; an outlined note says "long note, may
  hide several syllables" or "the model was unsure here" from the larger of ADR-0033's two terms (new optional
  `Note.stage_doubt`). Dev: 33.3% of cost caught at 20%; "long" flags catch 25.9% of joined vs 8.8% of chopped
  cost, "unsure" 6.8% vs 27.4%, so the reasons point at the right fix. The share itself is a product call.
- 2026-11-24, **08 resolved** (merged; verified: holdout ledger untouched, the manifest only adds entries, a
  status-flip mutant caught). **holdout4 = 24 songs** (uploads 2026-08-07 back to 03-24, from the OneDrive
  share; 86 skipped with reasons), **holdout4-tempo = 0** (tempo changes are rare in these USTs). Offsets were
  set without sona (`sona-eval-ust-align --no-verify`) and are `unverified`, because the shipped check uses
  sona's onset detector. **The pre-registered read's first step is that check**: songs it flags (35 ms
  shift / 30 ms drift) are dropped before any score; at least 20 must remain. Bundle every dev winner
  (05 bar 1, 07/lyrics fit, 11, 12, 13's confidence, lyrics-first 02) into that one read.
- 2026-11-24, **15 resolved** (`reports/beat-trackers.md`; rule committed before measuring, f17dad8; dev only).
  - **Bar 1:** Beat This! (code and weights MIT) and madmom (models CC BY-NC-SA) put bar 1 on the music's bar
    on **9/9** (today 5/9, +4 [+1, +7]) and the beat on 9/9.
  - Both **agree with the harmonic estimator on all 9**, the 4 weak labels included. That is two independent
    confirmations of the dev labels, but no ranking of the sources until holdout4's ear labels.
  - Neither wins outright: both fail the pre-registered speed gate as measured (Beat This 160 s per 4-minute
    song at load 40–62). Beat This is about 1 CPU-minute per song when pinned to one thread.
  - **16** (Beat This as a second bar-1 source, opt-in) is filed **needs-triage**: cost and integration
    (venv, ONNX via `ort`, or the MIT Rust port) are the user's call.
  - **BPM:** keep sona's estimator. Trackers give the felt tempo, half the UST's notation on 4–7/10 songs.
    Post hoc, sona's octave × a tracker's tempo makes the whole-BPM hint exact on 10/10 (sona 6/10). That
    needs a pre-registered holdout4-tempo test.
- 2026-11-24, **16 resolved, opt-in** (ADR-0036, [ticket](issues/16-beat-this-bar-one.md); dev only).
  - Beat This! is a user-installed tool: `scripts/setup-separators.sh beat-this` puts it in the lead venv,
    and `sona serve` / `sona transcribe --bar-one-tracker beat-this` run it on the mix, one thread, under the
    tool deadline.
  - Bar 1 is the beat most of its downbeats fall on (mod 4), and the harmony's bar 1 is kept beside it. The
    app offers First grid line / From the harmony / From Beat This (only when installed) and flags a
    disagreement.
  - Dev: 9/9, the same as the harmony; the Rust fold equals `score.py` on 9/9. Speed, one thread at load
    27–39: median 88 s CPU and 133 s wall per 4-minute song.
  - Ticket 05's holdout4 rule gains it as arm C, thresholds unchanged, with a tie-break to the harmony. The
    holdout read still needs `--estimate-bar-one` to get a `--split` path.
- 2026-11-24, **06 resolved, negative** (`reports/lean-toward-sung.md`; rule committed first, 499821a; Rust =
  prototype on 11 settings, default run byte-identical, 2 mutants caught; knob deleted after 43d9b8d). Leaning
  toward sung never pays on dev. A rest bias on the stage costs +1.2 [+0.2, +2.4] at b = 0.5. Drawing cells the
  stage believes sung but no source pitches costs +0.9 [−3.3, +4.1] at best: own notes chop, extensions join.
  Stacked with lyric lines and spaces, the best is +0.1 [−3.6, +2.8], and fills place slightly *more* moras, so
  the lyric-shift Fog does not bite here.
  - Ceilings: any GAME presence threshold −2.9 (GAME lead); a perfect per-run split-or-extend choice −4.4 (stage).
  - The missing notes are quiet moras (ん 25–34%) next to a sung note. The stage knows they are sung (AUC
    0.90–0.96), but no source gives their pitch or boundary, which makes them 07's problem.

## Fog

- **Lyrics.** A pasted lyric is split into mora and laid on notes in order, so one extra or missing note
  shifts every later mora in the phrase. If users paste lyrics in sona, extras may not be cheap at all.
  Measured in 07: the note count equals the mora count in 29.7% of the UST's phrases on GAME lead, 43.0% on
  the stage. Fitting a pasted phrase to its notes raises that to 45.4% and 71.1%.
  Measure the share of phrases whose note count equals their mora count once 01 exists.
- **Tempo changes and time signatures** (roadmap 2.2): the corpus has none by construction. Needs songs
  that have them before anything can be measured.
- **Time signature:** UST files don't carry one. 4/4 is assumed for bar offsets; 3/4 and 6/8 songs would
  be scored wrongly.
- **Do OpenUtau's MIDI and UST imports place notes the same way?** Unchecked. If users import MIDI, check
  the bar lines survive.
