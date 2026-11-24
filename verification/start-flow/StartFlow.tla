---- MODULE StartFlow ----
EXTENDS Naturals
CONSTANTS Songs, Tempos
NoTempo == 0
Usable(t) == t # NoTempo
VARIABLES kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs,
          hashFor, hashing, fileFor, sending, jobHash, editorSong
vars == <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs,
          hashFor, hashing, fileFor, sending, jobHash, editorSong>>
flow == <<hashFor, hashing, fileFor, sending, jobHash, editorSong>>

Init == /\ kind = "idle" /\ song = 0 /\ bpm = NoTempo /\ enteredFor = 0
        /\ hint = NoTempo /\ hintFor = 0 /\ pending = {} /\ jobFor = 0 /\ jobs = 0
        /\ hashFor = 0 /\ hashing = {} /\ fileFor = 0 /\ sending = "none" /\ jobHash = 0 /\ editorSong = 0

Choose(s) == /\ kind \in {"idle", "ready", "loaded"}
             /\ kind' = "ready" /\ song' = s /\ pending' = pending \cup {s}
             /\ bpm' = NoTempo /\ hint' = NoTempo
             /\ hashFor' = 0 /\ hashing' = hashing \cup {s} /\ fileFor' = s
             /\ UNCHANGED <<enteredFor, hintFor, jobFor, jobs, sending, jobHash, editorSong>>
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
YouSetTheTempo(t) == /\ kind = "ready" /\ bpm' = t /\ enteredFor' = song
                     /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow
YouPickTheHint == /\ kind = "ready" /\ Usable(hint) /\ bpm' = hint /\ enteredFor' = song
                  /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow
YouPressTranscribe == /\ kind = "ready" /\ Usable(bpm) /\ jobs < 2
                      /\ hashFor # 0 \/ fileFor = song
                      /\ kind' = "running" /\ jobFor' = song /\ jobs' = jobs + 1
                      /\ sending' = (IF hashFor # 0 THEN "hash" ELSE "upload") /\ jobHash' = hashFor
                      /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, hashFor, hashing, fileFor, editorSong>>
YouRetranscribe(t) == /\ kind \in {"idle", "ready", "loaded"} /\ editorSong # 0 /\ Usable(t) /\ jobs < 2
                      /\ kind' = "running" /\ song' = editorSong /\ bpm' = t /\ enteredFor' = editorSong
                      /\ hint' = NoTempo /\ jobFor' = editorSong /\ jobs' = jobs + 1
                      /\ hashFor' = editorSong /\ sending' = "hash" /\ jobHash' = editorSong
                      /\ UNCHANGED <<hintFor, pending, hashing, fileFor, editorSong>>
HashRefused == /\ kind = "running" /\ sending = "hash"
               /\ IF fileFor = song
                  THEN sending' = "upload" /\ UNCHANGED kind
                  ELSE kind' = "ready" /\ UNCHANGED sending
               /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs, hashFor, hashing, fileFor, jobHash, editorSong>>
TheJobEnds == /\ kind = "running"
              /\ \/ kind' = "loaded" /\ editorSong' = jobFor
                 \/ kind' = "ready" /\ UNCHANGED editorSong
              /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs, hashFor, hashing, fileFor, sending, jobHash>>
StartOver == /\ kind \in {"ready", "loaded"}
             /\ kind' = "idle" /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>> /\ UNCHANGED flow

Next == \/ \E s \in Songs : Choose(s) \/ HashArrives(s)
        \/ \E s \in Songs, t \in Tempos : TempoArrives(s, t)
        \/ \E t \in Tempos : YouSetTheTempo(t) \/ YouRetranscribe(t)
        \/ YouPickTheHint \/ YouPressTranscribe \/ HashRefused \/ TheJobEnds \/ StartOver

OnScreen == kind \in {"ready", "running"}
TempoIsForTheShownSong == OnScreen /\ hint # NoTempo => hintFor = song
BpmIsYours == OnScreen /\ Usable(bpm) => enteredFor = song
JobsHaveATempo == kind = "running" => Usable(bpm) /\ jobFor = song
HashIsForTheShownSong == OnScreen /\ hashFor # 0 => hashFor = song
HashJobsNameTheirSong == kind = "running" /\ sending = "hash" => jobHash = song
UploadsSendTheSong == kind = "running" /\ sending = "upload" => fileFor = song
====
