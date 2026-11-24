# 15 — Beat and downbeat trackers for the BPM hint and bar 1

Status: resolved
Type: research
Blocked by: none

## What

The user (2026-11-24) pointed to Essentia's beat detection
(https://essentia.upf.edu/tutorial_rhythm_beatdetection.html) as helpful for BPM.

Where it can help, given what's measured:
- **BPM hint**: users usually type the BPM (ADR-0028); the estimate is a hint, and a 2× or ½× hint is the
  expensive mistake (ADR-0022).
- **Bar 1**: `grid_phase(mix, one beat)` already matches the drums' beat on 9/9 dev songs, but the bar
  estimate (ADR-0031, chroma of the mix minus vocals) is weak and opt-in. sona's bar 1 is whole beats off
  the music on 4/9 (ticket 10). A downbeat tracker is the obvious candidate.

## Research

- Candidates: Essentia (RhythmExtractor2013, BeatTrackerMultiFeature, TempoCNN, PercivalBpmEstimator),
  Beat This! (CPJKU, beats and downbeats), madmom (DBN downbeat tracker), librosa, and anything newer.
- **Licences**: sona is MIT. Essentia is AGPL-3.0; check code and model weight licences for each
  (madmom's models may be non-commercial). Say how each could be used: linked, a separate tool the user
  installs (as with the separators), or only as an eval reference.
- Score on dev against the music-anchored frame (`sona-eval-frame`, manifest `music_*` fields, ADR-0031;
  `--clicks` writes overlays to listen to): tempo octave errors, beat phase, and bar 1 against sona's
  current estimators. Dev only; no holdout reads. Audio stays on the machine.

## Acceptance

`crates/sona-eval/reports/beat-trackers.md`: a table per candidate (licence, use, BPM octave errors, beat
phase, bar-1 hits on dev, speed). If one wins under a rule declared before measuring, a follow-up ticket
to adopt it, opt-in until holdout4.

## Comments

### 2026-11-24, claimed: selection rule, committed before any tracker is run on dev

**What is scored** (dev only; no holdout audio is opened):
- **BPM**: the 10 dev songs with a UST tempo (as `sona-eval-tempo`), the full mix. Each candidate's own tempo
  estimate (or, for a beat-only tracker, 60 / median inter-beat interval). Classes against the UST BPM within 2%:
  1×, 2×, ½×, other. **"Truth offered"** = the hint as the app shows it ("est", "½× est") contains the UST BPM,
  i.e. 1× or 2×. A ½× or "other" estimate offers no right button: that is the costly miss.
- **Beat phase**: the 9 aligned dev songs. The candidate's beats, folded at the UST (typed) BPM, vote for a cell of
  the beat; the winning cell vs the manifest's `music_beat_shift_cells`. A **hit** is the same cell. Also the
  median residual in ms. Reference: `grid_phase(mix, one beat)` (9/9 by A03).
- **Bar 1**: the same 9 songs. Each downbeat votes for a whole beat of the bar (mod 4 beats at the UST BPM, against
  the manifest's music bar line); the most voted beat is the candidate's bar 1. A **hit** is 0 beats off.
  Compared with today's rule (5/9) and the shipped harmonic estimator (ADR-0031; 9/9 but circular, since the
  music bar labels are its own output). Reported on all 9 and on the **5 songs whose label is corroborated**
  by ≥ 3 of 4 independent drum/bass cues (innocent-addict, kamaitachi-ni-yoru, telescope-nevermore,
  azemichi-ni-ochita-ginga, my-axiom); a miss on the other 4 (atamaro-deluxe, yamineko, amaten,
  fukagyakuteki-henyou) may be the label's error and goes to an ear check, not to a verdict.
- **Speed**: CPU and wall seconds per song on this machine (load reported alongside).
- Each candidate runs **once, in its default configuration, on the full mix**. Variants (other inputs or
  settings) may be reported as diagnosis but are not eligible to win.

**Rule for "wins" (the follow-up ticket is written only for a winner):**
- **Bar 1** (the main use): bar-1 hits ≥ today's + 3 songs on 9 (i.e. ≥ 8/9) with the paired song-bootstrap
  95% lower bound ≥ +1 (`sona_eval::frame::song_bootstrap`), **and** 5/5 on the corroborated songs, **and**
  beat hits ≥ 8/9. Among several winners: more bar-1 hits, then a licence sona can ship over one the user
  installs, then speed.
- **BPM hint**: truth offered on every dev song where sona's estimator offers it, **and** 1× on ≥ sona's
  count + 2 songs. sona's estimator was tuned on these songs (ADR-0022), so a tie is a tie in sona's favour.
- **Licence gate**: a candidate can win only if its code **and** weights can at least be run by a sona user in
  a venv sona does not ship (as with the separators). A non-commercial or AGPL licence does not stop a win; it
  becomes a decision for the user, named in the answer. Linking into sona needs MIT-compatible code and weights.
- **Speed gate**: ≤ 120 s wall per 4-minute song on this machine (demucs already takes minutes).
- The dev labels are unverified estimates; any winner stays **opt-in until holdout4** (ear-labelled frames,
  the rule pre-registered in 05).

## Answer

Report: `crates/sona-eval/reports/beat-trackers.md`; scripts in `crates/sona-eval/scripts/beat-trackers/`.
Dev only; no holdout audio was opened.

- **Bar 1:** Beat This! (code and weights MIT) and madmom (models CC BY-NC-SA 4.0) both put bar 1 on the
  music's bar line on **9/9** (today 5/9; +4 [+1, +7]). Both are 5/5 on the corroborated songs and put the beat
  on the music's beat cell on 9/9.
  - Both **agree with the harmonic estimator (ADR-0031) on all 9**, including the 4 weakly corroborated labels.
    So the dev labels now have two independent confirmations, and dev can't rank the three sources.
- **Rule outcome:** both pass the accuracy criteria and **fail the speed gate as measured**, so there is no
  outright winner.
  - As measured (default threads): Beat This median 160 s wall per 4-minute song at load 40–62; madmom 510 s.
  - Controlled runs show Beat This's failure is mostly the loaded machine: 57–60 s CPU per song on one thread,
    and 43–100 s wall per 4-minute song at load 14–32.
  - The tie-break (a licence sona can ship) picks Beat This. Ticket 16 is filed as **needs-triage** for the
    user's call on cost and integration.
- **BPM hint:** no candidate wins; keep sona's estimator. It offers the true BPM on 10/10, the trackers on
  3–6/10.
  - Trackers report the felt tempo, which is half the UST's notated tempo on 4–7/10 songs. The octave is a
    notation convention that audio can't settle.
  - Post-hoc lead: sona's octave × Beat This's (or madmom's) tempo makes the whole-BPM hint exact on 10/10
    (sona alone 6/10). It needs a pre-registered test on holdout4-tempo.
- Essentia (AGPL-3.0-only; MTG models CC BY-NC-SA 4.0) and librosa (ISC) have no downbeats; their beats hit
  8/9 and 3/9.

