# 03 — Baseline report and error breakdown

Status: resolved
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
- 2026-11-24 (coordinator): `cellstage-D` was trained on all 28 dev + holdout + holdout2 songs, so its numbers
  there are in-sample, dev included. Base the taxonomy, oracles and ranking on GAME lead + octave, with the
  stage beside it, and say whether the joined/chopped shares differ.

## Answer

Resolved 2026-11-24 on branch `utau-03-taxonomy` (from 3a63a9e). Full report: `crates/sona-eval/reports/edit-cost-baseline.md`.
Spent splits read once each as diagnostic re-reads at 0217da0 (three ledger entries); they confirm nothing.
`cellstage-D` is in-sample on dev, holdout and holdout2 and out of sample only on holdout3; GAME lead + octave
(not trained on the corpus) is the primary reference.

**GAME cache.** `sona-eval-cellstage` caches both GAME runs and the pYIN grid notes (keyed like ust-dev's:
song, stem fingerprints, GAME folder, GAME settings incl. seed, transcriber settings). Dev: cold 2 h 57 min,
warm 68 s, outputs byte-identical and equal to 01's numbers. `--dump DIR` writes per-song detail for offline
analysis; `crates/sona-eval/scripts/edit-cost-taxonomy/` reads it (its edit-cost port matches Rust's counts on
every song).

**Headline (cost/min, clean measures):**

| split | GAME lead + octave | learned stage |
|---|---|---|
| dev | 188.7, 16.2% | 180.2, 23.8% (in-sample) |
| holdout | 139.8, 15.0% | 132.9, 19.8% (in-sample) |
| holdout2 | 182.3, 8.2% | 178.9, 12.3% (in-sample) |
| holdout3 | 161.1, 14.8% | 156.3, 17.1% (out of sample) |

**Joined is the largest class on every split** (GAME lead 51.5% of dev cost; 39.7 / 36.0 / 32.0% on the spent
splits; the stage 1–4 points higher). It is the UST's one-note-per-mora writing:
- 62–78% of hidden notes have the previous note's pitch, and 60–82% of those are a bare vowel or ん with no
  consonant to hear;
- the UST's same-pitch boundaries are hidden at ん 53–79%, bare vowel 49–57%, consonant 7–18%;
  different-pitch boundaries 3–6%;
- hidden notes have a median of 92–125 ms; joined cost/min follows notes/s (Spearman +0.68 to +0.87 by split).
  amaten and atamaro are the fastest dev songs, but the same mechanism runs at lower rates everywhere
  (without them joined is still 51% of dev cost);
- no current source proposes most hidden boundaries (any source exactly there: 26–37%), so better selection
  fixes 10–16% of cost against a 32–53% joined oracle.

**Chopped** (12–24% of GAME lead's cost): same-pitch restarts by GAME 29–41%, 1–2 semitone steps 33–38%,
one-cell heads 17–28%, octave flips 0–3%. The learned stage chops more on every split and adds octave flips.

**Joined/chopped shares by stack.** Same ranking, small shifts: the stage's joined share is 1.3 (dev) to 3.5
points higher and its chopped share 4–9 points higher, because it removes missing cost and adds cuts of its own.

**Oracles** (GAME lead, cost removed; dev / holdout / holdout2 / holdout3): joined 52.5 / 40.2 / 36.5 / 32.4%;
joined or chopped 64 / 63 / 50 / 44%; pair errors 22 / 19 / 21 / 26%; chopped 14 / 24 / 14 / 13%; missing
11 / 14 / 25 / 24% (on the shipped stage 8 / 7 / 18 / 12%); extra 1–4%.

**Calibration.** Today's confidence is flat for cell-stage notes (91–99% score 0.60–0.65: fixed inputs from
`grid_segments`); its lowest 30% catch 30–32% of cost, like random flags. Stage probability catches 44–48%,
longest-notes-first 60–78% of joined cost. Highlighting by today's score would not catch the expensive errors.

**Ranking:**
1. **07, re-scoped to splitting notes at moras** (lyrics give the mora count when pasted; otherwise a
   same-pitch vowel/ん change detector). Largest oracle on every split. First measure the lyric oracle.
2. 07's chopped half (merge same-pitch restarts and 1–2 st wobbles).
3. New: boundary placement and pitch of one-to-one pairs (19–26%).
4. 06 after those, not dropped (shipped-stage ceiling 7–18%, realistic at most half, risk of joins).
5. Replace the cell-stage confidence before any doubtful-note UI.
05 is a frame fix (bar 1), outside the cost metric; its rank follows the frame numbers (02/10), not these.
