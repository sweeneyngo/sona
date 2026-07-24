# 02 — Per-stage metrics (mir_eval conventions, pure Rust)

Status: ready
Blocked by: 01
Phase: A (harness)

Implement the metric families, per stage, mirroring `mir_eval` so numbers compare to the literature (ADR-0011).

- **Pitch / f0:** Raw Pitch Accuracy, **Raw Chroma Accuracy** (octave-insensitive), Voicing Recall / False Alarm. ±50-cent tolerance.
- **Onsets:** Precision / Recall / F1 at **±50 ms**.
- **Notes:** P/R/F1 as **COnP** (onset+pitch) and **COnPOff** (+offset). ±50 ms onset, ±50-cent pitch, offset within 50 ms or 20 % of duration.
- **Confidence calibration:** bucket notes by confidence scalar, report note error rate per bucket (does the flag predict the wrong notes?).

Reimplement in pure Rust. **Validate once, offline, against `mir_eval`** on a couple of fixtures and record the agreement in the comments.

Done when: each metric is unit-tested against hand-worked small cases and cross-checked against `mir_eval` on a fixture; the harness can print a full per-stage report for one input.
