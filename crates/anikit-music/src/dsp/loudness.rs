//! ITU-R BS.1770-4 / EBU R128 loudness meter: K-weighting at any sample rate,
//! momentary (400 ms), short-term (3 s) and gated integrated loudness, loudness
//! range (EBU Tech 3342), sample peak and true peak (4× polyphase FIR
//! interpolation per BS.1770-4 Annex 2). Accumulation is in `f64`.

use serde::Serialize;

use super::util::dot;

/// Half-length (in input samples) of the true-peak interpolator.
pub const TP_HALF: usize = 16;
const TP_TAPS: usize = 2 * TP_HALF;

/// Coefficients of the three fractional phases (¼, ½, ¾) of the 4× true-peak
/// interpolator: Kaiser-windowed sinc (β = 8), 32 taps per phase, each phase
/// normalised to unity DC gain. Flat within ±0.01 dB to 20 kHz at 48 kHz.
/// BS.1770-4 Annex 2 gives a shorter 48-tap example filter; its phases differ
/// by 0.24 dB at DC and it over-reads by ~0.2 dB at fs/6, so this meter uses
/// a longer filter that meets the Annex's requirements with margin.
fn tp_coefs() -> &'static [[f32; TP_TAPS]; 3] {
    static C: std::sync::OnceLock<[[f32; TP_TAPS]; 3]> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let beta = 8.0;
        let i0b = super::util::bessel_i0(beta);
        let mut c = [[0.0f32; TP_TAPS]; 3];
        for (j, ph) in c.iter_mut().enumerate() {
            let frac = (j + 1) as f64 / 4.0;
            let mut w = [0.0f64; TP_TAPS];
            for (k, wk) in w.iter_mut().enumerate() {
                let d = TP_HALF as f64 - k as f64 - frac;
                let r = d / TP_HALF as f64;
                *wk = super::util::sinc(d) * super::util::bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / i0b;
            }
            let s: f64 = w.iter().sum();
            for (p, wk) in ph.iter_mut().zip(w) {
                *p = (wk / s) as f32;
            }
        }
        c
    })
}

/// Streaming 4× true-peak detector for one channel. Each step reports the
/// peak of the sample `TP_HALF` steps ago and the three interpolated points
/// after it.
#[derive(Debug, Clone)]
pub struct TruePeak {
    hist: [f32; 2 * TP_TAPS],
    pos: usize,
    c: &'static [[f32; TP_TAPS]; 3],
    /// Highest absolute (interpolated or sample) value seen so far.
    pub peak: f32,
}

impl Default for TruePeak {
    fn default() -> Self {
        TruePeak { hist: [0.0; 2 * TP_TAPS], pos: 0, c: tp_coefs(), peak: 0.0 }
    }
}

impl TruePeak {
    /// Feed one sample; returns max |value| over the sample `TP_HALF` steps ago
    /// and the three interpolated points that follow it (also folded into `peak`).
    #[inline(always)]
    pub fn push(&mut self, x: f32) -> f32 {
        self.pos = if self.pos == 0 { TP_TAPS - 1 } else { self.pos - 1 };
        self.hist[self.pos] = x;
        self.hist[self.pos + TP_TAPS] = x;
        let h = &self.hist[self.pos..self.pos + TP_TAPS];
        let mut m = h[TP_HALF].abs();
        for ph in self.c.iter() {
            let acc = dot(h, ph);
            m = m.max(acc.abs());
        }
        self.peak = self.peak.max(m);
        m
    }
    pub fn reset(&mut self) {
        *self = TruePeak::default();
    }
}

/// K-weighting filter coefficients for `rate` (two biquads), derived from the
/// analog prototype so they match BS.1770 at 48 kHz and work at any rate.
pub fn k_weighting(rate: f64) -> [[f64; 5]; 2] {
    // Stage 1: high-shelf pre-filter.
    let f0 = 1_681.974_450_955_533;
    let g = 3.999_843_853_973_347;
    let q = 0.707_175_236_955_419_6;
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let vh = 10f64.powf(g / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let s1 = [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
        2.0 * (k * k - 1.0) / a0,
        (1.0 - k / q + k * k) / a0,
    ];
    // Stage 2: RLB high-pass.
    let f0 = 38.135_470_876_024_44;
    let q = 0.500_327_037_323_877_3;
    let k = (std::f64::consts::PI * f0 / rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let s2 = [1.0, -2.0, 1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0];
    [s1, s2]
}

/// Loudness measurement results (LUFS / LU / dB). Silent input reports −inf as `-200`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Loudness {
    /// Gated integrated loudness (LUFS).
    pub integrated: f64,
    /// Loudness range (LU), EBU Tech 3342.
    pub lra: f64,
    /// Maximum momentary loudness (LUFS).
    pub max_momentary: f64,
    /// Maximum short-term loudness (LUFS).
    pub max_short_term: f64,
    /// True peak (dBTP).
    pub true_peak: f64,
    /// Sample peak (dBFS).
    pub sample_peak: f64,
}

const SILENT: f64 = -200.0;

fn lufs(ms: f64) -> f64 {
    if ms <= 0.0 {
        SILENT
    } else {
        -0.691 + 10.0 * ms.log10()
    }
}

/// Streaming BS.1770-4 meter for stereo (both channels weight 1.0).
#[derive(Debug, Clone)]
pub struct LoudnessMeter {
    kw: [[f64; 5]; 2],
    z: [[f64; 2]; 4],
    sub_len: usize,
    acc: f64,
    count: usize,
    /// Sum of K-weighted squares (both channels) per 100 ms sub-block.
    subs: Vec<f64>,
    tp: [TruePeak; 2],
    sample_peak: f32,
}

impl LoudnessMeter {
    pub fn new(rate: f32) -> LoudnessMeter {
        LoudnessMeter {
            kw: k_weighting(rate as f64),
            z: [[0.0; 2]; 4],
            sub_len: ((rate as f64) * 0.1).round() as usize,
            acc: 0.0,
            count: 0,
            subs: Vec::new(),
            tp: [TruePeak::default(), TruePeak::default()],
            sample_peak: 0.0,
        }
    }

    pub fn reset(&mut self) {
        let rate = self.sub_len as f32 * 10.0;
        *self = LoudnessMeter::new(rate);
    }

    #[inline(always)]
    fn kfilter(kw: &[[f64; 5]; 2], z: &mut [[f64; 2]; 4], ch: usize, x: f64) -> f64 {
        let mut v = x;
        for (s, c) in kw.iter().enumerate() {
            let st = &mut z[ch * 2 + s];
            let y = c[0] * v + st[0];
            st[0] = c[1] * v - c[3] * y + st[1];
            st[1] = c[2] * v - c[4] * y;
            v = y;
        }
        v
    }

    /// Feed stereo audio (any length).
    pub fn process(&mut self, l: &[f32], r: &[f32]) {
        for (&a, &b) in l.iter().zip(r) {
            let yl = Self::kfilter(&self.kw, &mut self.z, 0, a as f64);
            let yr = Self::kfilter(&self.kw, &mut self.z, 1, b as f64);
            self.acc += yl * yl + yr * yr;
            self.count += 1;
            if self.count == self.sub_len {
                self.subs.push(self.acc);
                self.acc = 0.0;
                self.count = 0;
            }
            self.tp[0].push(a);
            self.tp[1].push(b);
            self.sample_peak = self.sample_peak.max(a.abs()).max(b.abs());
        }
        for st in self.z.iter_mut() {
            st[0] = super::util::flush64(st[0]);
            st[1] = super::util::flush64(st[1]);
        }
    }

    /// Mean square over `n` sub-blocks ending at sub-block index `end` (exclusive).
    fn window(&self, end: usize, n: usize) -> f64 {
        let s: f64 = self.subs[end - n..end].iter().sum();
        s / (n * self.sub_len) as f64
    }

    fn windows(&self, n: usize) -> Vec<f64> {
        if self.subs.len() < n {
            return Vec::new();
        }
        (n..=self.subs.len()).map(|e| self.window(e, n)).collect()
    }

    /// Momentary loudness of the latest 400 ms (LUFS).
    pub fn momentary(&self) -> f64 {
        if self.subs.len() < 4 {
            return SILENT;
        }
        lufs(self.window(self.subs.len(), 4))
    }

    /// Short-term loudness of the latest 3 s (LUFS).
    pub fn short_term(&self) -> f64 {
        if self.subs.len() < 30 {
            return SILENT;
        }
        lufs(self.window(self.subs.len(), 30))
    }

    /// Gated integrated loudness (LUFS): 400 ms blocks, 75 % overlap, absolute
    /// gate −70 LUFS, relative gate −10 LU.
    pub fn integrated(&self) -> f64 {
        let blocks: Vec<f64> = self.windows(4).into_iter().filter(|&z| lufs(z) > -70.0).collect();
        if blocks.is_empty() {
            return SILENT;
        }
        let abs = blocks.iter().sum::<f64>() / blocks.len() as f64;
        let gate = lufs(abs) - 10.0;
        let rel: Vec<f64> = blocks.into_iter().filter(|&z| lufs(z) > gate).collect();
        if rel.is_empty() {
            return SILENT;
        }
        lufs(rel.iter().sum::<f64>() / rel.len() as f64)
    }

    /// Loudness range (LU) per EBU Tech 3342: 3 s windows at 10 Hz, absolute
    /// gate −70 LUFS, relative gate −20 LU, 10th–95th percentile spread.
    pub fn loudness_range(&self) -> f64 {
        let st: Vec<f64> = self.windows(30).into_iter().filter(|&z| lufs(z) > -70.0).collect();
        if st.is_empty() {
            return 0.0;
        }
        let abs = st.iter().sum::<f64>() / st.len() as f64;
        let gate = lufs(abs) - 20.0;
        let mut v: Vec<f64> = st.into_iter().map(lufs).filter(|&l| l > gate).collect();
        if v.is_empty() {
            return 0.0;
        }
        v.sort_by(|a, b| a.total_cmp(b));
        let pct = |p: f64| {
            let x = p * (v.len() - 1) as f64;
            let i = x.floor() as usize;
            let f = x - i as f64;
            v[i] + (v[(i + 1).min(v.len() - 1)] - v[i]) * f
        };
        pct(0.95) - pct(0.10)
    }

    /// Maximum momentary loudness so far.
    pub fn max_momentary(&self) -> f64 {
        self.windows(4).into_iter().map(lufs).fold(SILENT, f64::max)
    }

    /// Maximum short-term loudness so far.
    pub fn max_short_term(&self) -> f64 {
        self.windows(30).into_iter().map(lufs).fold(SILENT, f64::max)
    }

    /// True peak in dBTP (max of both channels).
    pub fn true_peak(&self) -> f64 {
        let p = self.tp[0].peak.max(self.tp[1].peak).max(self.sample_peak);
        to_db(p)
    }

    /// Sample peak in dBFS.
    pub fn sample_peak(&self) -> f64 {
        to_db(self.sample_peak)
    }

    /// All results.
    pub fn result(&self) -> Loudness {
        Loudness {
            integrated: self.integrated(),
            lra: self.loudness_range(),
            max_momentary: self.max_momentary(),
            max_short_term: self.max_short_term(),
            true_peak: self.true_peak(),
            sample_peak: self.sample_peak(),
        }
    }
}

fn to_db(p: f32) -> f64 {
    if p <= 0.0 {
        SILENT
    } else {
        20.0 * (p as f64).log10()
    }
}

/// Measure a whole stereo buffer.
pub fn measure(l: &[f32], r: &[f32], rate: f32) -> Loudness {
    let mut m = LoudnessMeter::new(rate);
    m.process(l, r);
    m.result()
}

/// Integrated loudness only (cheaper: no true-peak oversampling).
pub fn integrated_lufs(l: &[f32], r: &[f32], rate: f32) -> f64 {
    let mut m = LoudnessMeter::new(rate);
    // Same filter and blocks as `process`, without the peak detectors.
    for (&a, &b) in l.iter().zip(r) {
        let yl = LoudnessMeter::kfilter(&m.kw, &mut m.z, 0, a as f64);
        let yr = LoudnessMeter::kfilter(&m.kw, &mut m.z, 1, b as f64);
        m.acc += yl * yl + yr * yr;
        m.count += 1;
        if m.count == m.sub_len {
            m.subs.push(m.acc);
            m.acc = 0.0;
            m.count = 0;
        }
    }
    m.integrated()
}

/// True peak (dBTP) of a stereo buffer.
pub fn true_peak_db(l: &[f32], r: &[f32]) -> f64 {
    let mut a = TruePeak::default();
    let mut b = TruePeak::default();
    let mut sp = 0.0f32;
    for (&x, &y) in l.iter().zip(r) {
        a.push(x);
        b.push(y);
        sp = sp.max(x.abs()).max(y.abs());
    }
    to_db(a.peak.max(b.peak).max(sp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const FS: f32 = 48_000.0;

    /// Stereo sine segments: (seconds, dBFS peak), same signal in both channels.
    fn tone(freq: f64, segs: &[(f64, f64)]) -> (Vec<f32>, Vec<f32>) {
        let mut v = Vec::new();
        let mut i = 0usize;
        for &(sec, db) in segs {
            let a = 10f64.powf(db / 20.0);
            for _ in 0..(sec * FS as f64).round() as usize {
                v.push((a * (2.0 * PI * freq * i as f64 / FS as f64).sin()) as f32);
                i += 1;
            }
        }
        (v.clone(), v)
    }

    #[test]
    fn k_weighting_48k_reference() {
        // Published BS.1770 coefficients at 48 kHz.
        let k = k_weighting(48_000.0);
        let s1 = [1.535_124_859_586_97, -2.691_696_189_406_38, 1.198_392_810_852_85, -1.690_659_293_182_41, 0.732_480_774_215_85];
        let s2 = [1.0, -2.0, 1.0, -1.990_047_454_833_98, 0.990_072_250_366_21];
        for i in 0..5 {
            assert!((k[0][i] - s1[i]).abs() < 1e-8, "stage1[{i}] {}", k[0][i]);
            assert!((k[1][i] - s2[i]).abs() < 1e-8, "stage2[{i}] {}", k[1][i]);
        }
    }

    #[test]
    fn tech3341_integrated() {
        // Cases 1–5 of EBU Tech 3341 (1 kHz stereo sine): expected ±0.1 LU.
        let cases: [(&[(f64, f64)], f64); 5] = [
            (&[(20.0, -23.0)], -23.0),
            (&[(20.0, -33.0)], -33.0),
            (&[(10.0, -36.0), (60.0, -23.0), (10.0, -36.0)], -23.0),
            (&[(10.0, -72.0), (10.0, -36.0), (60.0, -23.0), (10.0, -36.0), (10.0, -72.0)], -23.0),
            (&[(20.0, -26.0), (20.1, -20.0), (20.0, -26.0)], -23.0),
        ];
        for (i, (segs, want)) in cases.iter().enumerate() {
            let (l, r) = tone(1000.0, segs);
            let got = measure(&l, &r, FS);
            assert!((got.integrated - want).abs() < 0.1, "case {}: {}", i + 1, got.integrated);
            assert!((integrated_lufs(&l, &r, FS) - got.integrated).abs() < 1e-9);
        }
        // Single channel: −3.01 LU lower.
        let (l, _) = tone(1000.0, &[(10.0, -23.0)]);
        let z = vec![0.0; l.len()];
        assert!((measure(&l, &z, FS).integrated + 26.01).abs() < 0.1);
    }

    #[test]
    fn tech3341_momentary_short_term() {
        // Cases 9/10-style: a constant −23 dBFS sine reads −23 momentary and short-term.
        let (l, r) = tone(1000.0, &[(5.0, -23.0)]);
        let mut m = LoudnessMeter::new(FS);
        m.process(&l, &r);
        assert!((m.momentary() + 23.0).abs() < 0.1);
        assert!((m.short_term() + 23.0).abs() < 0.1);
        assert!((m.max_momentary() + 23.0).abs() < 0.1);
    }

    #[test]
    fn tech3342_lra() {
        let cases: [(&[(f64, f64)], f64); 4] = [
            (&[(20.0, -20.0), (20.0, -30.0)], 10.0),
            (&[(20.0, -20.0), (20.0, -15.0)], 5.0),
            (&[(20.0, -40.0), (20.0, -20.0)], 20.0),
            (&[(20.0, -50.0), (20.0, -35.0), (20.0, -20.0), (20.0, -35.0), (20.0, -50.0)], 15.0),
        ];
        for (i, (segs, want)) in cases.iter().enumerate() {
            let (l, r) = tone(1000.0, segs);
            let got = measure(&l, &r, FS).lra;
            assert!((got - want).abs() < 1.0, "LRA case {}: {got}", i + 1);
        }
    }

    #[test]
    fn tech3341_true_peak() {
        // Cases 15–19: (freq as fraction of fs, phase °, peak dBFS) → expected dBTP (+0.2 / −0.4).
        let cases = [
            (0.25f64, 0.0f64, -6.0f64, -6.0f64),
            (0.25, 45.0, -6.0, -6.0),
            (1.0 / 6.0, 60.0, -6.0, -6.0),
            (0.125, 67.5, -6.0, -6.0),
            (0.25, 45.0, 0.0, 0.0),
        ];
        for (i, &(f, ph, db, want)) in cases.iter().enumerate() {
            let a = 10f64.powf(db / 20.0);
            let n = 48_000;
            // 50 ms fade-in: an abrupt onset has a genuine inter-sample overshoot.
            let x: Vec<f32> = (0..n).map(|k| (a * (k as f64 / 2400.0).min(1.0) * (2.0 * PI * f * k as f64 + ph.to_radians()).sin()) as f32).collect();
            let got = measure(&x, &x, FS);
            assert!(got.true_peak - want < 0.2 && want - got.true_peak < 0.4, "TP case {}: {}", i + 15, got.true_peak);
            assert!((true_peak_db(&x, &x) - got.true_peak).abs() < 1e-9);
            if i == 1 {
                // Samples land at ±45°: sample peak −9.01 dBFS.
                assert!((got.sample_peak + 9.01).abs() < 0.05, "{}", got.sample_peak);
            }
        }
    }

    #[test]
    fn other_rates() {
        for rate in [44_100.0f32, 96_000.0] {
            let n = (rate * 10.0) as usize;
            let a = 10f64.powf(-23.0 / 20.0);
            let x: Vec<f32> = (0..n).map(|k| (a * (2.0 * PI * 1000.0 * k as f64 / rate as f64).sin()) as f32).collect();
            let got = measure(&x, &x, rate).integrated;
            assert!((got + 23.0).abs() < 0.1, "{rate}: {got}");
        }
    }

    #[test]
    fn silence() {
        let z = vec![0.0f32; 48_000];
        let m = measure(&z, &z, FS);
        assert_eq!(m.integrated, SILENT);
        assert_eq!(m.true_peak, SILENT);
        assert_eq!(m.lra, 0.0);
    }
}
