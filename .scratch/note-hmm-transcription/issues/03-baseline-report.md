# 03 — Baseline report for the current heuristic segmenter

Status: ready
Blocked by: 02
Phase: A (harness) — **the gate**

Run the harness over Vocadito + golden fixtures against the **current ADR-0007 pipeline** (pYIN → energy onsets → heuristic segmentation → confidence). Record the numbers as the baseline every Phase-B change must beat.

- Full per-stage table: RPA/RCA, onset-F1, note-F1 (COnP/COnPOff), confidence calibration.
- **Answer the standing question**: is the "lack of notes" an onset-recall problem (stage 2) or over-merging in segmentation (stage 3)? The onset-F1 vs note-F1 split should say which.
- Commit the baseline report to this ticket's comments (and/or a checked-in `sona-eval/reports/baseline.md`).

Done when: baseline numbers are recorded and the onset-vs-segmentation attribution is called. **Phase B does not start landing until this is done.**
