# 03 — Baseline report and error breakdown

Status: ready-for-agent
Type: research
Blocked by: 01, 02

## What

Score the current default stack (`cells` engine, GAME + octave + learned stage) and the pYIN grid default
with 01 and 02, then write `crates/sona-eval/reports/edit-cost-baseline.md`:

- dev numbers; then the three spent splits as **diagnostic re-reads** (`--split NAME --reread --reason
  "edit-cost baseline, diagnosis only"`), logged in the ledger, labelled in the report as confirming
  nothing;
- weighted cost per minute, clean measures, frame pass rate, cell accuracy side by side;
- cost by error class × song, so the largest classes are obvious;
- an **oracle per class** (cost if that class were fixed perfectly), to rank tickets 05–07;
- confidence calibration: error rate per confidence bucket.

## Output

A ranking that says which of 05, 06, 07 to do first, and whether any should be dropped. Update `map.md`.

## Comments

- 2026-11-24 (after 01): the dev baseline exists (01's Answer). Still needed here: (1) **why notes are
  joined**: share where the hidden UST notes have the same pitch (re-articulation) vs a different pitch,
  by song, and whether Pifuyuu's one-note-per-mora writing on fast songs explains amaten/atamaro;
  (2) an oracle per class; (3) the spent splits as logged diagnostic re-reads; (4) confidence
  calibration. `sona-eval-cellstage` took ~2 h on dev because it doesn't cache GAME: add a GAME cache
  keyed like `sona-eval-ust-dev`'s first.
