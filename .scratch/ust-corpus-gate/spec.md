# Spec: UST corpus — the target-domain eval gate

Status: ready-for-agent

## Goal

Build an **in-domain evaluation corpus** from Pifuyuu's publicly shared UTAU USTs
and the original songs they transcribe, so sona's accuracy is finally gated
against **staccato, grid-aligned Vocaloid** — the material CONTEXT.md declares as
the target — instead of against Vocadito's general expressive singing.

The corpus's **first job is to re-measure, not to fix.** The standing offset
bottleneck (COnPOff-F1 0.508 vs COnP-F1 0.646,
[`note-hmm-sweep.md`](../../crates/sona-eval/reports/note-hmm-sweep.md)) was
measured on *raw, unquantized* output against *expressive-performance*
annotations. sona's actual deliverable is **hard-snapped to a 16th grid**
([ADR-0003](../../docs/adr/0003-hard-snap-quantization-for-vocaloid.md)) and the
UST reference is **tick-quantized**. At 150 BPM a 16th is 100 ms, so any raw
offset error under ~50 ms snaps to the *same grid line as the truth* and becomes
zero error. A material fraction of the measured gap may never reach the user as
correction work at all. We find out before building a mechanism.

## Why USTs

A UST is the **ideal MIDI** — what a skilled human transcriber decided the note
data should be — not a performance annotation. So quantized-vs-quantized scoring
is apples-to-apples and is *more* product-relevant than Vocadito, which sona can
only ever be measured against in raw form.

A UST's `[#SETTING]` block also carries `Tempo`, giving **exact BPM per song** for
free. That is strictly better than an estimate and keeps the offset measurement
uncontaminated by tempo error.

## Design

**Index = the YouTube channel, not the OneDrive folder.** Every file in the
OneDrive UST folder is dated `September 22` (bulk-migrated, `migratedtospo=true`),
so folder mtimes carry no authoring order. Upload dates come from
[`@pifuyuu`](https://www.youtube.com/@pifuyuu) (most recent upload: Nov 16, 2026),
whose descriptions link both the UST and the original-song reference.

**Selection — 20 most recent uploads, neutral by construction.** Recency is
chosen over domain-matching deliberately: it is independent of how sona performs,
so the corpus cannot be accused of being cherry-picked to flatter the engine.
This matters because
[ADR-0012](../../docs/adr/0012-note-offset-release-state.md) explicitly warns
against tuning on a favourable staccato slice.

**Stratum labelling, decoupled from selection.** Each song is labelled
`vocaloid-original` or `human-sung` and the two are **reported separately, never
pooled.** A UTAU cover's original may be synthesized (the target) or human-sung
(Vocadito's problem under a new filename); pooling would let a gain on one
stratum mask a regression on the other — the exact failure
[ADR-0011](../../docs/adr/0011-evaluation-harness-vocadito-metrics.md) exists to
prevent. The `human-sung` stratum is **watched, never tuned against.**

**Exclusions are skip-and-backfill, with recorded reasons.** Mashups
(`『エンヴィーベイビー』×『KING』`), arrangements (`和風アレンジ`), and
version-specific cuts (`Short ver`, `… Mix`) have no single original master to
align against. If one lands in the 20 most recent, skip it, take the next upload,
and record the rejection in the manifest so skips stay auditable rather than
silent.

**Alignment — manual phase, recorded in the manifest.** BPM comes exactly from
the UST, so alignment reduces to finding **phase within one beat period**. It is
set by hand and stored as data (reviewable, version-controlled), *not* computed.
Auto-aligning against sona's own onset detector is rejected as **circular** — it
would absorb detector bias into the alignment and let the score flatter itself.
Misalignment is silent and masquerades as the very thing being measured (a
constant 80 ms shift reads as "sona has an offset bias"), so the audit report
prints a per-song onset-delta distribution: deltas scattered around zero means
aligned; a constant shift means misaligned; *growing* drift means the single-BPM
assumption is broken for that song.

**Quantizer lives in `sona-eval`, written to be promotable.** Hard-snap exists
only in [`web/lib/grid.ts`](../../web/lib/grid.ts) (TypeScript, UI-side);
`sona-core` deliberately emits raw perception
([ADR-0004](../../docs/adr/0004-core-emits-raw-perception-ui-interprets.md)) and
CONTEXT.md lists a headless snapped-MIDI path as out of v1 scope. Scoring against
a quantized UST needs quantization in Rust. It goes in **`sona-eval`**, keeping
the core's contract intact and confining the duplication to the harness, where
drift is detectable by testing both implementations against shared fixtures. It
is written as a pure function over `(notes, spacing, anchor, confidence)` with no
eval-only dependencies, so promoting it into `sona-core` (or exposing it to the
UI via WASM) later is a file move plus a re-export — **the duplication is
explicitly temporary, not quietly permanent.**

**Audio and stems.** Source audio is fetched by a manifest-driven `yt-dlp`
wrapper the **maintainer runs** — which also makes the corpus reproducible by
anyone holding the manifest, exactly as
[`fetch-vocadito.sh`](../../crates/sona-eval/scripts/fetch-vocadito.sh) does for
Vocadito. Stem separation is scripted with **demucs**, installed as corpus-prep
tooling *outside* the Rust workspace, so sona's "no ML runtime, no C dep"
identity is untouched — demucs never enters the shipped product. The manifest
records the separation model and version per stem: different models leave
different bleed characteristics, and bleed is precisely what sona's voicing
sub-score guards against, so unrecorded model changes would silently invalidate
comparisons.

Corpus data is **git-ignored and locally sourced**, mirroring Vocadito's
licensing-gated pattern. Only the manifest, the parser, and the reports are
committed.

## Non-goals

- **No BPM estimation.** Deferred to a post-offset ticket. Corpus runs use the
  UST's exact `Tempo`; an estimate would fold tempo error into every offset
  number and make a regression ambiguous between "the offset model got worse" and
  "the tempo estimate drifted." The corpus *banks the gate* for it — golden BPM
  labels come free — with the metric already defined (exact-match plus a
  factor-of-2-tolerant variant, since half/double-tempo confusion is the classic
  failure mode).
- **No offset mechanism.** Measure first, then decide. If the in-domain number
  shows a real gap, the next move is re-sweeping the existing ADR-0012 Release
  path in-domain — and that ADR's overfitting warning must be argued explicitly
  in a new ADR, on the grounds that staccato Vocaloid is the *declared target
  domain* rather than a cherry-picked slice. An ML offset model stays off the
  table at this corpus size: 20 songs (~4–6k notes) is an evaluation set, not a
  training set.
- **No pitch-bend MIDI export — killed, not deferred.** For the target domain
  most intra-note f0 wobble is pYIN tracking noise, not expression (CONTEXT.md:
  synthesized, "f0 unusually clean and stable"), so exporting it as bend is the
  pitch-axis version of the mistake ADR-0003 identified for timing. The consumer
  that wants bend (UST/VSQX re-synthesis) is explicitly out of scope; COnP/COnPOff
  cannot read bend events so it is ungateable, which ADR-0011 rules out; and it
  adds a review surface rather than reducing correction work. Revisit only if the
  product target moves to expressive material.
- **No repeated-passage self-consistency check — deferred with a validation
  path.** Good idea, real domain fit (sequenced choruses repeat near-exactly),
  and a plausible *product* feature. But its value is inversely proportional to
  ground truth, which this corpus is about to make abundant in-domain; and it has
  an unresolved soundness problem — sona is deterministic, so near-identical
  passage audio yields near-identical output and the check finds nothing, while
  genuinely differing audio makes divergence ambiguous between a bug and a real
  audio difference. Revisit after the corpus lands, when its flags can be
  *validated* against UST-measured errors.
- **No source separation in the product.** The corpus's input contract stays
  identical to sona's: a stem. Separation is corpus-prep only.
- **No CREPE / basic-pitch backends, no lyric-to-note mapping.** Ship-quality
  work sequenced ahead of comparative standing and scope expansion.

## Done when

- `fetch-pifuyuu-ust.sh` populates a git-ignored UST cache; the 20-song manifest
  is committed with stratum labels, alignment offsets, exclusions-with-reasons,
  and separation-model provenance.
- UST parser + UST→MIDI/annotation converter, unit-tested against committed
  fixture USTs (no audio needed).
- Hard-snap ported to `sona-eval` as a promotable pure function, tested to agree
  with `grid.ts` on shared fixtures.
- **Format audit report** lands first and gates the rest: rest-terminated note
  fraction (how much genuine offset signal exists — a note followed by a rest
  carries a true musical end; a legato note's "offset" is just the next onset),
  grid-alignment tightness, and per-song onset-delta distributions.
- **In-domain report**: COnP/COnPOff on quantized output vs quantized UST, split
  by stratum, alongside the Vocadito baseline. This number decides whether the
  offset item stays open or closes.

## Sequencing

- **01** — UST format audit: parser + fixture USTs + audit report. No audio. Gates
  everything downstream.
- **02** — Manifest: walk `@pifuyuu`, resolve the 20 most recent to USTs, label
  strata, record exclusions. Produces the fetch list.
- **03** — `fetch-pifuyuu-ust.sh` + `fetch-pifuyuu-audio.sh` (yt-dlp, maintainer-run)
  + demucs separation script with recorded provenance.
- **04** — UST→annotation converter + hard-snap port to `sona-eval`, agreeing with
  `grid.ts` on shared fixtures.
- **05** — Manual phase alignment per song, stored in the manifest; verified by the
  onset-delta distributions from 01.
- **06** — The in-domain gate: quantized COnP/COnPOff vs UST, per stratum, vs the
  Vocadito baseline. Decide whether the offset bottleneck is real in-domain.
