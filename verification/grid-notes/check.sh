#!/bin/sh
# Model-check the grid decoder's note assembly (GridNotes.tla) with TLC.
#   ./check.sh           decoder rules + the known snap finding + mutants
#   ./check.sh --quick   decoder rules only
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

echo "Decoder rules (every cell path, NCells=5):"
expect "GridNotes.cfg" ok "$(tlc "$here/GridNotes.tla" "$here/GridNotes.cfg")"
[ "${1:-}" = "--quick" ] && { rm -rf "$tmp"; exit $fail; }

echo "Known finding (review-surface snap on a dragged grid):"
expect "GridNotesSnap.cfg" SnappedMonophonic "$(tlc "$here/GridNotes.tla" "$here/GridNotesSnap.cfg")"

# Mutants: each breaks one line of the modelled code; its rule must catch it.
small="$tmp/specs/small.cfg"
sed 's/NCells = 5/NCells = 4/' "$here/GridNotes.cfg" > "$small"
mutant() { # NAME RULE SED-EXPR
  mkdir -p "$tmp/specs/$1"; sed "$3" "$here/GridNotes.tla" > "$tmp/specs/$1/GridNotes.tla"
  cmp -s "$here/GridNotes.tla" "$tmp/specs/$1/GridNotes.tla" && { echo "  FAIL  mutant $1 did not apply"; fail=1; return; }
  cp "$small" "$tmp/specs/$1/GridNotes.cfg"   # TLC reads the config beside the spec
  expect "mutant $1" "$2" "$(tlc "$tmp/specs/$1/GridNotes.tla" "$tmp/specs/$1/GridNotes.cfg")"
}
echo "Mutants (each must be caught):"
mutant cont-ignores-adjacency RestsStayRests 's/\/\\ Last(notes).off = a \/\\ Last(notes).p = st.p/\/\\ Last(notes).p = st.p/'
mutant cont-ignores-pitch     SungCellsKept  's/\/\\ Last(notes).off = a \/\\ Last(notes).p = st.p/\/\\ Last(notes).off = a/'
mutant zero-length-note       DecodedOnGrid  's/ELSE Append(notes, Note(a, b, st.p))/ELSE Append(notes, Note(a, a, st.p))/'
mutant fill-off-by-one        RestsStayRests 's/gap <= FillCells \* Q/gap <= (FillCells + 1) * Q/'
mutant fill-to-next-offset    DecodedMonophonic 's/\[notes EXCEPT !\[i - 1\].off = notes\[i\].on\]/[notes EXCEPT ![i - 1].off = notes[i].off]/'
mutant snap-rounds-up         SnapKeepsDecodedGrid 's/(t - shift + Q \\div 2) \\div Q/(t - shift + Q) \\div Q/'
rm -rf "$tmp"
exit $fail
