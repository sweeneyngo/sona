# sona

A tool for transcribing a single isolated vocal line into MIDI and singing-synth projects (UST for UTAU, USTX for OpenUtau), specialized for vocals ripped from a finished mix (not dry studio stems). Scope grew on 2026-11-22 ([ADR-0018](docs/adr/0018-scope-utau-authoring.md)): lyrics, UST/USTX export and pitch curves (portamento, vibrato; [ADR-0019](docs/adr/0019-pitch-expression-portamento-vibrato.md); edited by dragging handles on the roll, and the singer's own pitch exported as OpenUtau's `pitd` curve with a pitch pen; [ADR-0021](docs/adr/0021-direct-pitch-editing-and-sung-pitch.md)) are in; one-click generation (`sona serve`: source separation + transcription from the app; [ADR-0020](docs/adr/0020-one-click-generation-sona-serve.md)) is in; a one-step start screen with tempo estimation ([ADR-0022](docs/adr/0022-tempo-estimation-and-one-step-start.md); it asks for the BPM and offers the estimate as a hint, [ADR-0028](docs/adr/0028-ask-for-the-bpm-first.md)) is in; re-transcribing from the editor at a corrected BPM, by the upload's hash and undoable ([ADR-0032](docs/adr/0032-retranscribe-by-upload-hash.md)), is in; notation and VSQX remain out. What's next: [docs/roadmap.md](docs/roadmap.md).

## Language

**Perception vs. interpretation**:
The core split. *Perception* = the Rust core's job: audio → raw note events (onset/offset in seconds), pitch, confidence, f0 contour, energy envelope. *Interpretation* = the UI's job: grid anchoring, quantization/snap, correction, lyrics, and MIDI/UST/USTX export. The core's output is invariant to musical settings.

**Annotation JSON**:
The primary artifact and the core↔UI contract: raw note events + confidence (scalar + sub-scores) + f0 contour + energy envelope + BPM, plus optional provenance (ADR-0024) and `bar_one` (ADR-0031). Rust types in `crates/sona-core/src/annotation.rs`, TypeScript in `web/lib/annotation.ts`. MIDI is derived from it (raw MIDI optionally from the core; the authoritative corrected MIDI is exported by the UI). Any producer that emits this JSON can be auditioned in the review surface.
_Avoid_: "the MIDI output" as the primary artifact

**Comparison harness**:
How "outperform basic-pitch" and "understand each backend" were planned to be measured: every producer emits the annotation JSON, so all can be auditioned in the same review surface over the same stem, one at a time (single-load). The contract names `pyin`, `crepe`, `basic-pitch` and `game`; only pYIN and GAME exist (the CREPE backend and the basic-pitch adapter were never built, ADR-0001). Since ADR-0011 backends are compared by metrics on labelled data (Vocadito, then Pifuyuu's USTs), with listening as a check.

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
Beat basic-pitch *on isolated vocal stems specifically* — cleaner note boundaries and fewer spurious notes when both are fed the same stem — not beat it at its own polyphonic, instrument-agnostic problem. The practical yardstick is "how much manual correction did the output need". First judged by ear; since ADR-0011 measured: COnP/COnPOff on Vocadito, then the share of grid cells matching Pifuyuu's USTs, and now the edit cost of fixing the result in OpenUtau (`sona_eval::correction::edit_cost`).
_Avoid_: "better than basic-pitch" (unqualified)

**Review surface**:
A web piano-roll where the user listens to sona's output against the stem, corrects flagged notes, attaches lyrics, and exports MIDI, UST or USTX. Modeled (as reference, not copy) on onlinesequencer.net — BPM, grid, bars (4/4 only; there is no time-signature setting), playback, export. Two jobs in one: (1) where a result is heard and checked by ear (the numbers come from the eval harness, ADR-0011), and (2) where the remaining ~20% of manual correction happens before export. Fed by the headless Rust core's JSON output; the core itself has no UI.
_Avoid_: viewer (it also edits), DAW plugin

**Confidence (per note)**:
A score that tells the user *where to listen* in the review surface, turning "re-listen to the whole song" into "check these few flagged notes." It is the thing that makes ear-based evaluation tractable. For the target material, confidence chiefly guards against **spurious notes from separation bleed** (penalize notes near the noise floor) and derives offset confidence from the **steepness of the energy drop** (sharp drop = high, slow tail = low) rather than from whether a fallback was used.

Composed as **per-dimension sub-scores** (pitch stability, voicing/bleed, onset clarity, offset clarity), each 0–1, combined by **weakest-link soft-min** into a single scalar — a note is only as trustworthy as its shakiest property; averaging would mask the very notes that need flagging. Both the scalar (drives the piano-roll heatmap + sort order) and the sub-scores (let the UI show *why* a note is flagged, e.g. "⚠ uncertain offset") are emitted. The **flag threshold** is a UI slider, calibrated by ear per song — not a hardcoded constant. The learned cell stage's notes are scored differently (ADR-0033): a stage result outlines each song's least confident **share** of notes instead (default 20%), and an outlined note gives the stage's own reason, a long note or an unsure model ([ADR-0034](docs/adr/0034-flag-doubtful-stage-notes-by-share.md)).

**Note boundary**:
The onset and offset of a single note. Offsets are the hardest sub-problem, made worse by wet stems: reverb/delay tails keep energy alive after the singer stops, so acoustic decay ≠ musical note end. For the target material (staccato Vocaloid) offsets are easier — sharp energy drops to the noise floor.

**Tempo**:
A single fixed BPM, typed by the user ([ADR-0028](docs/adr/0028-ask-for-the-bpm-first.md)); the estimate from the mix ([ADR-0022](docs/adr/0022-tempo-estimation-and-one-step-start.md)) is only a hint. Tempo changes are deferred (roadmap 2.2) — Vocaloid material is single-tempo and grid-locked. The mix's beat is tracked only to place bar 1, opt-in (ADR-0031).
_Avoid_: tempo map, tempogram (out of scope)

**Song lyric**:
The song's whole lyric, which the user gives on the start screen beside the BPM ([ADR-0035](docs/adr/0035-lyrics-are-required.md)): hiragana, with ー, small kana and っ, one **sung line** per line (a line is a phrase; spaces also separate). A job needs one; anything else (katakana, kanji, romaji, punctuation) is named and refused, never converted. It travels with the `sona serve` job, which keeps it as `job.lyric` in the annotation's provenance, so a saved session and a re-transcription keep it. Until lyrics-first 04, the notes take it a **mora** per note in time order, as the units module (`web/lib/units/`, lyrics-first 08) splits it: きゃ is one mora and ん its own; っ is no note (it marks the mora before **clipped**); a **long-vowel tail** (ー, or a vowel spelled out after its own, おう, えい, かあ) is an **optional** unit the fit may give no note, and laid in order only ー and おう are held in the note before. **Lyric transcription** (the fallback, for a user with no lyric) is shown but off until lyrics-first 06.
_Avoid_: "lyrics" for a note's own lyric (that is the note's **lyric**, one mora); "romaji" input (not taken yet)

**Grid mode**:
A producer ([ADR-0015](docs/adr/0015-grid-conditioned-cell-decoding.md)) for grid-locked material. Given the BPM and a division (16 or 24 cells per whole note), the core decodes notes per grid cell, not per frame, and estimates the phase itself. Its notes already sit on grid lines, so the UI's auto-anchored snap leaves them unchanged. It is the one place the core is *not* invariant to musical settings, deliberately. `sona serve` (the app) runs it on every job; `sona transcribe` runs it only with `--grid`, and is grid-free otherwise. GAME and the learned cell stage are snapped to the same kind of grid.
_Avoid_: "quantized output" for grid mode (nothing is snapped after the fact; the grid is the decode's state space)

**Grid anchor (phase)**:
Where the quantization grid sits in time — distinct from BPM, which only sets grid *spacing*. Auto-derived by assuming the first detected onset lands on a grid line; overridable by dragging the ruler in the review surface, which pins bar 1 and carries it along ([ADR-0029](docs/adr/0029-export-frame-bar-one.md)); before any drag or Shift-click, exports use only the anchor's phase within a cell. Pickup notes and leading silence are placed by choosing bar 1 (see Frame). Getting phase wrong makes hard-snap quantization actively harmful.

**Frame (bar 1)**:
The BPM, the grid (division + anchor) and **bar 1**, a downbeat in stem seconds ([ADR-0029](docs/adr/0029-export-frame-bar-one.md)). The review surface draws cell, beat and bar lines from it, and every export (MIDI, UST, USTX) puts tick 0 on one of its bar lines, so OpenUtau's beats and measures are the editor's. Bar 1 is unset by default (then it is the first grid line at or after 0 s, the pre-frame rule); Shift-click on the ruler sets it, and a ruler drag carries it along with the grid. 4/4 only. With the opt-in **bar 1 from the music** ([ADR-0031](docs/adr/0031-bar-one-from-the-music.md)), the annotation's `bar_one` (the mix's beat, the bar from harmonic change, a confidence) is where bar 1 starts and where Auto returns.
_Avoid_: "offset" for bar 1 (the grid anchor's phase is a sub-cell offset; bar 1 picks which line is a downbeat)
