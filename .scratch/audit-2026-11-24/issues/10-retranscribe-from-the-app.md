# 10 — Re-transcribe from the app without re-uploading

Status: resolved
Type: task
Blocked by: —

## Evidence

A04 (ADR-0030) made a re-run cheap on the server: stems are cached by upload hash, so a new BPM costs only
the transcription (25 s clip: 223 s first run, 17–23 s re-runs). The app still re-uploads the song and
offers no "re-transcribe at this BPM" after a tempo change in the editor, so users don't get the benefit
without starting over.

## Proposal

- Server: accept a job by upload hash when the cache has it (`409`/`404` when it doesn't, so the client
  falls back to uploading).
- App: after a BPM (or ½×/2×) change in the editor's tempo popover, offer **Re-transcribe at N BPM**; on
  the start screen, a song already cached starts without re-uploading. Rules in `startFlow.ts` (pure
  reducer), updated tests and TLA+ `start-flow` model if its states change.

## How to tell

serve tests: a hash-only job on a cached song runs; on an uncached one it is refused and the client
uploads. A browser driver changes the BPM in the editor and re-transcribes in seconds.

## Comments

## Answer

Done on branch `audit-a10-retranscribe`:
[ADR-0032](../../../docs/adr/0032-retranscribe-by-upload-hash.md) (the number may change at merge).

- **Server.** `POST /api/jobs?…&upload=<sha256>` with an empty body runs a job on an upload the server
  already has. The stem-cache entry now keeps the upload beside its stems (a hard link; older entries get
  it on their next hit). The entry is looked up under ADR-0030's key (hash, separation, separator
  versions); on a hit the kept upload is linked into a new job folder and the job runs as an upload job
  would, bar 1 included. 404 when there is no entry for that hash and separation (another song, another
  separation, `separation=none`, `--keep-stems 0`), 409 while the separators' versions are still being
  read; 400 for anything but 64 lowercase hex digits, or a body with a hash. Host and Origin checks first,
  as before. Provenance: `job.stems.by_hash: true`, and `job.separation` on every job.
- **App.** The start screen hashes a dropped song (Web Crypto) and sends the job by hash, the file when
  the server refuses; a song the server has starts without the upload. After a BPM (typed, ½×, 2×) or job
  grid change, the ♩ popover offers **Re-transcribe at N BPM** for notes from a `sona serve` job; it sends
  the hash, falls back to the file if the session still holds the song, and otherwise asks for the song
  again on the start screen. Rules in the pure reducer (`hashed`, `retranscribe`, `hash-refused`);
  `web/lib/retranscribe.ts` decides the offer and the job.
- **Edits.** No question asked: the re-transcription replaces notes, baseline, pitch line, provenance and
  stem as one undo step, and Undo brings all of it back (Redo the new one). A bar 1 the user chose stays.
- **Tests.** serve: hash hit (no separator, `by_hash`, same stem, bar 1 from the kept upload), hash miss
  then upload, other separation and `none`, `--keep-stems 0`, malformed hashes, hash plus body, foreign
  Origin/Host. vitest: the reducer (fixed paths + 3 000 random orders with hashes, re-transcriptions and
  refusals), the client (no body, 404/409 refused, other errors thrown, the hash equals `sha256sum`'s),
  the offer, provenance. TLA+ `start-flow`: three new rules, eight mutants caught (new:
  `stale-hash-wins`, `hash-carries-over`, `hash-of-another-song`, `upload-without-file`). Browser:
  `web/e2e/retranscribe.py` (serve driver, skipped in CI's `SONA_E2E_SKIP_SERVE=1` like the other two),
  run against `sona serve` with stub separators: the first Transcribe got 404 for its hash and sent the
  file; ½× then **Re-transcribe at 60 BPM** sent the hash alone and loaded in 43 s (28.6 s on a warm run;
  that is pYIN on a 30 s stem with the machine at load ~40, not separation); one Undo gave back the
  lyric-edited notes and their transcription; Redo the new ones.
- **Mutants caught:** server (any separation's entry served, a loose hash check, no `by_hash`, the upload
  not kept, a body accepted with a hash); reducer (a hash for any song, a hash carried to a new song, a
  re-transcription with the start screen's hash, a refusal always uploading); browser (an undo that
  doesn't restore the transcription).
