# 09 — Optional moras in the lyric fit

Status: needs-triage
Type: research
Blocked by: 01, 08

## What

`sona_core::lyric_fit::mora_counts` charges `MORA_WITHOUT_NOTE` (3.0) for every mora no note takes.
Ticket 08 marks long-vowel tails as optional, because the UST gives them a note only 18% of the time on
dev. Let an optional mora go unsung at a low cost, learned on dev. Its log-odds give a starting value of
about −ln 0.82 ≈ 0.2.

Do the same for the particle reading (は sung わ). It changes the unit's text, not its count, so it only
matters if 01's placement metric scores the mora's text.

## Acceptance

Edit cost and 01's lyric placement on dev with and without optional moras. Use song-bootstrap
intervals and a selection rule written here before measuring. The change stays opt-in until a holdout
confirms it.

## Comments
