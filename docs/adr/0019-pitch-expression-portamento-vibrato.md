# Pitch expression: portamento and vibrato in OpenUtau's model

**Status: Accepted (2026-11-22).** This is the second milestone of ADR-0018.

## Context

UTAU and OpenUtau sing flat notes unless each note carries pitch bends and vibrato. A singer's
glides and vibrato are in the stem's f0 contour, which every sona annotation already includes.
Before this milestone the review surface only drew that contour; nothing carried it into an export.

## Decision

1. **OpenUtau's own model is canonical.**
   - **What a note carries:** each working note may carry an `Expression`. That is a list of pitch
     points (`x` in ms from the note's start, `y` in tenths of a semitone, shapes `io`, `l`, `i`, `o`),
     OpenUtau's `snap_first` flag, and an optional vibrato (length %, period ms, depth cents,
     fade-in/out %, shift %, drift %).
   - **Source:** the semantics were read from OpenUtau's source (MIT): `UNote.cs` (`UVibrato.Evaluate`,
     `PitchPoint`, `UPitch`, `Validate`'s snap-first rule), `MusicMath.InterpolateShape`,
     `RenderPhrase.cs`'s pitch build, and `Classic/UstNote.cs`'s `WritePitch`, `ParsePitchBend` and
     `ParseVibrato`.
   - **Consequence:** USTX export is lossless, and UST export uses exactly the `PBS/PBW/PBY/PBM` and
     `VBR` lines OpenUtau writes and reads.
2. **Rendering.** `web/lib/expression.ts` `renderPitch` reproduces OpenUtau's pitch build:
   - flat tones first;
   - inside a note, its vibrato replaces the tone;
   - each note's point curve is then added, relative to the tone of the note that owns that moment.

   The review surface draws this curve (orange) over the singer's f0 contour (white).
3. **Fitting.** `web/lib/fitexpr.ts` reads each note from the contour in cents, after removing the
   song's tuning offset by circular mean.
   - **Portamento into a touching note:** from where the contour leaves the previous note's pitch
     (up to 200 ms or 60% of that note before the boundary) to where it settles within 35 cents for
     40 ms. `snap_first` starts it at the previous pitch.
   - **A scoop into a note after a rest:** the first voiced deviation, gliding to 0.
   - **A fall before a rest:** from the last settled moment to the tail's pitch.
   - **Vibrato** on the steady part after the glide:
     - rate from the autocorrelation of the detrended contour (3.5–9 Hz, periodicity at least 0.45);
     - depth from the 90th percentile of the interior amplitude envelope, at least 15 cents;
     - start and fade-in from the envelope;
     - phase from projection onto sine and cosine.
4. **Editing.** **〰 Fit pitch** fits every note. Select a note to edit its glide and vibrato in the
   expression bar (the values are OpenUtau's own clamps), or press **V** to toggle its vibrato.
   Expressions are saved in the session.

## Evidence

**Fidelity.** This is the mean |rendered curve − singer's f0| in cents, over the voiced frames inside
notes, on GAME transcriptions of lead stems.
- **Tuning songs:** yamineko and telescope-nevermore, the only ones the fitter's thresholds were set on.
- **Blind songs:** the rest, never tuned on.

| song | flat notes | + portamento | + vibrato |
|---|---|---|---|
| telescope-nevermore (tuning) | 36.4 | 27.4 | 26.6 |
| yamineko (tuning) | 30.8 | 26.1 | 25.9 |
| innocent-addict (blind) | 19.4 | 15.1 | 13.5 |
| kamaitachi-ni-yoru (blind) | 25.6 | 20.3 | 19.3 |
| my-axiom (blind) | 19.7 | 16.3 | 16.3 |
| denpa (blind; holdout3 song, audio only) | 33.9 | 31.2 | 30.8 |

On the songs the fitter never saw, the fitted curve is 9–30% closer to the singer than flat notes
(denpa 9%, the others 17–30%). Portamento gives most of it, and vibrato adds 0–1.6 cents on songs
with sustained notes.

Where the rest sits (yamineko, by position in the note):
- in the middle of notes, 16 cents, which is contour noise;
- at the first and last fifths of notes, 34 and 40 cents. That is mostly voiced consonants dipping the
  pitch at syllable boundaries, which a synthesizer produces itself, so it isn't fitted.

**Correctness.**
- Unit tests:
  - the renderer against OpenUtau's formulas (flat, snap-first glide, `UVibrato.Evaluate`);
  - 60 random vibratos recovered, with period within 12% and depth within 25%;
  - 50 random neutral sequences render exactly flat.
- UST lines round-trip through a port of OpenUtau's `ParsePitchBend`/`ParseVibrato`, for fixed and
  100 random point sets.
- The fitter's first depth estimate (the envelope peak) failed the random-vibrato property on vibratos
  with no fade-out. The edge-truncated detrend inflated the peak, so it is now an interior percentile.

**UI state.** The pstack statechart pass modelled the expression bar (`verification/expr-bar`, in CI).
- **What it found:** a value typed for one note, followed by a click on another note, committed to the
  note just clicked. Selection happens on mousedown, before the field's blur commits.
- **Confirmation:** the path was replayed in the browser before the fix.
- **The fix:** the bar is bound to the note it shows. Both the bug and dropping the
  don't-overwrite-while-editing guard are checked mutants.

## Consequences

- MIDI export stays notes-only. MIDI pitch-bend output is possible later from `renderPitch`, but it is
  not part of this milestone.
- Fitting depends on the annotation's f0 contour. An annotation without one can still be edited by hand.
- Synthesis-specific parameters (preutterance, envelopes, flags) remain defaults.
