# Same-pitch boundaries: split at moras, merge restarts (utau-edit-cost 07)

Date: 2026-11-24. Dev only (9 songs, 27.5 min). No holdout was read; the spent splits were not re-read.
Baseline (ticket 03, reproduced from the GAME cache byte for byte): **GAME lead + octave 188.7 cost/min,
16.2% clean measures** (out of sample: not trained on the corpus); learned stage 180.2 / 23.8% (in-sample on
dev). Selection rule (declared in the ticket before measuring): port a mechanism only if it lowers GAME
lead's pooled cost/min on dev with a song-bootstrap 95% interval on the paired difference that excludes 0,
and does not raise the stage's.

All intervals are paired song bootstraps (10 000 resamples) of the pooled difference against the baseline.
"Phrases =" is the share of the UST's phrases (runs of notes with no rest) in which the output has exactly
as many notes as the phrase has moras; that is when pasted lyrics land without shifting. The baseline is
29.7% on GAME lead and 43.0% on the stage, out of 817 phrases.

## Answer

- **Without lyrics, nothing pays.** A spectral same-pitch boundary detector (MFCC change, energy dip,
  vowel/ん changes) separates hidden mora onsets from other grid points only weakly: AUC 0.72 at
  best hand-made, 0.73 for a classifier trained leave-one-song-out. Hidden onsets are 8% of the interior grid
  points, and a split pays only above about 50% precision, so every threshold raises the cost. Merging
  same-pitch restarts the detector hears no change across reaches −1.8 [−3.8, +0.1] out of fold. Even its
  perfect version is worth only −8.2.
- **A whole song's lyric, aligned by DP to sona's notes, makes things worse** (GAME lead +0.9 to +15.0
  cost/min). The UST's own kana, given as perfect text, does no better. Song-level alignment cannot tell a
  joined note from a missed note or from a chopped one.
- **With the lyric pasted per phrase, it pays.** Fitting each phrase's kana to that phrase's notes (a DP
  over moras and notes, then a split or merge) lowers GAME lead by **−13.9 [−30.2, −3.8]** cost/min
  (188.7 → 174.9) and the stage by −17.8 [−38.0, −4.8] (180.2 → 162.4). Here a phrase is a run of touching
  notes in sona's output. The fit:
  - splits a note that holds extra moras so each extra one takes the note's last 2 cells;
  - joins a same-pitch note that holds none into the note before it.

  Phrases with the right note count go 29.7% → 45.4% on GAME lead and 43.0% → 71.1% on the stage. Clean
  measures barely move (16.2 → 15.9%, 23.8 → 25.0%).
  - The gain survives choosing the design on 8 songs and scoring the 9th: −11.5 [−28.4, −1.4] on GAME lead
    and −16.6 [−37.1, −3.1] on the stage.
  - The gain is concentrated: amaten gives −85.8 of its own 394 cost/min. 6 of 9 songs improve on GAME
    lead, and my-axiom worsens by 2.1.
  - **Ported** to `sona_core::lyric_fit` and the opt-in `sona-eval-cellstage --fit-phrase-lyrics`, which
    reproduces the prototype exactly. Not a default.
  - It is a user-in-the-loop mechanism: it needs someone to say which moras belong to which notes. In the
    eval, the "user" gives each sona phrase the moras of the UST notes sung inside it.
- **Where the ceiling is.** Knowing every note's true mora count is worth −81.4 with perfect placement, but
  only −51.9 with the 2-cell tail rule and −37.0 with equal division. The per-phrase fit reaches about a
  quarter of the tail-rule ceiling. The rest is allocation (which note of the phrase hides the mora) and
  placement.

## Oracle and mechanism table (dev, cost/min; Δ against the baseline)

GAME lead baseline by class (cost/min): joined 97.1 · chopped 24.3 · pairs (pitch, length, shift, moved) 42.0
· missing 22.7 · extra 2.5. Stage: 95.2 · 34.9 · 33.3 · 15.5 · 1.2.

| mechanism | evidence | GAME lead Δ [95%] | joined / chopped / pairs | stage Δ [95%] | phrases = (lead) |
|---|---|---|---|---|---|
| joined fixed perfectly (03) | UST notes | −99.0 (−52.5%) | — | −98.0 | — |
| **1a** split at true onsets | true count + place | −81.4 [−116.6, −50.3] | 1.7 / 25.5 / 56.4 | −84.3 [−123.1, −50.8] | 49.4% |
| 1a split, 2-cell tail | true count per note | −51.9 [−80.2, −30.0] | 7.4 / 31.2 / 69.9 | −55.6 [−87.8, −30.8] | 48.7% |
| 1a split, equal division | true count per note | −37.0 [−68.7, −16.8] | 7.7 / 33.8 / 81.8 | −40.5 [−77.6, −17.0] | 48.5% |
| 1a split, detector peaks | true count per note | −29.6 [−61.0, −10.3] | 16.7 / 42.1 / 74.0 | −37.1 [−72.5, −15.7] | 48.3% |
| 1a merge 0-mora same-pitch notes | true count per note | −4.6 [−7.0, −2.2] | 97.7 / 17.8 / 43.9 | −5.0 [−7.2, −2.7] | 29.6% |
| 1a phrase count, longest notes | true count per phrase | +36.3 [+17.7, +53.5] | 37.0 / 93.6 / 66.1 | +11.7 [−3.7, +25.8] | 76.7% |
| **1b** UST kana per UST phrase, DP | perfect text + UST phrases | −15.3 [−33.1, −4.3] | 55.5 / 36.4 / 54.2 | −22.0 [−44.0, −7.9] | 45.8% |
| 1b … + detector evidence in the DP | + spectrum | −19.0 [−36.5, −7.3] | 53.3 / 35.4 / 54.0 | −23.6 [−45.7, −7.7] | 46.0% |
| **1b per sona phrase (ported)** | kana pasted per phrase | **−13.9 [−30.2, −3.8]** | 43.5 / 45.0 / 57.2 | **−17.8 [−38.0, −4.8]** | **45.4%** |
| … design chosen leave-one-song-out | | −11.5 [−28.4, −1.4] | 45.6 / 45.6 / 56.6 | −16.6 [−37.1, −3.1] | |
| … phrases split at rests ≥ 2 cells | | −10.9 [−24.0, −2.2] | | −16.3 [−33.2, −4.7] | 45.4% |
| … phrases split at rests ≥ 4 cells | | −7.0 [−17.3, +0.1] | | −5.1 [−13.8, +1.3] | 39.7% |
| 1b whole song, UST kana, no line breaks | perfect text | +0.9 … +11.4 | | +0.5 … +10.4 | ≤ 36% |
| 1b whole song, UST kana, breaks at rests ≥ R | perfect text + breaks | +7.6 … +13.1 | | −5.5 … +1.9 | ≤ 37% |
| 1b whole song, wiki text read by fugashi | realistic text | +2.3 … +15.0 | | +1.7 … +9.8 | ≤ 33% |
| **2** split at detector peaks, thresholded | spectrum | +6.0 (q0.99) … +157 (q0.80) | | +6.5 … +160 | ≤ 34% |
| 2 one candidate per note (2-cell tail), perfect | true onset at candidate | −37.7 [−55.7, −21.5] | 53.3 / 29.4 / 43.9 | −38.8 [−58.3, −21.5] | 37.2% |
| 2 … with the detector, best threshold | spectrum | −1.1 [−3.3, +1.5] | | | |
| 2 learned classifier (LOSO), best threshold | spectrum + f0 + position | +0.7 [−0.8, +2.8] | | +0.9 [+0.0, +2.3] | |
| **3** merge restarts without a UST onset | perfect | −8.2 [−11.0, −5.3] | 105.3 / 14.3 / 37.9 | −8.6 [−11.9, −5.5] | 29.5% |
| 3 merge restarts the detector hears no change across | spectrum, best of 16 | −1.6 [−3.3, −0.1] | | −1.9 [−3.3, −0.6] | |
| … design and threshold chosen leave-one-song-out | | −1.8 [−3.8, +0.1] | | −2.0 [−3.9, −0.3] | |
| 3 learned classifier (LOSO) | spectrum + f0 + position | −0.2 [−0.5, +0.0] | | −0.4 [−1.0, +0.0] | |

Ranges ("+0.9 … +11.4") span the settings swept.

## How the fit works (`sona_core::lyric_fit`)

- **Mora kinds.** A mora is *open* (bare vowel, ん, a held ー) or a *consonant*. 03 measured how often the
  UST's same-pitch boundaries are hidden in a joined note: about 55% for open moras and 15% for consonants.
  Attaching a mora to the note before it costs `-ln P(hidden | kind)`.
- **DP.** Over a phrase's notes and moras, each note takes a block of 0–8 moras (at most one per cell). A
  note with none costs 2, an unsung mora costs 3, and a note's cells per mora is pulled toward the phrase's
  (`0.5 · |ln(cells/k ÷ phrase cells per mora)|`).
- **Split.** A note given k > 1 moras is split so that each extra mora takes the last 2 cells. On dev, the
  UST divides a 6-cell two-mora note 4+2 in 83 of 117 cases, and equal division (3+3) in 8.
- **Merge.** A note given no mora joins a touching note before it that has the same pitch.

Each of three parts is necessary. Without the duration term the GAME lead result is +2.9; with a flat
attach cost, −1.9; with equal division instead of the tail, −5.3; with a 1-cell or 3-cell tail, +1.5 or
+9.5. The other knobs barely matter (zero cost 1–4, skip cost 3–6, at most 4 or 8 moras per note:
−13.7 to −14.8). The leave-one-song-out design choice picked tail 2 and kind-aware costs on every fold;
β flipped between 0.5 and 1.0.

## Why the other levers fail

- **The spectrum.** Event-triggered MFCC novelty does rise at hidden onsets: AUC 0.78 for a consonant mora,
  0.67–0.68 for a vowel change or ん, and none at all for a same-vowel continuation (0.43–0.51). But a
  run-of-the-mill grid point inside a note looks much the same (vibrato, bleed, consonant noise). The rise
  sits at the onset (0 ms); timing is not the problem.
- **Song-level alignment.** A deficit of one mora in a phrase is as often a note sona *missed* (sona has a
  rest there) as a joined note. Of the phrases with exactly one more mora than sona has notes, 137 hold no
  joined note against 109 that hold one. Forcing the mora into a neighbour creates a chopped note. Letting
  rests absorb moras helps, but without the phrase boundaries the DP has no anchor, and its errors cost more
  than its fixes.
- **Placement matters as much as the count**: equal division keeps less than half of the true-placement
  gain.

## What else these numbers could be measuring

- **Leaked UST information in the ported eval.** The simulated user's kana for a sona phrase comes from the
  UST notes whose midpoint lies inside that phrase. A real user pastes what they hear: the same moras,
  except that a note sona missed at a phrase's edge would still be in the user's text. Its kana are also
  perfect. The realistic version without the UST (whole-song wiki text) fails, so the ported gain is
  conditional on per-phrase pasting.
- **Selection on dev.** The design was picked from about 40 settings on the same songs; the
  leave-one-song-out choice keeps most of it (−11.5 against −13.9). The P(hidden | kind) values come from
  03's dev taxonomy. Confirmation needs holdout4.
- **One song.** amaten is 55% of the pooled GAME lead gain (50% of the stage's); the rest of the songs move −15.8 … +2.1 each.
- **Metric artefacts.** A split off by one cell turns a joined note (4) into length/shift pairs (1–2 each),
  so even an imprecise split lowers the cost; that is why pairs rise (42.0 → 57.2) while joined falls
  (97.1 → 43.5). It is real editing work saved (a drag is cheaper than a split), but clean measures do not
  improve.
- **The Python port.** `ec.py` matches the Rust edit cost on every song (03), and the Rust port of the fit
  gives identical per-song class counts on all 18 song × output rows.

## Reproduce

```sh
# Rust (dev): adds two reports, "… each phrase's lyric fitted (opt-in)"; the default run is byte-identical.
ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
    --model ~/.cache/sona/cellstage-D --edit-cost --fit-phrase-lyrics
# Prototype (needs a --dump of dev; S07_WORK holds features and lyric streams, lyric-derived: keep out of the repo)
export S07_DUMP=… S07_WORK=…
python3 crates/sona-eval/scripts/same-pitch-boundaries/oracle1a.py            # 1a ceilings
python3 crates/sona-eval/scripts/same-pitch-boundaries/sonaphrase_sweep.py    # the ported fit, knobs, per song
python3 crates/sona-eval/scripts/same-pitch-boundaries/sonaphrase_loso.py     # design chosen leave-one-song-out
python3 …/feats.py "$S07_WORK/feat" lead <ids>    # librosa: lead-stem features for det*.py and learned.py
python3 …/mora_text.py <ids>                      # fugashi + unidic-lite: wiki text → mora stream for lyric_song.py
```

Spent splits were not re-read: nothing here needed a diagnosis that dev could not give.
