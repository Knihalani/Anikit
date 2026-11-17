//! RBJ biquads (transposed direct form II, `f64` state), a multi-band [`Eq`]
//! built from compact JSON params, Butterworth high/low-pass cascades and a
//! zero-delay-feedback state-variable filter ([`Svf`]) for sweeps.

use serde::Deserialize;
use std::f64::consts::PI;

use super::util::flush64;
use super::Processor;

/// Biquad response type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
    AllPass,
}

/// Normalised biquad coefficients (`a0 = 1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coefs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Coefs {
    /// Identity filter.
    pub const IDENTITY: Coefs = Coefs { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    /// RBJ cookbook design. `q` is the quality factor (for shelves: the shelf
    /// slope expressed as Q, std::f32::consts::FRAC_1_SQRT_2 = Butterworth-like); `gain_db` is used by
    /// peak and shelf types. Frequencies are clamped inside (1 Hz, 0.49·rate).
    pub fn new(kind: FilterKind, rate: f32, freq: f32, q: f32, gain_db: f32) -> Coefs {
        let fs = rate as f64;
        let f = (freq as f64).clamp(1.0, fs * 0.49);
        let q = (q as f64).max(1e-3);
        let w = 2.0 * PI * f / fs;
        let (sw, cw) = w.sin_cos();
        let alpha = sw / (2.0 * q);
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let (b0, b1, b2, a0, a1, a2) = match kind {
            FilterKind::LowPass => ((1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            FilterKind::HighPass => ((1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            FilterKind::BandPass => (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            FilterKind::Notch => (1.0, -2.0 * cw, 1.0, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            FilterKind::AllPass => (1.0 - alpha, -2.0 * cw, 1.0 + alpha, 1.0 + alpha, -2.0 * cw, 1.0 - alpha),
            FilterKind::Peak => (1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a),
            FilterKind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cw + s),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cw),
                    a * ((a + 1.0) - (a - 1.0) * cw - s),
                    (a + 1.0) + (a - 1.0) * cw + s,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cw),
                    (a + 1.0) + (a - 1.0) * cw - s,
                )
            }
            FilterKind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cw + s),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cw),
                    a * ((a + 1.0) + (a - 1.0) * cw - s),
                    (a + 1.0) - (a - 1.0) * cw + s,
                    2.0 * ((a - 1.0) - (a + 1.0) * cw),
                    (a + 1.0) - (a - 1.0) * cw - s,
                )
            }
        };
        Coefs { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    /// Magnitude response (linear) at `freq`.
    pub fn magnitude(&self, freq: f32, rate: f32) -> f64 {
        let w = 2.0 * PI * freq as f64 / rate as f64;
        let (s1, c1) = (-w).sin_cos();
        let (s2, c2) = (-2.0 * w).sin_cos();
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt()
    }
}

/// Q values of the second-order sections of an `order`-pole Butterworth filter.
pub fn butterworth_qs(order: usize) -> Vec<f32> {
    let n = order.max(2) & !1;
    (1..=n / 2)
        .map(|k| (1.0 / (2.0 * ((2 * k - 1) as f64 * PI / (2 * n) as f64).sin())) as f32)
        .collect()
}

/// A stereo biquad section (TDF-II, `f64` state).
#[derive(Debug, Clone)]
pub struct Biquad {
    pub c: Coefs,
    s: [f64; 4],
}

impl Biquad {
    pub fn new(kind: FilterKind, rate: f32, freq: f32, q: f32, gain_db: f32) -> Biquad {
        Biquad::from_coefs(Coefs::new(kind, rate, freq, q, gain_db))
    }
    pub fn from_coefs(c: Coefs) -> Biquad {
        Biquad { c, s: [0.0; 4] }
    }
    /// Change coefficients, keeping state (safe for slow parameter changes).
    pub fn set(&mut self, c: Coefs) {
        self.c = c;
    }
    /// One sample on channel `ch` (0 = left, 1 = right).
    #[inline(always)]
    pub fn tick(&mut self, ch: usize, x: f32) -> f32 {
        let c = &self.c;
        let x = x as f64;
        let (z1, z2) = (self.s[ch * 2], self.s[ch * 2 + 1]);
        let y = c.b0 * x + z1;
        self.s[ch * 2] = flush64(c.b1 * x - c.a1 * y + z2);
        self.s[ch * 2 + 1] = flush64(c.b2 * x - c.a2 * y);
        y as f32
    }
    /// Filter a mono buffer in place (uses the left state).
    pub fn process_mono(&mut self, x: &mut [f32]) {
        let c = self.c;
        let (mut z1, mut z2) = (self.s[0], self.s[1]);
        for v in x.iter_mut() {
            let xi = *v as f64;
            let y = c.b0 * xi + z1;
            z1 = c.b1 * xi - c.a1 * y + z2;
            z2 = c.b2 * xi - c.a2 * y;
            *v = y as f32;
        }
        self.s[0] = flush64(z1);
        self.s[1] = flush64(z2);
    }
}

impl Processor for Biquad {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let c = self.c;
        let [mut z1l, mut z2l, mut z1r, mut z2r] = self.s;
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            let xl = *a as f64;
            let xr = *b as f64;
            let yl = c.b0 * xl + z1l;
            let yr = c.b0 * xr + z1r;
            z1l = c.b1 * xl - c.a1 * yl + z2l;
            z1r = c.b1 * xr - c.a1 * yr + z2r;
            z2l = c.b2 * xl - c.a2 * yl;
            z2r = c.b2 * xr - c.a2 * yr;
            *a = yl as f32;
            *b = yr as f32;
        }
        self.s = [flush64(z1l), flush64(z2l), flush64(z1r), flush64(z2r)];
    }
    fn reset(&mut self) {
        self.s = [0.0; 4];
    }
}

fn default_low_freq() -> f32 {
    120.0
}
fn default_high_freq() -> f32 {
    8000.0
}
fn default_shelf_q() -> f32 {
    std::f32::consts::FRAC_1_SQRT_2
}
fn default_slope() -> u32 {
    12
}

/// EQ parameters. Example:
/// `{"low": -2, "lowFreq": 120, "mid": [[400, -3, 1.0]], "high": 2, "highFreq": 8000, "hp": 30, "lp": 18000}`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct EqParams {
    /// Low-shelf gain in dB (0 = off).
    pub low: f32,
    /// Low-shelf corner (Hz).
    pub low_freq: f32,
    /// Peaking bands: `[freq Hz, gain dB, Q]`.
    pub mid: Vec<[f32; 3]>,
    /// High-shelf gain in dB (0 = off).
    pub high: f32,
    /// High-shelf corner (Hz).
    pub high_freq: f32,
    /// Shelf slope as Q (std::f32::consts::FRAC_1_SQRT_2 = Butterworth-like, no overshoot).
    pub shelf_q: f32,
    /// Spectral tilt in dB around a 1 kHz pivot (+ = brighter): −tilt/2 low shelf, +tilt/2 high shelf.
    pub tilt: f32,
    /// High-pass (low-cut) corner in Hz, Butterworth.
    pub hp: Option<f32>,
    /// High-pass slope in dB/octave (12, 24, 36 or 48).
    pub hp_slope: u32,
    /// Low-pass (high-cut) corner in Hz, Butterworth.
    pub lp: Option<f32>,
    /// Low-pass slope in dB/octave.
    pub lp_slope: u32,
    /// Output gain in dB.
    pub gain: f32,
}

impl Default for EqParams {
    fn default() -> Self {
        EqParams {
            low: 0.0,
            low_freq: default_low_freq(),
            mid: Vec::new(),
            high: 0.0,
            high_freq: default_high_freq(),
            shelf_q: default_shelf_q(),
            tilt: 0.0,
            hp: None,
            hp_slope: default_slope(),
            lp: None,
            lp_slope: default_slope(),
            gain: 0.0,
        }
    }
}

/// A multi-band stereo equaliser (cascade of biquads).
#[derive(Debug, Clone)]
pub struct Eq {
    sections: Vec<Biquad>,
    gain: f32,
    rate: f32,
}

impl Eq {
    pub fn new(rate: f32, p: &EqParams) -> Eq {
        let mut s = Vec::new();
        let mut push = |kind, f: f32, q: f32, g: f32| s.push(Biquad::new(kind, rate, f, q, g));
        if let Some(f) = p.hp.filter(|f| *f > 0.0) {
            for q in butterworth_qs((p.hp_slope / 6).clamp(2, 8) as usize) {
                push(FilterKind::HighPass, f, q, 0.0);
            }
        }
        if p.low != 0.0 {
            push(FilterKind::LowShelf, p.low_freq, p.shelf_q, p.low);
        }
        if p.tilt != 0.0 {
            push(FilterKind::LowShelf, 1000.0, 0.5, -p.tilt * 0.5);
            push(FilterKind::HighShelf, 1000.0, 0.5, p.tilt * 0.5);
        }
        for &[f, g, q] in &p.mid {
            if g != 0.0 {
                push(FilterKind::Peak, f, q.max(0.05), g);
            }
        }
        if p.high != 0.0 {
            push(FilterKind::HighShelf, p.high_freq, p.shelf_q, p.high);
        }
        if let Some(f) = p.lp.filter(|f| *f > 0.0 && *f < rate * 0.49) {
            for q in butterworth_qs((p.lp_slope / 6).clamp(2, 8) as usize) {
                push(FilterKind::LowPass, f, q, 0.0);
            }
        }
        Eq { sections: s, gain: super::util::db_to_gain(p.gain), rate }
    }

    /// Magnitude response in dB at `freq`.
    pub fn response_db(&self, freq: f32) -> f64 {
        let m: f64 = self.sections.iter().map(|b| b.c.magnitude(freq, self.rate)).product();
        20.0 * (m * self.gain as f64).max(1e-30).log10()
    }

    /// Number of biquad sections.
    pub fn sections(&self) -> usize {
        self.sections.len()
    }
}

impl Processor for Eq {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        // Chunked so all sections stay in cache for long buffers.
        for (cl, cr) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            for s in self.sections.iter_mut() {
                s.process(cl, cr);
            }
            if self.gain != 1.0 {
                let g = self.gain;
                cl.iter_mut().for_each(|x| *x *= g);
                cr.iter_mut().for_each(|x| *x *= g);
            }
        }
    }
    fn reset(&mut self) {
        self.sections.iter_mut().for_each(|s| s.reset());
    }
}

/// State-variable filter output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SvfMode {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    AllPass,
}

/// Zero-delay-feedback (topology-preserving) state-variable filter
/// (Zavalishin / Simper). Stable under fast modulation, so it is the filter of
/// choice for sweeps: call [`Svf::set`] per sample or per few samples.
#[derive(Debug, Clone)]
pub struct Svf {
    mode: SvfMode,
    rate: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    s: [[f32; 2]; 2],
}

impl Svf {
    pub fn new(rate: f32, mode: SvfMode, freq: f32, q: f32) -> Svf {
        let mut f = Svf { mode, rate, k: 1.0, a1: 0.0, a2: 0.0, a3: 0.0, s: [[0.0; 2]; 2] };
        f.set(freq, q);
        f
    }
    /// Set cutoff (Hz) and resonance Q (0.5 = no peak, std::f32::consts::FRAC_1_SQRT_2 = Butterworth, higher = resonant).
    #[inline]
    pub fn set(&mut self, freq: f32, q: f32) {
        let f = freq.clamp(5.0, self.rate * 0.49);
        let g = (std::f32::consts::PI * f / self.rate).tan();
        self.k = 1.0 / q.max(0.02);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// Change the output type.
    pub fn set_mode(&mut self, mode: SvfMode) {
        self.mode = mode;
    }
    /// One sample on channel `ch`, returning (low, band, high).
    #[inline(always)]
    pub fn tick3(&mut self, ch: usize, x: f32) -> (f32, f32, f32) {
        let [ic1, ic2] = self.s[ch];
        let v3 = x - ic2;
        let v1 = self.a1 * ic1 + self.a2 * v3;
        let v2 = ic2 + self.a2 * ic1 + self.a3 * v3;
        self.s[ch] = [super::util::flush(2.0 * v1 - ic1), super::util::flush(2.0 * v2 - ic2)];
        (v2, v1, x - self.k * v1 - v2)
    }
    /// One sample on channel `ch` for the current mode.
    #[inline(always)]
    pub fn tick(&mut self, ch: usize, x: f32) -> f32 {
        let (lp, bp, hp) = self.tick3(ch, x);
        match self.mode {
            SvfMode::LowPass => lp,
            SvfMode::HighPass => hp,
            SvfMode::BandPass => bp,
            SvfMode::Notch => lp + hp,
            SvfMode::Peak => lp - hp,
            SvfMode::AllPass => lp + hp - self.k * bp,
        }
    }
    /// Filter a mono buffer (left state).
    pub fn process_mono(&mut self, x: &mut [f32]) {
        for v in x.iter_mut() {
            *v = self.tick(0, *v);
        }
    }
    /// Filter a stereo block while sweeping the cutoff exponentially from
    /// `from` to `to` Hz across the block (coefficients updated every 8 samples).
    pub fn process_sweep(&mut self, l: &mut [f32], r: &mut [f32], from: f32, to: f32, q: f32) {
        let n = l.len().min(r.len());
        if n == 0 {
            return;
        }
        let lr = (to.max(1.0) / from.max(1.0)).ln();
        for (i, (cl, cr)) in l[..n].chunks_mut(8).zip(r[..n].chunks_mut(8)).enumerate() {
            let t = (i * 8) as f32 / n as f32;
            self.set(from * (lr * t).exp(), q);
            for (a, b) in cl.iter_mut().zip(cr.iter_mut()) {
                *a = self.tick(0, *a);
                *b = self.tick(1, *b);
            }
        }
    }
}

impl Processor for Svf {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (a, b) in l.iter_mut().zip(r.iter_mut()) {
            *a = self.tick(0, *a);
            *b = self.tick(1, *b);
        }
    }
    fn reset(&mut self) {
        self.s = [[0.0; 2]; 2];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(c: &Coefs, f: f32) -> f64 {
        20.0 * c.magnitude(f, 48_000.0).log10()
    }

    /// Measure the gain of a processor at `f` by filtering a sine.
    fn measured_db(p: &mut dyn Processor, f: f32) -> f64 {
        let n = 48_000;
        let mut l: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * f * i as f32 / 48_000.0).sin()).collect();
        let mut r = l.clone();
        p.process(&mut l, &mut r);
        let rms = |x: &[f32]| (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        20.0 * (rms(&l[24_000..]) / std::f64::consts::FRAC_1_SQRT_2).log10()
    }

    #[test]
    fn rbj_responses() {
        let fs = 48_000.0;
        let lp = Coefs::new(FilterKind::LowPass, fs, 1000.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
        assert!((db(&lp, 1000.0) + 3.01).abs() < 0.02);
        assert!(db(&lp, 50.0).abs() < 0.01);
        assert!(db(&lp, 10_000.0) < -38.0);
        let hp = Coefs::new(FilterKind::HighPass, fs, 100.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
        assert!((db(&hp, 100.0) + 3.01).abs() < 0.02);
        assert!(db(&hp, 10.0) < -39.0);
        let pk = Coefs::new(FilterKind::Peak, fs, 400.0, 1.0, -3.0);
        assert!((db(&pk, 400.0) + 3.0).abs() < 1e-6);
        assert!(db(&pk, 20.0).abs() < 0.05);
        let ls = Coefs::new(FilterKind::LowShelf, fs, 120.0, std::f32::consts::FRAC_1_SQRT_2, -2.0);
        assert!((db(&ls, 10.0) + 2.0).abs() < 0.02);
        assert!((db(&ls, 120.0) + 1.0).abs() < 0.02);
        assert!(db(&ls, 5000.0).abs() < 0.01);
        let hs = Coefs::new(FilterKind::HighShelf, fs, 8000.0, std::f32::consts::FRAC_1_SQRT_2, 2.0);
        assert!((db(&hs, 8000.0) - 1.0).abs() < 0.02);
        assert!((db(&hs, 23_000.0) - 2.0).abs() < 0.05);
        let bp = Coefs::new(FilterKind::BandPass, fs, 2000.0, 2.0, 0.0);
        assert!(db(&bp, 2000.0).abs() < 1e-6);
        let notch = Coefs::new(FilterKind::Notch, fs, 1000.0, 2.0, 0.0);
        assert!(db(&notch, 1000.0) < -100.0);
        let ap = Coefs::new(FilterKind::AllPass, fs, 1000.0, 2.0, 0.0);
        assert!(db(&ap, 300.0).abs() < 1e-9 && db(&ap, 7000.0).abs() < 1e-9);
    }

    #[test]
    fn eq_from_json() {
        let p: EqParams = serde_json::from_str(
            r#"{"low": -2, "lowFreq": 120, "mid": [[400, -3, 1.0]], "high": 2, "highFreq": 8000, "hp": 30, "lp": 18000}"#,
        )
        .unwrap();
        let mut eq = Eq::new(48_000.0, &p);
        assert_eq!(eq.sections(), 5);
        assert!((eq.response_db(30.0) + 4.9).abs() < 0.3, "{}", eq.response_db(30.0));
        assert!(eq.response_db(10.0) < -18.0);
        assert!((eq.response_db(400.0) + 3.0).abs() < 0.3);
        assert!((eq.response_db(1000.0)).abs() < 1.0);
        // The sine measurement agrees with the analytic response.
        for f in [60.0, 400.0, 1000.0, 9000.0] {
            let a = eq.response_db(f);
            let m = measured_db(&mut eq, f);
            eq.reset();
            assert!((a - m).abs() < 0.05, "{f}: {a} vs {m}");
        }
        assert!(serde_json::from_str::<EqParams>(r#"{"lows": 1}"#).is_err());
    }

    #[test]
    fn butterworth_cascade() {
        let p = EqParams { hp: Some(100.0), hp_slope: 24, ..Default::default() };
        let eq = Eq::new(48_000.0, &p);
        assert_eq!(eq.sections(), 2);
        assert!((eq.response_db(100.0) + 3.01).abs() < 0.02);
        assert!((eq.response_db(50.0) + 24.1).abs() < 0.3, "{}", eq.response_db(50.0));
        assert!(eq.response_db(1000.0).abs() < 0.001);
    }

    #[test]
    fn svf_matches_theory() {
        let mut f = Svf::new(48_000.0, SvfMode::LowPass, 1000.0, std::f32::consts::FRAC_1_SQRT_2);
        assert!((measured_db(&mut f, 1000.0) + 3.01).abs() < 0.05);
        f.reset();
        assert!(measured_db(&mut f, 100.0).abs() < 0.05);
        f.reset();
        assert!(measured_db(&mut f, 10_000.0) < -25.0);
        let mut h = Svf::new(48_000.0, SvfMode::HighPass, 1000.0, std::f32::consts::FRAC_1_SQRT_2);
        assert!((measured_db(&mut h, 1000.0) + 3.01).abs() < 0.05);
        // A fast, wide sweep stays bounded.
        let mut s = Svf::new(48_000.0, SvfMode::BandPass, 100.0, 8.0);
        let mut l = vec![0.0f32; 48_000];
        let mut rng = crate::voice::Rng::new(1);
        l.iter_mut().for_each(|x| *x = rng.bi());
        let mut r = l.clone();
        s.process_sweep(&mut l, &mut r, 50.0, 20_000.0, 8.0);
        assert!(l.iter().all(|x| x.is_finite() && x.abs() < 20.0));
    }
}
