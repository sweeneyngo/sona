---------------------------- MODULE GridNotes ----------------------------
(***************************************************************************)
(* The grid decoder's note assembly, then the review surface's hard snap.  *)
(*                                                                         *)
(* Code this models (one action = one loop iteration in the code):        *)
(*  ReadCell  - crates/sona-core/src/grid.rs, notes_from_path: the loop    *)
(*              that turns the Viterbi state path into notes.              *)
(*  FillGap   - grid.rs, GridDecoder::fill_gaps.                           *)
(*  SnapNote  - web/lib/grid.ts quantizeNote (ported in                    *)
(*              crates/sona-eval/src/quantize.rs quantize_note).           *)
(*  Resolve   - web/lib/grid.ts resolveOverlaps (quantize.rs               *)
(*              resolve_overlaps), one step per adjacent pair. The code    *)
(*              walks in onset order; SnapKeepsOrder shows that is the     *)
(*              decode order here, so the model walks the sequence.        *)
(*                                                                         *)
(* Time is in quarter-cells (Q per cell) so the UI grid can sit a quarter, *)
(* half or three quarters of a cell off the decoded grid (the user dragged *)
(* the grid anchor). The Viterbi path is ANY sequence of cell states: the  *)
(* reader must be right even for paths the decoder never emits.            *)
(***************************************************************************)
EXTENDS Integers, Sequences, FiniteSets

CONSTANTS NCells, Pitches, FillCells, Q

Cells == 0 .. NCells - 1
Rest == [kind |-> "rest"]
CellStates == {Rest} \cup [kind : {"onset", "cont"}, p : Pitches]
Shifts == 0 .. Q - 1        \* UI anchor offset from the decoded grid
EscapeQ == 1                \* escapeDistanceCells = 0.25 of a cell

VARIABLES path, shift, clear, phase, c, i, notes, snapped
vars == <<path, shift, clear, phase, c, i, notes, snapped>>

Last(s) == s[Len(s)]
Note(on, off, p) == [on |-> on, off |-> off, p |-> p]

\* Math.round((t - anchor) / spacing) on a grid of Q-unit cells at `shift`.
SnapT(t) == shift + ((t - shift + Q \div 2) \div Q) * Q
Abs(x) == IF x < 0 THEN -x ELSE x

Init ==
  /\ path \in [Cells -> CellStates]
  /\ shift \in Shifts
  /\ clear \in [1 .. NCells -> BOOLEAN]   \* onset_clarity >= threshold, per note
  /\ phase = "read" /\ c = 0 /\ i = 1
  /\ notes = << >> /\ snapped = << >>

\* grid.rs decode: an onset state, or a cont that doesn't extend the last
\* note seamlessly at the same pitch, starts a note; a cont extends it.
ReadCell ==
  /\ phase = "read" /\ c < NCells
  /\ LET st == path[c]
         a == c * Q
         b == a + Q
     IN notes' =
          IF st = Rest THEN notes
          ELSE IF st.kind = "cont" /\ notes # << >>
                  /\ Last(notes).off = a /\ Last(notes).p = st.p
          THEN [notes EXCEPT ![Len(notes)].off = b]
          ELSE Append(notes, Note(a, b, st.p))
  /\ c' = c + 1
  /\ UNCHANGED <<path, shift, clear, phase, i, snapped>>

EndRead ==
  /\ phase = "read" /\ c = NCells
  /\ phase' = "fill" /\ i' = 2
  /\ UNCHANGED <<path, shift, clear, c, notes, snapped>>

\* grid.rs fill_gaps: a rest of at most FillCells cells between two notes
\* is absorbed by the earlier note.
FillGap ==
  /\ phase = "fill" /\ i <= Len(notes)
  /\ LET gap == notes[i].on - notes[i - 1].off
     IN notes' = IF gap > 0 /\ gap <= FillCells * Q
                 THEN [notes EXCEPT ![i - 1].off = notes[i].on]
                 ELSE notes
  /\ i' = i + 1
  /\ UNCHANGED <<path, shift, clear, phase, c, snapped>>

EndFill ==
  /\ phase = "fill" /\ i > Len(notes)
  /\ phase' = "snap" /\ i' = 1
  /\ UNCHANGED <<path, shift, clear, c, notes, snapped>>

\* grid.ts quantizeNote, applied to each note on its own.
SnapNote ==
  /\ phase = "snap" /\ i <= Len(notes)
  /\ LET n == notes[i]
         on == SnapT(n.on)
         off0 == SnapT(n.off)
         off == IF off0 <= on THEN on + Q ELSE off0
         escaped == Abs(n.on - on) >= EscapeQ /\ clear[i]
     IN snapped' = Append(snapped, IF escaped THEN n ELSE Note(on, off, n.p))
  /\ i' = i + 1
  /\ UNCHANGED <<path, shift, clear, phase, c, notes>>

EndSnap ==
  /\ phase = "snap" /\ i > Len(notes)
  /\ phase' = "resolve" /\ i' = 2
  /\ UNCHANGED <<path, shift, clear, c, notes, snapped>>

\* grid.ts resolveOverlaps: an overlap trims the earlier note; a collision
\* moves the later note to the earlier one's end, keeping at least a cell.
Resolve ==
  /\ phase = "resolve" /\ i <= Len(snapped)
  /\ LET prev == snapped[i - 1]
         next == snapped[i]
         moved == prev.off
     IN snapped' =
          IF next.on >= prev.off THEN snapped
          ELSE IF next.on > prev.on THEN [snapped EXCEPT ![i - 1].off = next.on]
          ELSE [snapped EXCEPT ![i].on = moved,
                               ![i].off = IF next.off <= moved THEN moved + Q ELSE next.off]
  /\ i' = i + 1
  /\ UNCHANGED <<path, shift, clear, phase, c, notes>>

EndResolve ==
  /\ phase = "resolve" /\ i > Len(snapped)
  /\ phase' = "done"
  /\ UNCHANGED <<path, shift, clear, c, i, notes, snapped>>

Done == phase = "done" /\ UNCHANGED vars

Next == ReadCell \/ EndRead \/ FillGap \/ EndFill \/ SnapNote \/ EndSnap
          \/ Resolve \/ EndResolve \/ Done

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Rules. Each says what a user exporting MIDI/UST is promised.            *)

Ordered(s) == \A x, y \in 1 .. Len(s) : x < y => s[x].off <= s[y].on

\* The decoder emits one voice: notes in order, never overlapping.
DecodedMonophonic == Ordered(notes)

\* Every decoded note starts and ends on a grid line and lasts a cell or more.
DecodedOnGrid ==
  \A x \in 1 .. Len(notes) :
    notes[x].on % Q = 0 /\ notes[x].off % Q = 0 /\ notes[x].off - notes[x].on >= Q

Covers(n, cell) == n.on <= cell * Q /\ cell * Q < n.off

\* Every sung cell ends up inside exactly one note, at that cell's pitch.
SungCellsKept ==
  phase \in {"snap", "done"} =>
    \A cell \in Cells : path[cell] # Rest =>
      \E x \in 1 .. Len(notes) : Covers(notes[x], cell) /\ notes[x].p = path[cell].p

\* A rest cell is only ever covered as part of a short gap before a note.
RestsStayRests ==
  phase \in {"snap", "done"} =>
    \A cell \in Cells : path[cell] = Rest =>
      \A x \in 1 .. Len(notes) : Covers(notes[x], cell) =>
        \E y \in 1 .. Len(notes) :
          notes[y].on > cell * Q /\ notes[y].on <= (cell + FillCells) * Q

\* With the review grid on the decoded grid, snapping changes nothing.
SnapKeepsDecodedGrid ==
  phase = "done" /\ shift = 0 => snapped = notes

\* Snapping never reorders notes (so resolving in decode order = onset order).
SnapKeepsOrder ==
  phase = "resolve" /\ i = 2 =>
    \A x, y \in 1 .. Len(snapped) : x < y => snapped[x].on < snapped[y].on

\* What the review surface exports is still one voice ...
SnappedMonophonic == phase = "done" => Ordered(snapped)

\* ... with every decoded note in it, none of them empty.
SnappedKeepsEveryNote ==
  phase = "done" =>
    /\ Len(snapped) = Len(notes)
    /\ \A x \in 1 .. Len(snapped) : snapped[x].off > snapped[x].on
=============================================================================
