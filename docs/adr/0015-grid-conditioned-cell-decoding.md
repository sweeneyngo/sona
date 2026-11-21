# Grid-conditioned cell decoding for grid-locked material (opt-in)

**Status: Accepted.** It amends [ADR-0004](0004-core-emits-raw-perception-ui-interprets.md) for an opt-in mode only; the default path is unchanged.

## Context

The in-domain gate (`crates/sona-eval/reports/ust-in-domain-gate.md`) found that on the target material sona's largest gap is **recall** (COnP R 0.25), not offsets. The frame-level note-HMM ([ADR-0010](0010-note-hmm-segmentation.md)) emits about 250 notes per song where the UST has 500–800.

The dev-split miss taxonomy (`sona-eval-ust-dev --diag`) puts most of the loss on frame-level boundary placement:
- **Consonants.** A Japanese syllable's consonant is unvoiced but loud, so pYIN unvoices the start of the note or the whole cell.
- **Glides.** Vocaloid pitch transitions drag a detected onset 50–150 ms off the line.
- **The prune floor.** The ~100 ms floor drops one-cell notes, which make up 15% of the corpus.

Meanwhile 9,013 of the 9,082 UST onsets sit exactly on the song's 16th or triplet grid, and every BPM in the corpus is an integer the user can type.

Sweeping the existing HMM's knobs gained at most +0.04 F1. The structure was the limit, not the tuning.

## Decision

Add **`GridDecoder`** (`crates/sona-core/src/grid.rs`), a Viterbi over **grid cells** rather than frames. Each cell is in one of three states: rest, a new note at pitch *p*, or the continuation of the note at *p*.

- **Emission:** a capped Gaussian over each cell's voiced frames, plus a small cost per unvoiced frame. A rest pays per voiced frame in proportion to the frame's voicing and loudness.
- **Transitions:**
  - rest ↔ note is cheap;
  - a pitch change pays a base cost plus an interval term;
  - a same-pitch re-articulation pays a cost discounted by a voicing-dip and energy-dip cue at the boundary.
- **Tuning.** The stem's global tuning offset is removed first.
- **Phase.** The grid phase is estimated: the decoder runs at 24 phases across one cell, and the lowest per-cell cost wins.
- **Gap fill.** A one-cell rest between two notes is filled by the earlier note, because it is almost always the next syllable's consonant.

**Opt-in, never the default.** It is selected by `Transcriber::grid: Option<GridMode>`, `sona transcribe --grid 16|24`, and `sona-eval-ust-gate --grid`. The output is the same `NoteSegment` contract, so confidence scoring, the annotation JSON, and the CLI are untouched. The UI's auto-anchor (first onset) lands on the decoded phase, so its hard snap is the identity on this output.

**Parameters** were fit on the dev split only. The holdout was read once, against a decision rule pre-registered in `.scratch/grid-cell-decoder/spec.md` (commit `b5391bb`).

## Why this does not break ADR-0004's reasons

ADR-0004 keeps the core invariant to musical settings for two reasons. It lets the user re-grid live, and it lets the backend benchmark run the core once and re-grid freely. Both still hold for the default path, which remains the grid-free perception path. Grid mode is a separate, explicitly grid-conditioned producer for the case where the grid is known up front and the goal is a UST-like transcription.

The cost: a grid-mode annotation is tied to the BPM and division it was decoded with. Re-gridding it in the UI to a different tempo is meaningless, so re-run the core instead.

## Overfitting (as ADR-0012 requires)

- The method was chosen from the error taxonomy before any tuning, with grid lines as the structural prior.
- The 13 knobs were fit by coordinate search on 9 dev songs (about 5,300 notes).
- On the 9 untouched holdout songs, pooled quantized COnP F1 rose 0.340 → 0.455 (+0.114, 95% song bootstrap [+0.064, +0.167]) and COnPOff F1 rose 0.158 → 0.303 (+0.145 [+0.093, +0.183]). No holdout song regressed.
- The holdout gain matches the dev gain (+0.13 / +0.12), so the fit is not dev-specific.
- The one human-sung song (watched, never tuned) also improves (COnP 0.23 → 0.36). With n = 1 that is not evidence.

## Rejected alternatives

- **Re-tuning the note-HMM** (prune floor, voicing prior, onset threshold). Each gained ≤ 0.04.
- **Masking the frame HMM's note changes to grid lines.** This keeps the consonant and prune-floor losses, which are per-frame.
- **A spectral-envelope cue for same-pitch syllable changes.** AUC 0.69, no better than the voicing and energy dips (0.70–0.73).
- **An energy-weighted rest cost to bridge consonants inside the decode.** It births spurious notes in unvoiced cells. The one-cell gap fill gets the offset gain (COnPOff +0.05) without touching onsets.

## Consequences

- **Limits.** Grid mode needs an exact BPM and a single tempo, and the user picks 16 or 24. A wrong BPM drifts the grid; phase estimation only corrects a constant offset. BPM and division estimation remain future work, and the corpus already has golden labels for them.
- **Remaining gaps.** Recall is now 0.45 (0.61 in monophonic regions). What remains:
  - harmony and doubled voices in the stem;
  - same-pitch re-articulations, which no cue tried separates well;
  - stems where separation failed (BassSousha, 盲目シャボン).
- **Verified.** `verification/grid-notes/` model-checks the path → notes → gap fill → snap assembly with TLA+/TLC on every path up to 5 cells, with six mutants each caught. The local `verify-grid-notes` skill says how to rerun it.
- **Known finding.** The review surface's escape hatch (ADR-0003) can overlap two neighbours when the user drags the grid ≥ ¼ cell off the decoded phase. This predates grid mode, and the fix belongs in `quantizeNote`.
