---- MODULE ServeJobs ----
EXTENDS Naturals, Sequences
CONSTANTS Jobs, Steps
VARIABLES phase, step, queue, running, served, seen, worker, hung, removed
vars == <<phase, step, queue, running, served, seen, worker, hung, removed>>

Final == {"done", "failed", "cancelled"}
InQueue(j) == \E i \in 1..Len(queue) : queue[i] = j

Set(j, p) == phase' = IF phase[j] \in Final THEN phase ELSE [phase EXCEPT ![j] = p]

Init == /\ phase = [j \in Jobs |-> "queued"] /\ step = [j \in Jobs |-> 0]
        /\ queue = <<>> /\ running = 0 /\ served = {} /\ seen = [j \in Jobs |-> {"queued"}]
        /\ worker = "up" /\ hung = FALSE /\ removed = {}

YouSubmit(j) == /\ phase[j] = "queued" /\ step[j] = 0 /\ running # j /\ ~InQueue(j)
                /\ queue' = Append(queue, j) /\ UNCHANGED <<phase, step, running, served, worker, hung, removed>>
YouCancel(j) == /\ phase[j] \notin Final /\ Set(j, "cancelled") /\ hung' = (hung /\ running # j)
                /\ UNCHANGED <<step, queue, running, served, worker, removed>>
YouFetchResults(j) == /\ phase[j] = "done" /\ j \notin removed /\ served' = served \cup {j}
                      /\ UNCHANGED <<phase, step, queue, running, worker, hung, removed>>

Working == running # 0 /\ worker = "up"
TheWorkerTakesAJob == /\ worker = "up" /\ running = 0 /\ queue # <<>> /\ queue' = Tail(queue)
                      /\ IF phase[Head(queue)] \in Final THEN UNCHANGED <<phase, step, running>>
                         ELSE running' = Head(queue) /\ Set(Head(queue), "running") /\ step' = [step EXCEPT ![Head(queue)] = 1]
                      /\ UNCHANGED <<served, worker, hung, removed>>
TheStepFinishes == /\ Working /\ ~hung
                   /\ IF phase[running] \in Final THEN running' = 0 /\ UNCHANGED <<phase, step>>
                      ELSE IF step[running] = Steps THEN Set(running, "done") /\ running' = 0 /\ UNCHANGED step
                      ELSE step' = [step EXCEPT ![running] = @ + 1] /\ UNCHANGED <<phase, running>>
                   /\ UNCHANGED <<queue, served, worker, hung, removed>>
TheStepFails == /\ Working /\ ~hung /\ phase[running] \notin Final
                /\ Set(running, "failed") /\ running' = 0 /\ UNCHANGED <<step, queue, served, worker, hung, removed>>
TheJobPanics == /\ Working /\ ~hung /\ Set(running, "failed") /\ running' = 0 /\ UNCHANGED <<step, queue, served, worker, hung, removed>>
TheToolHangs == /\ Working /\ ~hung /\ phase[running] \notin Final /\ hung' = TRUE
                /\ UNCHANGED <<phase, step, queue, running, served, worker, removed>>
TheDeadlinePasses == /\ Working /\ hung
                     /\ Set(running, "failed") /\ running' = 0 /\ hung' = FALSE /\ UNCHANGED <<step, queue, served, worker, removed>>
TheCleanerRemoves(j) == /\ worker = "up" /\ running = 0 /\ phase[j] \in Final /\ j \notin removed
                        /\ removed' = removed \cup {j} /\ UNCHANGED <<phase, step, queue, running, served, worker, hung>>

Act == \/ \E j \in Jobs : YouSubmit(j) \/ YouCancel(j) \/ YouFetchResults(j) \/ TheCleanerRemoves(j)
       \/ TheWorkerTakesAJob \/ TheStepFinishes \/ TheStepFails \/ TheJobPanics \/ TheToolHangs \/ TheDeadlinePasses
Track == seen' = [j \in Jobs |-> seen[j] \cup {phase'[j]}]
Idle == /\ queue = <<>> /\ running = 0 /\ UNCHANGED vars
Next == \/ Act /\ Track
        \/ Idle
Fairness == /\ WF_vars(TheWorkerTakesAJob /\ Track) /\ WF_vars(TheStepFinishes /\ Track)
            /\ WF_vars(TheDeadlinePasses /\ Track)
Spec == Init /\ [][Next]_vars /\ Fairness

CancelAndFailureStick == \A j \in Jobs : \A a, b \in seen[j] \cap Final : a = b
ResultsOnlyWhenDone == \A j \in served : phase[j] = "done"
WorkerNeverRunsADoneOrFailedJob == running # 0 => phase[running] \notin {"done", "failed"}
OnlyFinishedJobsAreRemoved == \A j \in removed : phase[j] \in Final
TheRunningJobKeepsItsFolder == running # 0 => running \notin removed
EveryJobFinishes == \A j \in Jobs : (InQueue(j) \/ running = j) ~> (phase[j] \in Final /\ running # j)
====
