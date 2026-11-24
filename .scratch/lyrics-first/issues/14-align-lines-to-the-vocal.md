# 14 — Place the pasted lines by aligning them to the vocal

Status: ready-for-agent
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
