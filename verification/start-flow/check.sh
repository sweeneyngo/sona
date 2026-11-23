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
        -workers auto -metadir "$work/meta" -config "$cfg" "$spec" 2>&1)
  if echo "$out" | grep -q "No error has been found"; then echo ok
  else echo "$out" | sed -n 's/^Error: Invariant \([A-Za-z]*\) is violated.*/\1/p' | head -1 | grep . || { echo "$out" | tail -20 >&2; echo "tlc-error"; }
  fi
}

expect() {
  label=$1 want=$2 got=$3
  if [ "$want" = "$got" ]; then echo "  pass  $label ($got)"; else echo "  FAIL  $label: wanted $want, got $got"; fail=1; fi
}

echo "Rules (every path, 3 songs, 3 tempos):"
expect "StartFlow.cfg" ok "$(violated_invariant "$here/StartFlow.tla" "$here/StartFlow.cfg")"
[ "${1:-}" = "--quick" ] && { rm -rf "$tmp"; exit $fail; }

mutant() {
  name=$1 rule=$2 edit=$3
  dir="$tmp/specs/$name"
  mkdir -p "$dir"; sed "$edit" "$here/StartFlow.tla" > "$dir/StartFlow.tla"
  cmp -s "$here/StartFlow.tla" "$dir/StartFlow.tla" && { echo "  FAIL  mutant $name did not apply"; fail=1; return; }
  cp "$here/StartFlow.cfg" "$dir/StartFlow.cfg"   # TLC reads the config beside the spec
  expect "mutant $name" "$rule" "$(violated_invariant "$dir/StartFlow.tla" "$dir/StartFlow.cfg")"
}
echo "Mutants (each must be caught):"
mutant stale-tempo-wins    TempoIsForTheShownSong "s/IF kind = \"detecting\" \/\\\\ song = s/IF kind = \"detecting\"/"
mutant start-without-tempo JobsHaveATempo         "s/kind = \"ready\" \/\\\\ Usable(bpm) \/\\\\ jobs < 2/kind = \"ready\" \/\\\\ jobs < 2/"
rm -rf "$tmp"
exit $fail
