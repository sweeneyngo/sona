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

**Baseline — Vocadito (40 real vocal stems; fetched via `scripts/fetch-vocadito.sh`) + golden fixtures.** This is the gate: **COnP-F1 0.515 / COnPOff-F1 0.348** on the corpus.

| data | RPA | RCA | onset F1 | COnP F1 | COnPOff F1 |
|---|---|---|---|---|---|
| **Vocadito (mean, n=40)** | 0.858 | 0.863 | 0.565 | **0.515** | **0.348** |
| fixture legato | 0.925 | 0.925 | 0.000 | 0.000 | 0.000 |
| fixture staccato | 0.874 | 0.874 | 1.000 | 1.000 | 1.000 |
| fixture vibrato | 0.942 | 0.942 | 1.000 | 1.000 | 1.000 |
| fixture bleed | 0.961 | 0.961 | 1.000 | 1.000 | 1.000 |

**Standing question — onsets (stage 2) or segmentation (stage 3)? → not segmentation over-merging; the ceiling is the pitch-contour→note step, worst at offset.** The onset metric scores the **detector's raw onsets** (`OnsetDetector::detect`) against reference note onsets, independent of the emitted notes, so `note-drop = onset-recall − COnP-recall` compares two independent note-birth signals. On the corpus `note-drop` is **negative** (mean −0.101; negative on 33/40 stems, positive on only 4 and ≤0.08): note recall (0.542) *exceeds* onset recall (0.441) because the plateau segmenter births notes from the pitch contour, recovering notes the energy onset detector never flags — so segmentation is net-additive, not where notes vanish. Pitch is solid (RPA 0.858) with negligible octave error (RCA−RPA 0.006, so ADR-0010's octave-error motivation is only weakly supported here). The biggest headroom is **offset** (COnPOff-F1 0.348 ≪ COnP-F1 0.515). The **fixtures showed the opposite** (`note-drop 0.000`, legato losing all notes at onset-recall 0.000), but that's a synthetic-cleanliness artifact — their notes are born one-to-one from onsets so the signals can't diverge; the corpus is the trustworthy read. This aims Phase B (note-HMM replacing the plateau segmenter) at the load-bearing pitch-contour→note step, with note-boundary/offset precision as the target; the weak energy onset detector (onset-F1 0.565) is a real but secondary finding.

**Harness fidelity (mir_eval 0.8.2, offline):** pitch metric matches mir_eval **exactly (0.0 abs diff)** when fed sona's aligned estimate; full resampler+metric agrees to **≤ 0.078**, always with mir_eval higher — the documented tail-extrapolation difference (pYIN's 2048-window can't reach the last ~8 frames; sona marks them unvoiced, which is uniformly stricter and applied to every producer, so comparisons stay fair). 51 crate tests run offline; harness scripts not committed (crate stays Python-free).

**Note (out of scope, for Phase B):** the legato onset miss is a live, informative baseline defect — the energy onset detector doesn't birth a note for a soft/legato attack. ADR-0010's note-HMM segments over the pitch contour and can create boundaries from pitch/energy, so it is well-positioned to recover exactly these notes; that improvement will show up as legato onset/note-F1 rising above 0.000 without regressing staccato.
