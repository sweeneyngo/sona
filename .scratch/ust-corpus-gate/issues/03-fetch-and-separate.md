# 03 — Fetch scripts + demucs separation with recorded provenance

Status: ready-for-human

Blocked by: 02

`ready-for-human` because the audio fetch is **run by the maintainer**, not by an
agent. The scripting is agent work; the execution is not.

## Division of labour, and why

- **`fetch-pifuyuu-ust.sh`** — agent-written, agent-runnable. USTs are freely
  shared fan work, 2–12 KB, published for reuse. Populates a git-ignored cache,
  mirroring [`fetch-vocadito.sh`](../../../crates/sona-eval/scripts/fetch-vocadito.sh).
- **`fetch-pifuyuu-audio.sh`** — agent-written, **maintainer-run**. A
  manifest-driven `yt-dlp` wrapper (`yt-dlp` and `ffmpeg` are already installed).
  An agent will not bulk-retrieve ~20 commercial masters on its own initiative.
  Being manifest-driven is also *better engineering* than hand-fetching: anyone
  holding the manifest can reproduce the corpus, exactly as Vocadito's script
  allows. Automation is preserved; only the hand on the trigger changes.
- **`separate-stems.sh`** — agent-written, agent-runnable. Local signal
  processing on audio already on disk; nothing leaves the machine.

## Separation

- **demucs**, installed as corpus-prep tooling **outside the Rust workspace**
  (it pulls torch, ~2 GB). It never enters the shipped product, so sona's "no ML
  runtime, no C dependency" identity ([ADR-0005](../../../docs/adr/0005-pyin-hand-rolled-median-filtered.md))
  is untouched. Nothing is installed until this ticket runs.
- Output: **mono WAV** vocal stems — sona's CLI input contract.
- **Record model + version per stem in the manifest.** Different separation models
  leave different bleed characteristics, and bleed is precisely what sona's
  voicing sub-score guards against (CONTEXT.md: "the dominant noise source is
  UVR separation bleed/artifacts"). An unrecorded model change would silently
  invalidate every cross-run comparison.

## Scope note

Source separation stays **corpus-prep only** — the spec puts it out of scope for
the product, and CONTEXT.md defines sona's input as a stem already extracted.
Keeping the corpus's input contract identical to sona's is deliberate.

## Done when

- All three scripts exist, are idempotent, and verify what they downloaded.
- Corpus data lands git-ignored; only scripts and manifest are committed.
- Manifest `stem.model` / `stem.version` / `stem.path` populated for every song.
