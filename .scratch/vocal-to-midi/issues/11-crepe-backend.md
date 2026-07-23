# 11 — CREPE backend (ONNX via ort)

Status: ready-for-agent
Blocked by: 03

Second `PitchTracker` implementation (ADR-0001): CREPE exported to ONNX, run via `ort`. One-time Python export of the model; runtime stays pure Rust. Proves neural + "solely Rust" coexist, and gives a more artifact-robust option for messy stems. Set `producer: "crepe"`.

Independent of the segmentation/UI chain — only needs the trait (03).

Done when: CREPE runs behind the trait and produces an annotation JSON auditable in the same UI.
