//! **UST → MIDI** — for eyeballing a UST's note data in a DAW or the review
//! surface's MIDI import.
//!
//!     cargo run -p sona-eval --bin sona-eval-ust-to-midi -- song/main.ust out.mid [--offset 0.25]
//!
//! Rests and breaths become gaps; `--offset` applies a manifest alignment
//! (`stem_time = ust_time + offset`). Written at the UST's own tempo, so for a
//! single-tempo song the MIDI ticks reproduce the UST grid exactly.

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::ExitCode;

use sona_core::{Annotation, Note, Producer, SubScores, SCHEMA_VERSION};
use sona_eval::Ust;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut offset_s = 0.0;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--offset" {
            match it.next().and_then(|v| v.parse::<f64>().ok()) {
                Some(v) => offset_s = v,
                None => return usage("--offset needs a number of seconds"),
            }
        } else {
            positional.push(PathBuf::from(a));
        }
    }
    let [input, output] = positional.as_slice() else {
        return usage("expected <in.ust> <out.mid>");
    };

    let ust = match Ust::load(input) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("sona-eval-ust-to-midi: {e}");
            return ExitCode::FAILURE;
        }
    };
    if !ust.tempo_changes().is_empty() {
        eprintln!("warning: the UST changes tempo mid-song; the MIDI uses one tempo, so ticks after the change won't match the UST grid");
    }
    let reference = ust.to_reference(offset_s);
    // A transient annotation only to reuse the core's MIDI writer; the producer
    // field is not written into the MIDI file.
    let annotation = Annotation {
        version: SCHEMA_VERSION,
        sample_rate: 44_100,
        bpm: ust.tempo,
        duration_s: reference.notes.last().map_or(0.0, |n| n.offset_s),
        producer: Producer::Pyin,
        notes: reference
            .notes
            .iter()
            .map(|n| Note {
                onset_s: n.onset_s,
                offset_s: n.offset_s,
                midi_pitch: n.midi_pitch,
                confidence: 1.0,
                sub_scores: SubScores { pitch_stability: 1.0, voicing: 1.0, onset_clarity: 1.0, offset_clarity: 1.0 },
            })
            .collect(),
        f0_contour: Vec::new(),
        energy_envelope: Vec::new(),
        provenance: None,
    };
    let result = File::create(output).and_then(|f| sona_core::write_midi(&annotation, BufWriter::new(f)));
    match result {
        Ok(()) => {
            println!("{} notes → {}", annotation.notes.len(), output.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("sona-eval-ust-to-midi: {}: {e}", output.display());
            ExitCode::FAILURE
        }
    }
}

fn usage(msg: &str) -> ExitCode {
    eprintln!("sona-eval-ust-to-midi: {msg}\nusage: sona-eval-ust-to-midi <in.ust> <out.mid> [--offset SECONDS]");
    ExitCode::FAILURE
}
