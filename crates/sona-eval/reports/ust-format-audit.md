# UST format audit (UST corpus gate, ticket 01)

Run: `cargo run --release -p sona-eval --bin sona-eval-ust-audit -- crates/sona-eval/data/pifuyuu-ust/*/main*.ust`
Corpus: the **5 most recent `@pifuyuu` uploads** as of 2026-11-20 (INNOCENT ADDICT, 金星,
かまいたちに因る, ノイズゴト, Telescope -nevermore-). That is the head of the recency list issue
02 will use. Only the **lead** USTs (`main*.ust`) are audited. Harmony (`cho*.ust`) is excluded
because sona transcribes one lead line. INNOCENT ADDICT splits its lead across `main1` + `main2`,
so there are 6 files.

The USTs are git-ignored and never committed (the author's terms forbid redistribution). This
report holds aggregates only. No audio was used.

Purpose: the corpus's value proposition is "USTs give in-domain offset labels." A UST is
contiguous, so a legato note's offset is just the next onset, which is the heuristic ADR-0010
retired. Only **rest-terminated** notes carry a genuine musical end. Measure that fraction before
building the pipeline.

## Result

Per-file lines are condensed from the full output. The pooled block is verbatim.

```
[1] INNOCENT ADDICT/main1.ust        198 BPM  rest-terminated  83 / 644 = 13%   below 100 ms:  56
[2] INNOCENT ADDICT/main2.ust        198 BPM  rest-terminated   4 /  35 = 11%   below 100 ms:   0
[3] Telescope -nevermore-/main.ust   123 BPM  rest-terminated  65 / 520 = 12%   below 100 ms:   1
[4] かまいたちに因る/main.ust          144 BPM  rest-terminated 102 / 439 = 23%   below 100 ms:   0
[5] ノイズゴト/main.ust               204 BPM  rest-terminated  43 / 342 = 13%   below 100 ms: 171
[6] 金星/main.ust                     138 BPM  rest-terminated  51 / 502 = 10%   below 100 ms:   1

== Pooled: 6 USTs, 2482 sung notes, 1046 s ==

rest-terminated     348 / 2482 = 14%   per-song min 10% · median 13% · max 23%
note starts         16th 2094 · 32nd 9 · triplet 373 · off-grid 6   → 84% on 16th   (off-16th residual median 25 ms, max 61 ms)
rest-term. ends     16th 300 · 32nd 1 · triplet 30 · off-grid 17   → 86% on 16th   (off-16th residual median 25 ms, max 37 ms)
below prune floor   229 / 2482 = 9%   (notes sona structurally cannot emit)
tempo changes       0 across 6 USTs
```

All 11 USTs in the 5 archives (lead and harmony) are Shift-JIS and decode cleanly. Every lyric is
plain one-kana-per-note CV. There are no VCV end tokens (`a R`) and no breath notes, so "next block
is `R`" identifies note ends unambiguously. For comparison only, the harmony USTs show the same
pattern (211 / 1493 = 14% rest-terminated), so the low fraction is how this author notates songs,
not a quirk of lead lines.

## Reading

**1. The rest-terminated fraction is THIN: 14%, between 10% and 23% for every song.** About 86% of
UST note "offsets" are just the next note's onset. Scored as-is, a pooled in-domain COnPOff would
mostly re-measure onset and segmentation accuracy, not where notes end. The corpus cannot gate the
offset bottleneck the way the spec assumed.

**2. The absolute count is still usable.** At about 70 genuine offset labels per song, 20 songs
give roughly 1,400. For comparison, Vocadito has 2,244 reference notes in total. That's enough to
score offsets **on the rest-terminated subset alone**. So the plan changes shape rather than dying:
the in-domain gate (06) must report COnPOff on the **rest-terminated subset** as *the* offset
number. Legato and all-notes COnPOff are labelled as segmentation measures and never read as offset
accuracy.

**3. The timing is extremely idealized, but not all on 16ths.** Only 6 of 2,482 starts (0.24%) are
off every grid. However, **15% of starts are on triplet lines**: ノイズゴト is 96% triplets, 金星 is
9%. ADR-0003's 16th hard-snap is the wrong grid for those notes. COnP tolerates it (only 1 start
lands more than 50 ms from a 16th), but the exported MIDI would be musically wrong. The quantizer
port (04) and the gate (06) need a per-song grid (16th vs triplet), or must report triplet songs
separately.

**4. The 100 ms prune floor is a hard recall ceiling in-domain.** Pooled, 9% of notes are shorter
than sona can emit, but this is concentrated: **50% of ノイズゴト** (204 BPM, triplet 16ths of
49 ms) and 9% of INNOCENT ADDICT. The "~100 ms literature floor" (ADR-0010) comes from general
singing. On fast Vocaloid it caps per-song recall before any detection happens. The floor should be
tempo-aware (e.g. a fraction of the song's shortest grid unit). This isn't decided here; 06 should
measure how much it costs.

**5. One BPM per song holds so far.** There are no mid-song tempo changes in any of the 5 songs.

## Consequences for the rest of the plan

- **02 (manifest):** each archive holds several USTs. The manifest must record which file is the
  lead and how split leads merge. INNOCENT ADDICT's `main2` fills gaps in `main1`, but 8 of its 35
  notes overlap `main1`, so the original has brief two-voice moments.
- **02/05/06 (harmony):** the `cho` USTs show where the original has harmony. A demucs vocal stem
  will contain those harmonies, and sona is monophonic. The `cho` timelines can be used to mask
  harmony regions out of scoring, or to report them separately.
- **04:** the quantizer port needs a triplet grid alongside the 16th grid (see 3).
- **06:** use the rest-terminated subset as the offset gate (see 2). Report the prune-floor recall
  ceiling per song (see 4).
