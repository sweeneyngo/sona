# Spec: sona as a public page at ifuxyl.dev/sona

Status: needs-triage (opened 2026-11-24)

## Question

Can sona be served publicly as a subpage of ifuxyl.dev, like the other projects there, and in what form?

## How ifuxyl.dev works today

- `sweeneyngo/sweeneyngo.github.io` is a Next.js static export on GitHub Pages with the custom domain
  ifuxyl.dev. Other repos appear as subpaths (e.g. `/suji`), which is how GitHub Pages serves a public
  project repo under the user site's custom domain. So `ifuxyl.dev/sona` would be the sona repo's own
  Pages site: **static files only**.
- `lemme` deploys a server to Fly.io, so a hosted backend has precedent.

## The problem

Transcription runs in `sona serve`: demucs and UVR separation (Python), GAME and the learned cell stage
(ONNX), pYIN (Rust). About 76 s for a 25 s clip on CPU. None of that runs on GitHub Pages.

## Options to evaluate

| | What runs where | Cost / effort | Main risk |
|---|---|---|---|
| A | Static editor on ifuxyl.dev/sona; transcription only through a **local** `sona serve` the user installs | Small: base path, CORS and Chrome's private-network rules for https → localhost | Still needs the install (roadmap 4.3) |
| B | Everything in the browser: WASM for the Rust core, onnxruntime-web for GAME/cells, a browser separator | Large | Separation speed and memory in a tab |
| C | Hosted backend (e.g. Fly.io) running the full pipeline | Medium to build, ongoing CPU cost | Cost, abuse, users uploading copyrighted songs to your server |
| D | Editor only (open/edit/export notes and UST), no transcription | Small | Little value alone |

## Licence and data blockers (apply to every option that ships models)

- **Learned cell stage:** trained on Pifuyuu's USTs, whose author forbids redistribution; the weights are
  deliberately local. Shipping them to browsers (B) or using them on a public server (C) needs a decision,
  probably permission from Pifuyuu, or a public build without them (pYIN grid or GAME only).
- **GAME:** CC BY-NC-SA. Non-commercial public use with attribution looks allowed; check ShareAlike for
  the cell stage, which consumes its output.
- **UVR_MDXNET_KARA_2 and htdemucs:** check their licences before redistribution or hosting.
- `sona serve` was built for localhost (Host check against DNS rebinding); it is not hardened for the
  internet.

## Tickets

`issues/01-hosting-decision.md`.
