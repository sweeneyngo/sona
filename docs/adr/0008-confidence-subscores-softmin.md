# Confidence: four sub-scores, weakest-link soft-min into a scalar

The spec's pipeline stage 4 (ticket 06) turns each `NoteSegment` (ticket 05) into
an annotation `Note` by attaching a confidence scalar and four sub-scores.
CONTEXT.md calls confidence the thing that makes ear-based evaluation tractable —
it tells the user *where to listen* — so the spec ranks this ticket as the
highest-leverage one. Two decisions had to be made concrete: what each sub-score
measures, and how the four combine into the scalar that drives the heatmap.

## Four sub-scores, each in `0..=1`

Each dimension is computed from raw perception the earlier stages already
produced, so confidence adds judgement without re-running DSP:

- **pitch_stability** — the standard deviation of the note's voiced f0, taken in
  **semitones** (musical, not Hz-biased toward high notes) and scored with a
  rational falloff `1 / (1 + (std/tol)²)` (`tol = 0.6` semitone). Zero spread → 1,
  one tolerance → 0.5, a wandering contour → toward 0. Bounded vibrato has bounded
  spread, so it stays high — the same "vibrato is a slow ripple, not instability"
  boundary ADR-0005/0007 drew. A note with no voiced frames scores 0.
- **voicing** — mean pYIN voicing over the note's frames **×** a bleed-guard
  factor. The factor ramps from 0 at `bleed_floor_frac` (0.1) to 1 at
  `note_level_frac` (0.3) of the stem's peak RMS, measured from the note's *peak*
  RMS. Multiplying (not averaging) means either a poorly-voiced note or a
  near-floor note scores low. This is the spec's primary bleed defence at the note
  level, mirroring the onset stage's peak-relative bleed floor (ADR-0006).
- **onset_clarity** — the segment's onset strength saturated by
  `s / (s + half)` (`half = 0.5`). A plateau-split note carries strength 0
  (ADR-0007), but its onset *is* well located — it is the pitch change — so it
  takes a middling baseline (`split_onset_clarity = 0.5`) rather than 0. That keeps
  it honestly below a real attack (the UI can surface "⚠ uncertain onset" for
  slides) without the soft-min's zero-collapse burying a real legato note among
  dead bleed. A missing *transient* is not a missing *note*.
- **offset_clarity** — the offset's energy-drop steepness passed straight through.
  The spec is explicit that offset confidence comes from the drop *steepness*
  (sharp = high, slow tail = low), **not** from which `OffsetKind` fired; ticket 05
  already recorded exactly that number, so this dimension is a read-through.

The exact formulas are empirical. Every knob is a public field on
`ConfidenceScorer`, so a comparison run can sweep them against real Vocaloid stems
without a code change — the same posture the segmenter and onset detector take.

## Weakest-link soft-min, not an average

A note is only as trustworthy as its shakiest dimension: a clean pitch, loud
level, and sharp attack must **not** paper over a mushy offset, because that mushy
offset is exactly what the user needs to re-check. Averaging would mask it, so the
scalar is a soft-min.

The soft-min is a negative-power mean, `(mean xᵖ)^(1/p)` with `p = -4`. It sits
just above the true minimum — one weak score pulls the scalar down near it while
the strong scores nudge it up slightly — giving weakest-link behaviour without a
hard `min`'s brittle all-or-nothing. A **zero anywhere forces the scalar to zero**
(that term's power is `+∞`, and `∞^(1/p) = 0` for `p < 0`), which falls out of the
arithmetic with no special-casing: a dead dimension means no confidence. `p` is
tunable; more negative approaches a hard min, toward 0 softens toward the mean.

## Consequences

- Confidence is where a `NoteSegment` becomes an annotation `Note`. Segmentation
  deliberately stopped short of judgement (ADR-0007); this stage supplies it, and
  the two types staying separate keeps each stage honest about what it knows.
- **Defence in depth against bleed.** The onset stage already refuses to emit an
  onset for near-floor bleed (ADR-0006), so in the normal pipeline a spurious bleed
  note never reaches confidence at all. The voicing bleed guard here is the second
  line — for a marginal note that clears the onset floor but still hovers low, or
  for a producer/adapter that feeds segments in without the onset stage. The
  integration test asserts the upstream cull; the note-level guard is unit-tested
  by constructing a segment at exactly bleed level.
- Both the scalar and the sub-scores are emitted. The scalar drives the heatmap
  and sort order; the sub-scores let the UI explain *why* a note is flagged. The
  flag **threshold** stays a UI slider (ADR-0003 / spec), calibrated by ear per
  song — this stage sets no cutoff, it only scores.
