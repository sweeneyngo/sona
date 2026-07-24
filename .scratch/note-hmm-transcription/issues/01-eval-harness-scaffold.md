# 01 — Eval harness scaffold: sona-eval crate + data

Status: ready
Phase: A (harness)

Stand up the measurement rig. No metrics logic yet — just the crate, the data plumbing, and the fixtures.

- New workspace crate **`sona-eval`** (binary/`xtask`-style), so dataset + metric deps never touch `sona-core`.
- **Vocadito** (Zenodo 5578807): a manifest + download script that fetches and caches the corpus into a **git-ignored** local dir. Verify checksums; fail loudly if absent rather than silently skipping.
- **3–5 golden fixtures** committed under `sona-eval/fixtures/`: short WAV + expected annotation JSON, covering clean legato, staccato/Vocaloid, vibrato, separation bleed. Keep them a few seconds each.
- A loader that reads Vocadito's annotation format into a common reference type (notes: onset/offset/pitch; f0: time/hz/voiced).

Done when: `sona-eval` builds; the download script populates the cache; fixtures load; the corpus run is gated (feature flag / ignored-by-default) so `cargo test` stays fast and offline.
