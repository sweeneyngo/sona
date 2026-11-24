# Roadmap: the next frontiers

Where sona stands on 2026-11-24, and what would move it next. Each item says what the evidence is, what it
would cost, and how to tell whether it worked. Items are ranked by value for cost within each area; the
top three overall are at the end.

## Where things stand

- **Notes.** Cell accuracy against Pifuyuu's USTs:
  - 87% on dev, with GAME plus the octave rule;
  - **78.8%** on the last fresh test, holdout3, for the full stack, which includes the learned stage. It
    is in Rust and runs in the app and CLI where its weights exist (ADR-0026), on a grid phase from the
    vocals' onsets (ADR-0027);
  - 72.5% for pYIN grid mode, which is what a fresh install runs.

  Measured ceilings (ADR-0017): perfect sung/rest decisions would give 91.4%, and the best available pitch
  per cell 95.6%. Fine-tuning GAME on 30 songs gave +0.8 (not adopted). Every holdout is spent.
- **Edit cost** (the current target, below). On dev the shipped stack costs **180.2** weighted edits per
  minute [95%: 115.9, 256.5] and leaves **23.8%** of 4/4 measures clean [13.8, 33.9]; joined notes are 53%
  of the cost (`.scratch/utau-edit-cost/issues/01-edit-cost-metric.md`). The stage was trained on the dev
  songs, so these numbers flatter it.
- **Tempo and bar 1.** The user types the BPM; the estimate from the mix is a hint (ADR-0028). It is exact
  on 75% of fresh songs, within an octave on 97%, never at half (ADR-0022). Every export's tick 0 is a bar
  line of the editor, and the user can set bar 1 (ADR-0029). Bar 1 from the music is opt-in (ADR-0031).
- **Pitch.** Fitted glides and vibrato are 9–30% closer to the singer than flat notes (ADR-0019). They can
  be dragged on the roll, and the singer's own pitch exports as a `pitd` curve (ADR-0021).
- **App.** Drop a song, type its BPM, then Transcribe; it runs locally through `sona serve` (ADR-0020,
  ADR-0028). Undo/redo covers every edit (ADR-0025). Separated stems are cached, so a second run of the same
  song skips separation (ADR-0030).

## 1. Transcription

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 1.1 | ✅ **Done (ADR-0026).** **Port the learned cell stage to Rust** (ONNX through `ort`) so the 78.8% stack ships in the app and CLI | It confirmed +2.2 on holdout3 but exists only as a Python prototype | M | Bit-for-bit parity with the prototype on dev, then the same dev numbers |
| 1.2 | **Fresh test songs (holdout4)**: older songs from Pifuyuu's OneDrive share, which lists 460 UST zips of which 42 are used (not BowlRoll: its terms forbid mechanical collection). Ticket: utau-edit-cost 08 | Every split is spent, so nothing new can be confirmed | M | A manifest with ≥ 20 new aligned songs and a pre-registered read |
| 1.3 | **Sung vs rest** (the 91.4% ceiling): breath and consonant gaps, his note-end conventions | It is the largest gap between today and a ceiling | M–L | Dev cells, then holdout4 |
| 1.4 | **More training data for GAME**: unfreeze the encoder; train on all aligned songs once holdout4 exists | Fine-tuning on 30 songs with the encoder frozen gave +0.8 (within noise) | L (CPU days) | The pre-registered rule from the fine-tune spec |
| 1.5 | **Doubled voices and harmonies**: choose the lead when two lines overlap | 3–4% of sung cells are "harmony" errors | M | The `poly_cells` breakdown on dev |
| 1.7 | **Train the cell stage from Rust features**: dump the port's features with labels, so retraining no longer needs the prototype's Python GAME runner | The port's features equal the prototype's exactly (ADR-0026) | S–M | A model retrained on Rust features matches the prototype on dev |
| 1.8 | ~~**Better grid-phase estimation**~~ **Done** ([ADR-0027](adr/0027-grid-phase-from-onsets.md)): the phase from the vocals' onsets is within 0.1 cell on 29/30 held-out songs (was 14/30), and the stage scores 78.84% on holdout3 (true phase 78.83%) | — | — | Open: whether onsets help sona's own grid decoder too |
| 1.6 | **Lyrics in the loop** (phrase anchoring) | Perfect UST onsets plus pauses would give 90.8%, but real alignment captured none of it | L | Dev cells; abandon if the second idea also fails |

## 2. Tempo and grid

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 2.1 | **Choose the octave** (½× or 1×) from the vocal's note lengths after transcription, and offer it as a one-click "sounds too fast?" hint | 7 of 8 test misses are slow songs read at double; on dev, note length separates them only partly (8/10 with oracle notes) | S–M | Exact tempo on a fresh set, ≥ 80% |
| 2.2 | **Tempo changes and time signatures**: detect a tempo map, export it to UST/USTX | Today every song has one tempo | M | Corpus songs with tempo changes (none yet: needs data) |
| 2.3 | ~~**Grid phase from the mix** (downbeats)~~ **Done, opt-in** ([ADR-0031](adr/0031-bar-one-from-the-music.md)): the beat from the mix, bar 1 from harmonic change; dev frame pass 5/9 → 9/9, but circular for the bar | No labelled bar 1 exists to confirm it | — | Open: the default needs ear-labelled frames on holdout4 (utau-edit-cost 08) |

## 3. Pitch controls

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 3.1 | **Ignore consonant dips** in the fit and in `pitd` (voicing plus a spectral consonant cue) | Most of the remaining fit error sits at note edges | M | Curve error on held-out songs |
| 3.2 | **Smooth and simplify `pitd`** with an adjustable amount, so the curve is editable in OpenUtau | It is written every 5 ticks and simplified to 2 cents | S | Point count against curve error |
| 3.3 | **Point shapes** (in/out/linear) on the roll, plus multi-note pitch edits | Only the shapes from the fit exist; edits are one note at a time | S | Browser driver plus property tests |
| 3.4 | **MIDI pitch bends** from `renderPitch` | MIDI export carries notes, lyrics and a 4/4 time signature (ADR-0029), no pitch | S | Round-trip through a MIDI reader |

## 4. App and UI

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 4.1 | ✅ **Done (ADR-0025).** **Undo/redo** for every edit | There is none: a mis-drag is permanent until a reload | M | Property test: undo after any edit sequence restores the state exactly |
| 4.2 | **Box-select and multi-note edits** (move, transpose, delete) | Edits are one note at a time | M | Browser drivers |
| 4.3 | **A packaged app**: one download with `sona serve`, the separators and a model picker; no Python setup | Setup needs two Python venvs, ffmpeg and ORT today | L | A clean machine goes from install to transcription without a terminal |
| 4.4 | **Live audio preview with a singing synth voice** instead of the sine synth | The notes are heard as a plain tone | M | — |
| 4.5 | **Mobile and touch** (pinch-zoom, drag handles by touch) | Desktop mouse only | M | Touch drivers |

## 5. Vocal separation

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 5.1 | **Compare separators on the corpus** (htdemucs_ft, MDX23C, BS-RoFormer, Mel-RoFormer karaoke) by cell accuracy downstream | Only htdemucs + UVR_MDXNET_KARA_2 has been measured | M (GPU helps) | Dev cells per separator, then holdout4 |
| 5.2 | **Faster separation**: GPU when present, a quality/speed choice (cache by file hash: done, ADR-0030; re-transcribe from the editor without re-sending the song: done, ADR-0032) | Separation dominates the 76 s run on CPU | S–M | Wall time per minute of audio |
| 5.3 | **Harmony and backing stems** as extra tracks (Pifuyuu's USTs have them) | Only the lead is transcribed | L | Cells on the corpus's `cho`/harmony USTs |

## 6. Reproducibility, logging and operations

| # | Frontier | Evidence | Cost | How to tell |
|---|---|---|---|---|
| 6.1 | ✅ **Done (ADR-0023).** **Seed GAME's segmenter** in sona (fixed `seed` on its random node at load) so a song always gives the same notes | Unseeded runs vary by about 2 notes in 70, and by 0.3–0.7 points of cell accuracy | S | Same notes on reruns; seeded dev numbers recorded |
| 6.2 | ✅ **Done (ADR-0024).** **Structured job logs**: per-phase timings, tool versions, model hashes and the seed in each job, with a log download in the app | The job log is a tail of child output | S | Every job's log answers "what produced this?" |
| 6.3 | **Errors a user can act on**: missing separators, ffmpeg and first-use downloads, and tempo errors now say what to do (audit A12). Left: why GAME or the cell model is greyed out, out of disk | Some errors still surface as raw tool output | S | Fault-injection tests in `crates/sona-cli/tests/serve.rs` |
| 6.4 | **Browser drivers in CI.** The twelve editor drivers run in CI (job `e2e`, `scripts/ci/e2e.sh`: a Vite dev server, the synthetic fixture). Left: `song_to_notes`, `cancel_and_retry` and `retranscribe` against `sona serve` with stub separators | The three serve drivers run locally only (`web/e2e/`) | S–M | All three green in CI |
| 6.5 | **Eval dashboards**: per-song history across commits for cells, tempo and curve error | Numbers live in reports and commit messages | M | — |

## Current effort (2026-11-24): edit cost in OpenUtau

The target moved from cell accuracy to **how hard sona's mistakes are to fix in OpenUtau**: pitch and
length errors are cheap there, while a wrong BPM or notes in the wrong measures are the hardest fixes.
Spec, map and tickets: [`.scratch/utau-edit-cost/`](../.scratch/utau-edit-cost/spec.md). It reorders
the list above: 2.3 (beat 1 of the measure) and chopped/joined notes come before more cell-accuracy work.
Done so far: the metric (utau 01), frame metrics (02, 10), the BPM first (04, ADR-0028), bar 1 from the
music, opt-in (05, ADR-0031). Next: the baseline taxonomy (03), chopped and joined notes (07; joined is half
the cost), holdout4 (08).

## Top three overall

1. ~~1.1, port the learned stage~~: done (ADR-0026); the app runs the 78% stack where its weights exist.
2. ~~6.1 + 6.2, seed GAME and log provenance~~: done (ADR-0023, ADR-0024).
3. **1.2, holdout4**: nothing else in section 1 can be confirmed without it.

After those: ~~4.1 (undo)~~ (done), then 2.1 (tempo octave) to remove the remaining manual step.
