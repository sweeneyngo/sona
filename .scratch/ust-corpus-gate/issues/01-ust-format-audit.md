# 01 — UST format audit (parser + fixtures + audit report)

Status: done

Gates everything downstream. **Needs no audio.** The point is to find out whether
USTs actually carry the offset signal this corpus is being built for — from a
handful of files, before any pipeline exists.

## Why this is first

The corpus's value proposition is "USTs give in-domain offset labels." That is an
empirical claim, not an assumption. A UST is a contiguous tick-space note list
with explicit rests (lyric `R`), so a **legato note's offset is just the next
note's onset** — the next-onset heuristic ADR-0007 tried and
[ADR-0010](../../../docs/adr/0010-note-hmm-segmentation.md) retired. Only
**rest-terminated notes** carry genuine musical-end information. If that fraction
is low, the corpus delivers far less offset signal than expected and the plan
changes shape. Better to learn that from 5 files than after a 20-song pipeline.

## Scope

- UST parser in `sona-eval`: `[#SETTING]` header (`Tempo`, `ProjectName`) and
  `[#NNNN]` note blocks (`Length` in ticks @ 480/quarter, `Lyric`, `NoteNum`).
  Ignore synthesis-only params (`PreUtterance`, `VoiceOverlap`, `Intensity`,
  `Modulation`, `Envelope`, `PBS`/`PBW`/`PBY`/`PBM`) — they affect voicebank
  sample placement, not musical note position.
- Handle the encoding reality: USTs are commonly **Shift-JIS**, not UTF-8, and
  lyrics are kana. Decode explicitly; do not assume UTF-8.
- Commit **3–5 small fixture USTs** for unit tests (they are 2–12 KB — small
  enough for git, same rationale as the golden audio fixtures).
- `sona-eval-ust-audit` binary reporting, per UST:
  - **rest-terminated note fraction** — the headline number
  - note count, total duration, tempo
  - **grid-alignment tightness**: what fraction of note starts land on a 16th /
    32nd / triplet grid, and the residual distribution (how idealized is the
    timing)
  - shortest/longest note, and the count below sona's ~100 ms prune threshold
    (ADR-0010 post-processing) — notes sona structurally cannot emit

## Done when

- Parser round-trips the fixtures; Shift-JIS lyrics decode correctly.
- Audit report runs on the fixtures and prints the numbers above.
- The rest-terminated fraction is **written into the report** with a plain reading
  of what it implies for offset signal.

## Explicitly not in scope

- No audio, no alignment, no scoring. Onset-delta distributions (which need audio)
  land in **05**.

## Comments

**2026-11-20 — parser, fixtures, and audit binary landed. Waiting on real USTs.**

Done:
- `crates/sona-eval/src/ust.rs`: UST parser. Keeps `Tempo`/`ProjectName` and
  `Length`/`Lyric`/`NoteNum`, plus note-level `Tempo=` changes; ignores synthesis
  params. Encoding is chosen explicitly: BOM, then `Charset=`, then strict UTF-8
  validation, then Shift-JIS (CP932). Malformed Shift-JIS is a located error, never
  mojibake. The chosen encoding is recorded on the parsed `Ust`. Round-trips text
  and bytes.
- `crates/sona-eval/src/ust_audit.rs` + `sona-eval-ust-audit`: per UST and pooled,
  prints the rest-terminated fraction with a plain reading, grid fit (16th / 32nd /
  triplet / off-grid, exclusive) for note starts and rest-terminated ends,
  off-16th residuals, notes below the prune floor (read from
  `NoteHmm::default().min_note_s` so it can't drift), and tempo changes.
- New dependency: `encoding_rs` (pure Rust, no C).

Deviations from the scope above:
- **Fixtures are synthetic, not Pifuyuu's.** The UST blog's terms
  (pifuyuuust.blogspot.com) forbid redistribution, converted forms included, and
  this repo is public. So `fixtures/ust/` holds four hand-authored USTs in the real
  format (see its README). Real USTs stay in the git-ignored cache. The same applies
  to 02–06: commit only aggregate reports, never per-note UST content.
- **Breaths count as terminators.** A note followed by `息`/`br`/`breath` has a
  genuine end, the same as one followed by a rest. Breath blocks aren't counted as
  notes.
- **Note-level tempo changes are parsed and flagged.** The spec assumes one BPM per
  song; any UST with a mid-song `Tempo=` breaks that, and the audit reports it.
- **Extra stat: grid fit of rest-terminated *ends*.** These are the only real offset
  labels, so how tightly they snap matters for quantized COnPOff.

Remaining (human): put 3–5 of the most recent USTs in
`crates/sona-eval/data/pifuyuu-ust/` (or point `SONA_UST_DIR` at them). The OneDrive
folder is linked from the blog; anonymous API access was not attempted. Then run
`cargo run -p sona-eval --bin sona-eval-ust-audit -- --corpus` and commit the
output with its reading as `crates/sona-eval/reports/ust-format-audit.md`. That
report closes this ticket and gates 02.

**2026-11-20 — Done. Gate result: THIN offset signal (14%). The plan changes shape; it isn't killed.**

The 5 most recent uploads were fetched from the OneDrive folder (with maintainer approval) into the
git-ignored `data/pifuyuu-ust/`. The audit ran on the 6 lead USTs. Report:
[`reports/ust-format-audit.md`](../../../crates/sona-eval/reports/ust-format-audit.md).

- **Rest-terminated: 348 / 2,482 = 14%** (10–23% per song). 86% of UST offsets are just the next
  onset. That's thin as a fraction but about 70 labels per song, roughly 1,400 at 20 songs, so
  enough to score offsets on the **rest-terminated subset only**. That subset is the offset gate.
- Timing is very idealized (only 0.24% of starts are off every grid), but **15% of starts are
  triplets** (ノイズゴト is 96%). A 16th-only hard-snap is the wrong grid for them.
- The **100 ms prune floor caps recall**: 9% of notes pooled, **50% of ノイズゴト**.
- No mid-song tempo changes. Archives hold several USTs (lead `main`, harmony `cho`, and split leads).

Also: the audit binary now labels files as `song/main.ust`, since archives name files generically.
