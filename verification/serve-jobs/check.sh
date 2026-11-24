#!/bin/sh
#   ./check.sh           every rule + mutants
#   ./check.sh --quick   rules only
# Needs Java 11+ and tla2tools.jar 1.7.4 (sha256 936a2620…0e88). Set TLA2TOOLS to
# the jar; default ~/.cache/tla/tla2tools.jar.
set -u
here=$(cd "$(dirname "$0")" && pwd)
jar=${TLA2TOOLS:-$HOME/.cache/tla/tla2tools.jar}
java=${JAVA:-java}
[ -f "$jar" ] || { echo "no tla2tools.jar at $jar (set TLA2TOOLS)"; exit 2; }
tmp=$(mktemp -d)
mkdir -p "$tmp/specs"
fail=0

violated_invariant() {
  spec=$1 cfg=$2
  work=$(mktemp -d "$tmp/tlc.XXXXXX")
  out=$(cd "$work" && "$java" -XX:+UseParallelGC -Djava.io.tmpdir="$work" -cp "$jar" tlc2.TLC \
        -workers auto -deadlock -metadir "$work/meta" -config "$cfg" "$spec" 2>&1)
  if echo "$out" | grep -q "No error has been found"; then echo ok
  elif echo "$out" | grep -q "Temporal properties were violated"; then echo EveryJobFinishes
  else echo "$out" | sed -n 's/^Error: Invariant \([A-Za-z]*\) is violated.*/\1/p' | head -1 | grep . || { echo "$out" | tail -20 >&2; echo "tlc-error"; }
  fi
}

expect() {
  label=$1 want=$2 got=$3
  if [ "$want" = "$got" ]; then echo "  pass  $label ($got)"; else echo "  FAIL  $label: wanted $want, got $got"; fail=1; fi
}

echo "Rules (every path, 2 jobs of 2 steps; liveness with the worker's own steps fair):"
expect "ServeJobs.cfg" ok "$(violated_invariant "$here/ServeJobs.tla" "$here/ServeJobs.cfg")"
[ "${1:-}" = "--quick" ] && { rm -rf "$tmp"; exit $fail; }

mutant() {
  name=$1 rule=$2 edit=$3
  dir="$tmp/specs/$name"
  mkdir -p "$dir"; sed "$edit" "$here/ServeJobs.tla" > "$dir/ServeJobs.tla"
  cmp -s "$here/ServeJobs.tla" "$dir/ServeJobs.tla" && { echo "  FAIL  mutant $name did not apply"; fail=1; return; }
  cp "$here/ServeJobs.cfg" "$dir/ServeJobs.cfg"   # TLC reads the config beside the spec
  expect "mutant $name" "$rule" "$(violated_invariant "$dir/ServeJobs.tla" "$dir/ServeJobs.cfg")"
}
echo "Mutants (each must be caught):"
mutant set-phase-not-absorbing  CancelAndFailureStick       's/^Set(j, p) == phase. = IF phase\[j\] \\in Final THEN phase ELSE \[phase EXCEPT !\[j\] = p\]/Set(j, p) == phase'"'"' = [phase EXCEPT ![j] = p]/'
mutant results-before-done      ResultsOnlyWhenDone         's/^YouFetchResults(j) == \/\\ phase\[j\] = "done" \/\\ /YouFetchResults(j) == \/\\ /'
mutant failure-keeps-worker     WorkerNeverRunsADoneOrFailedJob '/^TheStepFails/,/^TheJobPanics/s/^\( *\/\\ \)Set(running, "failed") \/\\ running. = 0/\1Set(running, "failed") \/\\ running'"'"' = running/'
mutant panic-kills-worker       EveryJobFinishes            's/^TheJobPanics == .*/TheJobPanics == \/\\ Working \/\\ ~hung \/\\ worker'"'"' = "down" \/\\ UNCHANGED <<phase, step, queue, running, served, hung, removed>>/'
mutant no-deadline              EveryJobFinishes            's/^TheDeadlinePasses == /TheDeadlinePasses == FALSE \/\\ /'
mutant cleaner-takes-queued     OnlyFinishedJobsAreRemoved  's/^\(TheCleanerRemoves(j) == \/\\ worker = "up" \/\\ running = 0\) \/\\ phase\[j\] \\in Final/\1/'
mutant cleaner-during-a-job     TheRunningJobKeepsItsFolder 's/^\(TheCleanerRemoves(j) == \/\\ worker = "up"\) \/\\ running = 0/\1/'
rm -rf "$tmp"
exit $fail
