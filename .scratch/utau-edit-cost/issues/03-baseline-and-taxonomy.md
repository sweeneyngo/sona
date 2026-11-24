# 03 — Baseline report and error breakdown

Status: needs-triage
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
