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
  shipped stack (learned stage): **180.2 cost/min [115.9, 256.5], clean measures 23.8% [13.8, 33.9]**, while
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

## Fog

- **Lyrics.** A pasted lyric is split into mora and laid on notes in order, so one extra or missing note
  shifts every later mora in the phrase. If users paste lyrics in sona, extras may not be cheap at all.
  Measure the share of phrases whose note count equals their mora count once 01 exists.
- **Tempo changes and time signatures** (roadmap 2.2): the corpus has none by construction. Needs songs
  that have them before anything can be measured.
- **Time signature:** UST files don't carry one. 4/4 is assumed for bar offsets; 3/4 and 6/8 songs would
  be scored wrongly.
- **Do OpenUtau's MIDI and UST imports place notes the same way?** Unchecked. If users import MIDI, check
  the bar lines survive.
