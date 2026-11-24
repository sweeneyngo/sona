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

- 2026-11-24, **13 resolved** (ADR-0033; verified: Rust reproduces the prototype on all 4 700 dev notes, notes
  and cells unchanged, mutants caught): the learned stage's note confidence is now `1 − σ(2.27 − 4.44·class
  prob + 0.58·ln cells)`. Dev, lowest 30% per song: **47.5% of cost caught** (old 30.9%, class prob 43.1%,
  random 30%), +4.4 [+1.8, +6.0] over the class prob; holdout3 (diagnosis) +4.4 [+3.4, +5.5]. Only 2% of
  notes fall under the editor's default 0.50 flag; the threshold and the inspector's "why" are open UI
  questions. GAME-only is flat too (length catches 46.9%), unchanged pending a measurement.

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
