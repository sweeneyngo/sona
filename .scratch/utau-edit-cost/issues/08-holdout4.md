# 08 — holdout4: older songs from Pifuyuu's OneDrive share

Status: resolved
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

## Answer

**2026-11-24 — Done: holdout4 has 24 songs; holdout4-tempo has 0.** Every song is vocaloid-original, grid 16, and has stems and a
sona-free offset. **None is verified yet** (see Alignment). Unread; `holdout-reads.jsonl` unchanged.

**Walk.** Channel rows 66–175: from ひかりの夏にひからびて (2026-08-09), the newest upload older than D-prostrate, back to アイ
(2026-03-24). That is 110 uploads. The included songs run from 2026-08-07 (brainrot) to 2026-03-24 (アイ). The walk stopped at the
**first 24 qualifying songs**, a count fixed before any alignment. That leaves at least 20 if the read-time check flags some (it
flagged 3 of 32 earlier songs).

**Grid rule made explicit:** at least 90% of the lead's note starts on one grid, 16 or 24. Every earlier corpus song has 91% or more
(kinsei is the lowest).

**Exclusions: 86**, all in `excluded` with reasons prefixed `holdout4 walk:`:
- **75** have no UST in `ust_share_url`. The share covers far less of this window than the ticket assumed: only 32 of the 110
  uploads have a zip. A whole 24-hour block of 矩歌やらお covers (2026-08-01/02) is almost entirely absent.
- **3** aren't UTAU covers (咖啡记录 vol.1–3).
- **5** fail the grid rule:
  - 零落: 87% of lead starts on 16ths.
  - 隣人は愛せない: mixed, 80% on 16ths and 85% on triplets.
  - Lost Forever: 64%.
  - あのこ行方不明: 80% on triplets.
  - この真っ白な夢の中でくらい忘れさせて。: 88%. It also has two tempos (160 and 180 BPM across files), but because it fails
    more than tempo it is not in holdout4-tempo.
- **2** fail the 1 s rule: Highway escape (+3 s) and asphylize (+2 s).
- **1** is an arrangement: D/N/A -REMIX-, ryo-shun's ボカコレ REMIX of Azari's D/N/A.

**Tempo.** None of the 32 zips in the window qualified for holdout4-tempo. Tempo changes are rare in Pifuyuu's USTs: of 53 zips
screened (rows 66–262), only one had more than one tempo, and none had a fractional BPM or a mid-file change. Frame metrics (02)
will need another source for tempo-change songs.

**Pipeline**, the same tools as earlier splits:
- `fetch-pifuyuu-ust.sh --record`: 24 sha256s.
- `sona-eval-ust-manifest --fill`: every song `ok`, all grid 16.
- `fetch-pifuyuu-audio.sh`: all 24 within ±5 s.
- `separate-stems.sh`: htdemucs 4.0.1, vocals and drums.
- `separate-lead.sh`: audio-separator 0.48.0 / UVR_MDXNET_KARA_2, the same as holdout2/3.

**Alignment: set without sona, not verified.** `sona-eval-ust-align` sets its offset without sona, but its status comes from
sona's onset detector: the onset-delta check of ticket 05. On this split that check would be a sona measurement, and onset timing
and phase are what this effort tunes (05, ADR-0027). It has two new options:
- `--no-verify` keeps the coarse and fine offset but never runs the detector. On dev it reproduces all 10 committed offsets exactly.
- `--split NAME` touches only that split.

`--split holdout4 --no-verify --write` gave offsets from −4.5 to −10.7 s. Drum peaks are 2.14–5.87, so none needed the coarse-only
fallback, and the drum-phase corrections are at most ±29 ms.

The status is `unverified (…)`, which every scorer (`ust-dev`, `cellstage`, `phase`, `ust-gate`) skips. The drum peak can't stand
in for the check: kieru-mama (3.77) and teni-suru-bug (2.98) were flagged, while anata-ni-okuru-ikkyoku (1.53) is aligned. **Before the
read:** pre-register how alignment is verified, for example running the onset-delta check as the first step of the read, and
dropping flagged songs by the 35 ms/30 ms rule before any score is computed.

**Code:**
- `ust-manifest` accepts tempo issues only in holdout4-tempo, fails a holdout4-tempo song whose tempo is fine, and now also treats
  a fractional BPM as a tempo failure.
- The manifest test pins 24/0 and requires every holdout4 song to be `unverified`. A mutant (one status flipped to `aligned`)
  fails it, and a mutant single-tempo song placed in holdout4-tempo fails the validator.
- The gate's per-split summary lists the new splits.
- `separate-stems.sh` bug: demucs read the loop's stdin, which is the song list. The first pass missed five holdout4 songs: three
  ids were eaten whole and two mangled (`left`→`eft`, `idl`→`dl`). Earlier runs were lucky. A second pass recovered them, and demucs now gets `</dev/null`.
  `fetch-pifuyuu-audio.sh` has the same pattern with yt-dlp, which didn't misbehave.
