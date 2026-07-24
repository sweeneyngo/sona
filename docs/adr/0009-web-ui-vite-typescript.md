# Vite + TypeScript build for the web review surface

The web review surface (ADR-0002) started as dependency-free, build-free static
assets — vanilla ES modules served by any file server, with pure logic unit-
tested under `node --test` and the core↔UI JSON contract hand-mirrored in a
`annotation.d.ts` comment file. That was the right default for tickets 08–10:
it kept effort on the transcription problem, not toolchain plumbing.

We now adopt **Vite + TypeScript + Vitest** for the web dir, superseding the
"no build tooling" stance recorded in `web/README.md` and tickets 08–10.

## Why

- **The contract is the whole point of the split (ADR-0004), and it was
  unenforced.** `annotation.d.ts` only *documented* the shape; nothing checked
  that `main.js` read it correctly or that the two stayed in sync with the Rust
  Serde types. Promoting it to real TypeScript (`lib/annotation.ts`) that flows
  through the grid/midi/view/synth modules makes the seam type-checked instead
  of hand-audited — the one place a drifting field would silently corrupt an
  export.
- **The UI grew past "a few scripts."** Tickets 09–10 added live quantization,
  an escape hatch, editing, and a hand-rolled MIDI encoder — enough surface that
  types earn their keep (the encoder's byte math and the seconds↔ticks↔pixels
  conversions are exactly where a wrong-typed value hides).
- **Cost is bounded.** The code was already ES modules with a clean pure/DOM
  split, so the migration is renames + type annotations, not a rewrite. Vitest
  keeps the existing `node:assert` test bodies; only the test runner's `test`
  import changes.

## Consequences

- The web dir now carries `node_modules`, a `package.json`, and a build step;
  it is no longer "serve the folder with `python3 -m http.server`". Dev is
  `npm run dev` (Vite, with HMR); a static deploy is `npm run build` → `dist/`.
- `node_modules` and `dist/` are git-ignored; the built output is still plain
  static files, so the deploy target is unchanged.
- Tests run under `npm test` (Vitest) instead of `node --test`.
- `base: "./"` in the Vite config keeps asset paths relative, so the build still
  works served from any path.
- The Rust core is untouched. This decision is scoped to `web/`.
