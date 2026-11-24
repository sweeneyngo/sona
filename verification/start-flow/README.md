# Start screen model (TLA+)

`StartFlow.tla` models the start screen (`web/lib/startFlow.ts`, ADR-0028, ADR-0032): you drop a song and
type its BPM; meanwhile `sona serve` estimates the tempo and the app hashes the file, both asynchronously
(answers arrive late and out of order); the estimate is shown as a hint you may pick; one transcription
job runs. A job names its song by the file's SHA-256 when the hash is known, and sends the file when the
server refuses the hash. The editor can re-transcribe the song it shows, by that song's hash; it holds
the song's file only if that song was dropped in this session. An answer with no tempo, whether no beat
was found or the server couldn't read the file (`tempo-failed`, which shows its reason), is the model's
`NoTempo`.

Rules, checked on every path with 3 songs and 3 tempos:
- **TempoIsForTheShownSong:** the estimate on screen was detected for the song on screen, never for one
  you replaced while it was being detected.
- **BpmIsYours:** a BPM on screen is one you typed or picked for the song on screen (on the start screen
  or in the editor). An estimate never fills it by itself, and a new song starts empty.
- **JobsHaveATempo:** a job runs only with a usable tempo, and for the song on screen.
- **HashIsForTheShownSong:** the hash the start screen holds is the hash of the song on screen.
- **HashJobsNameTheirSong:** a job sent by hash names its own song's hash, never another song's.
- **UploadsSendTheSong:** a job that falls back to uploading sends its own song's file; without it, the
  job comes back to the song and asks for the file.

`./check.sh` runs TLC on the rules and on eight mutants, each of which must be caught:
- `stale-tempo-wins`: a tempo answer applied whatever song it was for;
- `start-without-tempo`: Transcribe allowed with no tempo;
- `estimate-becomes-bpm`: a tempo answer fills the BPM field (ADR-0022's pre-fill);
- `bpm-carries-over`: a new song keeps the previous song's BPM;
- `stale-hash-wins`: a hash applied whatever song it was for;
- `hash-carries-over`: a new song keeps the previous song's hash;
- `hash-of-another-song`: a re-transcription names the start screen's song's hash instead of the editor's;
- `upload-without-file`: a refused hash always falls back to uploading, whatever file the app holds.

The same rules are property-tested on the real reducer (`web/lib/startFlow.test.ts`, 3 000 random
event orders). CI runs `check.sh` in the `model-check` job.
