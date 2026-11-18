//! Music features for shaders: `iAudio` (level, bass, mid, treble) and `iBeat`
//! (pulse, phase, beat number, bar pulse) at any time on the timeline.
//!
//! The music is the cue [`super::beat_markers`] uses (the first top-level
//! `audio` cue with a `src` that is not a voice-over). It is decoded and
//! analysed once, at compile time, and only when a shader reads `iAudio` or
//! `iBeat`. Deterministic: the same file always gives the same values.

use crate::audio::{analyze_pcm, decode_file};
use std::path::Path;

/// Seconds per analysis hop.
const HOP: f64 = 0.01;

pub(crate) struct AudioFeatures {
    /// (level, bass, mid, treble) per hop, 0..1.
    bands: Vec<[f32; 4]>,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
    /// Timeline time minus this is the time in the file (`at − trimStart`).
    offset: f64,
}

/// One-pole low-pass coefficient for cutoff `fc` at `rate`.
fn lp(fc: f64, rate: f64) -> f32 {
    (1.0 - (-2.0 * std::f64::consts::PI * fc / rate).exp()) as f32
}

impl AudioFeatures {
    /// Analyse `path`, placed on the timeline at `at` and trimmed by `trim` seconds.
    pub fn load(path: &Path, at: f64, trim: f64) -> Result<AudioFeatures, String> {
        let pcm = decode_file(path)?;
        let beats = super::beat_markers::refine(&analyze_pcm(&pcm));
        Ok(Self::from_mono(&pcm.mono(), pcm.rate, beats.beats, beats.downbeats, at - trim))
    }

    /// From mono samples at `rate`, with a known beat grid (in file time) and
    /// `offset` = timeline time of the first sample: generated `music`.
    pub fn from_mono(mono: &[f32], rate: u32, beats: Vec<f64>, downbeats: Vec<f64>, offset: f64) -> AudioFeatures {
        let rate = rate.max(1) as f64;
        let hop = ((HOP * rate) as usize).max(1);
        // Split with two one-pole low-passes: bass < 200 Hz, mid 200 Hz–3 kHz, treble above.
        let (a1, a2) = (lp(200.0, rate), lp(3000.0, rate));
        let (mut l1, mut l2) = (0.0f32, 0.0f32);
        let mut sums = [0.0f64; 4];
        let mut raw: Vec<[f32; 4]> = Vec::with_capacity(mono.len() / hop + 1);
        for (i, &x) in mono.iter().enumerate() {
            l1 += a1 * (x - l1);
            l2 += a2 * (x - l2);
            let b = [x, l1, l2 - l1, x - l2];
            for k in 0..4 {
                sums[k] += (b[k] * b[k]) as f64;
            }
            if (i + 1) % hop == 0 {
                raw.push(std::array::from_fn(|k| (sums[k] / hop as f64).sqrt() as f32));
                sums = [0.0; 4];
            }
        }
        // Normalise each band by its 98th percentile, then a fast-attack,
        // ~150 ms release envelope so hits read as pulses, not flicker.
        let mut bands = raw.clone();
        for k in 0..4 {
            let mut v: Vec<f32> = raw.iter().map(|r| r[k]).collect();
            v.sort_by(f32::total_cmp);
            let p = v.get(v.len() * 98 / 100).copied().unwrap_or(1.0).max(1e-6);
            let rel = (-HOP / 0.15).exp() as f32;
            let mut env = 0.0f32;
            for (i, r) in raw.iter().enumerate() {
                let x = (r[k] / p).min(1.0).powf(0.8);
                env = if x > env { x } else { env * rel + x * (1.0 - rel) };
                bands[i][k] = env;
            }
        }
        AudioFeatures { bands, beats, downbeats, offset }
    }

    /// (`iAudio`, `iBeat`) at timeline time `t`.
    pub fn at(&self, t: f32) -> ([f32; 4], [f32; 4]) {
        let a = t as f64 - self.offset;
        let audio = if a < 0.0 || self.bands.is_empty() {
            [0.0; 4]
        } else {
            let x = a / HOP;
            let i = (x.floor() as usize).min(self.bands.len() - 1);
            let j = (i + 1).min(self.bands.len() - 1);
            let f = (x - i as f64).clamp(0.0, 1.0) as f32;
            std::array::from_fn(|k| self.bands[i][k] + (self.bands[j][k] - self.bands[i][k]) * f)
        };
        // Last beat at or before `a`.
        let n = self.beats.partition_point(|&b| b <= a);
        let beat = if n == 0 {
            [0.0; 4]
        } else {
            let b = self.beats[n - 1];
            let period = self.beats.get(n).map_or_else(|| if n >= 2 { b - self.beats[n - 2] } else { 0.5 }, |nx| nx - b).max(1e-3);
            let since = a - b;
            let m = self.downbeats.partition_point(|&d| d <= a);
            let bar = if m == 0 { 0.0 } else { (-(a - self.downbeats[m - 1]) * 4.0).exp() as f32 };
            [(-since * 8.0).exp() as f32, (since / period).min(1.0) as f32, n as f32, bar]
        };
        (audio, beat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_values() {
        let f = AudioFeatures { bands: vec![[0.0, 0.0, 0.0, 0.0], [1.0, 0.5, 0.25, 0.0]], beats: vec![1.0, 1.5, 2.0], downbeats: vec![1.0], offset: 0.0 };
        let (_, b) = f.at(0.5);
        assert_eq!(b, [0.0; 4]);
        let (_, b) = f.at(1.25);
        assert!((b[1] - 0.5).abs() < 1e-5 && b[2] == 1.0 && b[0] < 0.2 && b[3] > 0.3);
        let (a, b) = f.at(0.005);
        assert!((a[0] - 0.5).abs() < 1e-4 && b[2] == 0.0);
        assert_eq!(f.at(1.5).1[2], 2.0);
    }
}
