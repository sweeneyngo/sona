# Off-time starts and ends under the revised weights (utau-edit-cost 18)

Date: 2026-11-24. Dev only (9 songs, 27.5 min); no holdout of any kind was read. Revised weights (`spec.md`): an
off-time start or end 4, near-miss pitch 3, octave 1, extra 1, missing 2, chopped/joined 4. Baseline reproduced
byte for byte from the dev cache: **GAME lead + octave 303.0 cost/min, 16.2% clean measures** (out of sample:
it decides), **learned stage 272.2 / 23.8%** (in-sample on dev: trained on these songs, shown beside it).

Selection rule (committed in the ticket before the Rust sweep, a52bd1f): port opt-in only if, on dev GAME lead,
the paired song-bootstrap 95% upper bound of Δ cost/min is below 0, clean measures do not fall, the missing count
does not rise (or the upper bound stays below 0 with missing re-weighted to 4), and a tuned parameter chosen
leave-one-song-out also passes.

A *pair* is one UST note matched to one output note. Its timing classes are **length** (cell counts differ),
**shift** (start one cell off) and **moved** (two or more), 4 each. So a note whose start alone is off pays 8 (shift
and length), and one legato boundary a cell off costs up to 12: the earlier note's length, the later one's shift and
length.

## Answer

- **Timing is 45.7% of GAME lead's cost (3 808 of 8 325 units) and 40.2% of the stage's, and nearly all of it is
  one-cell jitter.** Fixing every pair's start and end is worth −134.2 [−200.8, −85.3] cost/min on GAME lead
  (stage −105.1); fixing only the one-cell errors is −93.4 (stage −93.3). The rest is GAME's release (below).
  - **Starts** are almost all at legato boundaries (lead: 108 one cell late, 76 early, against 17 + 7 after a rest
    and 16 + 5 at a phrase start). Fixing starts after legato is worth −56.5 [−95.7, −25.7] (stage −59.7). Most of
    it is atamaro-deluxe, a 24-cell grid at 244 BPM (41 ms cells), where GAME's onsets sit 0.19 cell late even on
    right notes.
  - **Ends**: on GAME lead the largest group is the release: 225 notes run late into the short rest the UST
    writes before the next note, 169 of them by two cells (−38.1 [−46.0, −30.1] if fixed). The stage has mostly
    learned it (9 notes two cells late; −7.9 there), and its end errors are legato jitter (±1 cell, 187 notes).
- **No audio source says which way most of the jitter goes.** At GAME lead's legato boundaries that have UST truth,
  6.6% are a cell or more off (190 of 2 882). Each source's offer of the "right" cell breaks more right boundaries than it
  fixes. GAME on the vocals: 20 fixed, 67 broken. sona's pYIN grid: 49 fixed, 367 broken. The pitch track's
  crossing: 24 fixed, 573 broken. Voicing onset: 20 fixed, 821 broken. Energy rise and dip: worse still. The
  oracle "fix the edge wherever any source has it" is worth −63.5 on lead and −41.7 on the stage, but choosing
  among the sources is the hard part, and the stage already does that choosing.
- **What does carry information is GAME's own rounding residual.** A legato boundary whose raw GAME time sat
  0.4–0.5 cell from the cell it rounded to is wrong 40% of the time (51 of 127), against 1.6% under 0.1 cell. In
  84% of the wrong ones the UST is on the side the residual leans to. Alone it still breaks more than it fixes, but
  together with a second listen it pays.
- **Ported, opt-in, for GAME notes: `sona_core::near_ties::settle_near_ties`** (`--settle-near-ties`).
  - The rule: an edge whose raw GAME-lead time leans at least 0.35 cell to one side moves one cell that way, when
    GAME's run on the full vocals has an edge on that cell.
  - GAME lead: 303.0 → **297.8 (−5.2 [−8.4, −2.3])**. Clean measures +0.4 [+0.0, +1.0]. Missing +2 (upper bound
    −2.2 with missing at 4). 7 of 9 songs improve, none worsens by more than 0.6.
  - Leave-one-song-out picks 0.35 on all 9 folds. **It passes the rule.**
  - 66 edges move: 29 onto a UST edge, 17 off one, 20 between non-edges.
  - On top of the release trim it adds the same amount: 294.4 → 289.3 (−5.1 [−8.3, −2.2]).
  - It does nothing for the stage (−0.6 [−3.1, +1.5], clean −0.3, in-sample): 10 of its moves are fixes and 13
    are breaks. The stage already settled most of these ties.
- **Retuning 12's release trim for the new weights fails the rule.** Every trim also cuts quiet sung moras (mostly
  devoiced i/u moras) and turns a joined note (4) into a missing one (2), so missing rises with every setting.
  - 12's own setting is −8.6 [−19.7, +4.3], missing +55. The best single settings reach −10.7 [−16.0, −5.9]
    (quiet 0.2, tail 0.4, missing +18), and 27 of 132 settings pass one at a time.
  - The leave-one-song-out choice does not pass: −9.5 [−18.7, +2.3], missing +39, upper bound +8.7 with missing
    at 4. It is still not a fix for the stage: no setting lowers the stage's cost.
  - The lead stem's energy (no backing voices) does no better than the vocals'.
  - So under the revised weights, `--trim-releases` (12) no longer meets its own bar either: its interval now
    includes 0. That is flagged for the user and not deleted here.
- **Other mechanisms, all negative** (table below):
  - Correcting GAME's latency against the onset phase: −8.2 [−22.9, +1.6], clean −0.4, all atamaro.
  - Averaging GAME's two listens' raw edges: leave-one-song-out −2.2 [−5.2, +1.4].
  - Near-tie flips toward sona's grid edges: +1.8 to +22.0.
- **Recommendation (not done; the stage is not retrained on dev).**
  - The stage loses its timing at the same one-cell legato boundaries. Its oracles: −105.1 for every start and
    end, −93.3 for one-cell errors only, and −41.7 for a perfect choice among the sources it already sees.
  - A retrain whose loss charges a boundary error as the edit cost does (12 per legato boundary, rather than one
    cell of cross-entropy) could close part of that −41.7 ceiling. It needs a corpus that is not dev and a
    holdout4 read. GAME lead's raw rounding residual should be one of its features: it is the one signal here that
    predicts a wrong edge.
  - The same residual is a good **doubt mark** for ticket 14's flags. GAME lead boundaries with a residual of at
    least 0.3 cell are 10.8% of legato boundaries, and they hold 49% of the ones a cell off (94 of 190). That is
    30% precision against a 6.6% base rate.

## Taxonomy (dev)

Timing errors of pairs, by pattern (units = cost; share of all cost):

| pattern | GAME lead pairs | units | share | stage pairs | units | share |
|---|---|---|---|---|---|---|
| end only (length) | 512 | 2 048 | 24.6% | 320 | 1 280 | 17.1% |
| start only (shift/moved + length) | 169 | 1 352 | 16.2% | 183 | 1 464 | 19.6% |
| start and end, different amounts | 36 | 288 | 3.5% | 19 | 152 | 2.0% |
| whole note moved | 30 | 120 | 1.4% | 28 | 112 | 1.5% |

Start errors by the UST context before the note (GAME lead / stage): one cell late after legato 108 / 106, early
76 / 79; two cells after legato 14 / 19; late after a short rest 14 / 14; at a phrase start 18 / 9. End errors
(length Δ, start right): two cells late before a short rest 169 / 9; one cell late before legato 102 / 100, early
80 / 87; one cell late before a rest or phrase end 65 / 51, early 41 / 44. Against the output's own context, the
largest group of GAME lead's end errors is "UST rest, output legato, late" (215): GAME runs into the next note
where the UST rests.

By UST note length (GAME lead): 1 cell 18.7% of pairs off-time, 2 cells 15.6%, 3–4 24.1%, 5–8 28.3%, 9+ 56.3%;
by cost, notes of 2–4 cells carry 33% of all cost. Per song, timing cost/min on GAME lead: atamaro 362.6,
fukagyakuteki 214.6, innocent 146.0, yamineko 119.5, kamaitachi 98.3, my-axiom 89.1, amaten 78.5, azemichi 77.7,
telescope 56.9.

Where the stage's boundaries come from: at its wrong starts, GAME lead has the UST's edge 11.3% of the time, GAME on
the vocals 11.3%, sona 34.3%, any 40.9% (ends 16.5 / 16.5 / 37.8 / 44.0%). At GAME lead's own wrong starts, GAME on
the vocals has the edge 11.9% of the time, sona 45.1%. But sona also offers a different cell at 367 of GAME lead's 2 692
right legato boundaries.

## Oracles (dev; the pairs in the slice take the UST's start and/or end; real metric re-run)

| fixed perfectly | GAME lead Δ [95%] | clean | missing | stage Δ [95%] | stage clean |
|---|---|---|---|---|---|
| every start and end | −134.2 [−200.8, −85.3] | 32.0% | −4 | −105.1 [−163.9, −62.1] | 34.0% |
| every start (end kept) | −65.2 [−112.2, −29.6] | 17.8% | −4 | −66.4 [−107.5, −33.8] | 26.3% |
| every end (start kept) | −60.1 [−71.1, −49.1] | 26.5% | 0 | −31.9 [−41.4, −24.3] | 28.3% |
| one-cell errors only | −93.4 [−140.2, −58.2] | 23.0% | −4 | −93.3 [−138.9, −58.1] | 32.3% |
| errors of 2+ cells only | −36.3 [−49.6, −24.8] | 22.5% | 0 | −10.4 [−21.2, −3.3] | 24.9% |
| start-only errors (end right) | −54.2 [−84.4, −28.8] | 17.4% | −3 | −59.6 [−90.5, −33.1] | 25.7% |
| starts after legato | −56.5 [−95.7, −25.7] | 17.4% | −4 | −59.7 [−98.7, −28.4] | 25.9% |
| starts after a short rest | −4.4 [−10.4, −0.6] | 16.2% | 0 | −4.7 | 23.8% |
| starts at a phrase start | −4.1 [−7.3, −1.4] | 16.5% | 0 | −2.0 | 24.1% |
| ends late before a rest or phrase end (release) | −38.1 [−46.0, −30.1] | 23.1% | 0 | −7.9 [−11.6, −4.6] | 25.1% |
| ends early before a rest or phrase end | −6.3 | 18.2% | 0 | −6.8 | 26.1% |
| ends late, legato | −12.1 [−19.4, −5.8] | 16.4% | 0 | −13.3 | 24.1% |
| ends early, legato | −3.6 | 16.8% | 0 | −3.9 | 24.3% |
| notes of 1–2 UST cells | −57.3 | 20.0% | 0 | −44.2 | 25.5% |
| notes of 3+ UST cells | −73.9 | 25.2% | −4 | −57.5 | 30.2% |
| edges where GAME on the vocals has the UST's | −16.8 [−20.3, −12.6] | 18.4% | 0 | −14.0 | 25.1% |
| edges where sona has the UST's | −56.8 [−81.7, −34.5] | 21.8% | −2 | −34.9 | 26.9% |
| edges where any source has the UST's | −63.5 [−89.3, −40.4] | 23.0% | −2 | −41.7 [−60.8, −25.8] | 27.5% |

## Evidence at GAME lead's legato boundaries

2 882 boundaries with UST truth; 190 a cell or more off. "Fixed" means the offer moves the boundary to the UST's
cell, "broken" means it moves a right one.

| offer | agrees with GAME | fixed | broken | net |
|---|---|---|---|---|
| GAME on the vocals | 2 775 | 20 | 67 | −47 |
| sona's pYIN grid notes | 2 411 | 49 | 367 | −318 |
| pitch-track midpoint crossing (pitch changes only) | 1 008 | 24 | 573 | −549 |
| voicing onset after an unvoiced run | 1 742 | 20 | 821 | −801 |
| largest energy rise | 626 | 22 | 2 127 | −2 105 |

| GAME lead raw residual | boundaries | a cell off | on the residual's side |
|---|---|---|---|
| < 0.1 cell | 1 503 | 1.6% | 13 of 24 |
| 0.1–0.2 | 615 | 4.7% | 22 of 29 |
| 0.2–0.3 | 451 | 9.3% | 35 of 42 |
| 0.3–0.4 | 184 | 23.4% | 38 of 43 |
| 0.4–0.5 | 127 | 40.2% | 43 of 51 |

## Mechanisms tried (dev, GAME lead unless noted; leave-one-song-out where a parameter was tuned)

| mechanism | Δ cost/min [95%] | clean Δ | missing Δ | verdict |
|---|---|---|---|---|
| **near ties settled toward GAME on the vocals, lean ≥ 0.35 (ported)** | **−5.2 [−8.4, −2.3]** | **+0.4** | +2 (@4: −2.2) | **passes** |
| … chosen leave-one-song-out (0.35 on 9/9 folds) | −5.2 [−8.4, −2.3] | +0.4 | +2 | passes |
| … lean ≥ 0.30 / 0.40 / 0.45 | −3.6 / −3.9 / −2.3 | +0.3 / +0.4 / +0.1 | +2 / +2 / +1 | |
| … toward GAME on the vocals or sona | +1.8 … +22.0 (0.45: −3.7) | | +8 … +21 | worse |
| … on the stage (in-sample) | −0.6 [−3.1, +1.5] | −0.3 | +2 | no gain |
| near ties on raw times before snapping, lean ≥ 0.35 | −4.8 [−8.2, −1.7] | +0.4 | +1 | smaller than on notes |
| release trim retuned (132 settings: quiet, tail, min cells; vocals' or lead stem's energy) | best −12.0 [−19.8, −3.9] | +2.8 | +30 | 27 pass alone |
| … chosen leave-one-song-out | −9.5 [−18.7, +2.3] | +2.6 | +39 (@4: +8.7) | **fails** |
| … 12's setting (0.3, 0.8, 2), as shipped opt-in | −8.6 [−19.7, +4.3] | +2.5 | +55 | fails now |
| … with a voiced tail kept (pitch track veto) | −4.7 … −11.0 | | +18 … +51 | no better |
| … on the stage, any setting | +0.1 … +29.4 | ≤ 0 | | worse |
| GAME latency against the onset phase (circular mean), ×0.25–1 | −2.5 … −8.2 [−22.9, +1.6] | −0.1 … −0.4 | −1 … −3 | fails (clean, interval) |
| raw edges averaged with GAME on the vocals (reach, weight) | leave-one-song-out −2.2 [−5.2, +1.4] | +0.1 | +1 | fails |

Combinations (Rust report, paired bootstrap from its per-song counts):

| combination | Δ cost/min [95%] | clean | missing |
|---|---|---|---|
| GAME lead: near ties, then releases trimmed (12) | −13.7 [−25.3, +0.2] vs 303.0 | +3.0 | +57 |
| … against the trim alone | −5.1 [−8.3, −2.2] | +0.5 | +2 |
| stage: near ties, then unheard pieces merged (11) | −5.8 [−11.2, −1.0] vs 272.2 | +0.2 | +8 |
| … against the merge alone | −0.7 [−2.3, +0.9] | −0.3 | +2 |

## What else these numbers could be measuring

- **An artefact of the scorer's overlap trim.** The first version of the near-tie flip measured −6.8 on GAME lead
  and −2.6 on the stage. When it undid a move for one note but not for the neighbour on the same cell, the two notes
  overlapped. The scorer, like the ports, trims the earlier note to the later note's start, and that ate one-cell
  pieces (chopped −12). The port keeps one voice instead: a cell whose move would empty a note stays put for every
  note on it. That gives −5.2 and −0.6. The property test fails on the overlapping version (the "no guard" and "crossing moves kept" mutants).
- **One song.** No: 7 of 9 songs improve. The largest gains are yamineko −14.9 and innocent −9.5; the one song that
  gets worse is atamaro, +0.6, where most of the timing cost is.
- **Selection on dev.** The rule was chosen among about 40 near-tie settings (on raw times and on notes, three
  second sources, five leans). Leave-one-song-out picks the same one on every fold, and its neighbours (0.3–0.45)
  all lower the cost. It is still a dev number: a default needs holdout4.
- **Dropped notes.** Missing rises by 2, and the gain holds with missing weighted 4. Every trim, by contrast, wins
  partly by turning joined notes into missing ones.
- **The UST author's grid.** Pifuyuu's beat lines are a cell off the music on most dev songs (A03). A fix that only
  agreed with his grid would not help a user, but the near-tie flip reads no grid convention: it moves edges toward
  where a second listen of the audio hears them.
- **The in-sample stage.** Stage numbers on dev are in-sample. The conclusion drawn from them (no gain there) is
  conservative.

## Verification

- `sona-eval-cellstage --edit-cost` without the new flag is **byte-identical** to the baseline run (`cmp`).
- The Rust `settle_near_ties` gives the prototype's per-song class counts on all 18 song × output rows
  (`check_ties_port.py`), and the pooled numbers match: 297.8 / 271.6.
- `crates/sona-core/tests/near_ties.rs`: 9 behaviour tests through `settle_near_ties`.
  - A near-tie boundary moves where the second listen hears it.
  - An agreeing second listen, a clean rounding, the wrong side, or a missing listen leave the edge alone.
  - A release edge moves on its own.
  - A move that would empty a note is undone, and crossing moves are dropped.
  - A random property: the line stays one voice on the grid, every note is kept, and each edge moves at most a
    cell.
- Five mutants caught: no second listen required, moving away from the lean, no guard for emptied notes, crossing
  moves kept, `min_lean` ignored.

## Reproduce

```sh
# Rust (dev): adds "GAME lead + octave rule, near ties settled (opt-in)" and the stage's row; with the other two
# opt-ins also the combinations.
ORT_DYLIB_PATH=… SONA_GAME_DIR=… SONA_DEV_CACHE=… cargo run --release -p sona-eval --features game \
    --bin sona-eval-cellstage -- --model ~/.cache/sona/cellstage-D --edit-cost --settle-near-ties \
    [--trim-releases --merge-unheard-pieces] [--dump DIR]
# Prototype, on a --dump of dev (UST content: keep it, and the lead-stem energy, out of the repo)
export S18_DUMP=… S18_LEAD_RMS=…
cd crates/sona-eval/scripts/timing-errors
python3 -I lead_energy.py STEMS_DIR "$S18_LEAD_RMS"
python3 -I taxo18.py lead; python3 -I taxo18.py stage
python3 -I boundaries.py lead; python3 -I boundaries.py stage
python3 -I sweep18.py lead; python3 -I sweep18.py stage
python3 -I check_ties_port.py RUST_OUTPUT
python3 -I rust_rows.py RUST_OUTPUT "GAME lead + octave rule, onset phase" "GAME lead + octave rule, near ties settled (opt-in)"
```
