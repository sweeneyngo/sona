# 01 — Weighted edit-cost metric

Status: resolved
Type: task
Blocked by: —

## What

Extend `sona_eval::correction` (`crates/sona-eval/src/correction.rs`, port of `web/lib/correction.ts`) into
an edit-cost scorer for transcription vs UST, weighted by the tiers in `../spec.md`:

- extra note 1, wrong pitch (any interval, octave included) 1, wrong length 1, single-note shift by up to
  one cell 1, missing note 2;
- **chopped** (one UST note covered by two or more output notes) 4 per extra piece, **joined** (one output
  note covering two or more UST notes) 4 per hidden note. Neither exists yet; define them on grid cells.
- report cost per minute, per-class counts, and **clean measures** (share of 4/4 measures with zero cost).

Add `--edit-cost` output to `sona-eval-ust-dev` (per song and pooled, song-bootstrap 95% interval), or a
new `sona-eval-edit-cost` binary if that is cleaner. Keep the weights in one table so they can change.

## Acceptance

- Unit tests on synthetic note sets: each class alone gives its exact weight; a known mix gives the sum.
- Break the chopped and joined detectors once each and show a test fails (rules.md: mutants caught).
- `web/lib/correction.ts` and the Rust scorer still agree on shared fixtures for the classes both have.
- Runs on `dev` only. No holdout read in this ticket.
- clippy -D warnings, `cargo test --workspace --release` green.

## Comments

## Answer

Resolved 2026-11-24 on branch `utau-01-edit-cost` (from d261e6b). Dev split only; the holdout ledger is
unchanged.

### What was built

- `sona_eval::correction::edit_cost` (`crates/sona-eval/src/correction/edit_cost.rs`):
  - `edit_cost(ust, output, CellGrid)` returns per-class counts, scored and clean measures, and the
    UST span;
  - `bootstrap` resamples songs (10 000 times, fixed seed);
  - `report` prints the per-song table and the pool.
- The weights live in one table, `WEIGHTS`: extra 1, pitch 1, length 1, shift 1, moved 1, missing 2,
  chopped 4, joined 4.
- **Opt-in flags:**
  - `sona-eval-ust-dev --edit-cost` scores each `--cfg`'s quantized notes.
  - `sona-eval-cellstage --edit-cost` scores three outputs: the pYIN grid default on the vocals, GAME
    lead + octave on the onset phase, and the learned cell stage on the onset phase (which ships).
  - Without the flag, both tools print exactly what they printed before. For ust-dev this was checked
    with and without the flag on one song: the diff is only the added block.
- Shared fixtures `crates/sona-eval/fixtures/correction/cases.json` (6 cases), asserted by
  `web/lib/correction.test.ts` and by `correction.rs`. They check three things:
  - `correction.ts` and `correction_cost` give the same counts;
  - the edit cost matches them on the classes both have: extra = deleted, missing = added,
    pitch = pitch fixed, length = offset moved;
  - no other edit-cost class fires.

### Definitions (also in the module doc)

**Cells.** A note becomes `[start, end)` in whole cells of the UST's grid, with
`cell = round((t − anchor) / spacing)` and anchor = UST tick 0. The pitch is rounded to an integer.
- A note that rounds to zero cells is dropped.
- Each side is made monophonic: an overlapped note is cut at the next note's start.
- Only the UST's span counts, from its first note's start to its last note's end. Output notes with
  no cell in the span are ignored.
- Rounding to the UST's cells forgives a phase error under half a cell. Larger phase errors belong
  to the frame metrics (02).

**Relations.** `shared(o, r)` is the number of cells an output note `o` and a UST note `r` have in
common.
- `o` **covers** `r` when `2·shared > len(r)`.
- `o` is a **piece of** `r` when `2·shared > len(o)`.

Both sides are monophonic, so each note has at most one of each relation. Every relation is an edge.
Two monophonic lines overlap without cycles, so the edges form a forest.

**Classes.** Each connected component is one of:
- **missing**: a lone UST note;
- **extra**: a lone output note;
- **a pair** (one UST note, one output note), which can carry any of these independently:
  - **pitch**: the pitches differ (any interval, octave included);
  - **length**: the lengths in cells differ;
  - **shift**: the starts differ by exactly 1 cell;
  - **moved**: the starts differ by 2 or more cells;
- **larger components**: a UST note with `d` edges is **chopped** `d − 1` times (extra pieces). An
  output note with `d` edges is **joined** `d − 1` times (hidden UST notes). Over a component that is
  `a + b − 2` in all. Pitch, length and shift are not scored inside these, because rebuilding covers
  them.

Consequences:
- A pure one-cell shift costs 1. A note one cell early that ends on time costs a shift plus a length.
- In legato, one boundary a cell late costs 3: a length on one note, and a shift plus a length on the
  next.
- A note only grazed by its neighbours (no majority either way) is missing, and the neighbours pay a
  length each.

**Moved is my assumption.** The tiers cover only one-cell shifts. I priced a bigger move at 1 (one drag
in OpenUtau, whatever the distance) and count it separately so the weight can change. It is 0–1% of
cost on dev.

**Measures.** A 4/4 measure is `grid_division` cells, with measure 0 starting at UST tick 0.
- A measure is **scored** when any note on either side has a cell in it.
- It is **clean** when every such note is in an error-free pair.

**Cost per minute** = weighted cost / UST span in minutes. The pool is the sum of costs over the sum
of minutes, with a song-bootstrap 95% interval. The clean share is pooled the same way.

### Dev numbers (9 songs, 27.5 min; `sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost`)

Cell accuracy in the same run: GAME lead on the onset phase 86.76%, learned stage on the onset phase
92.69%.

| output | cost/min [95%] | clean measures [95%] | extra | pitch | length | shift | moved | missing | chopped | joined |
|---|---|---|---|---|---|---|---|---|---|---|
| pYIN grid default (vocals) | 338.4 [257.3, 447.5] | 3.1% [1.1, 5.4] | 401 | 287 | 1063 | 387 | 63 | 467 | 1105 | 435 |
| GAME lead + octave | 188.7 [127.3, 262.1] | 16.2% [9.4, 23.2] | 70 | 203 | 717 | 218 | 17 | 312 | 167 | 667 |
| **learned stage (ships)** | **180.2 [115.9, 256.5]** | **23.8% [13.8, 33.9]** | 33 | 163 | 522 | 209 | 21 | 213 | 240 | 654 |

Share of the shipped stack's cost by class:

| joined | chopped | length | missing | shift | pitch | extra | moved |
|---|---|---|---|---|---|---|---|
| 53% | 19% | 11% | 9% | 4% | 3% | 1% | 0% |

Per song, for the shipped stack:

| song | cost/min | clean measures |
|---|---|---|
| innocent-addict | 152.1 | 25.2% |
| kamaitachi-ni-yoru | 91.1 | 32.3% |
| telescope-nevermore | 81.0 | 35.4% |
| atamaro-deluxe | 350.5 | 7.2% |
| azemichi-ni-ochita-ginga | 54.3 | 50.8% |
| yamineko | 130.3 | 33.3% |
| my-axiom | 188.0 | 20.2% |
| amaten | 375.7 | 0.0% |
| fukagyakuteki-henyou | 262.3 | 9.8% |

**Reading it.** At 92.7% of cells, about three measures in four still need an edit. The joined class is
half of the cost, and cell accuracy cannot see most of it. Two consecutive same-pitch UST notes sung
as one output note have every cell right.
- This is likely, not checked: the share of joined events that are same-pitch is not yet measured.
  That belongs to tickets 03 and 07.
- Other things this number could be measuring, still to rule out in 03:
  - Pifuyuu's style of one note per mora, on fast songs (amaten, atamaro);
  - split-lead overlaps cut by the monophonic step;
  - UST notes finer than the grid, which merge or vanish when rounded.
- The pYIN arm is scored on its own phase. Its numbers include whatever phase error rounding doesn't
  forgive.

### Mutants (each was restored afterwards; all 15 edit-cost tests pass again)

| mutant | tests failing |
|---|---|
| Chopped detector, relation: drop "piece of" (`2·s > len(o)`) | 5 failed: `each_class_alone_costs_exactly_its_weight`, `chopping_costs_per_extra_piece_and_joining_per_hidden_note`, `a_long_piece_that_covers_most_of_the_note_still_chops_it`, `a_note_joined_with_half_of_the_next_which_is_then_chopped_is_one_of_each`, `a_known_mix_costs_the_sum_of_its_parts` |
| Chopped detector, count: `deg − 1` → 0 | the same 5 failed |
| Joined detector, relation: drop "covers" (`2·s > len(r)`) | 3 failed: `each_class_alone_costs_exactly_its_weight`, `chopping_costs_per_extra_piece_and_joining_per_hidden_note`, `a_note_joined_with_half_of_the_next_which_is_then_chopped_is_one_of_each` |
| Joined detector, count: `deg − 1` → 0 | the same 3 failed |

