# 05 — Transcribing the lyric when none is pasted

Status: resolved
Type: research
Blocked by: none

## What

Find a local speech recogniser that turns a separated Japanese vocal into hiragana well enough to be the
fallback. Candidates to check, not assume: Whisper and its Japanese fine-tunes (kotoba-whisper, etc.),
ReazonSpeech, and anything trained on singing. For each: licence (can a user download and run it? sona
won't redistribute weights), size, CPU speed on a 3-minute vocal, and how kanji output becomes hiragana
(a reading step and its errors).

Score on dev: the mora error rate against the lead UST's moras (insertions, deletions, substitutions), and
the same after 01's fit, so a wrong mora is counted where the user will see it. Run everything locally; no
audio or lyric leaves the machine. Song-level results stay in the scratchpad; only pooled numbers go in
the report.

## Acceptance

A report with a table per candidate, a recommendation, and an estimate of how much fixing a fallback lyric
costs the user compared with pasting one.

## Comments

## Answer

Report: `crates/sona-eval/reports/lyric-transcription.md` (dev only, 9 songs, 5 336 UST moras; no holdout
read; transcripts and lyric readings stayed in the scratchpad; the report has pooled and per-song error
rates, no text).

- **Use Whisper large-v3-turbo** (faster-whisper, int8, CPU; MIT, 1.6 GB, no torch) on the lead stem,
  with Silero VAD at threshold 0.2, and read its kanji with **fugashi + unidic-lite** (word readings,
  particles as pronounced). Mora error rate **37.1% [22.6, 49.7]**. whisper-ja-760M (34.9%, −2.3 [−5.9,
  +0.4]) and large-v3 (36.8%, −0.3 [−6.1, +6.0]) are tied with it; turbo is 2.3× faster than large-v3 and
  has the best word timestamps (91% within 0.5 s of the UST note).
- Every Japanese speech fine-tune and both lyric-trained models found are significantly worse on these
  synthesised voices: kotoba-whisper +16.1 points, anime-whisper +23.0, ReazonSpeech k2 +29.6, Echora
  Kiki +10.8 (3 songs), w2v-BERT lyrics CTC +47.9 (5 songs; it returns nothing on four).
- faster-whisper's default VAD threshold (0.5) drops 10% of the sung moras before any model runs; 0.2
  keeps 98%. Keep 0.2.
- **Fixing cost:** the fallback leaves about 73 moras to fix in 26 places per sung minute; the lyric-site
  text pasted through the same reading step needs 41 in 17 (20.7% [11.0, 31.1]); the user's own hiragana
  needs none. As an illustration, reviewing a 3-minute song takes about 11 minutes: as long as typing it
  by ear on the hard songs, a few minutes on the easy ones. Whisper's word probabilities can steer the
  review (the 31% of moras below 0.7 hold 57% of the wrong ones).
- 01's fit was not measured (01 and 02 are open). In the spec's flow the user corrects the lyric before
  the fit, so the lyric-box numbers are the fallback's own cost.
