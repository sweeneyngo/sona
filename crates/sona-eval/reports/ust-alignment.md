# UST phase alignment (UST corpus gate, ticket 05)

Run: `cargo run --release -p sona-eval --bin sona-eval-ust-align -- --write`
Corpus: the 20-song manifest (`corpus/ust-manifest.json`). Stems are demucs `htdemucs` 4.0.1 (`vocals` and `drums`).

BPM comes exactly from each UST, so alignment is one offset per song: `stem_time = ust_time + offset`. Ticket 05 rules out setting it with sona's own onset detector, because that would build the detector's bias into the ground truth. So the offset is **set without sona** and **verified with sona**:

1. **Coarse.** Cross-correlate the USTs' any-voice-singing mask with the vocal stem's log-RMS envelope (10 ms frames).
2. **Fine.** Within ±½ grid cell of the coarse offset, line the UST's grid up with drum-stem onsets (1 ms). The drums mark the grid independently of the vocal.
3. **Verify.** Compute the signed delta `sona onset − UST onset` over matched pairs (±100 ms). Flag a song for a constant shift (|median| > 35 ms), for drift (the median of the last third differs from the first third by more than 30 ms), or for fewer than 30 matches.

## Result: 19 / 20 aligned

```
song                             offset s  fine Δ  drums match    p10   p25   p50   p75   p90  drift  status
                                               ms   peak                  detected − UST onset (ms)
innocent-addict                    -4.913      -3   3.47   27%    -41   -26    -9    +2   +31     -0  aligned
kinsei                             -6.953     +17   3.62   40%    -50   -27    -9    -2   +10     +0  aligned
kamaitachi-ni-yoru                 -6.740     +20   5.26   52%    -55   -44   -32   -19    -3     +3  aligned
noizugoto                          -4.757     +13   2.09   30%    -48   -29   -17   +64   +89    +13  aligned
telescope-nevermore                -7.905      -5   4.32   51%    -31   -10    +0    +3    +8     +1  aligned
bass-sousha-e-no-henjin-ronsou     -6.053      +7   3.61   26%    -58   -28   -10   +74   +83     -3  aligned
plus-space                         -8.947     -37   1.42   45%    -20   +14   +38   +47   +50     +4  flagged: constant shift +38 ms
light-seeker                       -6.505      +5   2.47   56%    -44   -19    -6   +15   +37     +1  aligned
atamaro-deluxe                     -3.972      +8   1.86   45%    -56   -22    +3   +32   +82     +6  aligned
smolder                            -6.034      +6   2.54   34%    -75   -46   -21    -5    +3     -9  aligned
azemichi-ni-ochita-ginga           -4.934     +16   2.07   47%    -33   -14    -7    +0    +7     +3  aligned
dm                                -10.533      +7   7.53   50%    -38    -7    -2    +3   +12     -1  aligned
yamineko                           -3.897      -7   2.40   49%    -31   -15    -8   +10   +16     +4  aligned
sandatsu                           -5.745     +15   2.64   49%    -38   -23   -12    -3   +60     -5  aligned
my-axiom                           -4.997     +13   2.29   34%    -40   -27    -7    +1   +53     -3  aligned
emerald                            -6.662      +8   3.59   52%    -33   -21   -12    +1    +6     -2  aligned
amaten                             -5.918      +2   2.65   22%    -31    -8    +1   +91   +95     -3  aligned
moumoku-shabon                     -5.302      +8   1.92   27%    -46   -19    -8    -3    +7     +5  aligned
fukagyakuteki-henyou               -4.126      +4   1.92   34%    -33   -23   -13    +5   +41     +0  aligned
invariant                          -9.149      -9   6.57   32%    -29    -6    +1    +7    +9     +1  aligned
```

## Reading

- **The aligned songs read as aligned.** Each median delta is between −32 and +3 ms (typically about −8 ms), and no song drifts more than 13 ms. The −8 ms matches the **−11.6 ms early bias** that `onset-deviation.md` measured for sona's onset detector on Vocadito. Two independent measurements agree, so the small residual is sona's own perception, not the alignment.
- **No song breaks the single-BPM assumption.** Drift is flat for all 20. (KimiAiGimme, which has two tempos, was already excluded in 02.)
- **Long late tails** in fast songs are matching artifacts, not misalignment: ノイズゴト p90 +89, BassSousha +83, 甘天 +95. When notes are only 50–75 ms apart, a detected onset can pair with the *next* note. Each of these songs has a near-zero median and no drift.
- **かまいたちに因る at −32 ms is the borderline case.** Its spread is tight (−44 to −19) and its drum grid is strong (5.26), so it reads as sona firing early on this voice's consonants. It passes, and it's noted here.
- **Excluded: plus-space** (+38 ms constant shift). It's a piano-led duet with the weakest drum grid in the corpus (peak 1.42), and the drum step moved the coarse offset 37 ms the wrong way. It's not fixed by tuning the drum-trust threshold, because picking the threshold by agreement with sona would be circular. A by-ear alignment could bring it back.

## Method notes

- The fine search was first ±½ *beat*, aligning the UST's every-480-ticks lines with kick and snare hits. That moved INNOCENT ADDICT a whole 16th (+72 ms), because a UST's tick 0 can be a pickup, so "every 480 ticks" isn't reliably a beat. A UST's notes do sit on its 16th grid (`ust-format-audit.md`), so the search is now ±½ *cell* over every grid line. Regression test: `ust_align::tests::a_pickup_does_not_shift_the_phase_by_a_cell`.
- The match rate (22–56%) is sona's onset recall, not an alignment property. The Vocadito diagnostic measured 44%.
- Offsets are stored in the manifest as data and can be overridden by ear in the review surface; the 06 gate reads them from there.

## Known limit of the check (from the 06 adversarial review)

The check uses the onset **detector's** onsets. The gate scores the pipeline's **note** onsets, and the two
can disagree. In 金星 and Telescope, note onsets land +23 to +32 ms late (two-peaked, up to +74 ms in some
sections), while detector onsets read −9 and 0 ms. Shifting those references 35–45 ms later recovers 35–44%
more COnP matches.

The drum grid and the detector onsets both independently support the current offsets, and Vocadito shows no
such lag (its best shift is 0 ms). So this most likely reflects pitch-born note onsets settling after the
attack, not misalignment. A local misalignment can't be fully excluded.

A stronger check would also print note-onset deltas. The 06 offset verdict is robust either way (see
`ust-in-domain-gate.md`).
