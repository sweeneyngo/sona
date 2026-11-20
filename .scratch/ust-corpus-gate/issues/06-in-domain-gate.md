# 06 — The in-domain gate: quantized COnP/COnPOff vs UST, per stratum

Status: ready-for-agent

Blocked by: 05

The payoff ticket. **Measures; does not fix.**

## What it answers

Is the offset bottleneck real *in the target domain*, or largely an artifact of
how it was measured?

The standing number — COnPOff-F1 **0.508** vs COnP-F1 **0.646**
([`note-hmm-sweep.md`](../../../crates/sona-eval/reports/note-hmm-sweep.md)) — was
measured on **raw, unquantized** output against **expressive-performance**
annotations. sona's actual deliverable is **hard-snapped to a 16th grid**
([ADR-0003](../../../docs/adr/0003-hard-snap-quantization-for-vocaloid.md)) and
the UST reference is **tick-quantized**. At 150 BPM a 16th is 100 ms, so a raw
offset error under ~50 ms snaps to the **same grid line as the truth** and becomes
*zero* error.

Expect scores to become more **bimodal** under quantization than the raw metric:
post-snap an offset is either exact (same grid line) or a full grid cell out.
Report the distribution, not just the mean — a mean hides that structure.

## Report contents

- COnP / COnPOff F1 on **quantized** output vs **quantized** UST, **split by
  stratum** (`vocaloid-original` = the gate; `human-sung` = watched, never tuned
  against). **Never pooled.**
- The same metrics on **raw** output vs UST, side by side — this is what isolates
  how much of the gap quantization absorbs.
- The Vocadito baseline alongside, so the in-domain and proxy-domain numbers are
  directly comparable.
- Per-note-count weighting disclosed, and any songs dropped by 05 listed with
  reasons — **no silent truncation.**

## The decision this ticket produces

Write the conclusion plainly, including if it is negative or anticlimactic —
following [ADR-0012](../../../docs/adr/0012-note-offset-release-state.md)'s
precedent of recording a real negative result rather than explaining it away.

- **If the in-domain quantized COnPOff is high** (say ≳0.85): the offset
  bottleneck was substantially a measurement artifact for the target domain.
  **Close the item.** Do not build a mechanism.
- **If a real gap remains**: the next move is re-sweeping the existing ADR-0012
  **Release path** in-domain (it ships off because Vocadito's soft offsets defeat
  it; staccato Vocaloid is the sharp-cutoff material it was built for). That
  requires a new ADR arguing the overfitting tension **explicitly** — ADR-0012
  warns against tuning on a favourable staccato slice, and the only valid
  counter-argument is that staccato Vocaloid is the *declared target domain*
  rather than a cherry-picked slice. If that argument cannot be made honestly,
  do not make the change.
- **An ML offset model stays off the table at this corpus size.** 20 songs
  (~4–6k notes) is an evaluation set, not a training set. If the gap is real and
  large, the honest conclusion is "this needs a ~200-song corpus" — a decision to
  surface, not a thing to quietly start.

## Follow-ups this unlocks (not in scope here)

- **BPM estimation**, with golden labels free from the corpus; metric already
  defined (exact-match + factor-of-2-tolerant, for half/double-tempo confusion).
- **Repeated-passage self-consistency**, now *validatable* — check whether its
  flags correlate with UST-measured errors before adopting it.

## Comments

**2026-11-20 — Input from 01** ([audit](../../../crates/sona-eval/reports/ust-format-audit.md)).
Only 14% of UST notes are rest-terminated. Report COnPOff on the **rest-terminated subset** as the
offset number; all-notes or legato COnPOff mostly re-measures onsets. Also report the per-song
recall ceiling from the 100 ms prune floor (50% of ノイズゴト's notes are below it) and split out
triplet songs.

**2026-11-20 — Decision rule, pre-registered before any result** (pstack `principle-explain-the-number`:
write down the decision before you run).

- **The offset number** is offset accuracy given a correct onset and pitch, on **rest-terminated** reference
  notes, in **monophonic** regions, on **quantized** output, pooled over the `vocaloid-original` songs that 05
  aligned. (From 01: legato UST offsets are just the next onset, and harmony is in the stem.)
- **Close the offset item** if that number is **≥ 0.85** *and* the lower end of its 95% bootstrap interval
  (resampling songs) is **≥ 0.80**. Then the bottleneck was largely a measurement artifact in-domain.
- **A real gap remains** if the number is **< 0.75**, *or* its interval's upper end is **< 0.85**. Next move per
  the ticket: an ADR arguing the in-domain re-sweep of the ADR-0012 Release path.
- **Inconclusive** if the result falls between those cases, or if fewer than 150 rest-terminated monophonic pairs
  are scored. Report it as inconclusive and name the gap; don't round it toward either side.
- **Noise floor:** sona is deterministic, so runs don't vary. The spread that matters is song sampling, which the
  bootstrap interval measures.
- **What else the number could be measuring** (each to rule out or bound in the report): misalignment (05's
  onset-delta check), harmony bleed (the monophonic view), demucs artifacts, the quantizer anchor (the gate uses
  the UST-derived anchor, which is more optimistic than the UI's auto-anchor), the escape hatch, notes below the
  prune floor, and errors in the UST itself (it's a fan transcription, not the producer's MIDI).
