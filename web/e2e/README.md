# Browser drivers

Playwright scripts that use the real app the way a person does, print what they observed, and exit non-zero
when a check fails. They are how every UI change in ADR-0018 to ADR-0022 was verified. CI runs the editor drivers (job `e2e`,
`scripts/ci/e2e.sh`); the four that need `sona serve` run locally only (`docs/roadmap.md`, 6.4).

| Driver | Checks |
|---|---|
| `song_to_notes.py` | Drop a song: the BPM field is empty and focused, the estimate is only a hint, Transcribe refuses an empty field, picking the ½× hint fills it; Transcribe, notes loaded with fitted pitch; play/pause, the tempo popover, the View menu by keyboard, File → New song and Back. Needs `sona serve`. |
| `bar_one.py` | Needs no annotation (writes its own, and a silent stem). The default UST is the pre-frame one and the MIDI sits on its ticks; Shift-click on the ruler makes that line a bar line in the ruler and in the UST/MIDI; undo/redo; a session reload exports the same UST; a ruler drag moves the bars with the grid; **Bar 1 → At playhead**. |
| `pitch_handles.py` | Add a point by double-clicking the curve, drag it, Alt-click another away; drag the vibrato's fades (and depth), period and start; a typed value is committed before a drag. |
| `pitch_pen.py` | A pen stroke one row above a note draws that pitch over the note; Alt-drag erases part of it. |
| `sung_pitch_export.py` | "USTX with the singer's pitch" equals the plain USTX apart from its `pitd` curve. |
| `lyrics_export.py` | Paste lyrics, type lyrics, then UST/USTX export; a saved session reloads byte-identically. |
| `fit_and_export.py` | Fit pitch, edit a vibrato, export UST/USTX; a session reload exports the same UST. |
| `cancel_and_retry.py` | A decimal BPM typed right after the drop (`123,5`) is the job's BPM; Cancel stops the server job, keeps the song and BPM, and a retry starts at once. Needs `sona serve`. |
| `lyric_start.py` | The lyric box: Transcribe with none asks for it; a katakana line is named as you type and refused; a hiragana lyric goes with the job (line breaks kept), the notes take it a mora each, and **Edit → Lyrics…** holds it; Re-transcribe sends it again; a session reload keeps it. Needs `sona serve` (the song goes as a stem, so the synthetic fixture is enough). |
| `retranscribe.py` | Transcribe names the dropped song by its hash and sends the file only when refused; after a lyric edit and ½×, **Re-transcribe at 60 BPM** sends the hash alone, the new notes load, one Undo brings back the edited notes (and their transcription), Redo the new ones. Needs `sona serve` (stub separators are enough). |
| `undo_redo.py` | A tempo burst, a note drag, a lyric and a pen stroke undo one step each and redo exactly; Ctrl+Z in a text field leaves the document alone; reopening a file starts a fresh history. |
| `stale_hover.py` | Delete after a scroll acts on what is under the mouse now. |
| `lite_autosave.py` | With storage forced full, pen edits survive a reload and resume. |
| `motion.py` | Animations run, and are off under `prefers-reduced-motion`. |
| `help_and_grids.py` | The Help names every export, the BPM-first start, bar 1 and the setup notes; the grid menus list `lib/grid.ts`'s grids; a 1/16T session reopens on 1/16T with the same UST. |
| `doubtful_notes.py` | Needs no annotation (writes a learned-stage one from the fixture). Settings offers the share, not the threshold; 20% of the notes are outlined, each saying why ("long note…" on the held notes, "the model was unsure here" on two short ones), never a sub-score; 0% and 30% outline 0 and 14; a dragged note loses its outline and none takes its place. The plain (pYIN) fixture keeps the threshold and the weakest sub-score. Screenshot `shots/doubtful_notes.png`. |

## Running

```sh
pip install playwright && playwright install chromium
(cd web && npm run dev)                          # the editor drivers, http://localhost:5173
SONA_E2E_SKIP_SERVE=1 web/e2e/run_all.sh         # every editor driver, on the synthetic fixture
```

Or in one step, as CI runs them: `SONA_E2E_PYTHON=/path/to/python scripts/ci/e2e.sh` starts a dev server on
port 5180 (`SONA_E2E_PORT`), waits for it, runs the editor drivers into a fresh `$SONA_E2E_OUT` and stops the
server. CI pins Playwright 1.62.0 and uploads `$SONA_E2E_OUT` when a driver fails.

With nothing else set, the editor drivers use a **synthetic fixture** that `fixture.py` writes on first use
(and again whenever the file changes): eight two-bar phrases at 125 BPM, short legato notes gliding into a
held note with vibrato and a falling tail, as an annotation with its f0 contour and a three-harmonic stem
that sings it. Nothing is committed and no recording is involved.

To run on your own material, or to run `song_to_notes`, `cancel_and_retry`, `retranscribe` and `lyric_start`:

```sh
cargo run --release -p sona-cli -- serve               # song_to_notes, cancel_and_retry (build web/dist first)
export SONA_E2E_ANNOTATION=/path/to/annotation.json   # `sona transcribe … --grid 16`, with its f0 contour
export SONA_E2E_STEM=/path/to/stem.wav                 # that annotation's vocal stem
export SONA_E2E_SONG=/path/to/short-song.mp3           # any 20–30 s full mix
web/e2e/run_all.sh
```

`SONA_E2E_EDITOR_URL` and `SONA_E2E_SERVE_URL` point the drivers at a dev server or `sona serve` on another
port, and `SONA_E2E_PYTHON` at a Python with Playwright. Screenshots and downloads go to `$SONA_E2E_OUT`
(default `/tmp/sona-e2e`). Your audio and annotations stay local: never commit them (CI's corpus guard
enforces this for the UST corpus). A failed `/api` call is not counted as a console error: without
`sona serve`, the editor's capability probe fails by design.

No driver clicks a fixed screen position. Notes are found on the canvas as note-green bars (`FIND_NOTES`,
`FIND_NOTE`) and handles as handle-orange blobs (`FIND_HANDLES`); a note's row is read from its own first
handle, and which handle is which from where it sits against that row. The drivers zoom and pan by wheel and
then look again, so they work across viewports (checked at 1024 × 700 to 1920 × 1080) and on material
whose notes are at least ~100 px wide at two zoom steps. A note selection opens the pitch bar and shrinks
the roll, so positions found before a selection are found again after it.
