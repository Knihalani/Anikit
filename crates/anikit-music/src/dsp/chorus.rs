//! Chorus and string-ensemble: several modulated delay taps per channel
//! (cubic interpolation), LFO phases spread across voices and channels.
//! `ensemble` adds the fast "shimmer" LFO of the classic string ensembles.

use serde::Deserialize;

use super::util::DelayLine;
use super::Processor;

/// Chorus flavour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ChorusMode {
    #[default]
    Chorus,
    Ensemble,
}

/// Chorus parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ChorusParams {
    pub mode: ChorusMode,
    /// LFO rate in Hz.
    pub rate: f32,
    /// Modulation depth (ms, peak-to-peak).
    pub depth: f32,
    /// Centre delay (ms).
    pub delay: f32,
    /// Voices per channel (1..4).
    pub voices: u32,
    /// Stereo spread 0..1 (LFO phase offset between channels).
    pub spread: f32,
    /// Dry/wet.
    pub mix: f32,
}

impl Default for ChorusParams {
    fn default() -> Self {
        ChorusParams { mode: ChorusMode::Chorus, rate: 0.8, depth: 3.0, delay: 12.0, voices: 2, spread: 1.0, mix: 0.5 }
    }
}

const MAXV: usize = 4;

/// Multi-voice chorus / ensemble.
#[derive(Debug, Clone)]
pub struct Chorus {
    lines: [DelayLine; 2],
    voices: usize,
    centre: f32,
    depth: f32,
    mix: f32,
    ens: bool,
    /// Rotators (cos, sin) for the slow and fast LFOs.
    slow: (f32, f32),
    fast: (f32, f32),
    rs: (f32, f32),
    rf: (f32, f32),
    /// Per channel/voice phase offsets as (cos, sin).
    off: [[(f32, f32); MAXV]; 2],
}

impl Chorus {
    pub fn new(rate: f32, p: &ChorusParams) -> Chorus {
        let ens = p.mode == ChorusMode::Ensemble;
        let voices = if ens { 3 } else { p.voices.clamp(1, MAXV as u32) as usize };
        let depth = p.depth.max(0.0) * 0.001 * rate * 0.5;
        let centre = (p.delay.max(0.5) * 0.001 * rate).max(depth * 1.3 + 2.0);
        let rot = |hz: f32| {
            let w = 2.0 * std::f32::consts::PI * hz / rate;
            (w.cos(), w.sin())
        };
        let mut off = [[(1.0, 0.0); MAXV]; 2];
        for (c, row) in off.iter_mut().enumerate() {
            for (v, o) in row.iter_mut().enumerate() {
                let a = 2.0 * std::f32::consts::PI * (v as f32 / voices as f32 + c as f32 * 0.25 * p.spread.clamp(0.0, 1.0));
                *o = (a.cos(), a.sin());
            }
        }
        let max = (centre + depth * 1.2 + 8.0) as usize;
        Chorus {
            lines: [DelayLine::new(max), DelayLine::new(max)],
            voices,
            centre,
            depth,
            mix: p.mix.clamp(0.0, 1.0),
            ens,
            slow: (1.0, 0.0),
            fast: (1.0, 0.0),
            rs: rot(if ens { 0.6 * p.rate / 0.8 } else { p.rate }),
            rf: rot(if ens { 5.8 } else { 0.0 }),
            off,
        }
    }
}

#[inline(always)]
fn rotate(p: (f32, f32), r: (f32, f32)) -> (f32, f32) {
    (p.0 * r.0 - p.1 * r.1, p.0 * r.1 + p.1 * r.0)
}

impl Processor for Chorus {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let g = 1.0 / self.voices as f32;
        let (wet, dry) = (self.mix, 1.0 - self.mix);
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            self.slow = rotate(self.slow, self.rs);
            self.fast = rotate(self.fast, self.rf);
            self.lines[0].push(*a);
            self.lines[1].push(*b);
            let mut out = [0.0f32; 2];
            for (c, o) in out.iter_mut().enumerate() {
                for v in 0..self.voices {
                    let (oc, os) = self.off[c][v];
                    // sin(θ + φ) for both LFOs.
                    let s1 = self.slow.1 * oc + self.slow.0 * os;
                    let m = if self.ens {
                        let s2 = self.fast.1 * oc + self.fast.0 * os;
                        0.85 * s1 + 0.15 * s2
                    } else {
                        s1
                    };
                    *o += self.lines[c].read(self.centre + self.depth * m);
                }
            }
            *a = *a * dry + out[0] * g * wet;
            *b = *b * dry + out[1] * g * wet;
        }
        for p in [&mut self.slow, &mut self.fast] {
            let n = 1.0 / (p.0 * p.0 + p.1 * p.1).sqrt();
            *p = (p.0 * n, p.1 * n);
        }
    }
    fn reset(&mut self) {
        self.lines.iter_mut().for_each(|x| x.reset());
        self.slow = (1.0, 0.0);
        self.fast = (1.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chorus_is_bounded_and_stereo() {
        for mode in [ChorusMode::Chorus, ChorusMode::Ensemble] {
            let mut c = Chorus::new(48_000.0, &ChorusParams { mode, ..Default::default() });
            let x: Vec<f32> = (0..96_000).map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 48_000.0).sin() * 0.5).collect();
            let (mut l, mut r) = (x.clone(), x.clone());
            c.process(&mut l, &mut r);
            assert!(l.iter().chain(r.iter()).all(|v| v.is_finite() && v.abs() <= 0.51));
            let diff: f32 = l.iter().zip(&r).map(|(a, b)| (a - b).abs()).sum::<f32>() / l.len() as f32;
            assert!(diff > 0.01, "{mode:?} should decorrelate L/R");
        }
    }
}
