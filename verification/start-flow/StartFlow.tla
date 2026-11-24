---- MODULE StartFlow ----
EXTENDS Naturals
CONSTANTS Songs, Tempos
NoTempo == 0
Usable(t) == t # NoTempo
VARIABLES kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs,
          hashFor, hashing, fileFor, sending, jobHash, editorSong,
          lyricFor, jobLyric, editorLyric
vars == <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs,
          hashFor, hashing, fileFor, sending, jobHash, editorSong,
          lyricFor, jobLyric, editorLyric>>
lyrics == <<lyricFor, jobLyric, editorLyric>>
flow == <<hashFor, hashing, fileFor, sending, jobHash, editorSong, lyricFor, jobLyric, editorLyric>>

Init == /\ kind = "idle" /\ song = 0 /\ bpm = NoTempo /\ enteredFor = 0
        /\ hint = NoTempo /\ hintFor = 0 /\ pending = {} /\ jobFor = 0 /\ jobs = 0
        /\ hashFor = 0 /\ hashing = {} /\ fileFor = 0 /\ sending = "none" /\ jobHash = 0 /\ editorSong = 0
        /\ lyricFor = 0 /\ jobLyric = 0 /\ editorLyric = 0

Choose(s) == /\ kind \in {"idle", "ready", "loaded"}
             /\ kind' = "ready" /\ song' = s /\ pending' = pending \cup {s}
             /\ bpm' = NoTempo /\ hint' = NoTempo
             /\ hashFor' = 0 /\ hashing' = hashing \cup {s} /\ fileFor' = s /\ lyricFor' = 0
             /\ UNCHANGED <<enteredFor, hintFor, jobFor, jobs, sending, jobHash, editorSong, jobLyric, editorLyric>>
TempoArrives(s, t) == /\ s \in pending
                      /\ pending' = pending \ {s}
                      /\ IF kind \in {"ready", "running"} /\ song = s /\ hint = NoTempo
                         THEN hint' = t /\ hintFor' = s
                         ELSE UNCHANGED <<hint, hintFor>>
                      /\ bpm' = bpm
                      /\ UNCHANGED <<kind, song, enteredFor, jobFor, jobs>> /\ UNCHANGED flow
HashArrives(s) == /\ s \in hashing
                  /\ hashing' = hashing \ {s}
                  /\ IF kind \in {"ready", "running"} /\ song = s /\ hashFor = 0
                     THEN hashFor' = s
                     ELSE UNCHANGED hashFor
                  /\ UNCHANGED <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs, fileFor, sending, jobHash, editorSong>>
                  /\ UNCHANGED lyrics
YouSetTheTempo(t) == /\ kind = "ready" /\ bpm' = t /\ enteredFor' = song
                     /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow
YouPickTheHint == /\ kind = "ready" /\ Usable(hint) /\ bpm' = hint /\ enteredFor' = song
                  /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow
\* A lyric you type is one sona takes (hiragana) or not; one it can't take is no lyric.
YouTypeALyric(ok) == /\ kind = "ready" /\ lyricFor' = (IF ok THEN song ELSE 0)
                     /\ UNCHANGED <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>>
                     /\ UNCHANGED <<hashFor, hashing, fileFor, sending, jobHash, editorSong, jobLyric, editorLyric>>
YouPressTranscribe == /\ kind = "ready" /\ Usable(bpm) /\ jobs < 2
                      /\ hashFor # 0 \/ fileFor = song
                      /\ lyricFor = song
                      /\ kind' = "running" /\ jobFor' = song /\ jobs' = jobs + 1
                      /\ sending' = (IF hashFor # 0 THEN "hash" ELSE "upload") /\ jobHash' = hashFor
                      /\ jobLyric' = lyricFor
                      /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, hashFor, hashing, fileFor, editorSong, lyricFor, editorLyric>>
\* The editor's notes carry their job's lyric; notes transcribed without one send you to the start
\* screen, song and tempo filled in, to paste it.
YouRetranscribe(t) == /\ kind \in {"idle", "ready", "loaded"} /\ editorSong # 0 /\ Usable(t) /\ jobs < 2
                      /\ song' = editorSong /\ bpm' = t /\ enteredFor' = editorSong /\ hint' = NoTempo
                      /\ hashFor' = editorSong /\ lyricFor' = editorLyric
                      /\ IF editorLyric = editorSong
                         THEN /\ kind' = "running" /\ jobFor' = editorSong /\ jobs' = jobs + 1
                              /\ sending' = "hash" /\ jobHash' = editorSong /\ jobLyric' = editorLyric
                         ELSE /\ kind' = "ready" /\ UNCHANGED <<jobFor, jobs, sending, jobHash, jobLyric>>
                      /\ UNCHANGED <<hintFor, pending, hashing, fileFor, editorSong, editorLyric>>
\* Opening a saved session or annotation: its notes may have a lyric (a job's) or not (older files).
YouOpenAFile(s, withLyric) == /\ kind # "running"
                              /\ editorSong' = s /\ editorLyric' = (IF withLyric THEN s ELSE 0)
                              /\ UNCHANGED <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>>
                              /\ UNCHANGED <<hashFor, hashing, fileFor, sending, jobHash, lyricFor, jobLyric>>
HashRefused == /\ kind = "running" /\ sending = "hash"
               /\ IF fileFor = song
                  THEN sending' = "upload" /\ UNCHANGED kind
                  ELSE kind' = "ready" /\ UNCHANGED sending
               /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs, hashFor, hashing, fileFor, jobHash, editorSong>>
               /\ UNCHANGED lyrics
TheJobEnds == /\ kind = "running"
              /\ \/ kind' = "loaded" /\ editorSong' = jobFor /\ editorLyric' = jobLyric
                 \/ kind' = "ready" /\ UNCHANGED <<editorSong, editorLyric>>
              /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs, hashFor, hashing, fileFor, sending, jobHash>>
              /\ UNCHANGED <<lyricFor, jobLyric>>
StartOver == /\ kind \in {"ready", "loaded"}
             /\ kind' = "idle" /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow

Next == \/ \E s \in Songs : Choose(s) \/ HashArrives(s)
        \/ \E s \in Songs, t \in Tempos : TempoArrives(s, t)
        \/ \E t \in Tempos : YouSetTheTempo(t) \/ YouRetranscribe(t)
        \/ \E ok \in BOOLEAN : YouTypeALyric(ok)
        \/ \E s \in Songs, l \in BOOLEAN : YouOpenAFile(s, l)
        \/ YouPickTheHint \/ YouPressTranscribe \/ HashRefused \/ TheJobEnds \/ StartOver

OnScreen == kind \in {"ready", "running"}
TempoIsForTheShownSong == OnScreen /\ hint # NoTempo => hintFor = song
BpmIsYours == OnScreen /\ Usable(bpm) => enteredFor = song
JobsHaveATempo == kind = "running" => Usable(bpm) /\ jobFor = song
HashIsForTheShownSong == OnScreen /\ hashFor # 0 => hashFor = song
HashJobsNameTheirSong == kind = "running" /\ sending = "hash" => jobHash = song
UploadsSendTheSong == kind = "running" /\ sending = "upload" => fileFor = song
LyricIsForTheShownSong == OnScreen /\ lyricFor # 0 => lyricFor = song
JobsHaveALyric == kind = "running" => jobLyric = song
====
