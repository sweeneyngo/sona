# Start screen model (TLA+)

`StartFlow.tla` models the start screen (`web/lib/startFlow.ts`): you drop a song, its tempo is
detected by `sona serve` asynchronously (answers arrive late and out of order), you may change it, and
one transcription job runs.

Rules, checked on every path with 3 songs and 3 tempos:
- **TempoIsForTheShownSong:** the tempo on screen was detected for the song on screen, never for one you
  replaced while it was being detected.
- **JobsHaveATempo:** a job runs only with a usable tempo, and for the song on screen.

`./check.sh` runs TLC on the rules and on two mutants, each of which must be caught:
- `stale-tempo-wins`: a tempo answer applied whatever song it was for;
- `start-without-tempo`: Transcribe allowed with no tempo.

The same rules are property-tested on the real reducer (`web/lib/startFlow.test.ts`, 3 000 random
event orders). CI runs `check.sh` in the `model-check` job.
