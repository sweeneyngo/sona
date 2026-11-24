# 15 — Beat and downbeat trackers for the BPM hint and bar 1

Status: ready-for-agent
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
