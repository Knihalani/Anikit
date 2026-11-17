//! Small shared helpers for the physical models, synths and drum machines:
//! fast branch-free maths, a cheap noise source, the output stage (gain ramp,
//! pan, kill fade) and silence detection.

use std::f32::consts::PI;

use std::sync::atomic::{AtomicBool, Ordering};

static BASELINE: AtomicBool = AtomicBool::new(false);

/// Force the baseline (SSE2) render paths even on AVX machines (tests use it
/// to check that both paths are bit-identical).
pub fn force_baseline(on: bool) {
    BASELINE.store(on, Ordering::Relaxed);
}

/// True when the AVX-compiled render paths may be used. They run exactly the
/// same IEEE operations in the same order, only 8 lanes wide, so the output
/// is bit-identical to the baseline path.
#[inline]
pub fn avx() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        !BASELINE.load(Ordering::Relaxed) && std::is_x86_feature_detected!("avx")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

/// −90 dBFS: voices whose output stays below this are dropped.
pub const SILENCE: f32 = 3.2e-5;

/// Flush values below 1e-20 to zero (denormal guard for decaying state).
#[inline(always)]
pub fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

/// Fractional part, branch-free, valid for |x| < 2³¹.
#[inline(always)]
pub fn fract(x: f32) -> f32 {
    let t = x - (x as i32) as f32;
    t + ((t < 0.0) as i32 as f32)
}

/// Fractional part for 0 ≤ x < 2²², without integer conversion (vectorises).
#[inline(always)]
pub fn fract_pos(x: f32) -> f32 {
    let r = (x + 12_582_912.0) - 12_582_912.0;
    let t = x - r;
    t + ((t < 0.0) as i32 as f32)
}

/// Branch-free minimum (maps to `minps`).
#[inline(always)]
pub fn fmin(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// Branch-free maximum (maps to `maxps`).
#[inline(always)]
pub fn fmax(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

/// `sin(2π·t)` for any `t` (polynomial, max error ≈ 1.4e-5 ≈ −97 dB).
#[inline(always)]
pub fn sin2pi(t: f32) -> f32 {
    sin2pi_unit(fract(t))
}

/// `sin(2π·t)` for `t` in [0, 1].
#[inline(always)]
pub fn sin2pi_unit(t: f32) -> f32 {
    let y = 2.0 * t - 1.0;
    let y2 = y * y;
    // sin(π·y) ≈ y·(1 − y²)·P(y²): exact zeros at 0 and ±1.
    let p = 3.141_559_8 + y2 * (-2.024_926_6 + y2 * (0.517_383_4 - 0.063_336_58 * y2));
    -(y * (1.0 - y2) * p)
}

/// Soft clipper ≈ tanh (rational Padé, clamped; monotone, |out| ≤ 1).
#[inline(always)]
pub fn tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Cheap cubic soft clipper: linear near 0, smooth knee, |out| ≤ 1 (for |x| ≥ 1.5).
#[inline(always)]
pub fn soft(x: f32) -> f32 {
    let x = fmax(fmin(x, 1.5), -1.5);
    x - x * x * x * (1.0 / 6.75)
}

/// One-pole smoothing coefficient for a time constant `tau` seconds.
#[inline]
pub fn pole(tau: f32, rate: f32) -> f32 {
    if tau <= 0.0 {
        0.0
    } else {
        (-1.0 / (tau * rate)).exp()
    }
}

/// Per-sample multiplier that decays by 60 dB in `t60` seconds.
#[inline]
pub fn t60_coef(t60: f32, rate: f32) -> f32 {
    if t60 <= 0.0 {
        0.0
    } else {
        (-6.907_755 / (t60 * rate)).exp()
    }
}

/// `tan(π·fc/rate)` (bilinear prewarp), clamped below Nyquist.
#[inline]
pub fn prewarp(fc: f32, rate: f32) -> f32 {
    (PI * (fc / rate).clamp(1e-5, 0.497)).tan()
}

/// Semitones → frequency ratio.
#[inline]
pub fn semis(s: f32) -> f32 {
    (s * (1.0 / 12.0)).exp2()
}

/// Decibels → linear gain.
#[inline]
pub fn db(d: f32) -> f32 {
    10f32.powf(d * 0.05)
}

/// Linear congruential white noise in [−1, 1); seeded from the voice seed.
#[derive(Clone, Copy, Debug)]
pub struct Noise(pub u32);

impl Noise {
    pub fn new(seed: u64) -> Noise {
        Noise((seed ^ (seed >> 32)) as u32 | 1)
    }
    #[inline(always)]
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 as i32) as f32 * (1.0 / 2_147_483_648.0)
    }
}

/// Output stage shared by every voice: a master gain that ramps per block
/// (velocity level, kill fade) and constant-power pan.
#[derive(Clone, Copy, Debug)]
pub struct Out {
    /// Current gain.
    pub g: f32,
    /// Kill fade step per sample (0 = not killing).
    pub kill_step: f32,
    pub pl: f32,
    pub pr: f32,
}

impl Out {
    pub fn new(gain: f32, pan: f32) -> Out {
        let (pl, pr) = crate::voice::pan_gains(pan);
        Out {
            g: gain,
            kill_step: 0.0,
            pl: pl * std::f32::consts::SQRT_2,
            pr: pr * std::f32::consts::SQRT_2,
        }
    }
    /// Start a ≈5 ms fade to silence.
    pub fn kill(&mut self, rate: f32) {
        if self.kill_step == 0.0 {
            self.kill_step = self.g.max(1e-9) / (0.005 * rate);
        }
    }
    /// True once a kill fade has finished.
    #[inline]
    pub fn dead(&self) -> bool {
        self.kill_step > 0.0 && self.g <= 0.0
    }
    /// Gain at the start and end of an `n`-frame block (advances the fade).
    #[inline(always)]
    pub fn ramp(&mut self, n: usize) -> (f32, f32) {
        let g0 = self.g;
        let g1 = (g0 - self.kill_step * n as f32).max(0.0);
        self.g = g1;
        (g0, (g1 - g0) / n.max(1) as f32)
    }
    /// Add mono `x` into `l`/`r` with the panned, ramped gain.
    #[inline(always)]
    pub fn mono(&mut self, x: &[f32], l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(x.len());
        let (g0, dg) = self.ramp(n);
        let (pl, pr) = (self.pl, self.pr);
        for i in 0..n {
            let g = g0 + dg * i as f32;
            l[i] += x[i] * g * pl;
            r[i] += x[i] * g * pr;
        }
    }
    /// Add stereo `xl`/`xr` into `l`/`r` (pan acts as balance).
    #[inline(always)]
    pub fn stereo(&mut self, xl: &[f32], xr: &[f32], l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(xl.len()).min(xr.len());
        let (g0, dg) = self.ramp(n);
        let (pl, pr) = (self.pl, self.pr);
        for i in 0..n {
            let g = g0 + dg * i as f32;
            l[i] += xl[i] * g * pl;
            r[i] += xr[i] * g * pr;
        }
    }
}

/// Peak of a slice (vectorises).
#[inline(always)]
pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// Counts consecutive quiet frames; a voice is over once the output stayed
/// under [`SILENCE`] for `hold` frames (≥ one period of its lowest component).
#[derive(Clone, Copy, Debug)]
pub struct Quiet {
    pub frames: u32,
    pub hold: u32,
}

impl Quiet {
    pub fn new(hold: u32) -> Quiet {
        Quiet { frames: 0, hold }
    }
    /// Feed one block's peak (already including output gain); returns true when silent for good.
    #[inline]
    pub fn feed(&mut self, peak: f32, n: usize) -> bool {
        if peak < SILENCE {
            self.frames += n as u32;
        } else {
            self.frames = 0;
        }
        self.frames >= self.hold
    }
}

/// Velocity → level curve `v^1.5` (−18 dB at 0.25, −2.9 dB at 0.8, 0 dB at 1).
#[inline]
pub fn vel_gain(vel: f32) -> f32 {
    let v = vel.clamp(0.0, 1.0);
    v * v.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sine_accuracy() {
        let mut e = 0.0f32;
        for i in -4000..4000 {
            let t = i as f32 / 997.0;
            e = e.max((sin2pi(t) - (2.0 * std::f32::consts::PI * t).sin()).abs());
        }
        assert!(e < 5e-5, "{e}");
        assert_eq!(fract(-0.25), 0.75);
        assert!(tanh(10.0) <= 1.0 && tanh(0.5) > 0.45);
    }
}
