# 01 — Project scaffold

Status: ready-for-agent
Blocked by: —

Set up the Rust workspace: a headless `sona-core` library crate + a `sona-cli` binary. Add the web-UI directory as a sibling (static assets, no build tooling assumptions yet). `git init` the repo. Wire chosen crates as deps: `midly`, a pitch crate (`pitch-detection` or `aubio-rs`), and `ort` (feature-gated, since CREPE is a later ticket).

Done when: `cargo build` succeeds, CLI runs and prints usage, repo is a git repo.
