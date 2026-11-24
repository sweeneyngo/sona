# Spec: score sona by how hard its mistakes are to fix in OpenUtau

Status: in progress (opened 2026-11-24)

## Goal

A user drops a song into sona and takes the notes into UTAU/OpenUtau. The result should be as close as
possible to a hand-made UST. When it is wrong, it should be wrong in ways that are cheap to fix **in
OpenUtau** (not in sona's editor).

Cell accuracy (ADR-0017, holdout3 78.8%) treats every wrong cell the same. It cannot tell a note one cell
long (one drag) from a song sitting two beats off its bar lines (the hardest fix there is). This effort
adds a metric that weights errors by editing difficulty, measures sona with it, and redirects work to the
expensive errors.

## Editing difficulty in OpenUtau (from the user, 2026-11-24)

| Tier | Error | Weight |
|---|---|---|
| Easy | Wrong pitch or octave (OpenUtau transposes, in batches too); wrong length; one note a cell early or late; an extra note to delete | 1 |
| Harder | A missing note that has to be drawn | 2 |
| Tedious | One held note chopped into several; two notes joined into one | 4 |
| Hard | A passage in the wrong measures (bar offset); a wrong BPM (multiple or drift) | per-song pass/fail, reported separately ("frame") |

Users usually know their song's exact BPM. Finding beat 1 of the measure therefore matters more than
estimating the BPM.

## Metrics

Headline:
- **Weighted edit cost per minute of song** (tiers above), from `sona_eval::correction` extended with
  chopped/joined detection. Lower is better.
- **Frame pass rate:** songs whose bar lines and BPM match the UST's.

Reported beside it:
- **Clean measures:** share of measures that need no edit at all. The most user-legible number.
- **Error breakdown:** count and weighted cost per error class, per song.
- **Cell accuracy**, the old headline, so the two can be compared.
- **Bar offset** in beats (given the true BPM), **tempo multiple** and **BPM drift** in beats over the
  song (with sona's own estimate).
- **Confidence calibration:** the error rate of notes by sona's confidence bucket. It decides whether
  highlighting doubtful notes would be useful.

## Rules (from `docs/agents/rules.md`)

- Tune on `dev`. The spent splits (holdout, holdout2, holdout3) may be **re-read for diagnosis only**:
  `--split NAME --reread --reason "…"`, logged in the ledger. They confirm nothing.
- A new default needs **holdout4** (issue 08) and a decision rule committed before it is read.
- New behaviour is opt-in until a holdout confirms it. The default path stays byte-identical.
- Song-bootstrap intervals on every comparison.
- Never commit Pifuyuu's USTs or any corpus audio.

## Known bias in the corpus

The manifest kept only songs with **one integer tempo** (holdout3 selection rule) and grid 16 or 24. The
frame metrics will therefore understate tempo changes and fractional BPMs. Real users' songs may have
both.

## Work

See `map.md` (decisions and open questions) and `issues/` (one ticket per file, `Status:` uses the labels
in `docs/agents/triage-labels.md`; `Blocked by:` lists tickets that must resolve first).
