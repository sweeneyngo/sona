# 10 — Re-transcribe from the app without re-uploading

Status: ready-for-agent
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
