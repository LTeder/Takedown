//! WAV input: any channel count, 8 to 32-bit integer or 32-bit float PCM,
//! downmixed to mono f32 at the file's own sample rate.

use std::path::Path;

pub struct Mono {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn read_mono(path: &Path) -> Result<Mono, String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    if spec.sample_rate == 0 {
        return Err(format!("{}: sample rate is 0", path.display()));
    }
    let channels = usize::from(spec.channels.max(1));
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{}: {e}", path.display()))?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / f64::from(1u32 << (spec.bits_per_sample.clamp(1, 32) - 1));
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| (f64::from(v) * scale) as f32))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("{}: {e}", path.display()))?
        }
    };
    let samples = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok(Mono {
        samples,
        sample_rate: spec.sample_rate,
    })
}
