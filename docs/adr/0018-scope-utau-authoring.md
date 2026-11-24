# Scope: from "stops at MIDI" to a transcribe-to-UTAU/OpenUtau tool

**Status: Accepted (2026-11-22, maintainer decision).** This supersedes the "Output stops at MIDI; notation, UST/VSQX, and source separation are out of scope" line in `CONTEXT.md`. Grid alignment amended by [ADR-0029](0029-export-frame-bar-one.md).

## Context

sona's evaluation target has, since ADR-0015, been hand-made UTAU USTs (Pifuyuu's corpus). The review surface already holds everything a UST needs except lyrics: corrected, grid-snapped notes, the tempo and the grid. Users going from a song to a singable UTAU/OpenUtau project currently have to export MIDI and then re-enter lyrics and re-time everything by hand in another editor.

## Decision

sona becomes an all-in-one path from a song to an editable singing-synth project, built in milestones. Each milestone keeps the perception/interpretation split (ADR-0004): the Rust core still emits raw perception, and everything musical (lyrics, snapping, export formats) lives in the review surface.

1. **Milestone 1 (this ADR's first slice): lyrics + UST/USTX export.**
   - **Lyrics.** Each working note may carry a lyric. Lyrics are interpretation, so they live in the UI and the saved session. The annotation JSON from the core stays lyric-free. You can type a lyric on a selected note, or paste a lyric text, which is split into morae and assigned to consecutive notes.
   - **Export.** The UST (UTAU, Shift-JIS by default, UTF-8 with `Charset=UTF-8` when a lyric has no Shift-JIS form) and the USTX (OpenUtau YAML) are written from **exactly the notes the MIDI export uses**: snapped and corrected. *(2026-11-24: the MIDI did not in fact use them; it was written in stem seconds, up to one cell off the grid. Since [ADR-0029](0029-export-frame-bar-one.md) all three formats are written from one sequence.)*
   - **Grid alignment.** Notes are aligned to the singing-synth's own grid (tick 0 = a grid line), so they sit on UTAU's bar lines. That moves the vocal by the grid phase, which is less than one cell, relative to the stem; the export status reports the shift. *(Amended by [ADR-0029](0029-export-frame-bar-one.md): tick 0 is a bar line of the editor's frame. By default that is still the first grid line; once the user chooses bar 1, OpenUtau's beats and bars fall on the editor's.)*
2. **Later milestones** (each gets its own ADR when built):
   - **Pitch curves:** portamento and vibrato fitted from the f0 contour, editable, and exported as USTX `pitd` / UST `PBS`/`PBW`/`VBR`, with MIDI pitch bend as the fallback.
   - **One-click generation:** a local server that runs separation and `sona transcribe` for the web app.

## Consequences

- `CONTEXT.md` is updated: UST/USTX export and lyrics are in scope. Source separation is planned but not yet built (since built: ADR-0020).
- **Shift-JIS output.** Browsers can decode Shift-JIS but not encode it, so the exporter builds its encoding table from the platform's own decoder at export time, with no table shipped.
- **Testing.**
  - The UST writer is checked against sona's existing Rust UST parser (`sona-eval`'s `ust.rs`), using a committed synthetic golden file.
  - The USTX writer is unit-tested on structure.
  - Neither format's synthesis parameters (preutterance, envelopes, flags) are written beyond defaults. The output is a starting point for UTAU/OpenUtau, not a tuned render.
- **Licences are unaffected.** Pifuyuu's USTs are still never committed; tests use synthetic notes only.
