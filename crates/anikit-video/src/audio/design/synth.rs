//! Small synthesis toolkit for the sound-design generators: noise colours,
//! damped modal resonators, envelopes, equal-power panning, a reverb send and
//! normalisation. Everything is planar stereo `f32` and deterministic.

use anikit_music::dsp::biquad::{Biquad, FilterKind, Svf, SvfMode};
use anikit_music::dsp::reverb::{Reverb, ReverbParams};
use anikit_music::dsp::Processor;
use anikit_music::Rng;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Planar stereo buffer.
#[derive(Clone, Debug, Default)]
pub struct St {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
}

impl St {
    pub fn new(n: usize) -> St {
        St {
            l: vec![0.0; n],
            r: vec![0.0; n],
        }
    }
    pub fn len(&self) -> usize {
        self.l.len()
    }
    pub fn is_empty(&self) -> bool {
        self.l.is_empty()
    }
    /// Grow (with silence) to at least `n` frames.
    pub fn ensure(&mut self, n: usize) {
        if self.l.len() < n {
            self.l.resize(n, 0.0);
            self.r.resize(n, 0.0);
        }
    }
    /// Add a mono signal at `at`, panned (−1..1, equal power) with gain `g`.
    pub fn add_mono(&mut self, at: usize, x: &[f32], pan: f32, g: f32) {
        self.ensure(at + x.len());
        let (gl, gr) = pan_gains(pan);
        for (i, &s) in x.iter().enumerate() {
            self.l[at + i] += s * gl * g;
            self.r[at + i] += s * gr * g;
        }
    }
    /// Add a mono signal whose pan moves linearly from `p0` to `p1`.
    pub fn add_moving(&mut self, at: usize, x: &[f32], p0: f32, p1: f32, g: f32) {
        self.ensure(at + x.len());
        let n = x.len().max(2) as f32 - 1.0;
        for (i, &s) in x.iter().enumerate() {
            let (gl, gr) = pan_gains(p0 + (p1 - p0) * i as f32 / n);
            self.l[at + i] += s * gl * g;
            self.r[at + i] += s * gr * g;
        }
    }
    pub fn add(&mut self, at: usize, o: &St, g: f32) {
        self.ensure(at + o.len());
        for i in 0..o.len() {
            self.l[at + i] += o.l[i] * g;
            self.r[at + i] += o.r[i] * g;
        }
    }
    pub fn peak(&self) -> f32 {
        self.l
            .iter()
            .chain(&self.r)
            .fold(0.0f32, |m, s| m.max(s.abs()))
    }
    pub fn rms(&self) -> f32 {
        let n = (self.l.len() * 2).max(1) as f64;
        ((self
            .l
            .iter()
            .chain(&self.r)
            .map(|s| (*s as f64) * (*s as f64))
            .sum::<f64>())
            / n)
            .sqrt() as f32
    }
    pub fn scale(&mut self, g: f32) {
        self.l
            .iter_mut()
            .chain(self.r.iter_mut())
            .for_each(|s| *s *= g);
    }
    /// Scale so the sample peak is `db` dBFS.
    pub fn normalize_peak(&mut self, db: f32) {
        let p = self.peak();
        if p > 1e-9 {
            self.scale(10f32.powf(db / 20.0) / p);
        }
    }
    /// Scale so the RMS is `db` dBFS.
    pub fn normalize_rms(&mut self, db: f32) {
        let r = self.rms();
        if r > 1e-12 {
            self.scale(10f32.powf(db / 20.0) / r);
        }
    }
    /// Trim trailing near-silence (below −80 dBFS), keeping at least `min` frames.
    pub fn trim_tail(&mut self, min: usize) {
        let mut n = self.len();
        while n > min && self.l[n - 1].abs().max(self.r[n - 1].abs()) < 1e-4 {
            n -= 1;
        }
        self.l.truncate(n);
        self.r.truncate(n);
    }
    /// Short fade over the last `n` frames (removes any click at the end).
    pub fn fade_end(&mut self, n: usize) {
        let len = self.len();
        let n = n.min(len);
        for k in 0..n {
            let g = (k as f32 + 0.5) / n as f32;
            self.l[len - 1 - k] *= g;
            self.r[len - 1 - k] *= g;
        }
    }
}

/// Equal-power pan gains for −1 (left) … 1 (right).
pub fn pan_gains(p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * 0.25 * PI;
    (
        a.cos() * std::f32::consts::SQRT_2,
        a.sin() * std::f32::consts::SQRT_2,
    )
}

/// Seconds → frames.
pub fn frames(sec: f32, sr: f32) -> usize {
    (sec.max(0.0) * sr).round() as usize
}

/// Noise generator: white, pink (Kellet) and brown, all deterministic.
pub struct Noise {
    rng: Rng,
    b: [f32; 7],
    brown: f32,
}

impl Noise {
    pub fn new(seed: u64) -> Noise {
        Noise {
            rng: Rng::new(seed),
            b: [0.0; 7],
            brown: 0.0,
        }
    }
    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }
    /// Uniform −1..1.
    #[inline]
    pub fn white(&mut self) -> f32 {
        self.rng.bi()
    }
    /// Pink (−3 dB/oct), roughly −1..1.
    #[inline]
    pub fn pink(&mut self) -> f32 {
        let w = self.rng.bi();
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + w * 0.0555179;
        b[1] = 0.99332 * b[1] + w * 0.0750759;
        b[2] = 0.96900 * b[2] + w * 0.153_852;
        b[3] = 0.86650 * b[3] + w * 0.3104856;
        b[4] = 0.55000 * b[4] + w * 0.5329522;
        b[5] = -0.7616 * b[5] - w * 0.0168980;
        let o = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
        b[6] = w * 0.115926;
        o * 0.11
    }
    /// Brown (−6 dB/oct), leaky integrator, roughly −1..1.
    #[inline]
    pub fn brown(&mut self) -> f32 {
        self.brown = (self.brown + 0.02 * self.rng.bi()) * 0.998;
        self.brown * 3.5
    }
    pub fn f(&mut self) -> f32 {
        self.rng.f32()
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        self.rng.range(a, b)
    }
}

/// A smooth random control signal: white noise through a one-pole low-pass at
/// `hz`, scaled to roughly −1..1.
pub struct Wander {
    y: f32,
    y2: f32,
    k: f32,
    gain: f32,
}

impl Wander {
    pub fn new(hz: f32, sr: f32) -> Wander {
        let k = 1.0 - (-TAU * hz / sr).exp();
        Wander {
            y: 0.0,
            y2: 0.0,
            k,
            gain: (2.0 / k).sqrt() * 0.9,
        }
    }
    #[inline]
    pub fn next(&mut self, n: &mut Noise) -> f32 {
        self.y += (n.white() - self.y) * self.k;
        self.y2 += (self.y - self.y2) * self.k;
        (self.y2 * self.gain).clamp(-1.0, 1.0)
    }
}

/// Bank of two-pole damped resonators (modal synthesis): each mode rings at
/// `freq` with amplitude `amp` and a −60 dB decay time of `t60` seconds when
/// struck by the excitation.
pub fn modal(exc: &[f32], modes: &[(f32, f32, f32)], sr: f32, len: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; len];
    for &(f, amp, t60) in modes {
        if f <= 20.0 || f >= sr * 0.45 || amp == 0.0 {
            continue;
        }
        let r = (-6.9078 / (t60.max(1e-3) * sr)).exp();
        let w = TAU * f / sr;
        let a1 = 2.0 * r * w.cos();
        let a2 = r * r;
        // Normalise so a unit impulse rings at ≈ amp.
        let g = amp * w.sin().max(1e-3);
        let (mut y1, mut y2) = (0.0f32, 0.0f32);
        for (i, o) in out.iter_mut().enumerate() {
            let x = if i < exc.len() { exc[i] } else { 0.0 };
            let y = g * x + a1 * y1 - a2 * y2;
            y2 = y1;
            y1 = if y.abs() < 1e-20 { 0.0 } else { y };
            *o += y;
        }
    }
    out
}

/// A short noise burst (strike excitation): `ms` long, low-passed at `bright` Hz.
pub fn strike(n: &mut Noise, ms: f32, bright: f32, sr: f32) -> Vec<f32> {
    let len = frames(ms / 1000.0, sr).max(2);
    let mut lp = Biquad::new(FilterKind::LowPass, sr, bright.min(sr * 0.45), 0.707, 0.0);
    (0..len)
        .map(|i| {
            let e = 1.0 - i as f32 / len as f32;
            lp.tick(0, n.white() * e * e)
        })
        .collect()
}

/// Attack/decay envelope value at sample `i`: linear attack over `a` samples,
/// exponential decay with time constant `tau` samples afterwards.
#[inline]
pub fn env_ad(i: usize, a: usize, tau: f32) -> f32 {
    if i < a {
        i as f32 / a.max(1) as f32
    } else {
        (-((i - a) as f32) / tau.max(1.0)).exp()
    }
}

/// A bell (raised-cosine) envelope over `n` samples whose peak sits at
/// fraction `peak` (0..1), with sharpness `pow`.
#[inline]
pub fn bell(i: usize, n: usize, peak: f32, pow: f32) -> f32 {
    let x = i as f32 / n.max(1) as f32;
    let p = peak.clamp(0.05, 0.95);
    let u = if x < p {
        x / p
    } else {
        1.0 - (x - p) / (1.0 - p)
    };
    (0.5 - 0.5 * (PI * u.clamp(0.0, 1.0)).cos()).powf(pow)
}

/// Band-passed noise whose centre follows `centre(i)` (Hz), Q `q`, through a
/// zero-delay SVF (stable under per-sample modulation).
pub fn swept_noise(
    n: &mut Noise,
    len: usize,
    sr: f32,
    q: f32,
    pink: bool,
    mut centre: impl FnMut(usize) -> f32,
    mut amp: impl FnMut(usize) -> f32,
) -> Vec<f32> {
    let mut f = Svf::new(sr, SvfMode::BandPass, 1000.0, q);
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if i % 16 == 0 {
            f.set(centre(i).clamp(30.0, sr * 0.45), q);
        }
        let x = if pink { n.pink() * 2.5 } else { n.white() };
        out.push(f.tick(0, x) * amp(i));
    }
    out
}

/// One-pole filters for quick tone shaping.
pub fn lowpass(x: &mut [f32], hz: f32, sr: f32) {
    let mut b = Biquad::new(FilterKind::LowPass, sr, hz.min(sr * 0.45), 0.707, 0.0);
    for s in x.iter_mut() {
        *s = b.tick(0, *s);
    }
}
pub fn highpass(x: &mut [f32], hz: f32, sr: f32) {
    let mut b = Biquad::new(FilterKind::HighPass, sr, hz.min(sr * 0.45), 0.707, 0.0);
    for s in x.iter_mut() {
        *s = b.tick(0, *s);
    }
}
pub fn bandpass(x: &mut [f32], hz: f32, q: f32, sr: f32) {
    let mut b = Biquad::new(FilterKind::BandPass, sr, hz.min(sr * 0.45), q, 0.0);
    for s in x.iter_mut() {
        *s = b.tick(0, *s);
    }
}
pub fn lowpass_st(st: &mut St, hz: f32, sr: f32) {
    let mut b = Biquad::new(FilterKind::LowPass, sr, hz.min(sr * 0.45), 0.707, 0.0);
    for i in 0..st.len() {
        st.l[i] = b.tick(0, st.l[i]);
        st.r[i] = b.tick(1, st.r[i]);
    }
}
pub fn highpass_st(st: &mut St, hz: f32, sr: f32) {
    let mut b = Biquad::new(FilterKind::HighPass, sr, hz.min(sr * 0.45), 0.707, 0.0);
    for i in 0..st.len() {
        st.l[i] = b.tick(0, st.l[i]);
        st.r[i] = b.tick(1, st.r[i]);
    }
}

/// Add a reverb tail (wet only, `wet` gain) from a preset, extending the
/// buffer by `tail` seconds.
pub fn reverb(st: &mut St, preset: &str, size: f32, wet: f32, tail: f32, sr: f32) {
    if wet <= 1e-3 {
        return;
    }
    let n0 = st.len();
    st.ensure(n0 + frames(tail, sr));
    let p = ReverbParams {
        preset: Some(preset.into()),
        size: Some(size.clamp(0.1, 2.0)),
        ..Default::default()
    };
    let mut rv = Reverb::new(sr, &p);
    rv.set_mix(1.0, 0.0);
    let (mut l, mut r) = (st.l.clone(), st.r.clone());
    rv.process(&mut l, &mut r);
    for i in 0..st.len() {
        st.l[i] += l[i] * wet;
        st.r[i] += r[i] * wet;
    }
}

/// Sine oscillator with an exponential pitch glide from `f0` to `f1` over
/// `glide` samples, amplitude `env(i)`.
pub fn sine_glide(
    len: usize,
    sr: f32,
    f0: f32,
    f1: f32,
    glide: usize,
    mut env: impl FnMut(usize) -> f32,
) -> Vec<f32> {
    let mut ph = 0.0f32;
    let ratio = (f1 / f0).max(1e-3);
    (0..len)
        .map(|i| {
            let k = (i as f32 / glide.max(1) as f32).min(1.0);
            let f = f0 * ratio.powf(k);
            ph += TAU * f / sr;
            if ph > TAU {
                ph -= TAU;
            }
            ph.sin() * env(i)
        })
        .collect()
}

/// Soft clip.
#[inline]
pub fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

/// Equal-power crossfade gain (0..1 → 0..1 along a quarter sine).
#[inline]
pub fn eqp(x: f32) -> f32 {
    (x.clamp(0.0, 1.0) * FRAC_PI_2).sin()
}
