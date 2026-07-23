# 07 — CLI: stem + BPM → annotation JSON

Status: ready-for-agent
Blocked by: 06

Wire the core into `sona-cli`: `sona <stem.wav> --bpm N` → annotation JSON (ticket 02). Optionally emit a raw, unquantized MIDI via `midly` for CLI convenience (not the authoritative export). Set `producer: "pyin"`.

Done when: a real Vocaloid stem produces a complete, valid annotation JSON end-to-end.
