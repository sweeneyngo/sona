//! WAV I/O for fixtures. The perception core is deliberately I/O-free — it takes
//! mono `&[f32]` (ADR-0002/0004) — so decoding lives here, as it does in the CLI.
//! Kept a small local pair rather than shared, so neither the core nor the CLI
//! grows a dependency on the other.

use std::path::Path;

use crate::error::EvalError;
use crate::synth::SAMPLE_RATE;

/// Decode a WAV file to mono `f32` samples in `-1.0..=1.0`, with its sample rate.
/// Integer PCM is normalized by full-scale; multi-channel is downmixed by
/// averaging (a vocal stem is effectively mono).
pub fn read_mono(path: &Path) -> Result<(Vec<f32>, u32), EvalError> {
    let mut reader = hound::WavReader::open(path).map_err(|e| EvalError::wav(path, e.to_string()))?;
    let spec = reader.spec();

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| EvalError::wav(path, e.to_string()))?,
        hound::SampleFormat::Int => {
            let full_scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / full_scale))
                .collect::<Result<_, _>>()
                .map_err(|e| EvalError::wav(path, e.to_string()))?
        }
    };

    let channels = spec.channels.max(1) as usize;
    let mono = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    if mono.is_empty() {
        return Err(EvalError::wav(path, "no audio samples"));
    }
    Ok((mono, spec.sample_rate))
}

/// Write mono `f32` samples (`-1.0..=1.0`) as a 16-bit PCM WAV at [`SAMPLE_RATE`].
/// Used only by the fixture generator; kept next to the reader so the encode and
/// decode conventions live together.
pub fn write_mono_16bit(path: &Path, samples: &[f32]) -> Result<(), EvalError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| EvalError::wav(path, e.to_string()))?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let v = (clamped * i16::MAX as f32).round() as i16;
        writer.write_sample(v).map_err(|e| EvalError::wav(path, e.to_string()))?;
    }
    writer.finalize().map_err(|e| EvalError::wav(path, e.to_string()))
}
