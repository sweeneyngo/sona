#!/bin/sh
# Model-check the review surface's inline lyric editor (LyricEditor.tla) with TLC.
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

# tlc SPEC CFG -> prints the violated invariant's name, or "ok"
tlc() {
  work=$(mktemp -d "$tmp/tlc.XXXXXX")
  out=$(cd "$work" && "$java" -XX:+UseParallelGC -Djava.io.tmpdir="$work" -cp "$jar" tlc2.TLC \
        -workers auto -metadir "$work/meta" -config "$2" "$1" 2>&1)
  if echo "$out" | grep -q "No error has been found"; then echo ok
  else echo "$out" | sed -n 's/^Error: Invariant \([A-Za-z]*\) is violated.*/\1/p' | head -1 | grep . || { echo "$out" | tail -20 >&2; echo "tlc-error"; }
  fi
}

expect() { # LABEL WANT GOT
  if [ "$2" = "$3" ]; then echo "  pass  $1 ($3)"; else echo "  FAIL  $1: wanted $2, got $3"; fail=1; fi
}

echo "Rules (every path, 3 notes):"
expect "LyricEditor.cfg" ok "$(tlc "$here/LyricEditor.tla" "$here/LyricEditor.cfg")"
[ "${1:-}" = "--quick" ] && { rm -rf "$tmp"; exit $fail; }

# Mutants: each breaks one line of the modelled code; its rule must catch it.
mutant() { # NAME RULE SED-EXPR
  mkdir -p "$tmp/specs/$1"; sed "$3" "$here/LyricEditor.tla" > "$tmp/specs/$1/LyricEditor.tla"
  cmp -s "$here/LyricEditor.tla" "$tmp/specs/$1/LyricEditor.tla" && { echo "  FAIL  mutant $1 did not apply"; fail=1; return; }
  cp "$here/LyricEditor.cfg" "$tmp/specs/$1/LyricEditor.cfg"   # TLC reads the config beside the spec
  expect "mutant $1" "$2" "$(tlc "$tmp/specs/$1/LyricEditor.tla" "$tmp/specs/$1/LyricEditor.cfg")"
}
echo "Mutants (each must be caught):"
# The bug found on 2026-11-22: opening the box scrolled time but not pitch.
mutant no-pitch-scroll        NoteBeingEditedIsOnScreen "s/inPitchView' = \[inPitchView EXCEPT !\[i\] = TRUE\]/inPitchView' = inPitchView/"
mutant enter-keeps-highlight  HighlightedIsEdited       "s/^Open(i) == \/\\\\ selected' = i/Open(i) == \/\\\\ selected' = selected/"
# A box left open on no note also leaves the old note highlighted; TLC reports that first.
mutant close-leaves-box-open  HighlightedIsEdited       "s/^Close == open' = FALSE/Close == open' = TRUE/"
rm -rf "$tmp"
exit $fail
