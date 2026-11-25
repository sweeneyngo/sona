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
  unidic). Tickets 08–13.
  [report](../../crates/sona-eval/reports/language-and-units.md)
- 2026-11-24 (01): pasting in order places 4.9% of moras. Lines (and spaces) give phrases nearly as well as the
  UST's own phrases (GAME lead −13.5 vs −14.1 cost/min) if the fit lets rests hold moras (`fit_span_moras`); the
  shipped fit on honest phrases costs +16.5. Only 49% of line breaks sit on a sona rest, so 02 places lines by
  a song-level DP. [01](issues/01-lyric-placement-metric.md), `crates/sona-eval/reports/lyric-placement.md`.
- 2026-11-24 (02): sona's notes cannot place the lines (a Viterbi over lines × notes drifts: 8–25% of moras
  placed against the ceiling's 57%; not ported). Line timing must come from the audio (align the kana) or the
  user (mark each line's first note) — the user's call. [02](issues/02-fit-notes-to-lines.md).
- 2026-11-24 (user, after 07): **hiragana only** (no kanji reader: 11 is wontfix for now); English words
  in a Japanese song are written as the **English word on its first note, then `+`** (12); **Japanese only
  for now** (13 deferred).
- 2026-11-24 (03): the start screen asks for the lyric beside the BPM (hiragana, ー, small kana, っ;
  anything else named, never converted); Transcribe needs it, the fallback is shown off until 06. It goes
  with the job (`lyric=`, kept as `job.lyric`), the notes take it in order until 04, and sessions and
  re-transcription keep it. [ADR-0035](../../docs/adr/0035-lyrics-are-required.md),
  [03](issues/03-lyric-on-start-screen.md)
- 2026-11-24 (08): `web/lib/units/` gives Japanese units with the UST's conventions (っ no note, marks
  `clipped`; tails `optional`; `mora_of` kinds; refused characters with positions). Dev 5.35% with tails
  optional vs pron-merge 5.14% (paired [−0.6, +1.2] pts). The gap is the particle は (3.64% vs 5.07%
  with particles scored equal). Laid in order, only ー and おう are held. The USTs give えい (67%) and a
  repeated vowel (68%) their own note, and skipping every tail is worse (6.7% vs 8.3%). Edit → Lyrics…
  is hiragana-only now. [08](issues/08-japanese-sound-units.md)
- 2026-11-24: the fallback recogniser is Whisper large-v3-turbo with fugashi/unidic-lite readings and the VAD
  at 0.2: 37.1% mora error on dev, against 20.7% for pasted lyric-site text. It gives a draft to review,
  not a finished lyric (05, `crates/sona-eval/reports/lyric-transcription.md`).
- 2026-11-24 (14): a local CTC forced aligner places the pasted lines. `mrfakename/w2v-bert-2.0-music-lyrics-ctc`
  (MIT), forced through the song's moras on the lead stem, starts 92.7% of lines within one note. Line ends come
  from the lead's energy. The lyric then lands as with the UST's own line times: GAME lead 175.2 cost/min (ceiling
  175.2, today 188.7), 51.7% placed (ceiling 56.7%, today 4.9%). The aligner's score flags 93% of the misplaced
  lines in its lowest 20%. A speech model (Apache-2.0) also passes and is the declared tie-break's pick, by 3
  lines; it lands worse (+8.9 cost/min). Which to port is the user's call. `fit_lines` and `--lyric-line-times`
  landed (opt-in). [14](issues/14-align-lines-to-the-vocal.md),
  `crates/sona-eval/reports/line-alignment.md`; port plan on 04.

## Fog

- Do users' line breaks match sung phrases? Partly (01): 59% of a lyric site's line breaks are UST rests,
  and 49% sit on a GAME lead rest. As phrases for the fit they work nearly as well as the UST's (01).
- Repeated choruses: a pasted lyric may write a chorus once and mean it twice.
- Long vowels written out (ああ) vs held (ー) vs not written at all. 07: a tail (ー or a spelled-out
  long vowel) has its own UST note 18% of the time; 09 makes tails optional in the fit.
- The particle は (and へ): UTAU users type it as sung (わ, え), and the dev USTs do. A user who types は gets
  "ha" sung. A one-line hint by the lyric box ("type particles as sung: わ, え") may be enough; nothing
  measured yet.
