//! **Raw MIDI emit** — the optional convenience export (spec §Pipeline 5). The
//! annotation JSON is the primary artifact and the authoritative corrected MIDI
//! comes from the UI (ADR-0004); this is a quick, *unquantized* `.mid` so the
//! core's output can be dropped into a DAW and auditioned without the web surface.
//!
//! "Raw / unquantized" means the note times stay in real seconds: each note's
//! `onset_s`/`offset_s` is placed on a metrical MIDI track through the carried
//! BPM (a single tempo meta), so the file plays back at the right wall-clock time
//! but nothing is snapped to a grid — snapping is the UI's job, not the core's.
//! A fixed velocity is used; the confidence scalar is *not* baked into velocity,
//! since a low-confidence note is a "check this", not a quiet note.

use crate::annotation::Annotation;
use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// MIDI ticks per quarter note for the emitted file. 480 is the common DAW
/// resolution — fine enough that rounding real seconds to ticks is inaudible.
pub const TICKS_PER_QUARTER: u16 = 480;

/// Fallback tempo when an annotation carries a non-positive / non-finite BPM, so
/// a raw MIDI can still be written. The UI is where a real tempo is set.
const FALLBACK_BPM: f64 = 120.0;

/// Note-on velocity for every emitted note. Constant on purpose — see the module
/// docs on why confidence is not mapped to velocity.
const VELOCITY: u8 = 96;

/// Largest value a tempo meta (microseconds per quarter, `u24`) can hold. Guards
/// the fallback path so an absurdly slow BPM can't overflow the field.
const MAX_TEMPO_US: f64 = 16_777_215.0;

/// Build a single-track, metrical [`Smf`] from an annotation's notes.
///
/// One tempo meta (from the annotation's BPM) sets playback speed; each note
/// becomes a note-on at its `onset_s` and a note-off at its `offset_s`, both
/// converted to ticks through that tempo. At a tick shared by an offset and an
/// onset the offset is emitted first, so adjacent notes don't momentarily stack.
pub fn to_smf(annotation: &Annotation) -> Smf<'static> {
    let bpm = if annotation.bpm.is_finite() && annotation.bpm > 0.0 {
        annotation.bpm
    } else {
        FALLBACK_BPM
    };
    let us_per_quarter = (60_000_000.0 / bpm).round().clamp(1.0, MAX_TEMPO_US) as u32;

    let tpq = TICKS_PER_QUARTER as f64;
    // Real seconds -> ticks via the tempo: ticks = s * (bpm/60) * tpq. This is
    // what keeps the export *unquantized* — the times are the perceived times,
    // not snapped to any grid.
    let sec_to_tick =
        |s: f64| -> u32 { (s.max(0.0) * bpm / 60.0 * tpq).round().min(u32::MAX as f64) as u32 };

    // Absolute-tick events: (tick, is_note_on, key). `is_note_on` as the second
    // sort key puts offs (0) before ons (1) at a shared tick.
    let mut events: Vec<(u32, u8, u8)> = Vec::with_capacity(annotation.notes.len() * 2);
    for note in &annotation.notes {
        let key = note.midi_pitch.min(127);
        let on = sec_to_tick(note.onset_s);
        // A note never ends before it starts, even if the annotation is degenerate.
        let off = sec_to_tick(note.offset_s).max(on);
        events.push((on, 1, key));
        events.push((off, 0, key));
    }
    events.sort_unstable();
    // sort_unstable orders by (tick, is_on, key) — offs before ons at a tick.

    let mut track: Vec<TrackEvent<'static>> = Vec::with_capacity(events.len() + 2);
    track.push(TrackEvent {
        delta: u28::from_int_lossy(0),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from_int_lossy(us_per_quarter))),
    });

    let mut prev_tick = 0u32;
    for (tick, is_on, key) in events {
        let delta = tick - prev_tick;
        prev_tick = tick;
        let message = if is_on == 1 {
            MidiMessage::NoteOn {
                key: u7::from_int_lossy(key),
                vel: u7::from_int_lossy(VELOCITY),
            }
        } else {
            MidiMessage::NoteOff {
                key: u7::from_int_lossy(key),
                vel: u7::from_int_lossy(0),
            }
        };
        track.push(TrackEvent {
            delta: u28::from_int_lossy(delta),
            kind: TrackEventKind::Midi {
                channel: u4::from_int_lossy(0),
                message,
            },
        });
    }

    track.push(TrackEvent {
        delta: u28::from_int_lossy(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    Smf {
        header: Header {
            format: Format::SingleTrack,
            timing: Timing::Metrical(u15::from_int_lossy(TICKS_PER_QUARTER)),
        },
        tracks: vec![track],
    }
}

/// Serialize an annotation's notes as a standard MIDI file to `writer`. Thin
/// convenience over [`to_smf`] so a caller (the CLI) needn't depend on `midly`.
pub fn write_midi<W: std::io::Write>(annotation: &Annotation, writer: W) -> std::io::Result<()> {
    to_smf(annotation).write_std(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::{Note, Producer, SubScores, SCHEMA_VERSION};

    fn note(onset_s: f64, offset_s: f64, midi_pitch: u8) -> Note {
        Note {
            onset_s,
            offset_s,
            midi_pitch,
            confidence: 0.9,
            sub_scores: SubScores {
                pitch_stability: 0.9,
                voicing: 0.9,
                onset_clarity: 0.9,
                offset_clarity: 0.9,
            },
        }
    }

    fn annotation(bpm: f64, notes: Vec<Note>) -> Annotation {
        Annotation {
            version: SCHEMA_VERSION,
            sample_rate: 44_100,
            bpm,
            duration_s: 1.0,
            producer: Producer::Pyin,
            notes,
            f0_contour: vec![],
            energy_envelope: vec![],
            provenance: None,
        }
    }

    /// Absolute-tick note events recovered by accumulating deltas.
    fn note_events(smf: &Smf) -> Vec<(u32, bool, u8)> {
        let mut out = Vec::new();
        let mut abs = 0u32;
        for ev in &smf.tracks[0] {
            abs += ev.delta.as_int();
            if let TrackEventKind::Midi { message, .. } = ev.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } => {
                        out.push((abs, vel.as_int() > 0, key.as_int()))
                    }
                    MidiMessage::NoteOff { key, .. } => out.push((abs, false, key.as_int())),
                    _ => {}
                }
            }
        }
        out
    }

    #[test]
    fn non_positive_bpm_falls_back_to_a_writable_tempo() {
        // A zero BPM would divide-by-zero / overflow the tempo field; the fallback
        // keeps the export valid rather than panicking.
        let smf = to_smf(&annotation(0.0, vec![note(0.0, 0.5, 69)]));
        let tempo = smf.tracks[0].iter().find_map(|ev| match ev.kind {
            TrackEventKind::Meta(MetaMessage::Tempo(us)) => Some(us.as_int()),
            _ => None,
        });
        assert_eq!(tempo, Some(500_000)); // 120 BPM fallback
    }

    #[test]
    fn degenerate_note_never_ends_before_it_starts() {
        // offset_s < onset_s must not produce a note-off before its note-on.
        let smf = to_smf(&annotation(120.0, vec![note(0.5, 0.2, 69)]));
        let events = note_events(&smf);
        assert_eq!(events.len(), 2);
        assert!(events[1].0 >= events[0].0, "off tick >= on tick");
    }

    #[test]
    fn out_of_range_pitch_is_clamped() {
        let smf = to_smf(&annotation(120.0, vec![note(0.0, 0.5, 200)]));
        let events = note_events(&smf);
        assert!(events.iter().all(|(_, _, key)| *key <= 127));
    }

    #[test]
    fn write_midi_produces_a_smf_header() {
        let mut buf = Vec::new();
        write_midi(&annotation(120.0, vec![note(0.0, 0.5, 69)]), &mut buf).expect("write");
        assert!(buf.starts_with(b"MThd"), "standard MIDI header chunk");
    }
}
