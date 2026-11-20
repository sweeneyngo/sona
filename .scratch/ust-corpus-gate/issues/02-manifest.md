# 02 — Corpus manifest (walk @pifuyuu, resolve 20, label strata)

Status: ready-for-agent

Blocked by: 01

Produces the committed manifest that drives every later ticket. This is where the
real judgment lives — the scripting is the easy part.

## Selection rule

**The 20 most recent uploads from [`@pifuyuu`](https://www.youtube.com/@pifuyuu).**
The YouTube channel is the index, *not* the OneDrive folder — every file there is
dated `September 22` (bulk-migrated, `migratedtospo=true`), so folder mtimes carry
no authoring order.

Recency is deliberate: it is independent of how sona performs, so the corpus
cannot be accused of being cherry-picked to flatter the engine — which matters
given [ADR-0012](../../../docs/adr/0012-note-offset-release-state.md)'s explicit
warning against tuning on a favourable staccato slice.

## Skip-and-backfill

Skip and take the next upload, **recording the reason in the manifest**, for:

- **mashups** — e.g. `『エンヴィーベイビー』×『KING』`: no single original master
- **arrangements** — e.g. `「夜明けと蛍」和風アレンジ`: UST is timed to a
  rearrangement, not the original
- **version-specific cuts** — e.g. `8番出口(Short ver)`, `1009 (20211009 Mix)`:
  the UST is timed to that exact cut; the standard release misaligns by an
  unknown amount
- **no resolvable original**, or uploads with no vocal to transcribe

Skips stay visible in the file. A reader should be able to see *why* a UST didn't
make the corpus.

## Stratum labelling (decoupled from selection)

Label each song `vocaloid-original` or `human-sung`. A UTAU cover's original may
be synthesized (the target domain) or human-sung (Vocadito's problem under a new
filename). **Reported separately, never pooled** — pooling lets a gain on one
stratum mask a regression on the other, the exact failure
[ADR-0011](../../../docs/adr/0011-evaluation-harness-vocadito-metrics.md) exists
to prevent. `human-sung` is **watched, never tuned against.**

## Manifest schema (committed)

```jsonc
{
  "songs": [{
    "id": "…",                      // stable slug
    "ust_file": "….ust",
    "ust_zip_url": "…",
    "youtube_url": "…",             // the ORIGINAL song, not the UTAU cover
    "cover_video_url": "…",         // provenance for how the UST was found
    "upload_date": "2026-11-16",    // channel order; the selection key
    "expected_title": "…",
    "stratum": "vocaloid-original", // | "human-sung"
    "bpm_from_ust": 150.0,          // exact, from [#SETTING] Tempo
    "alignment_offset_s": null,     // filled by hand in 05
    "stem": { "model": null, "version": null, "path": null }  // filled in 03
  }],
  "excluded": [{ "ust_file": "…", "reason": "mashup — no single original master" }]
}
```

## Done when

- Manifest committed with 20 included songs, strata labelled, exclusions recorded
  with reasons, and `bpm_from_ust` populated from the parser (01).
- Every `youtube_url` points at the **original song**, not the UTAU cover — the
  cover is a synthesized performance of the UST, not the material sona transcribes.

## Note

Do **not** fetch audio here. 03 builds the fetch script; the maintainer runs it.

## Comments

**2026-11-20 — Input from 01** ([audit](../../../crates/sona-eval/reports/ust-format-audit.md)).
The OneDrive folder is flat: 459 zips named by song title. Each zip holds a lead `main.ust`, harmony
`cho*.ust`, and `read me.txt`. Some leads are split (INNOCENT ADDICT has `main1` + `main2`, with 8
overlapping notes). The manifest must record which files are the lead, the merge rule for split
leads, and the `cho` files, which mark where the original has harmony (useful for masking it out
of scoring). The first 5 songs (the 5 most recent uploads) are already in the cache.
