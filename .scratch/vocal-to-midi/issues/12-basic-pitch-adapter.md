# 12 — basic-pitch adapter (comparison harness)

Status: ready-for-agent
Blocked by: 07

Run spotify/basic-pitch on a stem and convert its output into sona's annotation JSON — map its note events to `notes[]` and its note-level probabilities to the `confidence` scalar. Set `producer: "basic-pitch"`. This makes basic-pitch just another producer, auditable by ear in the same review surface (v1 = single-load).

Done when: a basic-pitch run on a Vocaloid stem loads in the UI beside a pYIN run for by-ear A/B.
