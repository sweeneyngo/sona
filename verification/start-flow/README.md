# Start screen model (TLA+)

`StartFlow.tla` models the start screen (`web/lib/startFlow.ts`, ADR-0028): you drop a song and type its
BPM; meanwhile `sona serve` estimates the tempo asynchronously (answers arrive late and out of order) and
the estimate is shown as a hint you may pick; one transcription job runs.

Rules, checked on every path with 3 songs and 3 tempos:
- **TempoIsForTheShownSong:** the estimate on screen was detected for the song on screen, never for one
  you replaced while it was being detected.
- **BpmIsYours:** a BPM on screen is one you typed or picked for the song on screen. An estimate never
  fills it by itself, and a new song starts empty.
- **JobsHaveATempo:** a job runs only with a usable tempo, and for the song on screen.

`./check.sh` runs TLC on the rules and on four mutants, each of which must be caught:
- `stale-tempo-wins`: a tempo answer applied whatever song it was for;
- `start-without-tempo`: Transcribe allowed with no tempo;
- `estimate-becomes-bpm`: a tempo answer fills the BPM field (ADR-0022's pre-fill);
- `bpm-carries-over`: a new song keeps the previous song's BPM.

The same rules are property-tested on the real reducer (`web/lib/startFlow.test.ts`, 3 000 random
event orders). CI runs `check.sh` in the `model-check` job.
