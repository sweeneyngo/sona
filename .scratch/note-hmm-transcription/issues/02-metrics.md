# 02 — Per-stage metrics (mir_eval conventions, pure Rust)

Status: done
Blocked by: 01
Phase: A (harness)

Implement the metric families, per stage, mirroring `mir_eval` so numbers compare to the literature (ADR-0011).

- **Pitch / f0:** Raw Pitch Accuracy, **Raw Chroma Accuracy** (octave-insensitive), Voicing Recall / False Alarm. ±50-cent tolerance.
- **Onsets:** Precision / Recall / F1 at **±50 ms**.
- **Notes:** P/R/F1 as **COnP** (onset+pitch) and **COnPOff** (+offset). ±50 ms onset, ±50-cent pitch, offset within 50 ms or 20 % of duration.
- **Confidence calibration:** bucket notes by confidence scalar, report note error rate per bucket (does the flag predict the wrong notes?).

Reimplement in pure Rust. **Validate once, offline, against `mir_eval`** on a couple of fixtures and record the agreement in the comments.

Done when: each metric is unit-tested against hand-worked small cases and cross-checked against `mir_eval` on a fixture; the harness can print a full per-stage report for one input.

## Comments

**Done.** Per-stage metrics in a new `crates/sona-eval/src/metrics/` module, pure Rust, each family mirroring `mir_eval`:

- **pitch** (`melody`): RPA, RCA (octave-folded), Voicing Recall / False Alarm, ±50 cents. Scores two f0 contours **already on a shared time grid** — `mir_eval`'s identity-resample path — with resampling a producer's hop onto the reference hop left to the caller (ticket 03).
- **onset** (`onset.f_measure`): P/R/F1 at ±50 ms. Uses interval containment (`est−w ≤ ref ≤ est+w`), matching `mir_eval`'s searchsorted form, so an onset exactly at the window edge still counts.
- **note** (`transcription`): P/R/F1 as **COnP** (offset ignored) and **COnPOff** (offset within `max(20 % dur, 50 ms)`). Pitch distance for integer MIDI is `100·|Δsemitone|`, so ±50 cents ⇒ exact-pitch match.
- **confidence calibration** (sona-specific, no `mir_eval` analogue): buckets est notes by confidence, reports per-bucket note error rate against the COnP matching. Unit-tested only.

Onset and note "correctness" is the size of a **maximum bipartite matching** (Kuhn augmenting paths in `matching.rs`), matching `mir_eval`'s Hopcroft–Karp cardinality — an estimate can't be double-counted against two references.

New types: `Estimate`/`EstNote` (like `Reference` but with per-note confidence); `Estimate::from_reference` is the identity estimate. `metrics::evaluate` returns a `StageReport`; `StageReport::format` renders the table the report bin now prints for each fixture (identity estimate ⇒ all-1.0 until a real producer lands in ticket 03).

**mir_eval cross-check (once, offline; `mir_eval` 0.8.2).** A throwaway harness emitted 6 (reference, estimate) cases — 2 hand-built + all 4 golden fixtures degraded — with sona's numbers, then fed the identical inputs to `mir_eval`. **Every metric agreed to 0.0 abs diff** (worst across all cases: `0.00e+00`). The hand-built cases are non-trivial, exercising the tricky paths:

- `pitch_handbuilt`: RPA 0.500, RCA 0.667 (octave error forgiven by chroma only), VR 0.833, VFA 0.500.
- `notes_handbuilt`: COnP F1 0.571 vs COnPOff F1 0.286 (offset constraint bites), onset F1 0.571, with two estimates contending for one reference note.

The harness was not committed (keeps the crate Python-free, as ticket 01 did); the committed unit tests are hand-worked small cases. 44 crate tests run offline in `cargo test`; full workspace green; clippy clean.
