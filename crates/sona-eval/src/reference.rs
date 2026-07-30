//! The **common reference type** — ground truth for one stem, in the shape the
//! metrics compare a pipeline's output against. Both Vocadito
//! ([`crate::vocadito`]) and the golden fixtures ([`crate::fixtures`]) load into
//! this, so the metrics never see a source-specific format.
//!
//! It mirrors the two annotation kinds every relevant dataset ships: **notes**
//! (onset/offset/pitch) and a **frame-level f0** track. Pitch is stored as
//! integer MIDI to match the annotation contract's [`Note::midi_pitch`]; f0 is
//! kept in Hz so the pitch metrics (RPA/RCA) can work at cent resolution.
//!
//! [`Note::midi_pitch`]: sona_core::Note

use serde::{Deserialize, Serialize};

/// Ground truth for one stem: reference notes and a reference f0 contour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Reference note events, in onset order.
    pub notes: Vec<RefNote>,
    /// Reference frame-level f0 contour.
    pub f0: Vec<RefF0>,
}

/// A reference note: onset/offset in seconds, pitch as integer MIDI. Matches the
/// annotation contract so a metric can compare like with like.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RefNote {
    /// Onset, in seconds from the start of the stem.
    pub onset_s: f64,
    /// Offset, in seconds from the start of the stem.
    pub offset_s: f64,
    /// MIDI pitch number (0–127); 69 = A4 = 440 Hz.
    pub midi_pitch: u8,
}

impl RefNote {
    /// Build a note from a pitch in **Hz**, rounding to the nearest MIDI note and
    /// clamping to the valid range. The single home for this policy, shared by
    /// every source that emits reference notes (Vocadito's Hz annotations and the
    /// fixture synthesizer).
    pub fn from_hz(onset_s: f64, offset_s: f64, pitch_hz: f32) -> Self {
        RefNote {
            onset_s,
            offset_s,
            midi_pitch: sona_core::hz_to_midi(pitch_hz).round().clamp(0.0, 127.0) as u8,
        }
    }
}

/// A reference f0 frame. `frequency_hz` is `0.0` where the frame is unvoiced;
/// `voiced` records that decision explicitly so a metric needn't re-derive it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RefF0 {
    /// Frame time, in seconds from the start of the stem.
    pub time_s: f64,
    /// Reference fundamental frequency, in Hz (`0.0` when unvoiced).
    pub frequency_hz: f32,
    /// Whether this frame is voiced.
    pub voiced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let reference = Reference {
            notes: vec![RefNote { onset_s: 0.1, offset_s: 0.5, midi_pitch: 69 }],
            f0: vec![
                RefF0 { time_s: 0.1, frequency_hz: 440.0, voiced: true },
                RefF0 { time_s: 0.2, frequency_hz: 0.0, voiced: false },
            ],
        };
        let json = serde_json::to_string(&reference).expect("serialize");
        let decoded: Reference = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reference, decoded);
    }
}
