# 04 — Pitch pre-filter for the HMM input

Status: ready
Blocked by: 03
Phase: B (note-HMM)

Prepare the contour the note-HMM decodes over.

- A short **3-tap median** on the voiced f0 track (SiPTH: 3 beat 5/7; wide medians eat short notes). pYIN already median-filters (`median_window: 5`) — decide whether the HMM consumes that or a dedicated width, and record why.
- Convert to a semitone/pitch representation the emission model reads.
- **Bilateral filter is deferred** (unproven here, can freeze real transitions) — leave a seam, don't build it unless 08's numbers ask for it.

Done when: the pre-filtered contour is available to the segmenter as (time, semitone, voicing) frames; no behaviour change yet (still feeds the old segmenter).
