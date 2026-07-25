# 07 — Post-processing + wire behind the Segmenter seam

Status: done
Blocked by: 06
Phase: B (note-HMM)

Turn the decoded path into `NoteSegment`s and swap it in.

- **Per-segment final pitch:** alpha-trimmed mean of a note's voiced frames → integer MIDI, decoupled from the 3-bin decode grid.
- **Post-process:** prune notes < **~100 ms**; merge adjacent same-pitch fragments.
- Still measure **energy-drop steepness** at each offset — retained purely as confidence raw material (ADR-0007's one surviving output).
- Wire the note-HMM behind the existing `Segmenter` → `Vec<NoteSegment>` seam; downstream (confidence, CLI, JSON) unchanged. Keep the old heuristic reachable (feature/flag) until 08 signs off.

Done when: the pipeline emits notes via the HMM end-to-end; annotation JSON schema unchanged; existing pipeline/CLI tests green.
