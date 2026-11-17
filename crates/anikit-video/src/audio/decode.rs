//! Decoding audio files (MP3, WAV, Ogg Vorbis, FLAC, AAC/MP4) to [`Pcm`] with
//! symphonia: mono is duplicated, surround is downmixed to stereo, and the
//! result is resampled to the requested rate.

use super::{Pcm, DEFAULT_RATE};
use std::io::Cursor;
use std::path::Path;
use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode a file at [`DEFAULT_RATE`].
pub fn decode_file(path: impl AsRef<Path>) -> Result<Pcm, String> {
    decode_file_at(path, DEFAULT_RATE)
}

/// Decode a file and resample it to `rate`.
///
/// Decoded files are cached for the process, keyed by path, size, modification
/// time and rate: one music track is otherwise decoded by the beat markers, by
/// every visualizer and again by the mixer (about 120 ms each for 36 s of FLAC).
pub fn decode_file_at(path: impl AsRef<Path>, rate: u32) -> Result<Pcm, String> {
    type Key = (std::path::PathBuf, u64, u128, u32);
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<(Key, std::sync::Arc<Pcm>)>>> = std::sync::OnceLock::new();
    let path = path.as_ref();
    let meta = std::fs::metadata(path).ok();
    let key: Option<Key> = meta.map(|m| {
        let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        (path.to_path_buf(), m.len(), mtime, rate)
    });
    let cache = CACHE.get_or_init(Default::default);
    if let Some(k) = &key {
        if let Some((_, p)) = cache.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(c, _)| c == k) {
            return Ok((**p).clone());
        }
    }
    let file = std::fs::File::open(path)
        .map_err(|e| format!("audio: cannot open {}: {e}", path.display()))?;
    let ext = path.extension().and_then(|e| e.to_str());
    let pcm = decode_source(Box::new(file), ext, rate).map_err(|e| format!("{e} ({})", path.display()))?;
    if let Some(k) = key {
        let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
        // A few tracks at most (bounded memory in long-lived processes).
        if c.len() >= 6 {
            c.remove(0);
        }
        c.push((k, std::sync::Arc::new(pcm.clone())));
    }
    Ok(pcm)
}

/// Decode an in-memory file at [`DEFAULT_RATE`]. `ext_hint` is a file
/// extension such as `"mp3"` that helps the format probe.
pub fn decode_bytes(bytes: &[u8], ext_hint: Option<&str>) -> Result<Pcm, String> {
    decode_bytes_at(bytes, ext_hint, DEFAULT_RATE)
}

/// Decode an in-memory file and resample it to `rate`.
pub fn decode_bytes_at(bytes: &[u8], ext_hint: Option<&str>, rate: u32) -> Result<Pcm, String> {
    decode_source(Box::new(Cursor::new(bytes.to_vec())), ext_hint, rate)
}

fn decode_source(src: Box<dyn MediaSource>, ext: Option<&str>, rate: u32) -> Result<Pcm, String> {
    let mss = MediaSourceStream::new(src, Default::default());
    let mut hint = Hint::new();
    if let Some(e) = ext {
        hint.with_extension(e.trim_start_matches('.'));
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("audio: unsupported or unrecognised format: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("audio: no audio track found")?;
    let id = track.id;
    let src_rate = track.codec_params.sample_rate;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("audio: unsupported codec: {e}"))?;

    let mut out: Vec<f32> = Vec::new();
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    let mut weights: Vec<(f32, f32)> = Vec::new();
    let mut rate_seen = src_rate;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("audio: read error: {e}")),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(Error::DecodeError(_)) => continue, // skip a corrupt packet
            Err(e) => return Err(format!("audio: decode error: {e}")),
        };
        let spec = *decoded.spec();
        rate_seen = Some(spec.rate);
        let nch = spec.channels.count().max(1);
        if sbuf
            .as_ref()
            .is_none_or(|b| b.capacity() < decoded.capacity() * nch)
        {
            sbuf = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
        }
        let buf = sbuf.as_mut().unwrap();
        buf.copy_interleaved_ref(decoded);
        let s = buf.samples();
        match nch {
            1 => s.iter().for_each(|&x| out.extend_from_slice(&[x, x])),
            2 => out.extend_from_slice(s),
            _ => {
                if weights.len() != nch {
                    weights = downmix_weights(spec.channels, nch);
                }
                for f in s.chunks_exact(nch) {
                    let (mut l, mut r) = (0.0, 0.0);
                    for (x, w) in f.iter().zip(&weights) {
                        l += x * w.0;
                        r += x * w.1;
                    }
                    out.extend_from_slice(&[l, r]);
                }
            }
        }
    }
    let src_rate = rate_seen.ok_or("audio: unknown sample rate")?;
    let pcm = Pcm {
        rate: src_rate,
        channels: 2,
        samples: out,
    };
    Ok(if src_rate == rate {
        pcm
    } else {
        pcm.resample(rate)
    })
}

/// Per-channel (left, right) weights for a surround → stereo downmix (ITU
/// style: centre and surrounds at −3 dB, LFE dropped), normalised so a
/// full-scale signal on every channel stays within ±1.
fn downmix_weights(layout: Channels, n: usize) -> Vec<(f32, f32)> {
    const H: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let mut w: Vec<(f32, f32)> = layout
        .iter()
        .map(|c| {
            if c == Channels::LFE1 || c == Channels::LFE2 {
                (0.0, 0.0)
            } else if c == Channels::FRONT_LEFT
                || c == Channels::FRONT_LEFT_WIDE
                || c == Channels::FRONT_LEFT_HIGH
            {
                (1.0, 0.0)
            } else if c == Channels::FRONT_RIGHT
                || c == Channels::FRONT_RIGHT_WIDE
                || c == Channels::FRONT_RIGHT_HIGH
            {
                (0.0, 1.0)
            } else if c == Channels::FRONT_CENTRE
                || c == Channels::REAR_CENTRE
                || c == Channels::TOP_CENTRE
                || c == Channels::FRONT_CENTRE_HIGH
            {
                (H, H)
            } else if c == Channels::SIDE_LEFT
                || c == Channels::REAR_LEFT
                || c == Channels::FRONT_LEFT_CENTRE
                || c == Channels::REAR_LEFT_CENTRE
            {
                (H, 0.0)
            } else if c == Channels::SIDE_RIGHT
                || c == Channels::REAR_RIGHT
                || c == Channels::FRONT_RIGHT_CENTRE
                || c == Channels::REAR_RIGHT_CENTRE
            {
                (0.0, H)
            } else {
                (0.5 * H, 0.5 * H)
            }
        })
        .collect();
    if w.len() != n {
        // Unknown layout: alternate left / right.
        w = (0..n)
            .map(|i| if i % 2 == 0 { (1.0, 0.0) } else { (0.0, 1.0) })
            .collect();
    }
    let (sl, sr) = w
        .iter()
        .fold((0.0f32, 0.0f32), |a, x| (a.0 + x.0, a.1 + x.1));
    let norm = sl.max(sr).max(1.0);
    w.iter_mut().for_each(|x| {
        x.0 /= norm;
        x.1 /= norm;
    });
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        assert!(decode_bytes(b"definitely not audio", None).is_err());
        assert!(decode_file("/no/such/file.mp3")
            .unwrap_err()
            .contains("cannot open"));
    }

    #[test]
    fn downmix_5_1_is_bounded() {
        let layout = Channels::FRONT_LEFT
            | Channels::FRONT_RIGHT
            | Channels::FRONT_CENTRE
            | Channels::LFE1
            | Channels::SIDE_LEFT
            | Channels::SIDE_RIGHT;
        let w = downmix_weights(layout, 6);
        let l: f32 = w.iter().map(|x| x.0).sum();
        assert!(l <= 1.0001 && l > 0.99);
        assert_eq!(w[3], (0.0, 0.0));
    }
}
