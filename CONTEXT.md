# sona

A tool for transcribing a single isolated vocal line into MIDI and singing-synth projects (UST for UTAU, USTX for OpenUtau), specialized for vocals ripped from a finished mix (not dry studio stems). Scope grew on 2026-11-22 ([ADR-0018](docs/adr/0018-scope-utau-authoring.md)): lyrics, UST/USTX export and pitch curves (portamento, vibrato; [ADR-0019](docs/adr/0019-pitch-expression-portamento-vibrato.md); edited by dragging handles on the roll, and the singer's own pitch exported as OpenUtau's `pitd` curve with a pitch pen; [ADR-0021](docs/adr/0021-direct-pitch-editing-and-sung-pitch.md)) are in; one-click generation (`sona serve`: source separation + transcription from the app; [ADR-0020](docs/adr/0020-one-click-generation-sona-serve.md)) is in; notation and VSQX remain out.

## Language

**Perception vs. interpretation**:
The core split. *Perception* = the Rust core's job: audio → raw note events (onset/offset in seconds), pitch, confidence, f0 contour, energy envelope. *Interpretation* = the UI's job: grid anchoring, quantization/snap, correction, lyrics, and MIDI/UST/USTX export. The core's output is invariant to musical settings.

**Annotation JSON**:
The primary artifact and the core↔UI contract: raw note events + confidence (scalar + sub-scores) + f0 contour + energy envelope + BPM. MIDI is derived from it (raw MIDI optionally from the core; the authoritative corrected MIDI is exported by the UI). Any producer that emits this JSON — including basic-pitch via an adapter — can be auditioned in the review surface.
_Avoid_: "the MIDI output" as the primary artifact

**Comparison harness**:
How "outperform basic-pitch" and "understand each backend" get measured: every producer — the pYIN backend, the CREPE backend, and basic-pitch (via a thin adapter mapping its note probabilities to sona's confidence) — emits the annotation JSON, so all are auditioned by ear in the same review surface over the same stem. v1 ships **single-load** comparison (load one, listen, load the next); a side-by-side colored overlay is a fast-follow.

**Vocal stem**:
The single-voice audio input to sona: extracted from a final mix (via UVR or similar), so it carries compression and separation artifacts. Not a dry/close-mic studio recording — but for the *target material* it is dry-ish in style (see below), so the dominant noise source is **UVR separation bleed/artifacts, not reverb tails.**
_Avoid_: dry vocal, acapella, DI

**Target material**:
Mostly Vocaloid songs with staccato, pitch-precise singing — synthesized, so f0 is unusually clean and stable, notes are short with clear gaps and sharp energy drops, and timing is tightly aligned to the grid. Heavy-reverb / spacey / melisma-heavy vocals are out of the primary target (still handled by confidence-flagging, not optimized for).
_Avoid_: "pop vocals" (too general — the spec's accuracy table is not the target profile)

**Monophonic transcription**:
sona's task: recover a sequence of single, non-overlapping notes (pitch, onset, offset) from one vocal stem. One note sounds at a time.
_Avoid_: polyphonic transcription, AMT (those describe basic-pitch's harder, generalist problem)

**Outperform**:
Beat basic-pitch *on isolated vocal stems specifically* — cleaner note boundaries and fewer spurious notes when both are fed the same stem — not beat it at its own polyphonic, instrument-agnostic problem. Judged **by ear** through the review surface, not by an automated metric or public dataset. The practical yardstick is "how much manual correction did the output need."
_Avoid_: "better than basic-pitch" (unqualified), mir_eval/COnP/COnPOff (deliberately not used)

**Review surface**:
A web piano-roll where the user listens to sona's output against the stem, corrects flagged notes, attaches lyrics, and exports MIDI, UST or USTX. Modeled (as reference, not copy) on onlinesequencer.net — BPM, grid, time signature, playback, export. Two jobs in one: (1) sona's *measurement instrument* (evaluation is by ear, so this is how every backend and tweak gets judged), and (2) where the remaining ~20% of manual correction happens before export. Fed by the headless Rust core's JSON output; the core itself has no UI.
_Avoid_: viewer (it also edits), DAW plugin

**Confidence (per note)**:
A score that tells the user *where to listen* in the review surface, turning "re-listen to the whole song" into "check these few flagged notes." It is the thing that makes ear-based evaluation tractable. For the target material, confidence chiefly guards against **spurious notes from separation bleed** (penalize notes near the noise floor) and derives offset confidence from the **steepness of the energy drop** (sharp drop = high, slow tail = low) rather than from whether a fallback was used.

Composed as **per-dimension sub-scores** (pitch stability, voicing/bleed, onset clarity, offset clarity), each 0–1, combined by **weakest-link soft-min** into a single scalar — a note is only as trustworthy as its shakiest property; averaging would mask the very notes that need flagging. Both the scalar (drives the piano-roll heatmap + sort order) and the sub-scores (let the UI show *why* a note is flagged, e.g. "⚠ uncertain offset") are emitted. The **flag threshold** is a UI slider, calibrated by ear per song — not a hardcoded constant.

**Note boundary**:
The onset and offset of a single note. Offsets are the hardest sub-problem, made worse by wet stems: reverb/delay tails keep energy alive after the singer stops, so acoustic decay ≠ musical note end. For the target material (staccato Vocaloid) offsets are easier — sharp energy drops to the noise floor.

**Tempo**:
A single fixed BPM, user-supplied, for v1. Beat-by-beat / tempo-change tracking is deliberately deferred — Vocaloid material is single-tempo and grid-locked.
_Avoid_: beat tracking, tempogram (out of v1 scope)

**Grid mode**:
An opt-in producer ([ADR-0015](docs/adr/0015-grid-conditioned-cell-decoding.md)) for grid-locked material. Given the BPM and a division (16 or 24 cells per whole note), the core decodes notes per grid cell, not per frame, and estimates the phase itself. Its notes already sit on grid lines, so the UI's auto-anchored snap leaves them unchanged. It is the one place the core is *not* invariant to musical settings, deliberately and only when asked. The default path stays grid-free.
_Avoid_: "quantized output" for grid mode (nothing is snapped after the fact; the grid is the decode's state space)

**Grid anchor (phase)**:
Where the quantization grid sits in time — distinct from BPM, which only sets grid *spacing*. Auto-derived by assuming the first detected onset lands on a grid line; overridable by dragging the grid offset in the review surface (needed for pickup notes / leading silence). Getting phase wrong makes hard-snap quantization actively harmful.
