---- MODULE ServeJobs ----
EXTENDS Naturals, Sequences
CONSTANTS Jobs, Steps
VARIABLES phase, step, queue, running, served, seen
vars == <<phase, step, queue, running, served, seen>>

Final == {"done", "failed", "cancelled"}

Set(j, p) == phase' = IF phase[j] \in Final THEN phase ELSE [phase EXCEPT ![j] = p]

Init == /\ phase = [j \in Jobs |-> "queued"] /\ step = [j \in Jobs |-> 0]
        /\ queue = <<>> /\ running = 0 /\ served = {} /\ seen = [j \in Jobs |-> {"queued"}]

YouSubmit(j) == /\ phase[j] = "queued" /\ step[j] = 0 /\ running # j /\ \A i \in 1..Len(queue) : queue[i] # j
                /\ queue' = Append(queue, j) /\ UNCHANGED <<phase, step, running, served>>
TheWorkerTakesAJob == /\ running = 0 /\ queue # <<>>
                      /\ running' = Head(queue) /\ queue' = Tail(queue)
                      /\ Set(Head(queue), "running") /\ step' = [step EXCEPT ![Head(queue)] = 1]
                      /\ UNCHANGED served
TheStepFinishes == /\ running # 0
                   /\ IF phase[running] \in Final THEN running' = 0 /\ UNCHANGED <<phase, step>>
                      ELSE IF step[running] = Steps THEN Set(running, "done") /\ running' = 0 /\ UNCHANGED step
                      ELSE step' = [step EXCEPT ![running] = @ + 1] /\ UNCHANGED <<phase, running>>
                   /\ UNCHANGED <<queue, served>>
TheStepFails == /\ running # 0 /\ phase[running] \notin Final
                /\ Set(running, "failed") /\ running' = 0 /\ UNCHANGED <<step, queue, served>>
YouCancel(j) == /\ phase[j] \notin Final /\ Set(j, "cancelled") /\ UNCHANGED <<step, queue, running, served>>
YouFetchResults(j) == /\ phase[j] = "done" /\ served' = served \cup {j} /\ UNCHANGED <<phase, step, queue, running>>

Act == \/ \E j \in Jobs : YouSubmit(j) \/ YouCancel(j) \/ YouFetchResults(j)
       \/ TheWorkerTakesAJob \/ TheStepFinishes \/ TheStepFails
Idle == /\ queue = <<>> /\ running = 0 /\ UNCHANGED vars
Next == \/ Act /\ seen' = [j \in Jobs |-> seen[j] \cup {phase'[j]}]
        \/ Idle

CancelAndFailureStick == \A j \in Jobs : ~({"cancelled", "done"} \subseteq seen[j]) /\ ~({"failed", "done"} \subseteq seen[j])
ResultsOnlyWhenDone == \A j \in served : phase[j] = "done"
WorkerNeverRunsADoneOrFailedJob == running # 0 => phase[running] \notin {"done", "failed"}
====
