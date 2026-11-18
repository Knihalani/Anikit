//! Audio for Anikit Video: decode, mix, beat analysis, procedural SFX and
//! PCM/WAV output.
//!
//! Everything works on [`Pcm`]: interleaved stereo `f32` at one sample rate
//! (48 kHz by default). The pipeline decodes sources with [`decode`], places
//! them on the timeline with [`mix`], analyses music with [`beats`] so cuts can
//! land on the beat, and hands the master to ffmpeg through [`wav`].

pub mod beats;
pub mod decode;
pub mod design;
pub mod mix;
pub mod sfx;
pub mod wav;

pub use beats::{analyze_pcm, Beats};
pub use decode::{decode_bytes, decode_bytes_at, decode_file, decode_file_at};
pub use mix::{load_sources, mix, mix_sources, Duck, Role, Source, Track};
pub use sfx::{sfx, sfx_at, SfxParams, SFX_NAMES};
pub use wav::{write_f32le, write_s16le, write_wav, write_wav_to, WavFormat};

/// The sample rate used when none is given.
pub const DEFAULT_RATE: u32 = 48_000;

/// Interleaved stereo `f32` audio.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pcm {
    /// Samples per second (per channel).
    pub rate: u32,
    /// Always 2: left and right are interleaved.
    pub channels: u16,
    /// `L R L R …`, nominally in −1…1.
    pub samples: Vec<f32>,
}

impl AsRef<Pcm> for Pcm {
    fn as_ref(&self) -> &Pcm {
        self
    }
}

impl Pcm {
    /// `frames` frames of silence.
    pub fn silence(rate: u32, frames: usize) -> Pcm {
        Pcm {
            rate,
            channels: 2,
            samples: vec![0.0; frames * 2],
        }
    }

    /// Stereo from mono (the signal is duplicated to both channels).
    pub fn from_mono(rate: u32, mono: &[f32]) -> Pcm {
        let mut samples = Vec::with_capacity(mono.len() * 2);
        for &s in mono {
            samples.push(s);
            samples.push(s);
        }
        Pcm {
            rate,
            channels: 2,
            samples,
        }
    }

    /// Number of frames (samples per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }

    /// Length in seconds.
    pub fn duration(&self) -> f64 {
        if self.rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.rate as f64
        }
    }

    /// Largest absolute sample value.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// Mono downmix `(L + R) / 2`.
    pub fn mono(&self) -> Vec<f32> {
        self.samples
            .chunks_exact(2)
            .map(|c| 0.5 * (c[0] + c[1]))
            .collect()
    }

    /// This audio at another sample rate (windowed-sinc, Blackman window,
    /// 32+ taps, anti-aliased when downsampling).
    pub fn resample(&self, rate: u32) -> Pcm {
        if rate == self.rate || self.rate == 0 || self.samples.is_empty() {
            return Pcm {
                rate,
                channels: 2,
                samples: self.samples.clone(),
            };
        }
        Pcm {
            rate,
            channels: 2,
            samples: resample_stereo(&self.samples, self.rate, rate),
        }
    }
}

/// Windowed-sinc resampling of interleaved stereo.
fn resample_stereo(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    const PHASES: usize = 1024;
    let n_in = input.len() / 2;
    let ratio = to as f64 / from as f64;
    // Cutoff relative to the input Nyquist; a little below the lower Nyquist.
    let cutoff = ratio.min(1.0) * 0.94;
    let half = (16.0 / cutoff).ceil() as usize; // taps per side
    let taps = 2 * half;
    // table[p][j] = h(j - half + 1 - p / PHASES)
    let mut table = vec![0.0f32; (PHASES + 1) * taps];
    for p in 0..=PHASES {
        let frac = p as f64 / PHASES as f64;
        let row = &mut table[p * taps..(p + 1) * taps];
        let mut sum = 0.0;
        for (j, v) in row.iter_mut().enumerate() {
            let x = j as f64 - half as f64 + 1.0 - frac;
            let sinc = if x.abs() < 1e-9 {
                1.0
            } else {
                (std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * cutoff * x)
            };
            let w = x / half as f64; // −1…1
            let win = if w.abs() >= 1.0 {
                0.0
            } else {
                let a = std::f64::consts::PI * w;
                0.42 + 0.5 * a.cos() + 0.08 * (2.0 * a).cos()
            };
            let h = sinc * win;
            *v = h as f32;
            sum += h;
        }
        // Unity DC gain for every phase.
        let inv = (1.0 / sum) as f32;
        row.iter_mut().for_each(|v| *v *= inv);
    }
    let n_out = ((n_in as f64) * ratio).round() as usize;
    let mut out = vec![0.0f32; n_out * 2];
    let step = from as f64 / to as f64;
    for (i, o) in out.chunks_exact_mut(2).enumerate() {
        let t = i as f64 * step;
        let n0 = t.floor() as isize;
        let p = ((t - n0 as f64) * PHASES as f64).round() as usize;
        let row = &table[p * taps..(p + 1) * taps];
        let first = n0 - half as isize + 1;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        if first >= 0 && (first as usize + taps) <= n_in {
            let src = &input[first as usize * 2..(first as usize + taps) * 2];
            for (h, s) in row.iter().zip(src.chunks_exact(2)) {
                l += h * s[0];
                r += h * s[1];
            }
        } else {
            for (j, h) in row.iter().enumerate() {
                let k = first + j as isize;
                if k >= 0 && (k as usize) < n_in {
                    l += h * input[k as usize * 2];
                    r += h * input[k as usize * 2 + 1];
                }
            }
        }
        o[0] = l;
        o[1] = r;
    }
    out
}

/// `10^(db / 20)`.
pub fn db_to_gain(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_keeps_a_sine() {
        let rate = 44_100;
        let mono: Vec<f32> = (0..44_100)
            .map(|i| (std::f32::consts::TAU * 1000.0 * i as f32 / rate as f32).sin() * 0.5)
            .collect();
        let out = Pcm::from_mono(rate, &mono).resample(48_000);
        assert_eq!(out.frames(), 48_000);
        // Compare with an ideal sine in the middle (away from the edges).
        let mut err = 0.0f32;
        for i in 1000..47_000 {
            let want = (std::f32::consts::TAU * 1000.0 * i as f32 / 48_000.0).sin() * 0.5;
            err = err.max((out.samples[i * 2] - want).abs());
        }
        assert!(err < 2e-3, "max error {err}");
    }

    #[test]
    fn downsampling_removes_content_above_nyquist() {
        // 15 kHz at 48 kHz → 22.05 kHz must be filtered out (Nyquist 11.025 kHz).
        let mono: Vec<f32> = (0..48_000)
            .map(|i| (std::f32::consts::TAU * 15_000.0 * i as f32 / 48_000.0).sin())
            .collect();
        let out = Pcm::from_mono(48_000, &mono).resample(22_050);
        let rms = (out.samples[2000..40_000].iter().map(|s| s * s).sum::<f32>() / 38_000.0).sqrt();
        assert!(rms < 0.01, "rms {rms}");
    }
}
