# Placing pasted lines by aligning them to the vocal (lyrics-first 14)

Date: 2026-11-24. **Dev only** (the 9 dev songs with a lyric text; 27.5 min, 384 lines, 5,245 UST notes). No
holdout split was read. GAME lead + octave is out of sample; the learned stage is in sample on dev. The landing,
the line metrics and the selection rule were committed before any result was read (2eb528d).

Pifuyuu's lyrics, USTs and stems stayed on this machine. Every per-song text, alignment and line-time file is in
the session scratchpad; this report carries pooled numbers and per-song rates only. The only downloads were
public models and packages.

## Answer

- **A singing-trained CTC model aligns the pasted kana well enough to place the lines.**
  `mrfakename/w2v-bert-2.0-music-lyrics-ctc` (MIT weights), forced through the whole song's moras on the lead
  stem, starts **84.6%** of lines on the UST's own first note and **92.7% within one note**. The median start
  error is 25 ms (21 ms early). Per mora, 78.5% land on their own note and 90.9% within one.
- **With these times the lyric lands as well as with the UST's own line times.** Lines and spaces, fitted with
  `fit_span_moras` (via the new `fit_lines`), on GAME lead:
  - edit cost **175.2** cost/min, the ceiling's 175.2 (Δ against the ceiling −0.0 [−1.9, +1.3]); today 188.7;
  - **51.7%** of moras placed (ceiling 56.7%, today 4.9%); **93.1** wrong moras per minute (ceiling 83.4, today
    183.2), so it keeps **90%** of the ceiling's placement gain.
  - On the stage (in sample): 169.6 cost/min (ceiling 167.4, today 180.2), 50.0% placed (ceiling 55.3%).
- **Two candidates pass the declared rule, and its tie-break falls on a difference inside the noise.**
  - The lyrics model passes: 93.1 ≤ 108.3 wrong moras per minute (paired against today −90.1 [−101.0, −77.3]);
    edit cost Δ against today −13.5 [−32.1, −2.5], upper bound under +2; the stage better than today on both.
  - The speech-trained Japanese wav2vec2 (`jonatasgrosman/wav2vec2-large-xlsr-53-japanese`, Apache-2.0)
    passes too: 98.4 wrong/min, edit cost −4.7 [−14.8, +1.8].
  - The tie-break declared for winners was "the most lines within one note". It picks the speech model, 93.5%
    against 92.7%: 3 lines, a paired difference of +0.8 [−2.4, +4.9] points.
  - On what the rule was meant to protect, the lyrics model is better:
    - edit cost −8.9 [−18.3, −2.4] cost/min against the speech model on GAME lead (stage −6.0 [−13.0, −0.8]);
    - wrong moras −5.3 [−12.2, +1.7] per minute;
    - its score flags bad lines (AUC 0.91 against 0.61), which ticket 15 needs.
  - The speech model is about twice as fast and half the size (300M against 600M parameters).

  The tie-break was the wrong one: it should have ranked winners on the landing. I recommend the lyrics model,
  and the choice is the user's (below). No other candidate passes.
- **A line ends where the lead goes quiet, not where CTC says.** A CTC model's last token ends at the onset peak
  of the line's last mora. Taken as the line's end, it drops each line's held last note: 32.8% placed and an edit
  cost of 193.0, above today. Ending each piece where the lead stem stays 20 dB below its loud level for 100 ms
  (at most 4 s after the last onset, never past the next piece's start) gives the numbers above. It is as good
  as the UST's own line ends (51.0% placed with aligned starts and the UST's ends). The rule's threshold was set
  on dev against the UST's note offsets (in sample).
- **What fails.** 28 of 384 lines are more than one note off, and 21 of them are in one song (amaten). There,
  two soft, slow sections of the lead (about 12 dB below the rest, 0.26 s per mora) aren't heard. The aligner
  squeezes their moras into a second and drags the neighbouring lines with them: two runs of 11 and 10 lines,
  up to 50 s off. It is not the separation: on the full vocals stem the same lines fail. The other 7 are:
  - two lines the lead stem doesn't carry (−53 dB where the UST sings: backing or doubled vocal);
  - five slips of 2–3 notes at a line break.

  Fast runs are not a failure class in themselves. 22 lines are faster than 0.12 s per mora, 20 of them in
  amaten. The 7 that fail are inside amaten's drift; the other 15 land within one note. Held vowels cost
  nothing either, once the end rule holds them.
- **The aligner's own score flags the bad lines.** The most doubtful 20% of lines by mean log-posterior catch
  26 of the 28 lines that are more than one note off (93%; ROC AUC 0.91). Without amaten, they catch 5 of 7
  (AUC 0.84). A pace check (seconds per mora far from the song's median: squeezed or stretched lines) does
  about as well (AUC 0.91). So the editor can mark doubtful lines for dragging (ticket 15).
- **The other candidates fail the rule on the edit cost or on placement.**
  - The hiragana wav2vec2 (speech, Apache-2.0) gets 87.2% of lines within one note. It is better than the
    lyrics model on amaten (70% against 51%) but worse on six songs. It lands nearly as many moras (94.1
    wrong/min) but doesn't lower the edit cost (−1.8 [−17.8, +8.9]).
  - The MMS aligner (CC-BY-NC weights) gets 88.3% within one note and 99.1 wrong/min, with edit cost −7.5
    [−30.4, +6.9].
  - Whisper large-v3 with the text forced (stable-ts) gets 93.5% within one note, but starts lines 163 ms early
    (median). It places 25.2%.
  - The lyrics model on the full vocals stem, backing included, is the same as on the lead (93.2 wrong/min).
  - The speech models' own scores flag their bad lines less well. Their most doubtful 20% catch 28% (Japanese
    kana+kanji), 59% (hiragana) and 71% (MMS) of their lines more than one note off; the lyrics model's catch 93%.

**Recommendation.** Port the lyrics CTC model as the line placer (ticket 04; plan in its comments):
- CTC forced alignment of the pasted moras on the lead stem;
- line and piece starts from the first token;
- ends from the lead's energy;
- `fit_lines` for the fit, which has landed in `sona_core::lyric_fit`;
- the per-line score as the doubt flag (ticket 15).

The weights are MIT, but 2.3 GB and slow on CPU: about 2.5 min per 3-minute song on 4 quiet cores, against
about 1 min for the speech model (see Speed). So it ships as a user-installed tool or an ONNX
export, opt-in until holdout4 confirms it.

**For the user to decide:** the lyrics model or the speech model.
- The lyrics model has the better landing and doubt flag. Its weights are MIT, but its card doesn't name the
  lyrics it was trained on.
- The speech model is the declared rule's letter and about twice as fast. Its weights are Apache-2.0, trained
  on Common Voice, CSS10 and JSUT; JSUT's audio is for research, non-commercial research and personal use.

The harness runs either; swapping is one model name and a vocabulary.

## Table (dev; GAME lead unless noted; lines and spaces, `fit_lines`; paired song bootstrap, 10 000 resamples)

Line columns: share of lines whose aligned start is nearest the line's own first UST note ("exact") or within
one note of it; |start error| median. "Wrong/min" and "placed": lyric placement (01's metric). Δ: paired
against today's paste in order (188.7, 183.2).

| landing | lines exact | within one note | \|start\| ms | cost/min | Δ cost [95%] | wrong/min | Δ wrong [95%] | placed | stage cost/min | stage placed |
|---|---|---|---|---|---|---|---|---|---|---|
| today: pasted in order | — | — | — | 188.7 | 0 | 183.2 | 0 | 4.9% | 180.2 | 4.3% |
| ceiling: lines placed by the UST (01, b+/r) | 100% | 100% | 0 | 175.2 | −13.5 [−31.1, −2.7] | 83.4 | −99.7 | 56.7% | 167.4 | 55.3% |
| **lyrics CTC (w2v-bert music lyrics), lead, energy ends** | **84.6%** | **92.7%** | **25** | **175.2** | **−13.5 [−32.1, −2.5]** | **93.1** | **−90.1 [−101.0, −77.3]** | **51.7%** | **169.6** | **50.0%** |
| … CTC's own ends | 84.6% | 92.7% | 25 | 193.0 | +4.3 [−20.3, +18.9] | 129.5 | −53.7 [−60.4, −47.5] | 32.8% | 180.4 | 29.9% |
| … ends extended to the next piece (≤ 2 s) | 84.6% | 92.7% | 25 | 179.0 | −9.8 [−28.1, +1.0] | 99.0 | −84.2 [−94.3, −72.1] | 48.6% | 171.2 | 48.4% |
| … the UST's own ends (diagnostic) | 84.6% | 92.7% | 25 | 178.6 | −10.1 [−24.0, −0.9] | 94.5 | −88.7 [−97.9, −77.5] | 51.0% | 169.9 | 49.8% |
| … on the full vocals stem (lead + backing) | 84.6% | 92.7% | 25 | 175.5 | −13.2 [−30.4, −2.6] | 93.2 | −90.0 [−101.2, −76.6] | 51.7% | 170.1 | 49.3% |
| hiragana wav2vec2 (speech), lead, energy ends | 78.9% | 87.2% | 18 | 186.9 | −1.8 [−17.8, +8.9] | 94.1 | −89.1 [−100.1, −76.6] | 51.2% | 179.1 | 48.0% |
| Japanese wav2vec2 kana+kanji (speech), lead, energy ends | 82.8% | 93.5% | 41 | 184.1 | −4.7 [−14.8, +1.8] | 98.4 | −84.8 [−94.3, −73.6] | 49.0% | 175.6 | 47.8% |
| MMS forced aligner (romanised; CC-BY-NC), lead, energy ends | 81.5% | 88.3% | 45 | 181.2 | −7.5 [−30.4, +6.9] | 99.1 | −84.1 [−93.9, −72.2] | 48.6% | 174.3 | 46.5% |
| Whisper large-v3 + stable-ts, lead, energy ends | 42.4% | 93.5% | 178 | 186.6 | −2.1 [−18.5, +9.0] | 144.1 | −39.1 [−49.6, −29.1] | 25.2% | 177.1 | 26.1% |
| … its own ends | 42.4% | 93.5% | 178 | 186.8 | −1.9 [−19.6, +9.3] | 165.9 | −17.3 [−26.7, −8.3] | 13.9% | 176.8 | 13.2% |

Lines alone (no spaces), lyrics CTC with energy ends: 178.5 cost/min, 103.0 wrong/min, 46.6% placed (ceiling
b/r: 176.7, 93.1, 51.7%). The paired difference to the ceiling, lines and spaces: edit cost −0.0 [−1.9, +1.3],
wrong moras +9.6 [+1.5, +20.4] per minute. The stage's: +2.3 [+0.9, +3.9] and +10.2 [+3.8, +20.4].

Within one note, song bootstrap: lyrics CTC [81.1, 99.4]; Japanese kana+kanji [85.7, 98.6]; hiragana [81.7, 92.4];
MMS [80.4, 96.2]; Whisper [87.1, 98.8].

**Per song** (GAME lead; placed % for the ceiling → lyrics CTC → today; lines within one note):

| song | lines within one note | placed: ceiling | lyrics CTC | today | cost/min: ceiling | lyrics CTC | today |
|---|---|---|---|---|---|---|---|
| amaten | 51% | 43.6% | 24.3% | 1.6% | 302.9 | 298.4 | 394.1 |
| atamaro-deluxe | 93% | 47.1% | 43.7% | 2.9% | 331.7 | 332.0 | 347.0 |
| azemichi-ni-ochita-ginga | 100% | 71.1% | 62.1% | 3.9% | 72.3 | 73.7 | 75.1 |
| fukagyakuteki-henyou | 98% | 37.8% | 31.6% | 3.2% | 247.9 | 249.7 | 256.3 |
| innocent-addict | 96% | 58.6% | 52.8% | 2.6% | 157.4 | 159.5 | 171.9 |
| kamaitachi-ni-yoru | 100% | 74.9% | 76.5% | 10.9% | 101.2 | 95.4 | 100.5 |
| my-axiom | 100% | 50.3% | 51.9% | 2.9% | 194.0 | 194.7 | 190.1 |
| telescope-nevermore | 100% | 78.0% | 72.3% | 7.3% | 88.3 | 88.6 | 93.8 |
| yamineko | 100% | 60.3% | 64.0% | 12.1% | 126.2 | 127.7 | 128.1 |

Placement improves on every song. As in 01, the edit-cost gain is mostly amaten's (−96 of its 394 cost/min,
even with its two drifting runs); the other songs move −15 … +5.

## Candidates

Weight licences are from the model cards; none of the four model repositories has a LICENSE file. Code licences
are from the projects' LICENSE files.

| candidate | what it is | licence (code / weights) | status |
|---|---|---|---|
| **w2v-bert-2.0 music lyrics CTC** (`mrfakename/w2v-bert-2.0-music-lyrics-ctc`, 600M) | character CTC trained on whole-song lyrics of full mixes; 921 tokens incl. hiragana | transformers Apache-2.0 / **MIT** (card; base `w2v-bert-2.0-music` MIT, `facebook/w2v-bert-2.0` MIT). The card doesn't name its training lyrics. | measured: **passes; recommended** |
| Japanese hiragana wav2vec2 (`vumichien/wav2vec2-large-xlsr-japanese-hiragana`, 300M) | XLSR-53 fine-tuned on speech (Common Voice), hiragana CTC | Apache-2.0 / Apache-2.0 | measured |
| Japanese wav2vec2 (`jonatasgrosman/wav2vec2-large-xlsr-53-japanese`, 300M) | XLSR-53 on speech (Common Voice 6.1, CSS10, JSUT), kana + kanji CTC | Apache-2.0 / Apache-2.0 (JSUT's audio: research, non-commercial research and personal use) | measured: passes; the declared tie-break's pick |
| MMS forced aligner (`MahmoudAshraf/mms-300m-1130-forced-aligner`; torchaudio's `MMS_FA` weights) | 1,130-language CTC on romanised text; moras romanised (Hepburn) | Apache-2.0 / **CC-BY-NC-4.0**: a user-installed tool only | measured |
| Whisper large-v3 (`Systran/faster-whisper-large-v3`) + stable-ts `align` | the text forced through the decoder, cross-attention word timestamps | MIT / MIT | measured |
| Montreal Forced Aligner, Japanese MFA acoustic model v3 | GMM-HMM on speech | MIT / CC BY 4.0 | not run: conda-only (Kaldi via kalpy); it aligns utterances it is given (≤ ~30 s with their text), so it needs the line times first. At most a refiner inside a placed line. |
| NeMo forced aligner + `nvidia/parakeet-tdt_ctc-0.6b-ja` | CTC Viterbi on long audio with a speech model | Apache-2.0 / CC BY 4.0 | not run: another speech-trained model, which the two wav2vec2s already test; a heavy install |
| SOFA (singing-oriented forced aligner) | phoneme aligner for DiffSinger/OpenUtau datasets | MIT / community models, licences vary | not run: per-utterance like MFA, with phoneme dictionaries; a candidate for mora-level refinement inside lines |

All the CTC candidates go through the same harness (`ctc_align.py`). Emissions come from 26 s windows with 2 s of
context on each side, centres kept. Then one Viterbi runs over the whole song's tokens: a gap between moras emits
the blank, and a gap between pieces (or at either end) emits the better of the blank and the model's `|` filler.
Kana missing from a vocabulary fall back to the katakana, unvoiced or large kana (the lyrics model lacks ぞ, ぴ, ぷ,
ぺ, ゅ and small vowels). No mora went without a token.

## How the lines were made and scored

- **Text.** Each line is the lead UST's moras between 01's line starts, which is what a user would paste (perfect
  text: っ never occurs, and long-vowel tails are as the UST sings them). Spaces split a line where 01 found
  them. Only the timing is new.
- **Line start error.** The aligned start minus the UST onset of the line's first mora. "In notes": the UST note
  whose onset is nearest the aligned start, against the line's first note. This count is lenient for an early
  start that falls in the rest before a line: it lands on the previous line's last note, one note off. That is
  why Whisper looks fine on it (93.5%) and lands badly; the ms column and the landing show it.
- **Landing.** The output notes whose midpoint lies in a piece's span get its moras by `fit_span_moras`; a note
  in no span carries none (`sona_core::lyric_fit::fit_lines`). This is 01's (b+/r) with the aligner's times in
  place of the UST's.

## What else these numbers could be measuring

- **Perfect text.** The pasted moras are the UST's own, so a real paste (repeats written once, tails written or
  not, typos) is harder. A CTC alignment can't skip text the singer doesn't sing; a chorus written once but sung
  twice will misplace the second one. That needs a check in 04.
- **The end rule is tuned on dev.** Its threshold (−20 dB, 100 ms) was set against the UST's note offsets on the
  same songs. Two checks: the rule-free "next" variant also passes the selection rule (99.0 wrong/min, Δ cost
  −9.8 [−28.1, +1.0]), and the UST's own ends do no better (94.5).
- **Starts leaking from the UST.** Starts come from the audio and the text alone. The UST's times enter only the
  scoring and the end-rule calibration above.
- **One song.** amaten carries most of the edit-cost gain and most of the failures. Without it, 7 of 341 lines
  are more than one note off.
- **The port.**
  - The Rust landing (`sona-eval-cellstage --lyric-line-times`) reproduces the prototype on all 36 song ×
    output × landing rows, for the energy ends and the next-start ends.
  - Fed the UST's own line times, it reproduces 01's b/r and b+/r rows exactly (36 of 36).
  - Without the flag, the default and `--lyric-lines` outputs are byte-identical to 01's.
- **Selection on dev.** Choosing between the candidates and the three end rules is in sample. Confirmation goes
  to holdout4, bundled with the other lyrics-first winners under the spec's rule.

## Speed

Measured on this machine while it ran other jobs (load average 25–45 on 16 cores; each process got about 1.2
cores):
- lyrics CTC: 1,300–2,200 s per song for the emissions (4 threads), plus 2–6 s for the Viterbi;
- Whisper + stable-ts (int8, 3–4 threads): 720–3,900 s per song.

Once the machine was quieter (load average 4), one process with 4 threads on the first 60 s of a lead stem,
model load included:

| model | wall | CPU | peak memory |
|---|---|---|---|
| lyrics CTC (600M, fp32) | 49 s | 185 s | 4.2 GB |
| Japanese wav2vec2 kana+kanji (300M) | 22 s | 86 s | — |
| hiragana wav2vec2 (300M) | 21 s | 83 s | — |

So a 3-minute song takes about 2.5 min with the lyrics model and about 1 min with a speech model on 4 cores of
this CPU. The Viterbi takes 2–6 s per song in numpy. An int8 or ONNX build has not been tried.

## Reproduce

```sh
# Prototype (lyric-derived outputs go to $L14_WORK; keep them out of the repo)
export L01_DUMP=…dump-dev L01_WORK=…l01 L14_WORK=…l14 SONA_STEMS=crates/sona-eval/data/pifuyuu-stems
crates/sona-eval/scripts/line-alignment/run_all.sh
# Rust: the landing on the aligned times, and the check against the prototype
ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
    --model ~/.cache/sona/cellstage-D --edit-cost --lyric-placement --lyric-lines "$L01_WORK/lines" \
    --lyric-line-times "$L14_WORK/times/lyrics-lead-energy" > rust.out
python3 crates/sona-eval/scripts/line-alignment/check_port.py rust.out "$L14_WORK/rows-energy.json" lyrics-lead-energy
```
