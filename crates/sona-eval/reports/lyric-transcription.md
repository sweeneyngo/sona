# Lyric transcription as the fallback (lyrics-first 05)

Date: 2026-11-24. Dev only: the 9 dev songs that have a lead stem (plus-space has none), 5 336 UST
moras over 27.2 sung minutes (196 moras a minute). No holdout song was touched. Everything ran on this
machine; no audio, lyric or transcript left it. Per-song transcripts and lyric readings stay in the
scratchpad; this report holds numbers only.

## Answer

- **Recommendation: Whisper large-v3-turbo through faster-whisper (int8, CPU), with fugashi + unidic-lite
  reading its kanji into kana.** Mora error rate on dev **37.1% [22.6, 49.7]**. The three best are tied:
  whisper-ja-760M 34.9% (−2.3 [−5.9, +0.4] against turbo) and Whisper large-v3 36.8% (−0.3 [−6.1, +6.0]).
  Turbo is the fastest of the three (2.3× faster than large-v3), has the best word timestamps (91% of
  word starts within 0.5 s of the UST note, median 0.16 s), is MIT-licensed from OpenAI, downloads ready
  to run (1.6 GB) and needs no torch.
- **A transcribed lyric is a rough draft.** It gets about two moras in three right. Per sung minute the
  user fixes about 73 moras in 26 places. The lyric-site text pasted through the same reading step needs
  41 moras in 17 places (20.7% [11.0, 31.1], 16.4 points better), and the user's own hiragana needs none.
  On the songs Whisper hears well (8–17% MER on four songs) the draft saves most of the typing; on the
  worst (amaten 54%, innocent-addict 63%) fixing it costs about as much as typing the lyric by ear.
- **Highlight doubtful words.** Whisper's word probabilities are calibrated enough to steer the review:
  the 31% of moras below probability 0.7 hold 57% of the wrong ones.
- **Set the voice-activity threshold to 0.2.** At faster-whisper's default (0.5) every model loses 10%
  of the moras before decoding, more than separates the top three; 0.2 keeps 98%.
- **Speech models trained on Japanese, and the singing-trained models, do worse.** kotoba-whisper
  (+16.1 points), anime-whisper (+23.0) and ReazonSpeech (+29.6) are significantly worse than turbo.
  The two lyric-trained models found, Echora Kiki and a w2v-BERT lyrics CTC, are worse still on these
  synthesised voices (Kiki +10.8 on 3 songs; the CTC model returns nothing on four of five).

## How it was measured

- **Input:** the lead stem sona already makes (demucs htdemucs, then UVR_MDXNET_KARA_2), 16 kHz mono.
- **Truth:** every note of the lead UST files (all lead parts, sorted by onset), one mora per kana as
  below. Pifuyuu's dev USTs are all CV hiragana, so they are the moras sung.
- **Mora rules** (both sides, `scripts/lyric-transcription/moras.py`): katakana becomes hiragana; a small
  kana joins the kana before it (きゃ is one mora) unless that is っ, ん or ー; っ and ん are moras of
  their own; ー is a mora, written as the vowel before it (the UST writes a held vowel as that vowel);
  を = お, づ = ず, ぢ = じ (one sound, two spellings). A VCV lyric `a か` or `- か` keeps its CV part
  `か`; `R` and breaths are not moras. No dev UST has a VCV lyric, so that rule never fired here.
- **Reading step** (kanji to kana): fugashi with unidic-lite, reading a whole segment at once. A word
  takes UniDic's written reading (東京 → とうきょう), a particle its pronunciation (は → わ, へ → え). Latin
  words and numerals have no reading and are dropped (counted as "unread"). pykakasi and pyopenjtalk
  were run as alternatives (below).
- **Mora error rate (MER)** = (substitutions + insertions + deletions) / UST moras, from a Levenshtein
  alignment per song, pooled over songs. 95% intervals are song bootstraps (10 000 resamples).
- **Voice activity:** every Whisper-family model and ReazonSpeech see the Silero VAD spans that
  faster-whisper computes, at threshold 0.2. The default 0.5 drops 10.2% of dev's UST moras before any
  recogniser runs (kamaitachi-ni-yoru loses 53%), which caps every model; 0.2 keeps 98.3%
  (`vad_cover.py`). The CTC model and Echora Kiki take the whole stem in windows, as their cards say.
- **Settings:** Whisper models through faster-whisper 1.2.1 (CTranslate2 int8, beam 5, language ja, no
  conditioning on the previous window, word timestamps); ReazonSpeech through sherpa-onnx (int8,
  modified beam search, spans up to 29 s); the CTC model greedy, 30 s windows with 2 s of context;
  Echora Kiki 60 s windows overlapping 12 s, lines kept by the window that owns their midpoint, greedy
  with repetition penalty 1.05, backing vocals (its parentheses) dropped.
- **Speed:** this machine is an Intel Alder Lake laptop CPU (16 threads, no CUDA GPU; Iris Xe only) and
  was heavily loaded by other jobs throughout (load average 12–69 on 16 threads). Each model ran on 6–8
  threads. "xRT" is seconds of processing per second of audio, wall clock; CPU-seconds are inflated by
  thread contention. Read the speeds as an upper bound and as a ranking, not as what a user's idle
  machine would see.
- **Timing quality:** for each correctly recognised mora that starts a timestamped word, the gap between
  the word's start and the UST note's onset.

## Results

Pooled over the 9 dev songs unless the songs column says less. "vs turbo" is the paired song bootstrap
of the MER difference against Whisper large-v3-turbo. Speed is wall-clock minutes for 3 minutes of
vocal on this loaded machine (6–8 threads, load 14–69). Timing is for correctly recognised word-initial
moras (segment starts for the two models without word timestamps).

| Candidate | Licence (weights) | Download | Min per 3-min vocal | Songs | MER [95% CI] | vs turbo | S / I / D | Timing: median gap, ≤ 0.5 s |
|---|---|---|---|---|---|---|---|---|
| Pasted lyric-site text, read by fugashi | — | — | — | 9 | **20.7%** [11.0, 31.1] | −16.4 [−25.6, −8.4] | 2.5 / 6.5 / 11.7 | — |
| whisper-ja-760M | Apache-2.0 | 1.5 GB | 10.7 | 9 | **34.9%** [21.3, 46.9] | −2.3 [−5.9, +0.4] | 16.2 / 6.6 / 12.1 | 0.34 s, 76% |
| Whisper large-v3 | MIT | 3.1 GB | 20.0 | 9 | **36.8%** [22.1, 50.8] | −0.3 [−6.1, +6.0] | 14.4 / 5.5 / 16.9 | 0.29 s, 81% |
| **Whisper large-v3-turbo** | MIT | 1.6 GB | **8.6** | 9 | **37.1%** [22.6, 49.7] | — | 16.2 / 5.8 / 15.1 | **0.16 s, 91%** |
| Whisper small | MIT | 0.5 GB | 3.2 | 9 | 42.3% [26.7, 56.7] | +5.1 [−0.8, +12.0] | 20.0 / 5.4 / 16.9 | 0.15 s, 92% |
| kotoba-whisper v2.0 | Apache-2.0 | 1.5 GB | 9.5 | 9 | 53.2% [39.2, 65.3] | +16.1 [+10.8, +21.7] | 26.6 / 6.0 / 20.7 | segments only |
| anime-whisper | MIT | 3.0 GB | 9.7 | 9 | 60.2% [44.1, 74.2] | +23.0 [+15.6, +29.8] | 28.4 / 7.5 / 24.3 | segments only |
| ReazonSpeech k2 v2 (spans ≤ 29 s) | Apache-2.0 | 0.16 GB | **0.9** | 9 | 66.8% [42.8, 87.2] | +29.6 [+18.5, +40.7] | 3.5 / 1.2 / 62.1 | 0.21 s, 89% |
| Echora Kiki (stopped after 3 songs) | Apache-2.0 | 1.8 GB | 30.4 | 3 | 56.3% [18.8, 73.9] | +10.8 [+2.3, +19.7] | 15.4 / 5.0 / 35.8 | 0.08 s, 92% (line starts) |
| w2v-BERT lyrics CTC (stopped after 5 songs) | MIT | 2.3 GB | 20.0 | 5 | 97.8% [92.6, 100] | +47.9 [+40.9, +59.1] | 0.1 / 0.0 / 97.7 | — |

ReazonSpeech with shorter spans: ≤ 12 s 63.4% [44.5, 79.2], ≤ 6 s 62.8% [47.0, 76.2]; at the default
VAD threshold (0.5) 70.1%. It hears the songs it hears well (yamineko 10.9%) and returns almost nothing
on five (≥ 89% MER, nearly all deletions). The CTC model returns nothing at all on four of its five songs,
on the stem and on the full mix alike (checked on a 30 s window of amaten): it does not hear these voices.
Echora Kiki was stopped after 3 songs to free the machine: it was worse than turbo on each of them and
ten times slower than real time.

Per song (MER; numbers only):

| Song | Pasted text | whisper-ja-760M | large-v3 | large-v3-turbo | small | kotoba v2.0 | anime-whisper | ReazonSpeech k2 |
|---|---|---|---|---|---|---|---|---|
| amaten | 39.9% | 56.1% | 71.5% | 54.2% | 79.1% | 80.4% | 87.3% | 93.7% |
| atamaro-deluxe | 17.8% | 43.9% | 43.4% | 56.7% | 49.6% | 64.7% | 82.3% | 94.6% |
| azemichi-ni-ochita-ginga | 7.2% | 17.9% | 15.7% | 16.5% | 19.2% | 30.5% | 30.7% | 23.3% |
| fukagyakuteki-henyou | 13.2% | 49.0% | 45.8% | 49.2% | 60.4% | 67.8% | 77.8% | 95.6% |
| innocent-addict | 49.2% | 61.6% | 62.9% | 63.2% | 62.3% | 66.7% | 65.7% | 89.5% |
| kamaitachi-ni-yoru | 10.5% | 12.3% | 21.0% | 13.0% | 20.5% | 35.1% | 39.0% | 36.7% |
| my-axiom | 22.7% | 34.7% | 31.4% | 34.2% | 41.2% | 62.6% | 70.5% | 93.2% |
| telescope-nevermore | 6.9% | 13.5% | 12.5% | 16.7% | 17.9% | 30.6% | 47.1% | 33.5% |
| yamineko | 5.5% | 6.2% | 7.0% | 7.9% | 10.1% | 22.1% | 20.3% | 10.9% |

**The reading step** (kanji → kana), on the same inputs:

| Reading step | Pasted lyric-site text | turbo's transcripts |
|---|---|---|
| fugashi + unidic-lite (word reading, particles as pronounced) | **20.7%** (S 2.5%) | **37.1%** |
| pykakasi (GPL-3.0) | 23.6% (S 5.4%) | 38.9% |
| pyopenjtalk-plus (pronunciation; reads Latin letter by letter) | 28.9% (S 7.2%) | 41.1% |

fugashi's own misreadings are visible in the pasted text, where the words are right: 私 as わたくし
where a song sings わたし, 明日 as あす, 言う as ゆう. They are 2.5% of moras. The pasted text's other
errors are the text itself: choruses written once (deletions, 11.7%), English lines with no kana reading
(692 Latin characters), and asides the UST doesn't sing.

**Which stem.** Turbo on demucs's vocals stem (before the UVR lead split, so backing vocals included)
scores 36.3% [22.7, 48.2], −0.9 [−3.7, +1.4] against the lead stem: no difference. The fallback can use
whichever stem the job has; the lead stem is the one the notes come from, so its timestamps line up.

## What fixing a fallback lyric costs, against pasting one

The fallback's errors are what the user fixes in the lyric box before the notes are fitted (the spec's
flow step 1), so the lyric-box MER is where the user sees them. Per minute of sung song (196 moras):

| Lyric source | Moras to fix per minute | Places to fix per minute | Edits in runs of ≥ 5 |
|---|---|---|---|
| The user's own hiragana (the main path) | ≈ 0 | ≈ 0 | — |
| A lyric site's kanji text, read by fugashi | 41 | 17 | 50% |
| Fallback: Whisper large-v3-turbo + fugashi | 73 | 26 | 50% |
| Typing it by ear from nothing | 196 | — | — |

"Places" are runs of consecutive edits: each is one find-and-select. The lyric-site text loses mostly in
blocks that are cheap to fix (choruses written once, English lines); the fallback's errors are spread
through the song, half of them in missing or garbled lines, half scattered single moras.

As an illustration only (assumed costs, not measured: 5 s to find and select a place, 0.5 s per mora
typed or deleted, one listen-through of the song to find the places), a 3-minute song costs about:

- **pasting the user's hiragana:** under a minute;
- **pasting lyric-site text through a converter:** about 8 minutes (60 + 17 × 5 + 41 × 0.5 s per minute);
- **reviewing the fallback:** about 11 minutes (60 + 26 × 5 + 73 × 0.5 s per minute);
- **typing by ear:** about 11 minutes (196 × 0.5 s plus two extra listens per minute).

So the fallback saves typing on the songs it hears well (yamineko 7.9%, kamaitachi-ni-yoru 13.0%,
azemichi-ni-ochita-ginga and telescope-nevermore about 17%: a few minutes of fixes) and costs as much as
typing on the ones it doesn't (amaten 54%, innocent-addict 63%). Pasting is several times cheaper in
every case.

**Highlighting doubtful words would cut the read-through.** Whisper's word probabilities are informative
(`confidence.py`, turbo, dev): the 31% of moras below probability 0.7 hold 57% of the wrong (substituted
or inserted) moras.

| word probability | share of moras | wrong |
|---|---|---|
| 0.0–0.3 | 11% | 50% |
| 0.3–0.5 | 8% | 51% |
| 0.5–0.7 | 12% | 34% |
| 0.7–0.9 | 17% | 24% |
| 0.9–1.0 | 52% | 12% |

Deletions have no word to highlight; a gap between segments where the stem has voice is the only sign.

01's fit (MER counted after the notes are fitted) is not measured: 01 and 02 are not resolved yet, and
in the spec's flow the user corrects the lyric before the fit, so the lyric-box numbers above are the
user-visible cost of the fallback itself.

## Setup a user would follow (for ticket 06)

Like the separators and GAME: the user installs it, sona never ships the weights. About 0.6 GB of Python
packages (no torch; a quarter of it is the UniDic dictionary) plus the 1.6 GB model, CPU only.

```sh
python3.12 -m venv ~/.local/share/sona/lyric-venv
~/.local/share/sona/lyric-venv/bin/pip install "faster-whisper==1.2.1" "fugashi==1.5.2" "unidic-lite==1.0.8"
# download the model once (1.6 GB, MIT, into the Hugging Face cache)
~/.local/share/sona/lyric-venv/bin/python -c \
  "from faster_whisper import WhisperModel; WhisperModel('large-v3-turbo', device='cpu', compute_type='int8')"
```

Then sona runs, on the lead stem it already has: `WhisperModel('large-v3-turbo', device='cpu',
compute_type='int8').transcribe(stem, language='ja', beam_size=5, vad_filter=True,
vad_parameters={'threshold': 0.2}, condition_on_previous_text=False, word_timestamps=True)`, reads each
segment with fugashi + unidic-lite as in `moras.py`, and shows the hiragana in the lyric box, one segment
per line, for review. `scripts/lyric-transcription/transcribe.py` and `moras.py` are the reference
implementation; 06 should port the mora rules and keep the VAD threshold.

## Reproduce (dev)

Scripts in `crates/sona-eval/scripts/lyric-transcription/` (no lyric data in them); every output they
write is lyric-derived and goes to a directory outside the repo (`$W` below).

```sh
S=crates/sona-eval/scripts/lyric-transcription; PY=~/.cache/sona/lyric-asr/venv/bin/python; export PYTHONPATH=$S
$S/run-dev.sh $W/out whisper-large-v3-turbo                  # and the other candidate names
$PY $S/score.py refs <sona-eval dump dir for dev> $W/refs     # the UST's moras, from 03's dumps
$PY $S/score.py text crates/sona-eval/data/pifuyuu-lyrics $W/refs $W/score/paste-fugashi
$PY $S/score.py asr $W/out/whisper-large-v3-turbo $W/refs $W/score/whisper-large-v3-turbo
$PY $S/score.py table $W/refs $W/score/*                      # numbers only
$PY $S/score.py paired $W/refs $W/score/whisper-large-v3-turbo $W/score/whisper-ja-760M
$PY $S/vad_cover.py $W/refs crates/sona-eval/data/pifuyuu-stems 0.5 0.2
$PY $S/confidence.py $W/out/whisper-large-v3-turbo $W/refs; $PY $S/reading_errors.py $W/out/whisper-large-v3-turbo $W/refs
$PY $S/score.py selftest; $PY $S/moras.py                     # alignment and mora-rule checks
```

## What else these numbers could be measuring

- **The VAD, not the model.** At faster-whisper's default threshold every Whisper model loses a tenth of
  the moras before decoding (kamaitachi-ni-yoru half). Measured directly and fixed (0.2 keeps 98.3%);
  ReazonSpeech went 70.1% → 66.8% MER with the change. The remaining 1.7% are deletions every VAD-fed
  model shares.
- **The reading step.** On the pasted lyric text, where the words are right, fugashi's readings leave
  2.5% substitutions (pykakasi 5.4%, pyopenjtalk 7.2%). In turbo's transcripts, moras read from kanji
  are substituted less often (16.5% of 2 055) than moras Whisper wrote in kana (21.2% of 2 475): most
  substitutions are mishearings, not misreadings (`reading_errors.py`).
- **English.** innocent-addict and amaten have English lines that the UST spells in kana. Whisper writes
  them in Latin letters, which have no reading here, so they count as deletions (turbo: 568 Latin
  characters dropped). pyopenjtalk reads Latin letter by letter, which is worse (innocent-addict 96.8%).
  An English-to-kana step is a possible gain on those two songs only.
- **Overlapping lead parts.** Four songs have two lead parts that overlap briefly (10–59 note pairs);
  the reference sorts them by onset, which interleaves the overlapping moras. The same for every
  candidate.
- **The singers.** All nine dev songs are sung by voice synthesisers (Hatsune Miku in four, Kasane Teto,
  Gackpoid, Yuki, Otomachi Una, めろう) at 123–250 BPM. Models may do better on human singers; nothing
  here measures that. ReazonSpeech and the CTC model return almost nothing (≥ 85% MER, nearly all
  deletions) on four or five of the songs, so their pooled numbers say "fails on many voices", not
  "slightly worse".
- **Two outlier songs.** amaten and innocent-addict (both with English lines) are the two worst for the
  pasted text and among the three worst for every Whisper model. Without them turbo is at 29.7%,
  whisper-ja-760M 26.6% and the pasted text 12.5%: the ranking holds, the levels drop.
- **Holdout.** No holdout song was read. The only choices made on dev are the VAD threshold (by coverage,
  not MER), ReazonSpeech's span cap and the reading step.

## Candidates checked

| Candidate | What it is | Code licence | Weights licence | Download |
|---|---|---|---|---|
| Whisper large-v3 | OpenAI, multilingual | MIT (openai-whisper, faster-whisper) | MIT (OpenAI's release; the HF card says Apache-2.0) | 3.1 GB (CTranslate2 fp16, Systran) |
| Whisper large-v3-turbo | large-v3 with a 4-layer decoder | MIT | MIT | 1.6 GB |
| Whisper small | size baseline | MIT | MIT | 0.5 GB |
| kotoba-whisper v2.0 | large-v3 distilled to a 2-layer decoder on ReazonSpeech Japanese | MIT | Apache-2.0 (the -faster conversion is tagged MIT) | 1.5 GB |
| anime-whisper (litagin) | kotoba-whisper v2.0 fine-tuned on 5 300 h of anime/visual-novel voice acting | MIT | MIT | 3.0 GB; 0.8 GB after a local int8 conversion |
| whisper-ja-760M (efwkjn) | turbo with a pruned vocabulary, decoder trained on 45 000 h of Japanese incl. anime; "also trained for lyrics but untested" | — | Apache-2.0 (LICENSE file; the card has no licence tag) | 1.5 GB; 0.8 GB after a local int8 conversion |
| ReazonSpeech k2 v2 | Zipformer transducer, 35 000 h Japanese TV (sherpa-onnx) | Apache-2.0 | Apache-2.0 | 0.16 GB (int8) |
| w2v-BERT 2.0 music lyrics CTC (mrfakename) | character CTC trained on whole songs (full mixes), multilingual incl. kana/kanji | MIT | MIT | 2.3 GB |
| Echora Kiki (hcxlabs) | MOSS-Transcribe-Diarize 0.9B (Whisper encoder + Qwen3 decoder) fine-tuned for lyrics, 60 s windows | Apache-2.0 (custom model code, run with `trust_remote_code`) | Apache-2.0 | 1.8 GB |

Every one of these can be downloaded and run locally by a user without an account or a gate, and none
needs to be redistributed by sona. Things a user (or the project) may care about:

- **Training data provenance.** anime-whisper is trained on a dataset of commercial visual-novel voice
  lines, whisper-ja-760M on several anime/visual-novel sets, and Echora Kiki on lyrics data its authors
  don't describe. The weights' licences allow use; whether the data was licensed is the authors' claim.
  Whisper, kotoba-whisper and ReazonSpeech come from organisations that publish their data sources
  (ReazonSpeech's corpus is usable for information analysis under Japanese copyright law art. 30-4).
- **Echora Kiki runs its repository's Python** (`trust_remote_code`). The three files were read before
  running (they import only torch and transformers) and the revision was pinned
  (b35a2d893c8c287c6f77da7c2931ac17df0b90df). It also needs transformers 5, so a separate venv.
- **pykakasi is GPL-3.0.** Only an alternative reading step here; fugashi (MIT/BSD) and unidic-lite are
  what the recommendation uses.

Found and not run, with the reason:

- **ReazonSpeech NeMo v2** (Apache-2.0, 2.5 GB): needs the NeMo toolkit; same training data as k2 v2,
  which was run.
- **kotoba-whisper v2.1 / v2.2**: their cards describe them as v2.0 plus punctuation (and, in v2.2,
  speaker diarisation) pipelines; the recogniser is v2.0, which was run.
- **Echora MOSS 0.9B v1** (hcX02, Apache-2.0): superseded by Kiki, which continues from it.
- **SongPrep-7B** (Tencent): Chinese and English only, 16 GB.
- **Sylla 600M** (GPL-3.0) and **NextFire mms-300m karaoke aligner** (CC BY-NC-SA 4.0, Japanese
  romaji): forced aligners, not recognisers. The aligner is a candidate for placing a pasted lyric's lines
  (02), not for the fallback.
- **Published numbers:** the IdolSongsJp corpus paper (arXiv 2507.01349) reports Whisper large at
  40–44% character error on Japanese idol songs, about the same on the mix, a demucs stem and the dry
  lead; insertions (hallucinations) dominate. No Japanese singing model with a published benchmark was
  found.
