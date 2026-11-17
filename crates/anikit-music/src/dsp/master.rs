//! Mastering: a true-peak look-ahead [`Limiter`] and the master chain
//! ([`master`] / [`master_with`]): low-cut + tilt EQ → glue compressor →
//! optional saturation → loudness-targeted gain → limiter, iterated until the
//! integrated loudness is within ±0.3 LU of the target and the true peak is at
//! or below the ceiling, as verified by the BS.1770 meter.

use serde::{Deserialize, Serialize};

use super::biquad::{Eq, EqParams};
use super::comp::{Compressor, CompressorParams, Detector};
use super::loudness::{measure, Loudness, TruePeak, TP_HALF};
use super::saturate::{Saturator, SaturatorParams};
use super::util::{db_to_gain, gain_to_db};
use super::{process_aligned, Processor};

/// Limiter parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct LimiterParams {
    /// Output ceiling in dBTP (or dBFS with `truePeak: false`).
    pub ceiling: f32,
    /// Look-ahead / attack time (ms). The gain ramps down smoothly over it.
    pub lookahead: f32,
    /// Release time constant (ms).
    pub release: f32,
    /// Detect inter-sample peaks (4× oversampled).
    pub true_peak: bool,
}

impl Default for LimiterParams {
    fn default() -> Self {
        LimiterParams { ceiling: -1.0, lookahead: 5.0, release: 120.0, true_peak: true }
    }
}

/// Brick-wall look-ahead limiter with true-peak detection.
///
/// Per input step the detector yields the peak `v` of a small time span; the
/// required gain `q = min(1, ceiling / v)` is turned into a smooth gain curve
/// by a sliding minimum over the look-ahead window, an exponential release and
/// a triangular (two box) smoother. The window covers the smoother's support
/// plus the detector span, so every output sample's gain is ≤ the bound of
/// every peak it contributes to: the gain is never late and never steps.
#[derive(Debug, Clone)]
pub struct Limiter {
    ceil: f32,
    /// Look-ahead (smoother support), detector span, window, delay.
    s: usize,
    d: usize,
    w: usize,
    a: usize,
    tp: [TruePeak; 2],
    true_peak: bool,
    /// Monotonic deque of (index, gain) for the sliding minimum.
    dq: Vec<(u64, f32)>,
    dq_head: usize,
    dq_len: usize,
    n: u64,
    rel: f32,
    r: f32,
    box1: Vec<f32>,
    box2: Vec<f32>,
    b1: (usize, f64),
    b2: (usize, f64),
    audio: [Vec<f32>; 2],
    apos: usize,
    min_gain: f32,
}

impl Limiter {
    pub fn new(rate: f32, p: &LimiterParams) -> Limiter {
        let s = ((p.lookahead.max(0.2) * 0.001 * rate).round() as usize).max(4);
        let d = if p.true_peak { TP_HALF } else { 0 };
        let w = s + d;
        let a = s + d - 1;
        let s1 = s / 2;
        let s2 = s - s1 + 1;
        Limiter {
            ceil: db_to_gain(p.ceiling),
            s,
            d,
            w,
            a,
            tp: [TruePeak::default(), TruePeak::default()],
            true_peak: p.true_peak,
            dq: vec![(0, 1.0); w + 2],
            dq_head: 0,
            dq_len: 0,
            n: 0,
            rel: (-1.0 / (p.release.max(1.0) * 0.001 * rate)).exp(),
            r: 1.0,
            box1: vec![1.0; s1],
            box2: vec![1.0; s2],
            b1: (0, s1 as f64),
            b2: (0, s2 as f64),
            audio: [vec![0.0; a + 1], vec![0.0; a + 1]],
            apos: 0,
            min_gain: 1.0,
        }
    }

    /// Change the ceiling (dB) without resetting state.
    pub fn set_ceiling(&mut self, db: f32) {
        self.ceil = db_to_gain(db);
    }

    /// Deepest gain reduction so far (dB, ≤ 0).
    pub fn max_reduction(&self) -> f32 {
        gain_to_db(self.min_gain)
    }

    #[inline]
    fn push_min(&mut self, q: f32) {
        let cap = self.dq.len();
        // Drop dominated entries from the back.
        while self.dq_len > 0 {
            let back = (self.dq_head + self.dq_len - 1) % cap;
            if self.dq[back].1 >= q {
                self.dq_len -= 1;
            } else {
                break;
            }
        }
        let slot = (self.dq_head + self.dq_len) % cap;
        self.dq[slot] = (self.n, q);
        self.dq_len += 1;
        // Expire entries older than the window.
        let oldest = (self.n + 1).saturating_sub(self.w as u64);
        while self.dq[self.dq_head].0 < oldest {
            self.dq_head = (self.dq_head + 1) % cap;
            self.dq_len -= 1;
        }
    }
}

impl Processor for Limiter {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let alen = self.a + 1;
        for (x, y) in l.iter_mut().zip(r.iter_mut()) {
            let mut v = x.abs().max(y.abs());
            if self.true_peak {
                v = v.max(self.tp[0].push(*x)).max(self.tp[1].push(*y));
            }
            let q = if v > self.ceil { self.ceil / v } else { 1.0 };
            self.push_min(q);
            let m = self.dq[self.dq_head].1;
            // Instant attack (the window already looks ahead), exponential release.
            self.r = if m < self.r { m } else { m + self.rel * (self.r - m) };
            // Two cascaded moving averages (triangular kernel of support `s`).
            let (i1, s1) = self.b1;
            let sum1 = s1 - self.box1[i1] as f64 + self.r as f64;
            self.box1[i1] = self.r;
            self.b1 = ((i1 + 1) % self.box1.len(), sum1);
            let g1 = (sum1 / self.box1.len() as f64) as f32;
            let (i2, s2) = self.b2;
            let sum2 = s2 - self.box2[i2] as f64 + g1 as f64;
            self.box2[i2] = g1;
            self.b2 = ((i2 + 1) % self.box2.len(), sum2);
            let g = ((sum2 / self.box2.len() as f64) as f32).min(1.0);
            self.min_gain = self.min_gain.min(g);
            // Delay the audio by `a` samples.
            let rd = (self.apos + 1) % alen;
            self.audio[0][self.apos] = *x;
            self.audio[1][self.apos] = *y;
            *x = self.audio[0][rd] * g;
            *y = self.audio[1][rd] * g;
            self.apos = rd;
            self.n += 1;
        }
        // Re-sum the box filters occasionally to cancel rounding drift.
        if self.n % 65_536 < l.len() as u64 {
            self.b1.1 = self.box1.iter().map(|v| *v as f64).sum();
            self.b2.1 = self.box2.iter().map(|v| *v as f64).sum();
        }
    }

    fn reset(&mut self) {
        self.tp = [TruePeak::default(), TruePeak::default()];
        self.dq_head = 0;
        self.dq_len = 0;
        self.n = 0;
        self.r = 1.0;
        self.box1.iter_mut().for_each(|v| *v = 1.0);
        self.box2.iter_mut().for_each(|v| *v = 1.0);
        self.b1 = (0, self.box1.len() as f64);
        self.b2 = (0, self.box2.len() as f64);
        self.audio.iter_mut().for_each(|b| b.iter_mut().for_each(|v| *v = 0.0));
        self.apos = 0;
        self.min_gain = 1.0;
    }

    fn latency(&self) -> usize {
        self.a
    }
}

impl Limiter {
    /// Look-ahead in samples (smoother support).
    pub fn lookahead(&self) -> usize {
        self.s
    }
    /// Detector span in samples.
    pub fn detector_span(&self) -> usize {
        self.d
    }
}

/// Master-chain parameters.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct MasterParams {
    /// Integrated loudness target (LUFS).
    pub target: f32,
    /// True-peak ceiling (dBTP).
    pub ceiling: f32,
    /// Low cut (Hz), 12 dB/oct; 0 disables.
    pub low_cut: f32,
    /// Spectral tilt (dB, + = brighter) around 1 kHz.
    pub tilt: f32,
    /// Extra EQ applied after the low cut / tilt.
    pub eq: Option<EqParams>,
    /// Glue compression on/off and its settings (levels relative to a
    /// programme normalised to −18 LUFS).
    pub glue: bool,
    pub glue_params: CompressorParams,
    /// Optional soft saturation.
    pub saturate: Option<SaturatorParams>,
    pub limiter: LimiterParams,
    /// Loudness tolerance (LU).
    pub tolerance: f32,
}

impl Default for MasterParams {
    fn default() -> Self {
        MasterParams {
            target: -14.0,
            ceiling: -1.0,
            low_cut: 25.0,
            tilt: 0.0,
            eq: None,
            glue: true,
            glue_params: CompressorParams {
                threshold: -21.0,
                ratio: 2.0,
                knee: 8.0,
                attack: 25.0,
                release: 250.0,
                makeup: 0.0,
                detector: Detector::Rms,
                rms_ms: 20.0,
                lookahead: 0.0,
                sc_hp: Some(90.0),
                mix: 1.0,
            },
            saturate: None,
            limiter: LimiterParams::default(),
            tolerance: 0.3,
        }
    }
}

/// What the master chain did and achieved.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MasterReport {
    /// Measurements before and after.
    pub input: Loudness,
    pub output: Loudness,
    pub target: f32,
    pub ceiling: f32,
    /// Gain into the limiter, relative to the input (dB).
    pub gain_db: f32,
    /// Deepest glue-compressor and limiter gain reduction (dB, ≤ 0).
    pub glue_reduction_db: f32,
    pub limiter_reduction_db: f32,
    /// Loudness/peak iterations used.
    pub iterations: u32,
    /// `|output.integrated − target| ≤ tolerance`.
    pub loudness_ok: bool,
    /// `output.truePeak ≤ ceiling`.
    pub true_peak_ok: bool,
}

/// Master to `target_lufs` with a `ceiling_dbtp` true-peak ceiling (in place).
pub fn master(l: &mut [f32], r: &mut [f32], rate: u32, target_lufs: f32, ceiling_dbtp: f32) -> MasterReport {
    master_with(l, r, rate, &MasterParams { target: target_lufs, ceiling: ceiling_dbtp, ..Default::default() })
}

/// Master with full parameters (in place).
pub fn master_with(l: &mut [f32], r: &mut [f32], rate: u32, p: &MasterParams) -> MasterReport {
    let fs = rate as f32;
    let n = l.len().min(r.len());
    let (l, r) = (&mut l[..n], &mut r[..n]);
    let input = measure(l, r, fs);
    let mut report = MasterReport {
        input,
        output: input,
        target: p.target,
        ceiling: p.ceiling,
        gain_db: 0.0,
        glue_reduction_db: 0.0,
        limiter_reduction_db: 0.0,
        iterations: 0,
        loudness_ok: false,
        true_peak_ok: input.true_peak <= p.ceiling as f64,
    };
    if input.integrated <= -70.0 || n == 0 {
        return report;
    }
    // 1. Normalise to the chain's reference level, then EQ → glue → saturation.
    let reference = -18.0f32;
    let pre = (reference as f64 - input.integrated) as f32;
    let mut pl: Vec<f32> = l.iter().map(|x| x * db_to_gain(pre)).collect();
    let mut pr: Vec<f32> = r.iter().map(|x| x * db_to_gain(pre)).collect();
    let base = EqParams {
        hp: (p.low_cut > 0.0).then_some(p.low_cut),
        tilt: p.tilt,
        ..Default::default()
    };
    Eq::new(fs, &base).process(&mut pl, &mut pr);
    if let Some(eq) = &p.eq {
        Eq::new(fs, eq).process(&mut pl, &mut pr);
    }
    if p.glue {
        let mut c = Compressor::new(fs, &p.glue_params);
        process_aligned(&mut c, &mut pl, &mut pr);
        report.glue_reduction_db = c.take_max_reduction();
    }
    if let Some(s) = &p.saturate {
        let mut sat = Saturator::new(fs, s);
        process_aligned(&mut sat, &mut pl, &mut pr);
    }
    // 2. Gain + limiter, iterated on the measured result.
    let processed = super::loudness::integrated_lufs(&pl, &pr, fs);
    let mut gain = (p.target as f64 - processed) as f32;
    let mut ceil = p.ceiling;
    let tol = p.tolerance.max(0.05) as f64;
    let mut best: Option<(f64, Loudness, f32, f32)> = None;
    for it in 1..=10u32 {
        report.iterations = it;
        let g = db_to_gain(gain.min(48.0));
        for ((a, b), (x, y)) in l.iter_mut().zip(r.iter_mut()).zip(pl.iter().zip(&pr)) {
            *a = x * g;
            *b = y * g;
        }
        let mut lim = Limiter::new(fs, &LimiterParams { ceiling: ceil, ..p.limiter.clone() });
        process_aligned(&mut lim, l, r);
        let out = measure(l, r, fs);
        let err = out.integrated - p.target as f64;
        let over = out.true_peak - p.ceiling as f64;
        let score = err.abs().max(tol) + over.max(0.0) * 10.0;
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, out, gain, ceil));
        }
        report.limiter_reduction_db = lim.max_reduction();
        if err.abs() <= tol && over <= 0.0 {
            break;
        }
        if over > 0.0 {
            ceil -= over as f32 + 0.02;
        }
        // Limiting makes loudness grow slower than gain: step a little further.
        gain -= (err * if err < 0.0 { 1.15 } else { 1.0 }) as f32;
    }
    let (_, out, g, c) = best.expect("at least one iteration");
    if g != gain || c != ceil {
        // Re-render the best attempt.
        let gl = db_to_gain(g.min(48.0));
        for ((a, b), (x, y)) in l.iter_mut().zip(r.iter_mut()).zip(pl.iter().zip(&pr)) {
            *a = x * gl;
            *b = y * gl;
        }
        let mut lim = Limiter::new(fs, &LimiterParams { ceiling: c, ..p.limiter.clone() });
        process_aligned(&mut lim, l, r);
        report.limiter_reduction_db = lim.max_reduction();
    }
    report.output = out;
    report.gain_db = pre + g;
    report.loudness_ok = (out.integrated - p.target as f64).abs() <= tol;
    report.true_peak_ok = out.true_peak <= p.ceiling as f64 + 1e-9;
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::Rng;

    /// A crude "mix": kick-like thumps, a bass line, a pad and noise hats.
    pub(crate) fn test_mix(secs: f32, seed: u64) -> (Vec<f32>, Vec<f32>) {
        let fs = 48_000.0;
        let n = (secs * fs) as usize;
        let mut rng = Rng::new(seed);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        let beat = 0.5;
        for i in 0..n {
            let t = i as f32 / fs;
            let tb = t % beat;
            let kick = (std::f32::consts::TAU * (50.0 + 120.0 * (-tb * 30.0).exp()) * tb).sin() * (-tb * 9.0).exp() * 0.6;
            let bass = (std::f32::consts::TAU * 55.0 * t).sin() * 0.2 * (1.0 - (-tb * 20.0).exp());
            let pad = ((std::f32::consts::TAU * 220.0 * t).sin() + (std::f32::consts::TAU * 277.2 * t).sin() * 0.7 + (std::f32::consts::TAU * 329.6 * t).sin() * 0.5) * 0.06;
            let th = (t + 0.25) % beat;
            let hat = rng.bi() * (-th * 60.0).exp() * 0.15;
            l[i] = kick + bass + pad + hat * 0.8;
            r[i] = kick + bass + pad * 0.9 + hat;
        }
        (l, r)
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let (mut l, mut r) = test_mix(6.0, 3);
        // Drive it 14 dB too hot, plus full-scale inter-sample-peak bait.
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            *a *= 5.0;
            *b *= 5.0;
            if (100_000..100_400).contains(&i) {
                let v = (std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * i as f32).sin() * 2.0;
                *a = v;
                *b = -v;
            }
        }
        for ceiling in [-1.0f32, -0.1, -3.0] {
            let mut lim = Limiter::new(48_000.0, &LimiterParams { ceiling, ..Default::default() });
            let (mut x, mut y) = (l.clone(), r.clone());
            process_aligned(&mut lim, &mut x, &mut y);
            let tp = super::super::loudness::true_peak_db(&x, &y);
            assert!(tp <= ceiling as f64 + 0.05, "ceiling {ceiling}: true peak {tp}");
            assert!(x.iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn limiter_is_transparent_below_ceiling() {
        let (l, r) = test_mix(2.0, 4);
        let mut lim = Limiter::new(48_000.0, &LimiterParams { ceiling: 0.0, ..Default::default() });
        let (mut x, mut y) = (l.clone(), r.clone());
        process_aligned(&mut lim, &mut x, &mut y);
        let pk = l.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(pk < 0.95);
        assert_eq!(x, l);
        assert_eq!(y, r);
    }

    #[test]
    fn master_hits_target() {
        for (target, ceiling, scale) in [(-14.0f32, -1.0f32, 1.0f32), (-14.0, -1.0, 0.05), (-9.0, -0.3, 1.0), (-23.0, -2.0, 3.0)] {
            let (mut l, mut r) = test_mix(20.0, 9);
            l.iter_mut().chain(r.iter_mut()).for_each(|v| *v *= scale);
            let rep = master(&mut l, &mut r, 48_000, target, ceiling);
            assert!(rep.loudness_ok, "{target}: {rep:?}");
            assert!(rep.true_peak_ok, "{ceiling}: {rep:?}");
            let check = measure(&l, &r, 48_000.0);
            assert!((check.integrated - target as f64).abs() <= 0.3);
            assert!(check.true_peak <= ceiling as f64);
        }
    }

    #[test]
    fn master_silence_is_untouched() {
        let mut l = vec![0.0f32; 48_000];
        let mut r = l.clone();
        let rep = master(&mut l, &mut r, 48_000, -14.0, -1.0);
        assert!(!rep.loudness_ok);
        assert!(l.iter().all(|v| *v == 0.0));
    }
}
