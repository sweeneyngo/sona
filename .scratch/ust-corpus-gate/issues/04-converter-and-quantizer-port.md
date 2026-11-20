# 04 — UST→annotation converter + hard-snap port to `sona-eval`

Status: done

Blocked by: 01

Needs no audio. Can run in parallel with 02/03.

## Converter

UST note list → the reference note format `sona-eval` already scores against
(same shape as the Vocadito reference loader), so the UST corpus drops into the
existing metric code rather than growing a parallel path.

- Ticks → seconds via the UST's exact `Tempo` (480 ticks/quarter).
- Rests (lyric `R`) become **gaps**, not notes — they are what makes a preceding
  note's offset a genuine musical end rather than the next onset.
- `NoteNum` is already integer MIDI.
- Optionally emit a `.mid` for eyeballing in the review surface (the "UST→MIDI
  tool" the corpus work needs anyway).

## Hard-snap port

Hard-snap currently exists **only** in [`web/lib/grid.ts`](../../../web/lib/grid.ts)
(`snapTime` / `quantizeNote`), in TypeScript, UI-side. `sona-core` deliberately
emits raw perception
([ADR-0004](../../../docs/adr/0004-core-emits-raw-perception-ui-interprets.md))
and `midi.rs` writes raw, unquantized MIDI. Scoring against a **quantized** UST
requires quantization in Rust.

**It goes in `sona-eval`, not `sona-core`** — the core's raw-perception contract
stays intact, and the duplication is confined to the harness where drift is
detectable by test.

**Write it to be promotable:** a pure function over
`(notes, spacing, anchor, confidence)` with no eval-only dependencies, so moving
it into `sona-core` later — or exposing it to the UI via WASM, making the core
the single source of truth — is a file move plus a re-export, not a rewrite. The
duplication is **explicitly temporary**; record that as an ADR consequence so it
does not become quietly permanent.

Port faithfully, including the **confidence-aware escape hatch**
([ADR-0003](../../../docs/adr/0003-hard-snap-quantization-for-vocaloid.md)): a
note far from grid *with* high onset confidence is left in place and flagged, not
snapped. This is not a trivial port — the escape hatch reads confidence.

## Done when

- Converter unit-tested on the 01 fixtures: rests become gaps; tick→second
  conversion exact; a rest-terminated note's offset is the musical end.
- **Rust hard-snap agrees with `grid.ts`** on a shared fixture set — the same
  inputs through both implementations produce the same output, including escape-hatch
  cases. This cross-test is the drift guard and is the reason duplication is
  acceptable.
- An ADR records the quantizer's temporary home and the promotion path.

## Comments

**2026-11-20 — Input from 01** ([audit](../../../crates/sona-eval/reports/ust-format-audit.md)).
15% of real note starts sit on triplet lines (ノイズゴト is 96%), so a 16th-only port of `grid.ts`
would quantize them onto the wrong grid. The converter can build on `ust::Ust::timeline()` (absolute
ticks and seconds, tempo changes applied). Breath blocks are already classified separately.

**2026-11-20 — Done.**
- **Converter:** `Ust::to_reference(offset_s)` returns a `UstReference` (notes plus a parallel
  `rest_terminated` flag). Rests and breaths become gaps, tick→second conversion is exact (tempo
  changes included), and `UstReference::merge` combines split leads. Tested on the 01 fixtures.
- **Quantizer:** `src/quantize.rs` is a faithful port of `grid.ts`, including the escape hatch and
  JavaScript's half-rounding (`Math.round(-0.5) = -0`, which Rust's `round` gets wrong). Std-only,
  so it can be promoted.
- **Drift guard:** `fixtures/quantize/cases.json` (13 cases) is asserted by both
  `quantize.rs` and `web/lib/grid.test.ts`.
- **ADR-0014** records the temporary home and the promotion path.
- **Triplets:** `grid.ts` already accepts any spacing, so no code change was needed. The grid
  division is a per-song manifest field (02).
- **Optional `.mid`:** `sona-eval-ust-to-midi <in.ust> <out.mid> [--offset s]`.
