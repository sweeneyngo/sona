# ADR-0028: The start screen asks for the song's BPM; the estimate is a hint

Status: accepted, 2026-11-24. Amends [ADR-0022](0022-tempo-estimation-and-one-step-start.md), decision 2
(the start screen). Ticket: `.scratch/utau-edit-cost/issues/04-bpm-first-start-screen.md`.

## Context

ADR-0022 pre-filled the BPM field with the tempo estimate and put ½× and 2× beside it. Two things make
that the wrong default:
- **Users usually know their song's exact BPM** (user, 2026-11-24). Vocaloid songs publish it, and the
  UST being written will carry it.
- **A wrong BPM is the costliest fix in OpenUtau** (`.scratch/utau-edit-cost/spec.md`, tier "Hard"). The
  estimator searches only 140–280 BPM, so every slow song arrives at double tempo (all 7 in ADR-0022's
  test set). A pre-filled value is easy to accept without reading. And at ±2%, even a "right" estimate
  can be a fraction off a BPM the user knows exactly.

## Decision

1. **The BPM field starts empty and asks for the song's BPM.** It has focus as soon as a song is dropped,
   so the user can type before the estimate arrives. It accepts decimals, with a point or a comma
   (`123.5`, `123,5`), as a text field with a decimal keypad.
2. **The estimate is a hint beside the field:** "detected ≈ `246` `½× 123`". Each value is a button, and
   clicking one fills the field. The hint offers whole BPMs because the estimate is good to about 2%,
   so its tenths claim a precision it doesn't have. The hint shows "finding the tempo…" while the
   estimate is still coming and "no steady beat found" when there is none. The ½×/2× buttons on the
   start screen are gone. The editor's tempo popover keeps them.
3. **Transcribe still needs a usable BPM (20–400).** With the field empty it says "Enter the song's BPM
   (20–400): it sets the grid the notes snap to." and starts nothing.
4. **The reducer (`web/lib/startFlow.ts`) keeps its guarantees and gains one:**
   - The separate "detecting" state is gone. A dropped song is `ready` at once, with
     `detected: "pending"`.
   - A tempo answer only ever sets the hint, and only for the song it was asked about. It is also
     taken while a job runs, so Cancel comes back with the hint (before, a cancel dropped it).
   - **New rule:** the BPM is one the user typed or picked for this song. An answer never fills it, and
     a new song starts empty.
   - One job at a time, as before.

## Evidence

- **Reducer** (`web/lib/startFlow.test.ts`): fixed paths and 3 000 random event orders. Each step
  checks that:
  - a running job has a usable BPM and stays on its song;
  - the hint shown is the estimate of the song shown;
  - a tempo answer leaves the BPM unchanged;
  - a new song starts with no BPM.

  Two broken reducers were tried and caught: an answer that fills the BPM failed 4 tests, and an answer
  applied to any song failed 2.
- **Model** (`verification/start-flow`, TLA+): a new invariant, `BpmIsYours`. The run covers 3 songs and
  3 tempos. All 4 mutants are caught:
  - `stale-tempo-wins` → TempoIsForTheShownSong;
  - `start-without-tempo` → JobsHaveATempo;
  - `estimate-becomes-bpm` (new) → BpmIsYours;
  - `bpm-carries-over` (new) → BpmIsYours.
- **Browser** (Chromium against `sona serve`, `web/e2e`):
  - `song_to_notes` on the 25 s sample: on drop, the field is empty and focused. The estimate arrived
    as "detected ≈ `246` `½× 123`" and left the field empty. Transcribe with the field empty showed the
    message and started nothing. Picking ½× filled `123`, which is the UST's tempo.
  - `cancel_and_retry`: `123,5` typed straight after the drop, before the estimate, was submitted as
    `bpm=123.5`, and the field kept `123.5` after Cancel.

## Consequences

- A user who knows the BPM types it once and never meets the octave error. A user who doesn't picks
  one of two values. That is still one click, as ½× was, but it is now a choice the user makes rather
  than a default they accept.
- Roadmap 2.1 (choose the octave automatically) matters less for the app. It still matters for a
  "detected" hint that should lead with the likelier value.
