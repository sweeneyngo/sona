# UST fixtures (synthetic)

Hand-authored USTs for the UST parser and format-audit unit tests
(`src/ust.rs`, `src/ust_audit.rs`). **None of these are Pifuyuu's USTs.** The
author's terms forbid redistributing the USTs, converted forms (VSQX/SVP/CCS…)
included, and this repo is public. The real corpus lives only in the git-ignored
cache (`$SONA_UST_DIR`, default `crates/sona-eval/data/pifuyuu-ust`).

They copy the real format exactly: `[#VERSION]` / `[#SETTING]` / `[#NNNN]` /
`[#TRACKEND]` sections, 480 ticks per quarter, kana lyrics, `R` rests, and UTAU
synthesis parameters the parser must ignore. Each file covers one case:

| File | Encoding | Covers |
| --- | --- | --- |
| `staccato.ust` | Shift-JIS, CRLF | 150 BPM; every note rest-terminated; full synthesis params (`PBS`, `Envelope`, `Flags`…); a 50 ms 32nd below the 100 ms prune floor; 32nd-grid note ends |
| `legato.ust` | Shift-JIS, CRLF | 120 BPM; contiguous notes; 8th-triplets; one start nudged 30 ticks off-grid; a `息` breath that terminates the note before it |
| `tempo-change.ust` | Shift-JIS, CRLF | katakana; lowercase `r` rest; a note-level `Tempo=` change (128 → 64) mid-song |
| `utf8.ust` | UTF-8, LF | `Charset=UTF-8` declaration; VCV lyrics (`a ら`, `- ん`); `ゔ`, which has no CP932 code point and so only survives the UTF-8 path |
| `sona-export.ust` | Shift-JIS, CRLF | **Generated, not hand-authored**: the review surface's UST exporter (`web/lib/ust.ts`) output for four synthetic notes (`web/lib/export.test.ts`, regenerate with `UPDATE_GOLDEN=1`). The Rust parser must read back the exported notes; the web test must reproduce these bytes. |

The tests check exact expected values for these files (rest-terminated counts,
grid classes, residuals, durations), so editing one means updating its test.
To regenerate the Shift-JIS files from UTF-8 sources:
`iconv -f UTF-8 -t CP932 src.ust | sed 's/$/\r/' > staccato.ust`.
