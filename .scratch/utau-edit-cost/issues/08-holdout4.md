# 08 — holdout4: older songs from Pifuyuu's OneDrive share

Status: ready-for-agent
Type: task
Blocked by: —

## What

Every split is spent, so nothing in 05–07 can become a default without a fresh set. Build split
`holdout4` with **at least 20 aligned songs** (20, not 10, so a +2 change is detectable).

## Source (checked 2026-11-24)

- **Not BowlRoll.** Its robots.txt disallows `/file/*/download-execution/` and its terms (Art. 8-1-2) forbid
  mechanical collection; the user won't download by hand either.
- **Pifuyuu's public OneDrive share** (`ust_share_url` in the manifest) lists **460 UST zips; 42 are
  used**. The used splits cover uploads 2026-08-14 → 2026-11-16, and the share's gaps were among recent
  uploads. Older uploads are largely in the share.

## Selection rule (same as holdout3, continued)

Walk the @pifuyuu channel's uploads in order, starting with the newest upload **older than holdout3's
oldest (2026-08-14)**, going back. Apply the manifest's skip-and-backfill rules with a recorded reason:
vocaloid-original only; a resolvable original within 1 s of the cover; mashups, arrangements and
version-specific cuts skipped; UST zip present in the OneDrive share; grid 16 or 24.

**Tempo:** keep "one integer tempo" for `holdout4` itself, so its numbers compare with earlier splits.
Do **not** discard songs that fail only on tempo (tempo changes or a fractional BPM): list them in a
separate split `holdout4-tempo`, also never read, for the frame metrics (02) later.

## How

Use the existing tools: `fetch-pifuyuu-ust.sh` (extend the manifest, `--record` sha256s),
`fetch-pifuyuu-audio.sh`, `separate-stems.sh`, `separate-lead.sh`, `sona-eval-ust-align` for
`alignment_offset_s`. Read `.scratch/ust-corpus-gate/issues/02-manifest.md` for the judgment rules.

## Hard rules

- **Never score sona on these songs.** No transcription, no metric, no `--split holdout4`. Building the
  split is not a read; the ledger must be unchanged.
- Never commit USTs, audio, stems or lyrics; `scripts/ci/check-no-corpus-data.sh` must pass.
- Credit and no-redistribution terms: see `ust_terms` in the manifest.

## Comments
