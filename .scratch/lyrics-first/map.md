# Map: lyrics first

Spec: `spec.md`. Tickets: `issues/NN-*.md`, claimed and resolved as in `docs/agents/issue-tracker.md`.

## Notes

- 01 (metric and oracle) decides how lines become phrases. 03 (the start screen) needs no measurement and
  runs in parallel. 05 (fallback research) runs in parallel too.
- 04 wires the fit into transcription once 02 has picked the mechanism.

## Decisions so far

- 2026-11-24: lyrics are required; the fallback is lyric transcription; hiragana only for now (user).
- 2026-11-24: the fallback recogniser is Whisper large-v3-turbo with fugashi/unidic-lite readings and the VAD
  at 0.2: 37.1% mora error on dev, against 20.7% for pasted lyric-site text. It gives a draft to review,
  not a finished lyric (05, `crates/sona-eval/reports/lyric-transcription.md`).

## Fog

- Do users' line breaks match sung phrases? Pifuyuu's lyric texts have lines, but those are a lyric site's
  lines, not his phrases. Measure it (01).
- Repeated choruses: a pasted lyric may write a chorus once and mean it twice.
- Long vowels written out (ああ) vs held (ー) vs not written at all.
