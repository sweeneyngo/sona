# 08 — holdout4: fresh songs from BowlRoll

Status: needs-info
Type: task
Blocked by: —

## What

Every split is spent. Nothing in 05–07 can become a default without a fresh set. Build a fetcher for
Pifuyuu's USTs posted only on BowlRoll, add the next ≥ 20 aligned songs as split `holdout4` (20, not 10,
so a +2 change is detectable), with the manifest's skip-and-backfill rules.

## BowlRoll: no automated fetching (checked 2026-11-24)

- `https://bowlroll.net/robots.txt` disallows `/file/*/download-execution/`, the download step itself.
- Terms (`https://bowlroll.net/about/rules`, revised 2019-06-28), Art. 8-1-2 prohibits
  「他の利用者を含む第三者または管理人の情報やデータ等を、管理人の許可なく機械的な方法で収集する行為」:
  collecting other users' data by mechanical means without the operator's permission.
- No API. Whether downloads need a login is not documented (the help page says there is no help).

So a scraper/fetcher is out. Ways forward, in order of preference:
1. **The user downloads the files by hand** in a browser (ordinary use of the site), into a local folder.
   An agent then writes an **ingest** tool that reads that folder, unpacks, and adds manifest entries
   with the usual skip-and-backfill rules. Nothing about BowlRoll is automated.
2. Ask BowlRoll's operator for permission (`https://bowlroll.net/about/inquiry`). Slow, uncertain.
3. Ask Pifuyuu for a fuller share. Also slow.

## Needs from the user

- Willing to download about 25 files by hand (option 1)?
- Should holdout4 keep the "one integer tempo" rule, or include songs with tempo changes so the frame
  metrics see them?

## Comments
