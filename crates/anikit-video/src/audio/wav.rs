//! WAV and raw PCM writers (for ffmpeg muxing: `-f f32le -ar 48000 -ac 2`
//! or `-f s16le …`).

use super::Pcm;
use std::io::{BufWriter, Write};
use std::path::Path;

/// WAV sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WavFormat {
    /// 16-bit integer PCM.
    #[default]
    Pcm16,
    /// 32-bit IEEE float.
    Float32,
}

/// Write `pcm` as a WAV file.
pub fn write_wav(path: impl AsRef<Path>, pcm: &Pcm, format: WavFormat) -> Result<(), String> {
    let path = path.as_ref();
    let f = std::fs::File::create(path)
        .map_err(|e| format!("audio: cannot create {}: {e}", path.display()))?;
    let mut w = BufWriter::new(f);
    write_wav_to(&mut w, pcm, format)
        .and_then(|_| w.flush())
        .map_err(|e| format!("audio: write {}: {e}", path.display()))
}

/// Write `pcm` as WAV to any writer.
pub fn write_wav_to<W: Write>(w: &mut W, pcm: &Pcm, format: WavFormat) -> std::io::Result<()> {
    let ch = 2u16;
    let (tag, bits): (u16, u16) = match format {
        WavFormat::Pcm16 => (1, 16),
        WavFormat::Float32 => (3, 32),
    };
    let block = ch * bits / 8;
    let data_len = (pcm.samples.len() * (bits as usize / 8)) as u32;
    let float = format == WavFormat::Float32;
    // Float WAVs carry an 18-byte fmt chunk and a `fact` chunk.
    let fmt_len: u32 = if float { 18 } else { 16 };
    let fact_len: u32 = if float { 12 } else { 0 };
    let riff_len = 4 + (8 + fmt_len) + fact_len + 8 + data_len;
    let mut h = Vec::with_capacity(64);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&riff_len.to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&fmt_len.to_le_bytes());
    h.extend_from_slice(&tag.to_le_bytes());
    h.extend_from_slice(&ch.to_le_bytes());
    h.extend_from_slice(&pcm.rate.to_le_bytes());
    h.extend_from_slice(&(pcm.rate * block as u32).to_le_bytes());
    h.extend_from_slice(&block.to_le_bytes());
    h.extend_from_slice(&bits.to_le_bytes());
    if float {
        h.extend_from_slice(&0u16.to_le_bytes());
        h.extend_from_slice(b"fact");
        h.extend_from_slice(&4u32.to_le_bytes());
        h.extend_from_slice(&(pcm.frames() as u32).to_le_bytes());
    }
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data_len.to_le_bytes());
    w.write_all(&h)?;
    match format {
        WavFormat::Pcm16 => write_s16le(w, pcm),
        WavFormat::Float32 => write_f32le(w, pcm),
    }
}

/// Raw interleaved little-endian `f32` samples.
pub fn write_f32le<W: Write>(w: &mut W, pcm: &Pcm) -> std::io::Result<()> {
    write_chunked(w, &pcm.samples, 4, |s, out| {
        out.copy_from_slice(&s.to_le_bytes())
    })
}

/// Raw interleaved little-endian `i16` samples (rounded, clamped).
pub fn write_s16le<W: Write>(w: &mut W, pcm: &Pcm) -> std::io::Result<()> {
    write_chunked(w, &pcm.samples, 2, |s, out| {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.copy_from_slice(&v.to_le_bytes())
    })
}

fn write_chunked<W: Write>(
    w: &mut W,
    samples: &[f32],
    size: usize,
    conv: impl Fn(f32, &mut [u8]),
) -> std::io::Result<()> {
    let mut buf = vec![0u8; 16_384 * size];
    for chunk in samples.chunks(16_384) {
        let out = &mut buf[..chunk.len() * size];
        for (s, o) in chunk.iter().zip(out.chunks_exact_mut(size)) {
            conv(*s, o);
        }
        w.write_all(out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::decode::decode_bytes_at;
    use super::*;

    fn tone(rate: u32) -> Pcm {
        let samples = (0..rate as usize / 2).flat_map(|i| {
            let t = i as f32 / rate as f32;
            [
                (std::f32::consts::TAU * 440.0 * t).sin() * 0.5,
                (std::f32::consts::TAU * 660.0 * t).sin() * -0.25,
            ]
        });
        Pcm {
            rate,
            channels: 2,
            samples: samples.collect(),
        }
    }

    #[test]
    fn wav_round_trip_float() {
        let pcm = tone(44_100);
        let mut bytes = Vec::new();
        write_wav_to(&mut bytes, &pcm, WavFormat::Float32).unwrap();
        let back = decode_bytes_at(&bytes, Some("wav"), 44_100).unwrap();
        assert_eq!(back, pcm);
    }

    #[test]
    fn wav_round_trip_16_bit_and_file() {
        let pcm = tone(48_000);
        let dir = std::env::temp_dir().join(format!("anikit-audio-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.wav");
        write_wav(&path, &pcm, WavFormat::Pcm16).unwrap();
        let back = super::super::decode::decode_file(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(back.frames(), pcm.frames());
        let err = back
            .samples
            .iter()
            .zip(&pcm.samples)
            .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(err < 1.0 / 16_000.0, "{err}");
    }

    #[test]
    fn raw_writers() {
        let pcm = Pcm {
            rate: 48_000,
            channels: 2,
            samples: vec![1.0, -1.0, 0.5, 2.0],
        };
        let mut a = Vec::new();
        write_s16le(&mut a, &pcm).unwrap();
        assert_eq!(a, [0xff, 0x7f, 0x01, 0x80, 0x00, 0x40, 0xff, 0x7f]);
        let mut b = Vec::new();
        write_f32le(&mut b, &pcm).unwrap();
        assert_eq!(b.len(), 16);
        assert_eq!(&b[8..12], &0.5f32.to_le_bytes());
    }
}
