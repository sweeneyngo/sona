# 06 — The lyric fallback in the app

Status: ready-for-agent
Type: task
Blocked by: 03

## What

Run 05's recogniser on the vocal stem when the user chose the fallback. Show its hiragana in the lyric box
for review before the notes are fitted. Document the model download in the README (it isn't
distributed).

## Comments

- 2026-11-24 (triage, after 05): 05 is resolved, so only 03 (the lyric box) blocks this. What to build,
  from `crates/sona-eval/reports/lyric-transcription.md`:
  - **Recogniser:** Whisper large-v3-turbo through faster-whisper, `device="cpu"`, `compute_type="int8"`,
    `language="ja"`, `beam_size=5`, `vad_filter=True`, `vad_parameters={"threshold": 0.2}` (not the
    default 0.5, which drops 10% of sung moras), `condition_on_previous_text=False`,
    `word_timestamps=True`, on the lead stem.
  - **Reading:** fugashi + unidic-lite on each whole segment (a word alone may be one kanji), the word's
    written reading, a particle's pronunciation (は → わ). Port the mora rules in
    `crates/sona-eval/scripts/lyric-transcription/moras.py`. Latin words have no reading: show them for the
    user to respell rather than dropping them silently (the spec rejects silent conversion).
  - **Lines:** one Whisper segment per line in the lyric box. Its word timestamps are good (91% within
    0.5 s of the UST note), so keep them for later line alignment.
  - **Review:** highlight words with probability below about 0.7 (they hold 57% of the wrong moras in 31%
    of the text). Expect about 37% of moras to need a fix; say so in the UI.
  - **Setup:** a script like `scripts/setup-separators.sh` making `~/.local/share/sona/lyric-venv`
    (Python 3.10–3.12; `faster-whisper==1.2.1`, `fugashi==1.5.2`, `unidic-lite==1.0.8`; about 0.6 GB)
    and downloading the model once (1.6 GB, MIT). The README documents it next to GAME. ADR-0005 holds:
    a venv the server calls, never a crate dependency.
  - **Speed:** about 9 minutes per 3-minute song on the loaded dev machine (2.9× real time), so it is a
    queued job with progress, like separation.
