---- MODULE StartFlow ----
EXTENDS Naturals
CONSTANTS Songs, Tempos
NoTempo == 0
Usable(t) == t # NoTempo
VARIABLES kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs
vars == <<kind, song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>>

Init == /\ kind = "idle" /\ song = 0 /\ bpm = NoTempo /\ enteredFor = 0
        /\ hint = NoTempo /\ hintFor = 0 /\ pending = {} /\ jobFor = 0 /\ jobs = 0

Choose(s) == /\ kind \in {"idle", "ready", "loaded"}
             /\ kind' = "ready" /\ song' = s /\ pending' = pending \cup {s}
             /\ bpm' = NoTempo /\ hint' = NoTempo
             /\ UNCHANGED <<enteredFor, hintFor, jobFor, jobs>>
TempoArrives(s, t) == /\ s \in pending
                      /\ pending' = pending \ {s}
                      /\ IF kind \in {"ready", "running"} /\ song = s /\ hint = NoTempo
                         THEN hint' = t /\ hintFor' = s
                         ELSE UNCHANGED <<hint, hintFor>>
                      /\ bpm' = bpm
                      /\ UNCHANGED <<kind, song, enteredFor, jobFor, jobs>>
YouSetTheTempo(t) == /\ kind = "ready" /\ bpm' = t /\ enteredFor' = song
                     /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>>
YouPickTheHint == /\ kind = "ready" /\ Usable(hint) /\ bpm' = hint /\ enteredFor' = song
                  /\ UNCHANGED <<kind, song, hint, hintFor, pending, jobFor, jobs>>
YouPressTranscribe == /\ kind = "ready" /\ Usable(bpm) /\ jobs < 2
                      /\ kind' = "running" /\ jobFor' = song /\ jobs' = jobs + 1
                      /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending>>
TheJobEnds == /\ kind = "running"
              /\ \/ kind' = "loaded"
                 \/ kind' = "ready"
              /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>>
StartOver == /\ kind \in {"ready", "loaded"}
             /\ kind' = "idle" /\ UNCHANGED <<song, bpm, enteredFor, hint, hintFor, pending, jobFor, jobs>>

Next == \/ \E s \in Songs : Choose(s)
        \/ \E s \in Songs, t \in Tempos : TempoArrives(s, t)
        \/ \E t \in Tempos : YouSetTheTempo(t)
        \/ YouPickTheHint \/ YouPressTranscribe \/ TheJobEnds \/ StartOver

OnScreen == kind \in {"ready", "running"}
TempoIsForTheShownSong == OnScreen /\ hint # NoTempo => hintFor = song
BpmIsYours == OnScreen /\ Usable(bpm) => enteredFor = song
JobsHaveATempo == kind = "running" => Usable(bpm) /\ jobFor = song
====
