# Segmentation: hybrid offsets, earliest-candidate-wins; pitch-plateau splitting

**Status: Superseded** by [ADR-0010](0010-note-hmm-segmentation.md) (the note-HMM); the knobs named below no longer exist.

The spec's pipeline stage 3 (ticket 05) turns the f0 contour + onsets + energy
envelope into note events, and calls it the highest-error area. Two decisions had
to be made concrete: how a note's **offset** is chosen, and how a **legato pitch
change with no onset** becomes two notes.

## Offsets: two candidates, the earlier one wins

The spec asks for a *hybrid* offset — "next-onset primary; energy-decay fallback
closes notes into rests." Making that concrete, `sona-core::segment` computes two
offset candidates per note and takes **whichever comes first in time**:

- **Next-onset** — eligible only when the next onset is within a musical
  threshold (`max_offset_gap_s`, default 0.18 s ≈ an eighth at a brisk tempo). A
  next onset farther than that implies a rest, not a note boundary, so it is not
  allowed to end the note.
- **Energy-decay** — the first frame past a minimum note duration whose RMS has
  fallen to a peak-relative rest floor (`rest_floor_frac`, default 0.15). This is
  the staccato-Vocaloid common case: short note, clean gap, sharp drop into a
  rest.

Earliest-wins is what makes it genuinely hybrid rather than a mode switch: a
back-to-back pair keeps its energy up across the boundary, so next-onset wins; a
staccato note decays first, so the fallback closes it early and leaves the rest.
A long held note whose energy never decays and whose next onset is past the
threshold still closes honestly on that next onset (there is nothing earlier).
The peak-relative rest floor mirrors the onset stage's bleed floor (ADR-0006) so
both stages track the stem's level rather than an absolute amplitude.

## Offset steepness is recorded here, scored in ticket 06

Per offset the stage records an **energy-drop steepness** in `0..=1`: the
fractional fall in RMS across the offset over a short window, so it reads the
*rate* of the drop (sharp staccato → near 1, slow tail → near 0) rather than its
eventual depth. This is the raw material offset-clarity confidence (ticket 06)
consumes — the spec is explicit that offset confidence comes from the drop
steepness, **not** from which fallback fired. The `OffsetKind` (next-onset /
energy-decay / plateau-split / stem-end) is carried too, but only so the UI can
explain a boundary; it does not set the score.

## Pitch-plateau splitting owns the legato case

Onset detection is energy-based (ADR-0006) and deliberately does *not* split two
legato pitches at steady energy — that was left here. Within each onset's span
the contour is quantized to MIDI pitches and grouped into runs; a run sustained
past `min_plateau_s` (default 0.08 s) becomes its own note. Shorter excursions —
a vibrato swing crossing a semitone, a slide passing through a pitch — are
**absorbed** into a neighbour (closest in pitch, ties to the longer run) so the
stage doesn't shatter a held note into a stutter of fragments. This is the same
"vibrato is a slow ripple, splitting it is segmentation's job" boundary ADR-0005
drew for the pYIN median filter, now paid off.

A plateau-split note carries `split_from_plateau = true`, `onset_strength = 0`
(no transient of its own), and the split boundary is seamless — the previous
note's offset is the next note's onset.

## Consequences

- Segmentation's output is a `NoteSegment`, **not** an annotation `Note`: it has
  no confidence scalar or sub-scores yet. Ticket 06 composes those from the raw
  material each segment carries (onset strength, drop steepness) plus the contour
  it re-reads. Keeping the types separate stops segmentation from pretending to
  know confidence.
- Notes come out in raw seconds with no quantization — grid snapping is the UI's
  job (ADR-0004). A segment can still be zero-rest adjacent to the next; the UI
  decides what a rest means musically.
- The four thresholds (`max_offset_gap_s`, `rest_floor_frac`, `min_plateau_s`,
  `drop_window_s`) are empirical defaults tuned for staccato Vocaloid; the spec
  expects them re-tuned by ear against real stems. They are public fields on
  `Segmenter`, so a caller (or a future comparison run) can sweep them without a
  code change.
- A span with no voiced f0 frames still yields one note (pitch 0), not a silent
  drop: culling a spurious near-floor note is the confidence stage's bleed guard
  (ticket 06), so segmentation never quietly discards an onset the earlier stage
  accepted.
