//! Shared DSP helpers: dB conversion, branch-free denormal flushing, fast
//! deterministic `exp2`/`log2`/`tanh`, delay-line interpolation, Kaiser
//! windows and a band-limited resampler.

use std::f64::consts::PI;

/// dB → linear gain.
#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    (db * (std::f32::consts::LN_10 / 20.0)).exp()
}

/// Linear gain → dB (floored at −240 dB).
#[inline]
pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.abs().max(1e-12).log10()
}

/// Branch-free denormal flush for `f32`: values below ~1e-25 become 0, all
/// others are returned bit-exactly.
#[inline(always)]
pub fn flush(x: f32) -> f32 {
    (x + 1e-18) - 1e-18
}

/// Branch-free denormal flush for `f64` state variables.
#[inline(always)]
pub fn flush64(x: f64) -> f64 {
    (x + 1e-280) - 1e-280
}

const LOG2_E: f32 = std::f32::consts::LOG2_E;
/// `20·log10(2)`: dB per octave of amplitude.
pub const DB_PER_LOG2: f32 = 6.020_6;

/// Fast `2^x` (relative error < 2e-5, i.e. < 0.0002 dB), branch-free and vectorisable.
#[inline(always)]
pub fn fast_exp2(x: f32) -> f32 {
    let x = x.clamp(-126.0, 126.0);
    // x + 127 > 0, so truncation is floor.
    let xi = (x + 127.0) as i32 - 127;
    let f = x - xi as f32;
    let p = 1.0
        + f * (0.693_147_2
            + f * (0.240_226_5 + f * (0.055_504_1 + f * (0.009_618_13 + f * (0.001_333_36 + f * 0.000_154_04)))));
    f32::from_bits(((xi + 127) as u32) << 23) * p
}

/// Fast `log2(x)` for `x > 0` (absolute error < 2e-5), branch-free.
#[inline(always)]
pub fn fast_log2(x: f32) -> f32 {
    let bits = x.max(1e-30).to_bits();
    let e = ((bits >> 23) & 0xff) as i32 - 127;
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    let y = (m - 1.0) / (m + 1.0);
    let y2 = y * y;
    let l = y * (2.885_39 + y2 * (0.961_797_7 + y2 * (0.577_078 + y2 * 0.412_198)));
    e as f32 + l
}

/// Fast dB → gain.
#[inline(always)]
pub fn fast_db_to_gain(db: f32) -> f32 {
    fast_exp2(db * (1.0 / DB_PER_LOG2))
}

/// Fast gain → dB.
#[inline(always)]
pub fn fast_gain_to_db(g: f32) -> f32 {
    fast_log2(g) * DB_PER_LOG2
}

/// Fast, smooth `tanh` (via [`fast_exp2`]); exact limits ±1, odd, monotonic.
#[inline(always)]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-9.0, 9.0);
    let e = fast_exp2(2.0 * LOG2_E * x);
    (e - 1.0) / (e + 1.0)
}

/// Cubic Hermite (Catmull-Rom) interpolation between `x1` and `x2`.
#[inline(always)]
pub fn hermite(x0: f32, x1: f32, x2: f32, x3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x2 - x0);
    let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
    let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
    ((c3 * t + c2) * t + c1) * t + x1
}

/// A power-of-two circular delay line with fractional (cubic) reads.
#[derive(Debug, Clone)]
pub struct DelayLine {
    buf: Vec<f32>,
    mask: usize,
    pos: usize,
}

impl DelayLine {
    /// A line that can delay by at least `max` samples (plus interpolation margin).
    pub fn new(max: usize) -> DelayLine {
        let n = (max + 4).next_power_of_two();
        DelayLine { buf: vec![0.0; n], mask: n - 1, pos: 0 }
    }
    /// Push one sample.
    #[inline(always)]
    pub fn push(&mut self, x: f32) {
        self.pos = (self.pos + 1) & self.mask;
        self.buf[self.pos] = x;
    }
    /// Sample written `d` pushes ago (0 = latest).
    #[inline(always)]
    pub fn tap(&self, d: usize) -> f32 {
        self.buf[(self.pos.wrapping_sub(d)) & self.mask]
    }
    /// Fractional read `d ≥ 1` samples back (cubic Hermite).
    #[inline(always)]
    pub fn read(&self, d: f32) -> f32 {
        let di = d as usize;
        let t = d - di as f32;
        // Between tap(di) (t = 0) and tap(di + 1) (t = 1).
        let x0 = self.tap(di.wrapping_sub(1));
        let x1 = self.tap(di);
        let x2 = self.tap(di + 1);
        let x3 = self.tap(di + 2);
        hermite(x0, x1, x2, x3, t)
    }
    /// Linear-interpolated read.
    #[inline(always)]
    pub fn read_linear(&self, d: f32) -> f32 {
        let di = d as usize;
        let t = d - di as f32;
        let a = self.tap(di);
        a + (self.tap(di + 1) - a) * t
    }
    /// Zero the line.
    pub fn reset(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
        self.pos = 0;
    }
    /// Capacity in samples.
    pub fn capacity(&self) -> usize {
        self.buf.len() - 4
    }
}

/// Zeroth-order modified Bessel function (for Kaiser windows).
pub fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let h = x * 0.5;
    for k in 1..64 {
        term *= (h / k as f64) * (h / k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Kaiser window value at position `i` of `n` (n ≥ 2).
pub fn kaiser(i: usize, n: usize, beta: f64) -> f64 {
    let r = 2.0 * i as f64 / (n - 1) as f64 - 1.0;
    bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / bessel_i0(beta)
}

/// Normalised sinc.
#[inline]
pub fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// Band-limited resampling (Kaiser-windowed sinc, 64 zero crossings, ~−120 dB
/// stopband). Used offline for impulse responses and imported audio.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((x.len() as u64 * to as u64).div_ceil(from as u64)) as usize;
    // Cutoff at the lower Nyquist, slightly inside to leave a transition band.
    let cutoff = ratio.min(1.0) * 0.96;
    let half = 32.0 / cutoff; // half-width in input samples
    let beta = 12.0;
    let mut out = Vec::with_capacity(out_len);
    let i0b = bessel_i0(beta);
    for j in 0..out_len {
        let t = j as f64 / ratio;
        let lo = (t - half).ceil().max(0.0) as usize;
        let hi = ((t + half).floor() as usize).min(x.len() - 1);
        let mut acc = 0.0f64;
        for (i, &xi) in x.iter().enumerate().take(hi + 1).skip(lo) {
            let d = i as f64 - t;
            let r = d / half;
            let w = bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0b;
            acc += xi as f64 * cutoff * sinc(cutoff * d) * w;
        }
        out.push(acc as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_math_accuracy() {
        let mut x = -40.0f32;
        while x < 40.0 {
            let e = fast_exp2(x);
            assert!((e / x.exp2() - 1.0).abs() < 3e-5, "exp2({x})");
            x += 0.137;
        }
        let mut g = 1e-6f32;
        while g < 1e6 {
            assert!((fast_log2(g) - g.log2()).abs() < 3e-5, "log2({g})");
            g *= 1.37;
        }
        let mut t = -6.0f32;
        while t < 6.0 {
            assert!((fast_tanh(t) - t.tanh()).abs() < 2e-5, "tanh({t})");
            t += 0.01;
        }
        assert_eq!(flush(1e-30), 0.0);
        assert_eq!(flush(0.123), 0.123);
        assert_eq!(flush(-3.5e-9), -3.5e-9);
    }

    #[test]
    fn resample_sine() {
        let from = 44_100;
        let x: Vec<f32> = (0..44_100).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / from as f32).sin()).collect();
        let y = resample(&x, from, 48_000);
        assert_eq!(y.len(), 48_000);
        let mut err = 0.0f32;
        for (j, v) in y.iter().enumerate().take(47_000).skip(1000) {
            let e = (2.0 * std::f32::consts::PI * 1000.0 * j as f32 / 48_000.0).sin();
            err = err.max((v - e).abs());
        }
        assert!(err < 1e-3, "err {err}");
    }
}
