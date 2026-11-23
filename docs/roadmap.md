# Roadmap: the next frontiers

Where sona stands on 2026-11-23, and what would move it next. Each item says what the evidence is, what it
would cost, and how to tell whether it worked. Items are ranked by value for cost within each area; the
top three overall are at the end.

## Where things stand

- **Notes.** Cell accuracy against Pifuyuu's USTs:
  - 87% on dev, with GAME plus the octave rule;
  - **78.8%** on the last fresh test, holdout3, for the full stack, which includes a learned stage that
    exists only in Python;
  - 72.5% for the default pYIN grid mode.

  Measured ceilings (ADR-0017): perfect sung/rest decisions would give 91.4%, and the best available pitch
  per cell 95.6%. Fine-tuning GAME on 30 songs gave +0.8 (not adopted). Every holdout is spent.
- **Tempo.** Suggested from the mix: exact on 75% of fresh songs, within an octave on 97%, never at half
  (ADR-0022).
- **Pitch.** Fitted glides and vibrato are 9–30% closer to the singer than flat notes (ADR-0019). They can
  be dragged on the roll, and the singer's own pitch exports as a `pitd` curve (ADR-0021).
- **App.** Drop a song, then Transcribe; it runs locally through `sona serve` (ADR-0020, ADR-0022).

## 1. Transcription

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 1.1 | **Port the learned cell stage to Rust** (ONNX through `ort`) so the 78.8% stack ships in the app and CLI | It confirmed +2.2 on holdout3 but exists only as a Python prototype | M | Bit-for-bit parity with the prototype on dev, then the same dev numbers |
| 1.2 | **Fresh test songs (holdout4)**: a BowlRoll fetcher for the rest of Pifuyuu's USTs | Every split is spent, so nothing new can be confirmed | M | A manifest with ≥ 10 new aligned songs and a pre-registered read |
| 1.3 | **Sung vs rest** (the 91.4% ceiling): breath and consonant gaps, his note-end conventions | It is the largest gap between today and a ceiling | M–L | Dev cells, then holdout4 |
| 1.4 | **More training data for GAME**: unfreeze the encoder; train on all aligned songs once holdout4 exists | Fine-tuning on 30 songs with the encoder frozen gave +0.8 (within noise) | L (CPU days) | The pre-registered rule from the fine-tune spec |
| 1.5 | **Doubled voices and harmonies**: choose the lead when two lines overlap | 3–4% of sung cells are "harmony" errors | M | The `poly_cells` breakdown on dev |
| 1.6 | **Lyrics in the loop** (phrase anchoring) | Perfect UST onsets plus pauses would give 90.8%, but real alignment captured none of it | L | Dev cells; abandon if the second idea also fails |

## 2. Tempo and grid

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 2.1 | **Choose the octave** (½× or 1×) from the vocal's note lengths after transcription, and offer it as a one-click "sounds too fast?" hint | 7 of 8 test misses are slow songs read at double; on dev, note length separates them only partly (8/10 with oracle notes) | S–M | Exact tempo on a fresh set, ≥ 80% |
| 2.2 | **Tempo changes and time signatures**: detect a tempo map, export it to UST/USTX | Today every song has one tempo | M | Corpus songs with tempo changes (none yet: needs data) |
| 2.3 | **Grid phase from the mix** (downbeats), not only from the vocal | The phase is estimated from vocal onsets today | S | Phase error against the UST's alignment offset |

## 3. Pitch controls

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 3.1 | **Ignore consonant dips** in the fit and in `pitd` (voicing plus a spectral consonant cue) | Most of the remaining fit error sits at note edges | M | Curve error on held-out songs |
| 3.2 | **Smooth and simplify `pitd`** with an adjustable amount, so the curve is editable in OpenUtau | It is written every 5 ticks and simplified to 2 cents | S | Point count against curve error |
| 3.3 | **Point shapes** (in/out/linear) on the roll, plus multi-note pitch edits | Only the shapes from the fit exist; edits are one note at a time | S | Browser driver plus property tests |
| 3.4 | **MIDI pitch bends** from `renderPitch` | MIDI export is notes only | S | Round-trip through a MIDI reader |

## 4. App and UI

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 4.1 | **Undo/redo** for every edit | There is none: a mis-drag is permanent until a reload | M | Property test: undo after any edit sequence restores the state exactly |
| 4.2 | **Box-select and multi-note edits** (move, transpose, delete) | Edits are one note at a time | M | Browser drivers |
| 4.3 | **A packaged app**: one download with `sona serve`, the separators and a model picker; no Python setup | Setup needs two Python venvs, ffmpeg and ORT today | L | A clean machine goes from install to transcription without a terminal |
| 4.4 | **Live audio preview with a singing synth voice** instead of the sine synth | The notes are heard as a plain tone | M | — |
| 4.5 | **Mobile and touch** (pinch-zoom, drag handles by touch) | Desktop mouse only | M | Touch drivers |

## 5. Vocal separation

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 5.1 | **Compare separators on the corpus** (htdemucs_ft, MDX23C, BS-RoFormer, Mel-RoFormer karaoke) by cell accuracy downstream | Only htdemucs + UVR_MDXNET_KARA_2 has been measured | M (GPU helps) | Dev cells per separator, then holdout4 |
| 5.2 | **Faster separation**: GPU when present, a quality/speed choice, cache by file hash | Separation dominates the 76 s run on CPU | S–M | Wall time per minute of audio |
| 5.3 | **Harmony and backing stems** as extra tracks (Pifuyuu's USTs have them) | Only the lead is transcribed | L | Cells on the corpus's `cho`/harmony USTs |

## 6. Reproducibility, logging and operations

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 6.1 | **Seed GAME's segmenter** in sona (fixed `seed` on its random node at load) so a song always gives the same notes | Unseeded runs vary by about 2 notes in 70, and by 0.3–0.7 points of cell accuracy | S | Same notes on reruns; seeded dev numbers recorded |
| 6.2 | **Structured job logs**: per-phase timings, tool versions, model hashes and the seed in each job, with a log download in the app | The job log is a tail of child output | S | Every job's log answers "what produced this?" |
| 6.3 | **Errors a user can act on**: a missing separator or model, an unreadable file, out of disk, each with the fix | Some errors still surface as raw tool output | S | Fault-injection tests in `tests/serve.rs` |
| 6.4 | **Browser drivers in CI** (Playwright against `sona serve` with stub separators) | They run locally only (`web/e2e/`) | M | Green in CI |
| 6.5 | **Eval dashboards**: per-song history across commits for cells, tempo and curve error | Numbers live in reports and commit messages | M | — |

## Top three overall

1. **1.1, port the learned stage**: it puts the confirmed 78.8% stack into the app.
2. **6.1 + 6.2, seed GAME and log provenance**: cheap, and every later measurement depends on them.
3. **1.2, holdout4**: nothing else in section 1 can be confirmed without it.

After those: 4.1 (undo) for usability, then 2.1 (tempo octave) to remove the remaining manual step.
