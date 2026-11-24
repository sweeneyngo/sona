# ADR-0032: Re-transcribe from the editor by the upload's hash; the new notes are one undoable step

Status: accepted, 2026-11-24. Builds on [ADR-0030](0030-stem-cache-cleanup-and-job-failures.md) (the stem
cache), [ADR-0028](0028-ask-for-the-bpm-first.md) (the start screen's reducer) and
[ADR-0025](0025-undo-redo.md) (undo). Ticket: `.scratch/audit-2026-11-24/issues/10-retranscribe-from-the-app.md`.

## Context

ADR-0030 made a second run on a song cheap on the server: its stems are cached by the upload's SHA-256, so
another BPM costs only the transcription. The app didn't use it. It sent the whole song again, and the
editor had no way to ask for a new BPM: a user who found the tempo wrong after transcribing had to go back
to the start screen, drop the file again and lose the edits.

## Decision

1. **A job can name its upload instead of sending it:** `POST /api/jobs?…&upload=<sha256>` with an empty
   body.
   - The stem-cache entry now keeps the upload (`input.<ext>`, hard-linked from the job folder like the
     stems). Entries made before this get it on their next hit by an upload job.
   - The server computes the entry's key from the hash, the job's separation and the separators' versions
     now (ADR-0030's key, unchanged). On a complete entry it links the kept upload into a new job folder
     and queues the job as if the file had been sent. The job then runs exactly as an upload job would:
     the stems come from the cache, bar 1 (`bar_one=1`) reads the kept upload as the mix, and the
     provenance names the same upload. If the entry is evicted between the request and the run, the job
     separates the kept upload again.
   - **Refusals the client answers by sending the file:** 404 for no entry for that hash and separation
     (another song, another separation, `separation=none`, `--keep-stems 0`), 409 while the start-up
     probe hasn't read the separators' versions yet. 400 for a hash that isn't 64 lowercase hex digits,
     or for a body sent with a hash. The Host and Origin checks come first, as for every `/api/` request.
   - **Provenance:** `job.stems.by_hash: true` on a job that named its upload, and `job.separation`
     (`none`, `vocals`, `lead`) on every job, so the app can ask for the same separation again. Both are
     optional in the annotation contract; older annotations read as before.
2. **The start screen hashes a dropped song** (Web Crypto; `127.0.0.1` and `localhost` are secure
   contexts) and sends the job by hash when the hash is ready, the file when the server refuses it. A song
   the server already has starts without the upload. A new song costs one extra empty request.
3. **The editor offers "Re-transcribe at N BPM"** in the ♩ popover once the tempo (typed, ½×, 2×) or the
   job grid differs from the transcription's, for notes that came from a `sona serve` job. It asks for the
   original job's separation, engine and bar-1 option where the server still offers them, and the
   editor's grid when a job can use it (1/16 or 1/16T). It sends the hash; when the server refuses it, it
   sends the file if this session still holds the song, else it comes back to the start screen with
   "sona serve no longer has this song: drop it here again". Progress shows on the start screen as for any
   job.
4. **Edits are protected by undo, not a question.** The new transcription (notes, baseline, pitch line,
   bar-1 estimate, provenance and stem) replaces the old one as one undo step: Undo brings back the
   edited notes with the transcription and audio they belong to, Redo the new ones. The undo snapshot
   gains the transcription by identity, and each transcription keeps its decoded stem. A bar 1 the user
   chose stays. A job from the start screen still asks before replacing unsaved edits, as before.
5. **The start-screen rules stay in the pure reducer** (`web/lib/startFlow.ts`): a dropped song's hash
   (`hashed`), the editor's re-transcription (`retranscribe`), and the server's refusal (`hash-refused`)
   are events. A song's hash only ever lands on that song, a job sent by hash names its own song's hash,
   and a job only falls back to uploading the file it holds for that song.

## Evidence

- **Server** (`crates/sona-cli/tests/serve.rs`, stub separators):
  - hit: after one lead job, a job named by hash at 60 BPM on the 1/16T grid with bar 1 ran no separator,
    gave A4 and B4 from the same stem bytes, and its provenance reads `reused: true, by_hash: true`, the
    same upload SHA-256 and `separation: lead`;
  - miss: another song's hash, the same song with `vocals` or `none`, and any hash under
    `--keep-stems 0` are 404s that ran nothing, and the upload that follows runs;
  - malformed: 63 or 65 digits, upper case, a non-hex digit, empty, a path, and a body with a hash are
    400s; a foreign Origin or Host is 403.
  - The stem cache's unit test covers keeping an upload in an older entry and not in a missing one.
- **Reducer** (`web/lib/startFlow.test.ts`): fixed paths, and 3 000 random event orders that now include
  hashes, re-transcriptions and refusals. Each step checks that a hash belongs to the song shown, a job sent
  by hash names its own song's hash, an upload holds the file, and a new song starts with no hash.
- **Client** (`web/lib/serveClient.test.ts`, `web/lib/retranscribe.test.ts`, `web/lib/provenance.test.ts`):
  the hash job sends no body and every setting; 404 and 409 come back as a refusal, other errors throw;
  the browser's hash equals `sha256sum`'s; the offer appears only when the tempo or job grid changed, and
  keeps the original separation and engine where the server has them.
- **Model** (`verification/start-flow`, TLA+): new rules `HashIsForTheShownSong`,
  `HashJobsNameTheirSong` and `UploadsSendTheSong`, and the editor's re-transcription as an action.
  All 8 mutants are caught; the new ones are `stale-hash-wins`, `hash-carries-over`,
  `hash-of-another-song` (a re-transcription names the start screen's song's hash) and
  `upload-without-file`.
- **Browser** (`web/e2e/retranscribe.py`, against `sona serve` with stub separators on a fresh cache, the
  synthetic 30 s fixture as the song): the first Transcribe sent the hash, got 404 and sent the file;
  after a lyric edit and ½×, the popover offered "Re-transcribe at 60 BPM", which sent the hash alone and
  loaded 60 notes at 60 BPM in 43 s (28.6 s on a second run; pYIN on a machine at load ~40). One Undo
  gave back the edited notes byte for byte, BPM 120's transcription and its provenance, and the offer;
  Redo gave back the new notes. The driver runs locally only, like the other two serve drivers.
- **Mutants** (each applied and caught): server — any separation's entry served (miss test), a loose
  hash check (malformed test), no `by_hash` (hit test), the upload not kept (hit test), a body accepted
  with a hash (malformed test); reducer — a hash taken for any song, a hash carried to a new song, a
  re-transcription using the start screen's hash, a refusal always uploading; browser — an undo that
  doesn't restore the transcription (three checks of `retranscribe.py` fail).

## Consequences

- A tempo fix after transcribing is a click in the editor and costs the transcription only, and it can be
  undone. The song crosses the wire once per server cache entry.
- The stem cache's disk use grows by one upload per entry (a hard link while the job folder that sent it
  still exists).
- A re-transcription replaces every edit made since the last one, all at once; getting the old notes back
  is Undo, not a merge. Carrying edits (lyrics, say) onto the new notes is not attempted.
- The hash names a file; anyone who can reach the server's own pages and knows a song's SHA-256 can run a
  job on it. That is the same reach as uploading it, which the Host and Origin checks already bound.
- The serve-jobs TLA+ model is unchanged: a job named by hash is queued, run, cancelled and cleaned up
  exactly as an uploaded one.
