# Browser drivers

Playwright scripts that use the real app the way a person does and print what they observed. They are
how every UI change in ADR-0018 to ADR-0022 was verified. They are not in CI yet (`docs/roadmap.md`, 6.4).

| Driver | Checks |
|---|---|
| `song_to_notes.py` | Drop a song, tempo suggested, ½×, Transcribe, notes loaded with fitted pitch; play/pause, the tempo popover, the View menu by keyboard, File → New song and Back. Needs `sona serve`. |
| `pitch_handles.py` | Drag, add and remove pitch points; drag the vibrato's start, fades, depth and period; a typed value is committed before a drag. |
| `pitch_pen.py` | The pitch pen draws and erases the white line; the singer's-pitch export carries it. |
| `sung_pitch_export.py` | "USTX with the singer's pitch" equals the plain USTX apart from its `pitd` curve. |
| `lyrics_export.py` | Paste lyrics, type lyrics, then UST/USTX export; a saved session reloads byte-identically. |
| `fit_and_export.py` | Fit pitch, edit a vibrato, export UST/USTX; a session reload exports the same UST. |
| `cancel_and_retry.py` | Cancel stops the server job, keeps the song and tempo, and a retry starts at once. Needs `sona serve`. |
| `undo_redo.py` | A tempo burst, a note drag, a lyric and a pen stroke undo one step each and redo exactly; Ctrl+Z in a text field leaves the document alone; reopening a file starts a fresh history. |
| `stale_hover.py` | Delete after a scroll acts on what is under the mouse now. |
| `lite_autosave.py` | With storage forced full, pen edits survive a reload and resume. |
| `motion.py` | Animations run, and are off under `prefers-reduced-motion`. |

## Running

```sh
pip install playwright && playwright install chromium
(cd web && npm run dev)                     # the editor drivers, http://localhost:5173
cargo run --release -p sona-cli -- serve    # song_to_notes (build web/dist first)

export SONA_E2E_ANNOTATION=/path/to/annotation.json   # `sona transcribe … --grid 16`, with its f0 contour
export SONA_E2E_STEM=/path/to/stem.wav                 # that annotation's vocal stem
export SONA_E2E_SONG=/path/to/short-song.mp3           # any 20–30 s full mix
web/e2e/run_all.sh                                     # SONA_E2E_SKIP_SERVE=1 skips song_to_notes
```

Screenshots and downloads go to `$SONA_E2E_OUT` (default `/tmp/sona-e2e`). The audio and annotations are
yours and stay local: never commit them (CI's corpus guard enforces this for the UST corpus).

Some drivers find notes and handles by colour on the canvas, and a few use fixed screen positions tuned on
a 1440–1500 × 860 viewport with the *Denpa* annotation at 125 BPM. On other material, adjust the
positions, or read the driver's printout rather than its exit code.
