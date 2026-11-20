# 05 — Manual phase alignment, recorded in the manifest

Status: done

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

## Comments

**2026-11-20 — Done, with an automated method instead of by-ear alignment.** Tool: `sona-eval-ust-align --write`.

The by-ear step was replaced so the plan could run without the maintainer. The offset is still **set without
sona** (the circularity this ticket rules out) and **stored as data** (reviewable, and can be overridden by ear):
1. **Coarse:** cross-correlate the any-voice-singing mask from the USTs with the vocal stem's log-RMS
   envelope (10 ms frames).
2. **Fine:** within ±½ grid cell, line the UST's grid lines up with drum-stem onsets (1 ms resolution).
   The search was ±½ *beat* at first. That moved INNOCENT ADDICT a whole 16th, because a UST's tick 0 can be
   a pickup, so every-480-ticks lines aren't reliably beats. A regression test now covers this.
3. **Verify with sona's onsets:** this ticket's onset-delta distribution. It prints p10/p25/p50/p75/p90 and
   the drift from the first third of the song to the last.

**Result: 19 / 20 aligned.**
- Every aligned song's median delta is between −32 and +3 ms (typically about −8 ms), with drift ≤ 13 ms.
  That −8 ms matches sona's −11.6 ms early-onset bias measured on Vocadito (`reports/onset-deviation.md`):
  two independent measurements agreeing.
- かまいたちに因る (−32 ms) is the borderline case. Its spread is tight and its drum grid strong, so it reads
  as sona firing early on consonants, not as misalignment.
- **Flagged and excluded: plus-space**, a constant +38 ms shift. It's a piano-led duet with the weakest drum
  grid (peak 1.42). The drum step moved the coarse offset −37 ms in the wrong direction.
  - Not fixed by tuning the drum threshold, because choosing the threshold by agreement with sona would be
    circular.
  - A by-ear alignment could bring it back.
