# 01 — Eval harness scaffold: sona-eval crate + data

Status: done
Phase: A (harness)

Stand up the measurement rig. No metrics logic yet — just the crate, the data plumbing, and the fixtures.

- New workspace crate **`sona-eval`** (binary/`xtask`-style), so dataset + metric deps never touch `sona-core`.
- **Vocadito** (Zenodo 5578807): a manifest + download script that fetches and caches the corpus into a **git-ignored** local dir. Verify checksums; fail loudly if absent rather than silently skipping.
- **3–5 golden fixtures** committed under `sona-eval/fixtures/`: short WAV + expected annotation JSON, covering clean legato, staccato/Vocaloid, vibrato, separation bleed. Keep them a few seconds each.
- A loader that reads Vocadito's annotation format into a common reference type (notes: onset/offset/pitch; f0: time/hz/voiced).

Done when: `sona-eval` builds; the download script populates the cache; fixtures load; the corpus run is gated (feature flag / ignored-by-default) so `cargo test` stays fast and offline.

## Comments

**Done** (commit `50423df`). New `crates/sona-eval` crate: common `Reference` type, Vocadito CSV loader (f0 `time,hz`; notes `onset,hz,dur`, A1/A2, Hz→MIDI) with filename-pattern corpus discovery that fails loudly when missing *or* empty, `fetch-vocadito.sh` (md5-verified download into git-ignored `data/`), four golden fixtures (legato/staccato/vibrato/bleed) generated from one shared synth spec via the `sona-eval-gen-fixtures` bin, and a `sona-eval-report` bin (fixtures + opt-in `--corpus`). Corpus integration test is `#[ignore]`d; 15 crate tests run offline in `cargo test` (91 workspace tests green). Metrics deferred to ticket 02 as specified. Two-axis review passed (Spec: faithful, no scope creep; Standards: fixed a `Wav`-error constructor inconsistency, centralized Hz→MIDI in `RefNote::from_hz`, and made an empty corpus dir fail loudly).
