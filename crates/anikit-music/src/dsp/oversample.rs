//! Polyphase half-band 2× up/down-samplers (Kaiser-windowed, linear phase).
//! Cascade two for 4×. Used by the saturator to keep harmonics from aliasing.

use super::util::{kaiser, sinc};

/// Design the non-trivial (even-index) taps of a half-band low-pass with
/// `n = 4m + 3` taps. The centre tap (0.5) is implicit.
fn halfband(m: usize, beta: f64) -> Vec<f32> {
    let n = 4 * m + 3;
    let c = (n - 1) / 2;
    let mut taps: Vec<f64> = (0..n)
        .step_by(2)
        .map(|i| {
            let k = i as f64 - c as f64;
            0.5 * sinc(k / 2.0) * kaiser(i, n, beta)
        })
        .collect();
    // Normalise the polyphase branch to exactly 0.5 DC gain.
    let s: f64 = taps.iter().sum();
    taps.iter_mut().for_each(|t| *t *= 0.5 / s);
    taps.into_iter().map(|t| t as f32).collect()
}

/// Quality presets: (m, beta). Stage "hq" ~ −110 dB stopband, transition 20→28 kHz at 48 kHz in.
fn design(hq: bool) -> (usize, f64) {
    if hq {
        (21, 11.0)
    } else {
        (6, 9.0)
    }
}

/// 2× half-band interpolator. `hq` selects the steep filter for the first
/// stage (input at the base rate); the cheap one suffices for a second stage.
#[derive(Debug, Clone)]
pub struct Upsampler2 {
    taps: Vec<f32>,
    hist: Vec<f32>,
    pos: usize,
    m: usize,
}

impl Upsampler2 {
    pub fn new(hq: bool) -> Upsampler2 {
        let (m, beta) = design(hq);
        let taps = halfband(m, beta);
        let k = taps.len();
        Upsampler2 { taps, hist: vec![0.0; 2 * k], pos: 0, m }
    }
    /// Latency in output (2×) samples.
    pub fn latency2(&self) -> usize {
        2 * self.m + 1
    }
    /// `out.len()` must be `2 · x.len()`.
    #[inline]
    pub fn process(&mut self, x: &[f32], out: &mut [f32]) {
        let k = self.taps.len();
        for (i, &v) in x.iter().enumerate() {
            self.pos = if self.pos == 0 { k - 1 } else { self.pos - 1 };
            self.hist[self.pos] = v;
            self.hist[self.pos + k] = v;
            let h = &self.hist[self.pos..self.pos + k];
            let mut acc = 0.0f32;
            for (a, b) in h.iter().zip(&self.taps) {
                acc += a * b;
            }
            out[2 * i] = 2.0 * acc;
            out[2 * i + 1] = h[self.m];
        }
    }
    pub fn reset(&mut self) {
        self.hist.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// 2× half-band decimator.
#[derive(Debug, Clone)]
pub struct Downsampler2 {
    taps: Vec<f32>,
    even: Vec<f32>,
    odd: Vec<f32>,
    pos: usize,
    m: usize,
}

impl Downsampler2 {
    pub fn new(hq: bool) -> Downsampler2 {
        let (m, beta) = design(hq);
        let taps = halfband(m, beta);
        let k = taps.len();
        Downsampler2 { taps, even: vec![0.0; 2 * k], odd: vec![0.0; 2 * k], pos: 0, m }
    }
    /// Latency in input (2×) samples.
    pub fn latency2(&self) -> usize {
        2 * self.m + 1
    }
    /// `x.len()` must be `2 · out.len()`.
    #[inline]
    pub fn process(&mut self, x: &[f32], out: &mut [f32]) {
        let k = self.taps.len();
        for (o, pair) in out.iter_mut().zip(x.chunks_exact(2)) {
            // `odd` lags by one so the centre tap reads v[2n − K].
            let prev_odd_pos = self.pos;
            self.pos = if self.pos == 0 { k - 1 } else { self.pos - 1 };
            self.even[self.pos] = pair[0];
            self.even[self.pos + k] = pair[0];
            let h = &self.even[self.pos..self.pos + k];
            let mut acc = 0.0f32;
            for (a, b) in h.iter().zip(&self.taps) {
                acc += a * b;
            }
            // odd history: odd[j] = v[2j+1]; centre needs odd[n − m − 1].
            let c = self.odd[prev_odd_pos + self.m];
            *o = acc + 0.5 * c;
            self.odd[self.pos] = pair[1];
            self.odd[self.pos + k] = pair[1];
        }
    }
    pub fn reset(&mut self) {
        self.even.iter_mut().for_each(|x| *x = 0.0);
        self.odd.iter_mut().for_each(|x| *x = 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_is_delayed_identity() {
        for hq in [true, false] {
            let mut up = Upsampler2::new(hq);
            let mut down = Downsampler2::new(hq);
            let n = 4096;
            let x: Vec<f32> = (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5 + (i as f32 * 0.31).sin() * 0.3).collect();
            let mut u = vec![0.0; 2 * n];
            let mut y = vec![0.0; n];
            up.process(&x, &mut u);
            down.process(&u, &mut y);
            let d = (up.latency2() + down.latency2()) / 2;
            assert_eq!((up.latency2() + down.latency2()) % 2, 0);
            let mut err = 0.0f32;
            for i in 200..n - 200 {
                err = err.max((y[i] - x[i - d]).abs());
            }
            assert!(err < if hq { 2e-4 } else { 3e-3 }, "hq={hq} err={err}");
        }
    }

    #[test]
    fn image_rejection() {
        // A 15 kHz tone upsampled to 96 kHz must not leave an image at 33 kHz.
        let mut up = Upsampler2::new(true);
        let n = 8192;
        let x: Vec<f32> = (0..n).map(|i| (2.0 * std::f64::consts::PI * 15_000.0 * i as f64 / 48_000.0).sin() as f32).collect();
        let mut u = vec![0.0; 2 * n];
        up.process(&x, &mut u);
        // Correlate (Hann-windowed) against the image frequency.
        let m = (u.len() - 200) as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, v) in u.iter().enumerate().skip(200) {
            let ph = 2.0 * std::f64::consts::PI * 33_000.0 * i as f64 / 96_000.0;
            let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * (i - 200) as f64 / m).cos();
            re += w * *v as f64 * ph.cos();
            im += w * *v as f64 * ph.sin();
        }
        let amp = 4.0 * (re * re + im * im).sqrt() / m;
        assert!(20.0 * amp.log10() < -95.0, "image {} dB", 20.0 * amp.log10());
    }
}
