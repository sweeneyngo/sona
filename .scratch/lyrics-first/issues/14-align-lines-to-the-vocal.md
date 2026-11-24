# 14 — Place the pasted lines by aligning them to the vocal

Status: claimed
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
