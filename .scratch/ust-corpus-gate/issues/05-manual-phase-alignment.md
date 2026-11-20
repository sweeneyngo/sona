# 05 — Manual phase alignment, recorded in the manifest

Status: ready-for-human

Blocked by: 03, 04

`ready-for-human` because the alignment is set **by ear**, per song, by the
maintainer. ~20 songs is an afternoon.

## Why manual, and why not automatic

BPM comes **exactly** from the UST's `Tempo`, so alignment is not a 2-D search —
it reduces to finding **phase within one beat period**. That is quick to set and
quick to verify in the review surface.

Auto-aligning by cross-correlating UST onsets against sona's detected onsets is
**rejected as circular**: it would use sona's own onset detector to position the
ground truth sona is then scored against, absorbing any systematic detector bias
into the alignment and letting the score flatter itself.

The offset is stored in the manifest as **data** — reviewable, version-controlled
— not recomputed on each run.

## The failure mode this ticket exists to make visible

**Misalignment is silent and masquerades as the thing being measured.** A
constant 80 ms offset error reads as "sona has a systematic onset/offset bias,"
indistinguishable from a real perception bug — and would send someone chasing a
phantom through the HMM.

So the audit report prints a **per-song onset-delta distribution**:

- deltas scattered around zero → aligned
- a **constant shift** → misaligned; fix the offset
- **growing drift** from song start to end → the single-BPM assumption is broken
  for that song (tempo drift, or mid-song tempo changes in the UST). Flag the song
  rather than forcing an alignment that cannot hold.

This is also prior art in the project: CONTEXT.md's *Grid anchor (phase)* notes
that "getting phase wrong makes hard-snap quantization actively harmful."

## Done when

- `alignment_offset_s` populated for every included song.
- Onset-delta distribution printed per song, and every song either reads as
  aligned or is explicitly flagged (drift / unalignable) with a reason.
- Flagged songs are excluded from the gate in 06 rather than silently degrading it.
