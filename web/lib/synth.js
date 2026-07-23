// Pure synth helpers for the review surface. No DOM or Web Audio here — this is
// the schedule math the browser layer (main.js) hands to `OscillatorNode`s, kept
// pure so it can be unit-tested under `node --test`.

/** Equal-tempered MIDI pitch → frequency in Hz. 69 (A4) anchors to 440 Hz. */
export function midiToFreq(midi) {
  return 440 * Math.pow(2, (midi - 69) / 12);
}

/**
 * Turn annotation notes into playable voices: one `{ freq, start, duration,
 * gain }` per note, in seconds relative to the stem start. Every note plays at
 * the same reference gain — the point is to hear each note *equally* against
 * the stem to judge it, so confidence deliberately does not touch playback (it
 * drives the on-roll heatmap only). Non-positive-length notes are clamped to a
 * short blip so they're still audible.
 */
export function buildSchedule(notes, { peakGain = 0.25, minDuration = 0.02 } = {}) {
  return notes.map((note) => ({
    freq: midiToFreq(note.midi_pitch),
    start: note.onset_s,
    duration: Math.max(minDuration, note.offset_s - note.onset_s),
    gain: peakGain,
  }));
}
