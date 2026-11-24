# ADR-0031: Bar 1 from the music, opt-in

Status: accepted (opt-in), 2026-11-24. Ticket: `.scratch/utau-edit-cost/issues/05-bar-one-detection.md`.
Builds on [ADR-0027](0027-grid-phase-from-onsets.md) (grid phase from onsets) and
[ADR-0029](0029-export-frame-bar-one.md) (the export frame and its bar 1). Study: `crates/sona-eval/reports/bar-one.md` (A03).

## Context

With a typed BPM, the export's frame passes against the music only when its bar 1 is the music's bar 1.
Today's rule (tick 0 = the first grid line at or after 0 s) gets that on 5/9 dev songs, by where the file
happens to start (utau-edit-cost 10). A passage in the wrong measures is the costliest fix in OpenUtau.
A03 found the music's beat with `grid_phase` on the mix (9/9 against the drums) and its bar with a Python
harmonic-change prototype that has no ground truth.

## Decision

1. **`sona_core::bar_one::estimate(mix, vocals, rate, bpm, division)`**, beside `tempo`:
   - **Beat:** the vocals' cell line (`grid_phase(vocals, cell)`) nearest `grid_phase(mix, 60/BPM)`; the
     first such beat at or after 0 s.
   - **Bar:** chroma of mix − vocals (4096-point STFT, 1024 hop, 65–2100 Hz, each frame scaled to max 1).
     At every beat, the cosine distance between the summed chroma of the 1, 2 and 4 beats before and after
     it. Each window's mean change per beat of the bar (mod 4) is scaled to sum to 1, the three are summed
     and the largest wins.
   - **Confidence:** the share of the song's 8-bar blocks whose own vote is the song's.
     Below 0.5 (`MIN_CONFIDENCE`, set before any dev read, not tuned), bar 1 falls back to the beat.
2. **The annotation** gains an optional `bar_one: { time_s, confidence, harmonic }`: the first bar line at
   or after 0 s. It is absent unless asked for, so the JSON is unchanged by default.
3. **Opt-in only:** `sona transcribe --bar-one MIX.wav` (vocals from `--vocals`, else the stem), and
   `sona serve` with `bar_one=1` on the job (a vocals or lead separation; a stem upload has no mix, so 400).
   The app's start screen has an unticked **Bar 1 from the music** option.
4. **The app** starts the frame's bar 1 at `bar_one.time_s` only when the annotation has it. The ♩ popover
   says "from the music (N%)" (or "the music's beat (bar unsure, N%)"), and **Auto** returns to the
   estimate instead of the first grid line. Without `bar_one`, everything is as before.

## Evidence (dev 9, typed UST BPM; `sona-eval-frame --estimate-bar-one`)

Frame pass against the manifest's music frame (`unverified-estimate`, seeded from A03), i.e. bar offset 0:

| rule | pass | 95% song bootstrap | vs today |
|---|---|---|---|
| today: first grid line at or after 0 s | 5/9 | [2, 8] | |
| oracle beat, first at or after 0 s | 5/9 | [2, 8] | +0 [−3, +3] |
| oracle beat, first at or before 0 s | 2/9 | [0, 5] | −3 [−7, +1] |
| estimated beat alone | 5/9 | [2, 8] | +0 [−3, +3] |
| estimated beat + harmonic bar (shipped) | **9/9** | [9, 9] | **+4 [+1, +7]** |
| oracle bar (perfect bar 1) | 9/9 | by construction | +4 |

- **The beat alone closes none of the gap.** It puts tick 0 on the beat (|residual| ≤ 0.01 beat on 9/9,
  against 5/9 today) but leaves the bar to chance: it fixes my-axiom and breaks azemichi.
- **The port reproduces the prototype**: its bar call equals A03's on 9/9 songs, and its beat is within
  0.05 cell of the drums' on 9/9. The 9/9 is **circular** for the bar: the reference bar *is* A03's
  harmonic estimate. It shows the re-derivation is faithful, not that the bar is right.
- **Confidence** (share of agreeing 8-bar blocks): 0.57 fukagyakuteki, 0.60 my-axiom, 0.61 innocent,
  0.70 azemichi, 0.85 amaten, 0.92 atamaro, 1.00 kamaitachi, telescope, yamineko. Nothing fell back.
  It measures the harmony's self-consistency, not agreement with A03's drum and bass cues: yamineko
  (0/4 cues) is 1.00, and only fukagyakuteki (1/4 cues) is lowest.
- Tests: synthetic songs whose chords change on beat 0–3 of the bar while a louder sung line changes
  pitch on another beat, at 150–240 BPM; disagreeing 8-bar blocks fall back to the beat; the JSON omits
  `bar_one` by default; the CLI and server fill it only on request and leave the notes unchanged; the app's
  Auto and readout; the default export byte for byte (ADR-0029's 420-session golden). Each check failed
  on a mutant (ticket `## Answer`).

## Consequences

- Default output is byte-identical: no `bar_one`, no new provenance setting, the same job query.
- With the option, the annotation's provenance gains `bar_one: mix` and the mix's fingerprint, and a
  `sona serve` job writes `mix.wav` beside its stems.
- Making it the default needs holdout4 with **ear-labelled** music frames, under the rule pre-registered
  in the ticket. 4/4 and one tempo only; a song typed at half its UST tempo has a different bar.
