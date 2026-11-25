# 04 — Transcription fits the notes to the lyric

Status: needs-triage
Type: task
Blocked by: 03, 14

## What

`sona transcribe --lyric FILE` and the server job use 02's fit: the notes are split and joined to the
lyric, and each note carries its mora into the annotation and every export (UST `Lyric=`, USTX, MIDI lyric
meta events). Check that OpenUtau imports the lyrics from each format (the user checks by hand).

## Comments

- 2026-11-24 (02): placing lines from sona's notes failed; the line timing must come from the audio or from the
  user marking each line's first note. Pick one (user's call) before this ticket; the fit after it is
  `lyric_fit::fit_span_moras`.

- 2026-11-24, lead: 02 is resolved negative (sona's notes can't place lines). The user chose line timing from
  the audio (14), with drag-to-fix (15). The fit is `fit_span_moras` on the aligned lines.

- 2026-11-24 (14), port plan for the line placer. Report: `crates/sona-eval/reports/line-alignment.md`.
  1. **Aligner model** (the user picks; 14 recommends the first):
     - `mrfakename/w2v-bert-2.0-music-lyrics-ctc`: MIT, 600M, 2.3 GB, about 2.5 min per song on 4 cores;
     - `jonatasgrosman/wav2vec2-large-xlsr-53-japanese`: Apache-2.0, 300M, about 1 min.

     Either is a user download, never committed. ADR-0005 (no ML runtime in the crates) means it runs like
     the separators: `scripts/setup-aligner.sh` makes a venv and fetches the weights, and `sona serve` runs
     it on the job's lead stem. Later, the encoder can be exported to ONNX and run through ort as GAME is,
     with the Viterbi in Rust.
  2. **Alignment** (port `scripts/line-alignment/ctc_align.py`, about 100 lines):
     - emissions over 26 s windows with 2 s of context;
     - the pasted moras become tokens, with kana the vocabulary lacks falling back to katakana, unvoiced or
       large kana;
     - one Viterbi over the whole song, where a gap between pieces may also emit the `|` filler.

     Output per mora: its first and last frame and its mean log-posterior.
  3. **Line and piece spans.** The start is the first mora's first frame. The end is where the lead stays 20 dB
     below its 95th-percentile level for 100 ms, at most 4 s after the last mora's onset and never past the
     next piece's start (`score.py --end energy`). sona already computes an energy envelope; check that it
     matches the rule's 20 ms RMS.
  4. **Fit.** `sona_core::lyric_fit::fit_lines(notes, moras, &[LineSpan])` (landed, tested), with lines and
     spaces as pieces, on the output notes in grid cells. Notes outside every span carry no mora.
  5. **Doubt (ticket 15).** Each line's mean log-posterior; flag the lowest 20%, or use a threshold set on dev.
     On dev that catches 93% of the lines more than one note off. Pace (seconds per mora far from the song's
     median) is an equal second signal. Dragging a line re-runs `fit_lines` for it and its neighbours.
  6. **Checks before shipping.**
     - A pasted chorus written once but sung twice: CTC can't skip sung text, so test what happens and
       consider letting a piece be skipped at a cost.
     - Real pastes with tails written or not (09).
     - Opt-in until holdout4 confirms it, bundled with the other lyrics-first winners.
     - The default transcription stays byte-identical.
