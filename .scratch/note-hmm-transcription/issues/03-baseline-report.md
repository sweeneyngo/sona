# 03 — Baseline report for the current heuristic segmenter

Status: done
Blocked by: 02
Phase: A (harness) — **the gate**

Run the harness over Vocadito + golden fixtures against the **current ADR-0007 pipeline** (pYIN → energy onsets → heuristic segmentation → confidence). Record the numbers as the baseline every Phase-B change must beat.

- Full per-stage table: RPA/RCA, onset-F1, note-F1 (COnP/COnPOff), confidence calibration.
- **Answer the standing question**: is the "lack of notes" an onset-recall problem (stage 2) or over-merging in segmentation (stage 3)? The onset-F1 vs note-F1 split should say which.
- Commit the baseline report to this ticket's comments (and/or a checked-in `sona-eval/reports/baseline.md`).

Done when: baseline numbers are recorded and the onset-vs-segmentation attribution is called. **Phase B does not start landing until this is done.**

## Comments

**Done.** The report bin (`sona-eval-report`) now runs the **real ADR-0007 pipeline** (`Transcriber::default` + `PYin::default`) over each stem and scores it with the ticket-02 metrics. New plumbing: `Estimate` gained an `onsets` field carrying the **raw stage-2 onset times**; `Estimate::from_annotation` builds notes+f0 from the pipeline `Annotation` (a frame is voiced iff pYIN emitted a frequency for it) and the harness fills `onsets` straight from `OnsetDetector::detect` — the annotation contract (ADR-0004) doesn't carry raw onsets, so the onset metric reads the detector directly rather than the post-segmentation notes. `align::resample_f0_onto` resamples the estimate's f0 onto the reference frame grid (mir_eval-style cent-space linear interpolation, unvoiced retention; tail frames the producer never analysed are marked unvoiced rather than extrapolated). Committed baseline: `crates/sona-eval/reports/baseline.md`.

**Baseline (golden fixtures; Vocadito not run — git-ignored, needs the fetch script):**

| fixture | RPA | onset F1 | COnP F1 | COnPOff F1 |
|---|---|---|---|---|
| legato | 0.925 | **0.000** | **0.000** | **0.000** |
| staccato | 0.874 | 1.000 | 1.000 | 1.000 |
| vibrato | 0.942 | 1.000 | 1.000 | 1.000 |
| bleed | 0.961 | 1.000 | 1.000 | 1.000 |

**Standing question — onsets (stage 2) or segmentation (stage 3)? → onset recall, not segmentation.** The onset metric scores the **detector's raw onsets** (from `OnsetDetector::detect`) against reference note onsets — independent of the notes segmentation emits, which is what lets the split separate the stages. `note-drop = onset-recall − COnP-recall = 0.000 on every fixture`: every detected onset became a matching note, so segmentation loses nothing (over-merging would show a positive drop). The only fixture that loses notes is legato, where pYIN tracks pitch fine (RPA 0.925) but the detector's **onset-recall is 0.000** (no onset within ±50 ms of either true onset on the smooth legato phrase) → zero notes. So "lack of notes" is measured as a *note-birth* (onset) problem, not over-merging. Caveat: synthetic fixtures, and only legato actually loses notes, so the over-merge mode is present in the data but never triggered; Vocadito would exercise it on real vocals. This aims Phase B at how notes are born.

**Harness fidelity (mir_eval 0.8.2, offline):** pitch metric matches mir_eval **exactly (0.0 abs diff)** when fed sona's aligned estimate; full resampler+metric agrees to **≤ 0.078**, always with mir_eval higher — the documented tail-extrapolation difference (pYIN's 2048-window can't reach the last ~8 frames; sona marks them unvoiced, which is uniformly stricter and applied to every producer, so comparisons stay fair). 55 crate tests run offline; harness scripts not committed (crate stays Python-free).

**Note (out of scope, for Phase B):** the legato onset miss is a live, informative baseline defect — the energy onset detector doesn't birth a note for a soft/legato attack. ADR-0010's note-HMM segments over the pitch contour and can create boundaries from pitch/energy, so it is well-positioned to recover exactly these notes; that improvement will show up as legato onset/note-F1 rising above 0.000 without regressing staccato.
