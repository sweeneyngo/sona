# web — sona review surface

The **interpretation** half of sona (ADR-0002): a static web piano-roll,
modeled as reference (not copy) on onlinesequencer.net. It loads the annotation
JSON from `sona-core`, plays stem-vs-synth, computes grid anchoring and
hard-snap 16th quantization live, lets the user correct flagged notes, and
exports MIDI.

Static assets only — no build tooling is assumed yet. Serve the directory with
any static file server, e.g.:

```sh
python3 -m http.server --directory web 8000
```

The UI itself is scaffolding; the real piano-roll, quantization, and export
land in tickets 08–10.
