# 03 — The bar offset is sub-beat: measure it in cells; "nearest grid line to 0 s" is a candidate prior

Status: needs-triage
Type: research
Blocked by: utau-edit-cost 02 (frame metrics), to repeat with sona's own phase; 02 above, to make any prior reach the export

## Evidence

- utau-edit-cost ticket 02 plans to report the bar offset "in beats mod 4". The export picks tick 0
  modulo one **cell** (`web/lib/sequence.ts:43-57`; see 02), so the offset can be any whole number of
  cells, and a beat count rounds sub-beat offsets away.
- A metadata-only check on the **dev** split. It uses only `ust-manifest.json` (`bpm_from_ust`,
  `grid_division`, `alignment_offset_s`): no UST, no transcription, no holdout. It assumes sona's phase is
  exactly right (the best case) and that UST tick 0 is a bar line, as the spec assumes. Under those
  assumptions, the UST bar line nearest to stem time 0 is:

  | song | cell | nearest UST bar line to t = 0 |
  |---|---|---|
  | innocent-addict | 75.8 ms | −64.5 ms (−0.85 cell) |
  | kamaitachi-ni-yoru | 104.2 ms | −73.3 ms (−0.70) |
  | telescope-nevermore | 122.0 ms | −100.1 ms (−0.82) |
  | plus-space | 138.9 ms | −58.1 ms (−0.42) |
  | atamaro-deluxe (24) | 41.0 ms | −37.6 ms (−0.92) |
  | azemichi-ni-ochita-ginga | 76.5 ms | −36.0 ms (−0.47) |
  | yamineko | 60.0 ms | −57.0 ms (−0.95) |
  | my-axiom | 76.9 ms | −73.9 ms (−0.96) |
  | amaten | 92.0 ms | −28.4 ms (−0.31) |
  | fukagyakuteki-henyou | 63.6 ms | −58.2 ms (−0.92) |

  On all 10 songs it sits just **before** 0 s, by 0.3–0.96 of a cell (always bar index 4 of the UST). The
  export rule "tick 0 = first grid line at or after 0 s" therefore skips it. Even with a perfect phase,
  sona's bar 1 lands **one 16th after** the UST's bar line on **10/10 dev songs**. Every note then sits one
  cell late relative to OpenUtau's measures. "Beats mod 4" would round 0.25 beat to 0 and call it a pass.
- Under the same assumptions, a rule of "the grid line **nearest** 0 s" would hit the bar line on 10/10.

## What else this could be (rule out before acting)

- **Alignment bias.** `alignment_offset_s` comes from matching UST notes to the stem's onsets
  (`sona-eval-ust-align`). If detected onsets are consonant starts, about 30–100 ms before the vowel that
  UTAU places on the note, every offset shifts early by that much. That would produce exactly this
  pattern. ADR-0027's phase is scored against the same offsets, so sona's grid shares the bias, and the
  conclusion "one cell late in OpenUtau" holds only if the bias is in the *audio*, not the reference.
  Check by listening to two songs against a click at the UST's bar lines.
- **Corpus cut.** These stems come from originals that start on a downbeat; real user files (YouTube rips,
  encoder delay, spoken intros) may not. Ask the user what they drop, or check a handful of their files.
- **One integer tempo** by selection (spec "Known bias"), so drift is not in play here.

## Proposal

1. Fold into utau-02: report the bar offset in **cells mod cells-per-bar**, with the beat offset derived
   from it, and add this table's metric with sona's own phase (dev).
2. If the bias check says the pattern is real, prototype "nearest grid line to 0 s, allowing tick 0 up to
   half a cell before the audio" as the default bar-1 prior on dev, and compare it with utau-05's downbeat
   estimator. It is a one-line rule and costs nothing to run, so it is the oracle-cheap baseline 05 should
   beat.
3. Opt-in until holdout4 (utau-08), under a rule committed before the read.

## Cost (S/M/L) and how to tell it worked

S for the measurement and the prior (a few lines in the frame binary of utau-02). It worked if, on dev with sona's own phase, the frame pass rate
under the prior beats today's rule (which this table predicts is near 0/10, unverified), each failing
song is explained, and the bias check is written down either way.

## Comments
