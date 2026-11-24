# Map: lyrics first

Spec: `spec.md`. Tickets: `issues/NN-*.md`, claimed and resolved as in `docs/agents/issue-tracker.md`.

## Notes

- 01 (metric and oracle) decides how lines become phrases. 03 (the start screen) needs no measurement and
  runs in parallel. 05 (fallback research) runs in parallel too.
- 04 wires the fit into transcription once 02 has picked the mechanism.

## Decisions so far

- 2026-11-24: lyrics are required; the fallback is lyric transcription; hiragana only for now (user).
- 2026-11-24 (07): each language's units are known; Japanese first. っ is never a note, and a long-vowel
  tail is optional (18% have a note). Units from hiragana are 12.4% wrong today, 5.3% with no
  dictionary and 4.3% with an analyser. The best kanji readers tie at 3.8–4.1% (OpenJTalk family,
  unidic). Tickets 08–13; 11 and 13 wait on the user.
  [report](../../crates/sona-eval/reports/language-and-units.md)

## Fog

- Do users' line breaks match sung phrases? Pifuyuu's lyric texts have lines, but those are a lyric site's
  lines, not his phrases. Measure it (01).
- Repeated choruses: a pasted lyric may write a chorus once and mean it twice.
- Long vowels written out (ああ) vs held (ー) vs not written at all. 07: a tail (ー or a spelled-out
  long vowel) has its own UST note 18% of the time; 09 makes tails optional in the fit.
