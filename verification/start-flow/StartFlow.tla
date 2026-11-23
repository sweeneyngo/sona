---- MODULE StartFlow ----
EXTENDS Naturals
CONSTANTS Songs, Tempos
NoTempo == 0
Usable(t) == t # NoTempo
VARIABLES kind, song, bpm, detectedFor, pending, jobFor, jobs
vars == <<kind, song, bpm, detectedFor, pending, jobFor, jobs>>

Init == /\ kind = "idle" /\ song = 0 /\ bpm = NoTempo /\ detectedFor = 0
        /\ pending = {} /\ jobFor = 0 /\ jobs = 0

Choose(s) == /\ kind \in {"idle", "detecting", "ready"}
             /\ kind' = "detecting" /\ song' = s /\ pending' = pending \cup {s}
             /\ UNCHANGED <<bpm, detectedFor, jobFor, jobs>>
TempoArrives(s, t) == /\ s \in pending
                      /\ pending' = pending \ {s}
                      /\ IF kind = "detecting" /\ song = s
                         THEN kind' = "ready" /\ bpm' = t /\ detectedFor' = s
                         ELSE UNCHANGED <<kind, bpm, detectedFor>>
                      /\ UNCHANGED <<song, jobFor, jobs>>
YouSetTheTempo(t) == /\ kind = "ready" /\ bpm' = t
                     /\ UNCHANGED <<kind, song, detectedFor, pending, jobFor, jobs>>
YouPressTranscribe == /\ kind = "ready" /\ Usable(bpm) /\ jobs < 2
                      /\ kind' = "running" /\ jobFor' = song /\ jobs' = jobs + 1
                      /\ UNCHANGED <<song, bpm, detectedFor, pending>>
TheJobEnds == /\ kind = "running"
              /\ \/ kind' = "loaded"
                 \/ kind' = "ready"
              /\ UNCHANGED <<song, bpm, detectedFor, pending, jobFor, jobs>>
StartOver == /\ kind \in {"ready", "loaded", "detecting"}
             /\ kind' = "idle" /\ UNCHANGED <<song, bpm, detectedFor, pending, jobFor, jobs>>

Next == \/ \E s \in Songs : Choose(s)
        \/ \E s \in Songs, t \in Tempos : TempoArrives(s, t)
        \/ \E t \in Tempos : YouSetTheTempo(t)
        \/ YouPressTranscribe \/ TheJobEnds \/ StartOver

TempoIsForTheShownSong == kind = "ready" /\ detectedFor # 0 /\ bpm # NoTempo => detectedFor = song
JobsHaveATempo == kind = "running" => Usable(bpm) /\ jobFor = song
====
