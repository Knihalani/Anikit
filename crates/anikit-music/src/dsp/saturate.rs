//! Tape / tube / soft saturation, 2× or 4× oversampled with linear-phase
//! half-band filters so the added harmonics do not alias. The dry path is
//! delayed to match, so `mix` blends without comb filtering.

use serde::Deserialize;

use super::biquad::{Biquad, FilterKind};
use super::oversample::{Downsampler2, Upsampler2};
use super::util::{db_to_gain, fast_tanh, DelayLine};
use super::Processor;

/// Saturation character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SatMode {
    /// tanh with pre-/de-emphasis: highs saturate a little earlier, as on tape.
    #[default]
    Tape,
    /// Asymmetric (biased) curve: adds even harmonics.
    Tube,
    /// Cubic soft clip: gentlest knee, mostly 3rd harmonic.
    Soft,
}

/// Saturator parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SaturatorParams {
    pub mode: SatMode,
    /// Input drive in dB.
    pub drive: f32,
    /// Asymmetry for `tube` (0..1).
    pub bias: f32,
    /// Dry/wet.
    pub mix: f32,
    /// Output gain in dB (after auto gain).
    pub output: f32,
    /// Compensate the level change of the drive (matched at −12 dBFS).
    pub auto_gain: bool,
    /// Oversampling factor: 1, 2 or 4.
    pub oversample: u32,
}

impl Default for SaturatorParams {
    fn default() -> Self {
        SaturatorParams { mode: SatMode::Tape, drive: 6.0, bias: 0.3, mix: 1.0, output: 0.0, auto_gain: true, oversample: 4 }
    }
}

const CHUNK: usize = 64;

#[derive(Debug, Clone)]
struct Chan {
    up1: Upsampler2,
    up2: Upsampler2,
    dn2: Downsampler2,
    dn1: Downsampler2,
    z2: f32,
    pre: Biquad,
    post: Biquad,
    dc: Biquad,
    dry: DelayLine,
}

/// Oversampled waveshaper.
#[derive(Debug, Clone)]
pub struct Saturator {
    mode: SatMode,
    drive: f32,
    bias: f32,
    off: f32,
    makeup: f32,
    mix: f32,
    os: u32,
    lat: usize,
    ch: [Chan; 2],
}

impl Saturator {
    pub fn new(rate: f32, p: &SaturatorParams) -> Saturator {
        let os = match p.oversample {
            0 | 1 => 1,
            2 | 3 => 2,
            _ => 4,
        };
        let drive = db_to_gain(p.drive);
        let bias = if p.mode == SatMode::Tube { p.bias.clamp(0.0, 1.0) * 0.6 } else { 0.0 };
        let mk = || {
            let (up1, dn1) = (Upsampler2::new(true), Downsampler2::new(true));
            Chan {
                up1,
                up2: Upsampler2::new(false),
                dn2: Downsampler2::new(false),
                dn1,
                z2: 0.0,
                pre: Biquad::new(FilterKind::HighShelf, rate, 3000.0, 0.7071, 4.0),
                post: Biquad::new(FilterKind::HighShelf, rate, 3000.0, 0.7071, -4.0),
                dc: Biquad::new(FilterKind::HighPass, rate, 8.0, 0.7071, 0.0),
                dry: DelayLine::new(128),
            }
        };
        let c = mk();
        let lat = match os {
            1 => 0,
            2 => (c.up1.latency2() + c.dn1.latency2()) / 2,
            _ => (c.up1.latency2() + c.dn1.latency2()) / 2 + (c.up2.latency2() + c.dn2.latency2() + 2) / 4,
        };
        let mut s = Saturator {
            mode: p.mode,
            drive,
            bias,
            off: fast_tanh(bias),
            makeup: 1.0,
            mix: p.mix.clamp(0.0, 1.0),
            os,
            lat,
            ch: [c, mk()],
        };
        let reference = 0.25;
        let g = if p.auto_gain { reference / s.curve(reference * drive).abs().max(1e-6) } else { 1.0 };
        s.makeup = g * db_to_gain(p.output);
        s
    }

    #[inline(always)]
    fn curve(&self, x: f32) -> f32 {
        match self.mode {
            SatMode::Tape => fast_tanh(x),
            SatMode::Tube => fast_tanh(x + self.bias) - self.off,
            SatMode::Soft => {
                let v = (x * (2.0 / 3.0)).clamp(-1.0, 1.0);
                1.5 * (v - v * v * v / 3.0)
            }
        }
    }

    fn shape(&self, x: &mut [f32]) {
        let d = self.drive;
        for v in x.iter_mut() {
            *v = self.curve(*v * d);
        }
    }

    fn run_chan(&mut self, c: usize, x: &mut [f32]) {
        let tape = self.mode == SatMode::Tape;
        let n = x.len();
        if tape {
            self.ch[c].pre.process_mono(x);
        }
        match self.os {
            1 => self.shape(x),
            2 => {
                let mut b2 = [0.0f32; 2 * CHUNK];
                self.ch[c].up1.process(x, &mut b2[..2 * n]);
                self.shape(&mut b2[..2 * n]);
                self.ch[c].dn1.process(&b2[..2 * n], x);
            }
            _ => {
                let mut b2 = [0.0f32; 2 * CHUNK];
                let mut b4 = [0.0f32; 4 * CHUNK];
                let ch = &mut self.ch[c];
                ch.up1.process(x, &mut b2[..2 * n]);
                // One 2×-rate sample of delay makes the total latency an integer.
                for v in b2[..2 * n].iter_mut() {
                    std::mem::swap(v, &mut ch.z2);
                }
                ch.up2.process(&b2[..2 * n], &mut b4[..4 * n]);
                self.shape(&mut b4[..4 * n]);
                let ch = &mut self.ch[c];
                ch.dn2.process(&b4[..4 * n], &mut b2[..2 * n]);
                ch.dn1.process(&b2[..2 * n], x);
            }
        }
        let ch = &mut self.ch[c];
        if tape {
            ch.post.process_mono(x);
        }
        if self.bias != 0.0 {
            ch.dc.process_mono(x);
        }
    }
}

impl Processor for Saturator {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut dry = [0.0f32; CHUNK];
        for (c, buf) in [l, r].into_iter().enumerate() {
            for blk in buf[..n].chunks_mut(CHUNK) {
                let m = blk.len();
                if self.mix < 1.0 {
                    let line = &mut self.ch[c].dry;
                    for (d, &v) in dry[..m].iter_mut().zip(blk.iter()) {
                        line.push(v);
                        *d = line.tap(self.lat);
                    }
                }
                self.run_chan(c, blk);
                let (wet, mix) = (self.makeup * self.mix, 1.0 - self.mix);
                if mix > 0.0 {
                    for (v, d) in blk.iter_mut().zip(&dry[..m]) {
                        *v = *v * wet + d * mix;
                    }
                } else {
                    blk.iter_mut().for_each(|v| *v *= wet);
                }
            }
        }
    }
    fn reset(&mut self) {
        for c in self.ch.iter_mut() {
            c.up1.reset();
            c.up2.reset();
            c.dn1.reset();
            c.dn2.reset();
            c.z2 = 0.0;
            c.pre.reset();
            c.post.reset();
            c.dc.reset();
            c.dry.reset();
        }
    }
    fn latency(&self) -> usize {
        self.lat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Amplitude of `f` in `x` (Hann-windowed correlation).
    fn tone_db(x: &[f32], f: f64, rate: f64) -> f64 {
        let n = x.len() as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n).cos();
            let ph = 2.0 * std::f64::consts::PI * f * i as f64 / rate;
            re += w * *v as f64 * ph.cos();
            im += w * *v as f64 * ph.sin();
        }
        20.0 * (4.0 * (re * re + im * im).sqrt() / n).max(1e-20).log10()
    }

    #[test]
    fn latency_is_compensated_and_small_signals_pass() {
        for os in [1, 2, 4] {
            let p = SaturatorParams { oversample: os, drive: 0.0, mode: SatMode::Soft, ..Default::default() };
            let mut s = Saturator::new(48_000.0, &p);
            let n = 4800;
            let x: Vec<f32> = (0..n).map(|i| 0.01 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 48_000.0).sin() as f32).collect();
            let (mut l, mut r) = (x.clone(), x.clone());
            super::super::process_aligned(&mut s, &mut l, &mut r);
            let err = l.iter().zip(&x).skip(200).take(4000).fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
            assert!(err < 2e-4, "os {os}: err {err}");
        }
    }

    #[test]
    fn oversampling_suppresses_aliasing() {
        // A hot 7 kHz tone: harmonics at 21, 35, 49 … kHz. 35 kHz aliases to
        // 13 kHz at 48 kHz unless the shaper runs oversampled.
        let rate = 48_000.0;
        let n = 48_000;
        let x: Vec<f32> = (0..n).map(|i| 0.9 * (2.0 * std::f64::consts::PI * 7000.0 * i as f64 / rate).sin() as f32).collect();
        let mut alias = Vec::new();
        for os in [1, 4] {
            let p = SaturatorParams { oversample: os, drive: 18.0, mode: SatMode::Tape, ..Default::default() };
            let mut s = Saturator::new(48_000.0, &p);
            let (mut l, mut r) = (x.clone(), x.clone());
            s.process(&mut l, &mut r);
            alias.push(tone_db(&l[4800..], 13_000.0, rate) - tone_db(&l[4800..], 7000.0, rate));
        }
        assert!(alias[0] > -40.0, "1× aliases: {}", alias[0]);
        assert!(alias[1] < -70.0 && alias[0] - alias[1] > 30.0, "4× alias {} dB", alias[1]);
    }

    #[test]
    fn tube_adds_even_harmonics_without_dc() {
        let rate = 48_000.0;
        let n = 48_000;
        let x: Vec<f32> = (0..n).map(|i| 0.5 * (2.0 * std::f64::consts::PI * 200.0 * i as f64 / rate).sin() as f32).collect();
        let p = SaturatorParams { mode: SatMode::Tube, drive: 12.0, ..Default::default() };
        let mut s = Saturator::new(48_000.0, &p);
        let (mut l, mut r) = (x.clone(), x.clone());
        s.process(&mut l, &mut r);
        let h2 = tone_db(&l[24_000..], 400.0, rate) - tone_db(&l[24_000..], 200.0, rate);
        assert!(h2 > -40.0, "2nd harmonic {h2}");
        let dc: f32 = l[24_000..].iter().sum::<f32>() / 24_000.0;
        assert!(dc.abs() < 1e-3, "dc {dc}");
        assert!(l.iter().all(|v| v.is_finite()));
    }
}
