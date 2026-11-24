# ADR-0022: Tempo estimation, and a start screen with one action

Status: accepted (2026-11-23). Decision 2 amended by [ADR-0028](0028-ask-for-the-bpm-first.md): the start screen asks for the BPM, and the estimate is a hint rather than a pre-filled value.

## Context

One-click generation (ADR-0020) still asked a newcomer for five things before anything happened: a
file, a BPM, a grid, a separation and a note engine, in a dialog. The BPM was the real blocker: most
people don't know their song's tempo, and a wrong one puts every note on the wrong grid.

## Decision

1. **Estimate the tempo from the full mix** (`sona_core::tempo::estimate_bpm`). The method:
   - log-magnitude spectral flux, with its slow trend removed;
   - its autocorrelation, and a comb over the beat and its 8th, 16th and 32nd pulses;
   - a search in **140–280 BPM**, with a prior centred on 190 and a ±1.5% refine.

   The window is deliberate. A beat tracker's natural answer for a 240 BPM song is 120, and that
   makes a 16th grid too coarse for the song's short notes. Reading a slow song at double tempo only
   makes the grid finer: the notes still land, and playback is identical. So the estimator errs towards
   double, never towards half.

   `sona serve` exposes it as `POST /api/tempo`, which decodes anything that isn't WAV with ffmpeg.
2. **The start screen is one drop zone.** Dropping a song fetches its tempo and shows a single field,
   pre-filled, with ½× and 2× beside it, and one **Transcribe** button. Separation, note engine and a
   triplet grid sit under *Options*, defaulting to the best the server has: the lead vocal and GAME
   when available. Progress is shown in place. Open files and Resume are small links. The rules live
   in a pure reducer (`web/lib/startFlow.ts`): a tempo answer counts only for the song it was asked
   about, Transcribe needs a usable tempo, and one job runs at a time.
3. **After a transcription, pitch is fitted automatically** (ADR-0019), so the export already carries
   the singer's glides and vibrato.
4. **The editor shows only what's used every minute.**
   - The header holds the menus (File, Edit, View, Export), one ▶/⏸ button, and a **♩ tempo · grid**
     button whose popover holds the BPM (with ½×/2×), the grid and the detected key.
   - Display toggles and mutes moved into *View*. The colour legend moved into Help, and the
     corrections list into *Edit*.
   - The pitch bar shows Vibrato, Fit, Reset and *Values*. The numbers are folded under *Values*,
     because the handles on the roll are the primary editor (ADR-0021).
   - Copy is cut to a few words; status messages are short.

## Evidence

**Tempo**, pre-registered at dec0416 (`.scratch/tempo-estimation/spec.md`), measured against the exact
tempos in the 42 corpus songs' USTs (`sona-eval-tempo`):

| | songs | exact (±2%) | within an octave | half-tempo errors |
|---|---|---|---|---|
| dev (tuned on) | 10 | 8 | 10 | 0 |
| test (read once, logged) | 32 | **24 (75.0%)** | 31 (96.9%) | **0** |

- **The pre-registered bar (80% exact) is not met.** As the rule says, the estimate therefore ships as
  a *suggestion*: it is pre-filled and the tempo stays required.
- **All 7 slow songs (92–139 BPM) read at double.** That is the window's chosen failure. One song
  (denpa, 125 BPM) read at 4/3.
- **Disclosure:** the test songs' tempo *labels* were seen before the read, though no estimate was made
  on them. The method was fixed on dev and committed before the read.
- The Rust port reproduces the numpy prototype to 0.1 BPM on every dev song. Its trend window sits one
  frame later than numpy's `convolve(..., "same")`. The test numbers above are for the Rust code as
  shipped.

**Start screen.**
- The reducer is tested on fixed paths and 3 000 random event orders.
- `verification/start-flow` (TLA+, in CI) checks that the tempo shown belongs to the song shown and
  that no job runs without a usable tempo. Both mutants (a stale answer wins; Transcribe without a
  tempo) are caught.
- In Chromium against `sona serve`, the 25 s sample went from drop to loaded notes in 76 s. Its tempo
  was suggested at 246.1, ½× gave 123.1 (the UST says 123), and the pitch was fitted on load.
- Zero inputs are visible on the start screen or in the editor. Play/pause, the tempo popover (½×/2×,
  Escape) and the View menu by keyboard all work, and File → New song returns to the start screen and
  Back leaves it. The earlier drivers (handles, pen, singer's-pitch export, lyrics round-trip, lite
  autosave, stale hover, motion) pass on the new layout.

**Review (comment-sicko pass)** found four races in the start screen's job handling. All four are now
fixed:
- **Cancel during the upload** left the server job running and holding the queue. An upload that comes
  back for an abandoned attempt is now cancelled on the server.
- **Cancel, then Transcribe again**, could adopt the first, cancelled job. Each attempt now carries an id,
  and only its own job is accepted (unit-tested).
- **A poll already in flight** when Cancel was pressed could still load the results. The state is now
  re-checked after every await.
- **An unreadable result** left the screen stuck at "Done". It now fails visibly.

## Consequences

- A slow song needs one ½× click. A better octave choice is a frontier (`docs/roadmap.md`).
- GAME's segmenter was unseeded, so the same song gave slightly different note counts from run to run
  (69 and 67 notes on the sample). Fixed in ADR-0023.
