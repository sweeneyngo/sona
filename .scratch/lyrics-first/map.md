# Map: lyrics first

Spec: `spec.md`. Tickets: `issues/NN-*.md`, claimed and resolved as in `docs/agents/issue-tracker.md`.

## Notes

- 01 (metric and oracle) decides how lines become phrases. 03 (the start screen) needs no measurement and
  runs in parallel. 05 (fallback research) runs in parallel too.
- 04 wires the fit into transcription once 02 has picked the mechanism.

## Decisions so far

- 2026-11-24: lyrics are required; the fallback is lyric transcription; hiragana only for now (user).
- 2026-11-24 (03): the start screen asks for the lyric beside the BPM (hiragana, ー, small kana, っ;
  anything else named, never converted); Transcribe needs it, the fallback is shown off until 06. It goes
  with the job (`lyric=`, kept as `job.lyric`), the notes take it in order until 04, and sessions and
  re-transcription keep it. [ADR-0035](../../docs/adr/0035-lyrics-are-required.md),
  [03](issues/03-lyric-on-start-screen.md)

## Fog

- Do users' line breaks match sung phrases? Pifuyuu's lyric texts have lines, but those are a lyric site's
  lines, not his phrases. Measure it (01).
- Repeated choruses: a pasted lyric may write a chorus once and mean it twice.
- Long vowels written out (ああ) vs held (ー) vs not written at all.
