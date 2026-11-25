# 14 — Place the pasted lines by aligning them to the vocal

Status: resolved
Type: research
Blocked by: 01

## What

The user (2026-11-24): line timing comes from the audio; they drag only the lines it got wrong.

01/02 showed that a pasted lyric lands well (57% of moras, GAME lead 188.7 → 175.2 cost/min with
`fit_span_moras`) only when each line is placed in time, and that sona's notes can't place it (8–25%).
Find a local forced aligner that places each line (and ideally each mora) of known hiragana on the
separated lead vocal.

## Research

- Candidates, checked not assumed: CTC forced alignment with a Japanese kana or phoneme model
  (wav2vec2/HuBERT fine-tunes, torchaudio's MMS forced aligner), Whisper-family word timestamps with the
  text given as a prompt, the Montreal Forced Aligner's Japanese model, NeMo's aligner, and anything for
  singing. Ticket 05 (lyric transcription) may already have venvs and models: reuse them.
- **Licences** (sona is MIT; code and weights; non-commercial weights may only be a user-installed tool).
- Metrics on dev: line-start error in notes and milliseconds against the UST (01's line files in the
  scratchpad give the truth), the share of lines within one note, and then the full landing: placement
  and edit cost with `fit_span_moras` on the aligned lines (`sona-eval-cellstage --lyric-placement`).
  Compare with the UST-placed ceiling (57%, 175.2) and today's paste in order (4.9%, 188.7).
- Which lines fail (fast runs, held vowels, backing vocals left in the stem), so the editor can mark them
  for the user to drag.

## Rules

Dev only; no holdout reads. Audio and lyrics stay on the machine; per-song text and alignments in the
scratchpad only. Venvs under `~/.cache/sona/`. Build into your own worktree's `target/`.

## Acceptance

`crates/sona-eval/reports/line-alignment.md` with a table per candidate, a recommendation, and if one
wins under a rule committed before measuring, the port plan (ticket 04 uses it).

## Answer

Status: resolved. Report: `crates/sona-eval/reports/line-alignment.md`. Dev only; no holdout read.

**A local CTC forced aligner places the pasted lines on the lead stem well enough that the lyric lands as well
as it does with the UST's own line times.** Lines and spaces, GAME lead (out of sample):

| landing | lines within one note | cost/min | wrong moras/min | placed |
|---|---|---|---|---|
| today, pasted in order | — | 188.7 | 183.2 | 4.9% |
| ceiling: lines placed by the UST (01) | 100% | 175.2 | 83.4 | 56.7% |
| **lyrics CTC (`mrfakename/w2v-bert-2.0-music-lyrics-ctc`, MIT), lead, energy ends** | **92.7%** | **175.2** | **93.1** | **51.7%** |
| Japanese wav2vec2 kana+kanji (speech, Apache-2.0) | 93.5% | 184.1 | 98.4 | 49.0% |
| hiragana wav2vec2 (speech, Apache-2.0) | 87.2% | 186.9 | 94.1 | 51.2% |
| MMS forced aligner (CC-BY-NC) | 88.3% | 181.2 | 99.1 | 48.6% |
| Whisper large-v3 + stable-ts (MIT) | 93.5% | 186.6 | 144.1 | 25.2% |

- 84.6% of lines start on their own first note, with a median error of 25 ms. The stage (in sample) gets
  169.6 cost/min and 50.0% placed.
- **Line ends come from the lead's energy, not the CTC.** A CTC token ends at the onset peak, so a held last
  note falls outside its line (32.8% placed, 193.0 cost/min). The lead going 20 dB quiet for 100 ms gives the
  numbers above, as good as the UST's own ends. The threshold was set on dev.
- **Failures.**
  - 21 of the 28 lines more than one note off are one song's (amaten). Two soft, slow sections aren't heard;
    the aligner squeezes them and drags 10–11 lines each. The full vocals stem fails the same way.
  - 2 lines the lead stem doesn't carry (backing).
  - 5 slips of 2–3 notes at line breaks.
  - Fast runs and held vowels are not failure classes.
- **Doubt flag:** the most doubtful 20% of lines by the aligner's mean log-posterior catch 93% of those lines
  (AUC 0.91; without amaten, 5 of 7). The editor can mark them for dragging (15).
- **Selection rule.** Two candidates pass: the lyrics CTC and the Japanese kana+kanji speech model. The declared
  tie-break (most lines within one note) picks the speech model, by 3 lines: +0.8 [−2.4, +4.9] points. The
  lyrics model is better on the landing: edit cost −8.9 [−18.3, −2.4] on lead, −6.0 [−13.0, −0.8] on the stage,
  and its doubt flag (AUC 0.91 against 0.61). The speech model is about twice as fast. The tie-break should have
  ranked the landing. **Recommendation: the lyrics model. The choice is the user's** (MIT weights from
  unnamed training lyrics, about 2.5 min per song on 4 cores; or Apache-2.0 weights partly trained on
  JSUT, whose audio is for research, non-commercial research and personal use, about 1 min).
- **Landed in code (opt-in).**
  - `sona_core::lyric_fit::fit_lines`: lines placed in time, fitted by `fit_span_moras`.
  - `sona-eval-cellstage --lyric-line-times DIR`.
  - The Rust landing reproduces the prototype on 36 of 36 rows, and on the UST's own times it reproduces
    01's ceiling rows.
  - The default output is byte-identical.
  - Five behaviour tests; three mutants each caught.
- **Not run:**
  - MFA (Japanese model CC BY 4.0) and SOFA (MIT): they align utterances they are given, so they need the line
    times first.
  - NeMo with parakeet-ja (CC BY 4.0): another speech model.

Port plan: comment on 04.

## Comments

### 2026-11-24 — claimed; design and selection rule (declared before reading any result)

- **Text:** each line's hiragana is the lead UST's moras between 01's line starts (what a user pastes; perfect
  text, っ never present, tails as the UST sings them). Lines and spaces are the phrases (spec), lines alone
  beside them. Only the timing is new: every line (and space-separated piece) gets its span
  `[first mora's start, last mora's end]` from the aligner, on the separated lead stem (`lead.wav`).
- **Landing:** 01's (b+/r) with those times in place of the UST's: the output notes whose midpoint lies in a
  piece's aligned span get its moras by `fit_span_moras`. Scored by `sona-eval-cellstage --lyric-placement`
  (an opt-in `--lyric-line-times DIR`; the default output stays byte-identical), GAME lead (out of sample) and
  the stage (in-sample).
- **Line metrics:** line-start error against the UST's first note of the line, in ms and in notes (UST notes
  between the aligned start and the true one); share of lines within one note.
- **Selection rule.** A candidate *wins* when, on GAME lead, lines and spaces, rests:
  1. it keeps **at least three quarters of the ceiling's placement gain** over today: wrong moras per minute
     ≤ 108.3 (today 183.2, ceiling 83.4), with the paired song-bootstrap interval against today excluding 0;
  2. the edit cost is **not raised** against today's 188.7 (paired Δ upper bound under +2 cost/min);
  3. the stage does not get worse than today on either number.
  Among winners: the most lines within one note, then a licence sona can name as a user download (code and
  weights runnable by a user; MIT/Apache/BSD/CC-BY preferred over non-commercial), then speed. Aligner
  settings tuned on dev (window, blank handling) are in-sample and reported as such; no holdout is read here
  (confirmation goes with holdout4, bundled later, under the spec's rule).
- **Doubt flag:** the aligner's own per-line score is kept; its usefulness is the share of the lines more than
  one note off that the lowest-scoring 20% of lines catch (and the ROC AUC), reported, not gated.
