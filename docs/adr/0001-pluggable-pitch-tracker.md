# Pluggable pitch-tracking backend

**Status: Accepted; the CREPE backend was never built.** The empty `crepe` cargo feature that stood in for it was removed in audit A07 (2026-11-24). A neural backend adds its own feature when it exists. `Producer::Crepe` stays in the annotation contract.

sona defines a `PitchTracker` trait with at least two implementations — pYIN (pure-Rust DSP) and CREPE (neural, exported to ONNX and run via the `ort` crate) — rather than committing to a single pitch engine. This is because a core project goal is to *understand and compare* the performance of different approaches on wet vocal stems, which is only possible if backends are swappable behind a common interface. It also dissolves the "must it be pure Rust?" concern: CREPE-via-`ort` runs a neural net inside the Rust binary, so "neural" and "runtime is solely Rust" coexist (the only Python is a one-time model export).

## Consequences

- Two backends to maintain, and the trait boundary must be designed before either is built.
- Enables a first-class A/B benchmark harness (same stem → pYIN vs CREPE → diff), which is the project's stated way of measuring "outperform."
