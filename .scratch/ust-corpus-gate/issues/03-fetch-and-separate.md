# 03 — Fetch scripts + demucs separation with recorded provenance

Status: done

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

## Comments

**2026-11-20 — Done.** All three scripts exist, are idempotent, and verify their output. All three were run;
the maintainer approved the audio fetch in-session.
- `fetch-pifuyuu-ust.sh`: lists the public OneDrive folder through the anonymous token its web viewer uses.
  Downloads are checked against the manifest's sha256 (`--record` fills it in) and extracted with zip-slip
  protection and Shift-JIS filename decoding. It also checks that every lead and other-voice file listed in
  the manifest exists.
- `fetch-pifuyuu-audio.sh`: yt-dlp, run from the manifest. Each download must match `original_duration_s`
  within ±5 s, which rejects the wrong video or a different cut. All 20 fetched.
  - YouTube returned 403s until yt-dlp was updated (2026.07.04 → 2026.10.26).
  - One song needed a retry.
- `separate-stems.sh`: demucs `htdemucs`, 4.0.1, torch 2.5.1, CPU, in a venv at
  `~/.local/share/sona/demucs-venv`, outside the workspace. The venv needs Python 3.10–3.12, and the script
  checks that by running the interpreter, because pyenv shims exist on PATH even when inactive.
  - Writes mono 44.1 kHz `vocals.wav`, plus `drums.wav` as an alignment aid (05).
  - Records `stem.model` / `version` / `path` for all 20 songs.
  - Took 63 min on 16 CPU cores, about 3 min per song, roughly 75% of the cores (compute-bound).
