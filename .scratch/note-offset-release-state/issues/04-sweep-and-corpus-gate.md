# 04 — Eval: Release sweep dimension + the corpus gate

Status: done
Blocked by: 03
Phase: gate

The Release defaults are eye-plausible placeholders. ADR-0010/0011 discipline requires proving them on the harness before they become authoritative and ADR-0012 moves Proposed → Accepted.

- Extend `sona-eval-sweep` with a Release dimension over `release_offset_bonus × release_drop_threshold` (and optionally `stable_to_release_frac`, `release_self`), caching upstream pYIN + onsets + envelope once per stem as the existing sweep does.
- Run the corpus path (`--corpus`) — **requires the git-ignored Vocadito set** (`crates/sona-eval/scripts/fetch-vocadito.sh`), which is not present in a fresh checkout.
- Record the result in `reports/note-hmm-sweep.md` (or a sibling), reporting the **COnPOff-F1 delta vs the recorded baseline** alongside COnP-F1 and the staccato fixture.

Gate (all three, per ADR-0012):
1. COnPOff-F1 improves on Vocadito.
2. COnP-F1 does not regress.
3. The staccato golden fixture does not regress.

If it passes, set the swept values as the `HmmParams` / `NoteHmm` defaults and flip ADR-0012 to Accepted. If it does not, the Release path stays available but its default fractions/threshold are set so the decay path is conservative (or disabled via `release_drop_threshold > 1.0`). *(Written pre-gate; it did not pass — see Comments for the outcome and the honest read.)*

Note: this needs the corpus fetched and a human to run/read the sweep — hence `ready-for-human`. The code (01–03) is complete and default-active behind these placeholder knobs.

## Comments

**Done.** Added `sona-eval-release-sweep` (`src/bin/release_sweep.rs`) — a clean A/B on the identical pipeline, Release off (baseline) vs on across `release_offset_bonus × release_drop_threshold`. Fetched Vocadito (40 stems, md5-verified) and ran `--corpus`.

**Result (`reports/release-sweep.md`): gate FAILS on Vocadito.** Baseline (Release off) reproduces the recorded ADR-0010 numbers exactly (COnP 0.646 / COnPOff 0.508), validating the A/B. **No setting improves COnPOff** — best (bonus 12 / threshold 0.65) is neutral (Δ −0.000, COnP +0.010); eager settings hurt (COnPOff → 0.417). Staccato fixture holds 1.000 throughout.

Honest read (not "Vocadito is the wrong distribution"): the mechanism can only tighten an offset that ends in a **sharp RMS fall**, and most singing — including the real ripped vocals sona targets — ends with soft decays / breaths / pitch-transitions instead. So the flag misfires on mid-note dynamics and clips notes. That's an intrinsic ceiling of an energy-fall-driven release; Vocadito, being real expressive singing, is a fair test of it.

**Action taken (the "does not pass" branch):** shipped the Release path **OFF** by default — `NoteHmm::release_drop_threshold = f32::INFINITY`, so default behaviour is exactly ADR-0010 — because the evidence says a pure-DSP energy-fall release isn't the offset fix. Capability + knobs retained and unit-tested (harmless off; may still help genuinely sharp-cutoff input); enable defaults set to the sweep's least-harmful (`release_offset_bonus = 12`, enable `release_drop_threshold ≈ 0.65`). ADR-0012 → Accepted-but-off. The offset bottleneck stays open for a method that models note ends from more than an energy fall.
