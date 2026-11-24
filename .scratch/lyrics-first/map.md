# Map: lyrics first

Spec: `spec.md`. Tickets: `issues/NN-*.md`, claimed and resolved as in `docs/agents/issue-tracker.md`.

## Notes

- 01 (metric and oracle) decides how lines become phrases. 03 (the start screen) needs no measurement and
  runs in parallel. 05 (fallback research) runs in parallel too.
- 04 wires the fit into transcription once 02 has picked the mechanism.

## Decisions so far

- 2026-11-24: lyrics are required; the fallback is lyric transcription; hiragana only for now (user).
- 2026-11-24 (01): pasting in order places 4.9% of moras. Lines (and spaces) give phrases nearly as well as the
  UST's own phrases (GAME lead −13.5 vs −14.1 cost/min) if the fit lets rests hold moras (`fit_span_moras`); the
  shipped fit on honest phrases costs +16.5. Only 49% of line breaks sit on a sona rest, so 02 places lines by
  a song-level DP. [01](issues/01-lyric-placement-metric.md), `crates/sona-eval/reports/lyric-placement.md`.
- 2026-11-24 (02): sona's notes cannot place the lines (a Viterbi over lines × notes drifts: 8–25% of moras
  placed against the ceiling's 57%; not ported). Line timing must come from the audio (align the kana) or the
  user (mark each line's first note) — the user's call. [02](issues/02-fit-notes-to-lines.md).

## Fog

- Do users' line breaks match sung phrases? Partly (01): 59% of a lyric site's line breaks are UST rests,
  and 49% sit on a GAME lead rest. As phrases for the fit they work nearly as well as the UST's (01).
- Repeated choruses: a pasted lyric may write a chorus once and mean it twice.
- Long vowels written out (ああ) vs held (ー) vs not written at all.
