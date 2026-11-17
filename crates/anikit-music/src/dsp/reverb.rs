//! Algorithmic reverb: a 16-line feedback delay network with a Hadamard
//! mixing matrix, slowly modulated delay lines (cubic interpolation, so no
//! metallic ringing), per-line two-shelf absorption filters for independent
//! low / mid / high RT60, a 4-stage allpass input diffuser per channel,
//! stereo early reflections, pre-delay, wet EQ and width.
//!
//! Presets: `room`, `hall`, `plate`, `chamber`, `cathedral`, `ambience`; any
//! field given alongside `preset` overrides it. With `wet: 1, dry: 0` the
//! reverb is a send effect; wet levels are normalised so presets of
//! different length sound similarly loud.

use serde::Deserialize;

use super::biquad::{Biquad, FilterKind};
use super::util::{flush, DelayLine};
use super::Processor;
use crate::voice::Rng;

/// Preset names.
pub const REVERB_PRESETS: &[&str] = &["room", "hall", "plate", "chamber", "cathedral", "ambience"];

/// Reverb parameters. Every field is optional and overrides the preset
/// (default preset `hall`).
#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ReverbParams {
    pub preset: Option<String>,
    /// Room size (scales delay lengths), 0.1..2.
    pub size: Option<f32>,
    /// Mid-band RT60 in seconds.
    pub decay: Option<f32>,
    /// Low-band RT60 as a multiple of `decay`.
    pub low_ratio: Option<f32>,
    /// High-band RT60 as a multiple of `decay` (< 1 = darker tail).
    pub high_ratio: Option<f32>,
    /// Low / mid crossover (Hz).
    pub low_cross: Option<f32>,
    /// Mid / high crossover (Hz).
    pub high_cross: Option<f32>,
    /// Pre-delay (ms).
    pub predelay: Option<f32>,
    /// Input diffusion 0..1.
    pub diffusion: Option<f32>,
    /// Early-reflection level 0..1.
    pub early: Option<f32>,
    /// Delay-line modulation depth 0..1 (1 ≈ ±0.5 ms).
    pub modulation: Option<f32>,
    /// Stereo width of the wet signal 0..1.5.
    pub width: Option<f32>,
    /// Wet high-pass (Hz).
    pub low_cut: Option<f32>,
    /// Wet low-pass (Hz).
    pub high_cut: Option<f32>,
    /// Wet gain (linear).
    pub wet: Option<f32>,
    /// Dry gain (linear).
    pub dry: Option<f32>,
}

/// Fully resolved reverb settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReverbSettings {
    pub size: f32,
    pub decay: f32,
    pub low_ratio: f32,
    pub high_ratio: f32,
    pub low_cross: f32,
    pub high_cross: f32,
    pub predelay: f32,
    pub diffusion: f32,
    pub early: f32,
    pub modulation: f32,
    pub width: f32,
    pub low_cut: f32,
    pub high_cut: f32,
    pub wet: f32,
    pub dry: f32,
}

fn preset(name: &str) -> Option<ReverbSettings> {
    let base = ReverbSettings {
        size: 1.0,
        decay: 2.4,
        low_ratio: 1.15,
        high_ratio: 0.45,
        low_cross: 300.0,
        high_cross: 4000.0,
        predelay: 22.0,
        diffusion: 0.75,
        early: 0.35,
        modulation: 0.5,
        width: 1.0,
        low_cut: 90.0,
        high_cut: 12_000.0,
        wet: 0.3,
        dry: 1.0,
    };
    Some(match name {
        "hall" => base,
        "room" => ReverbSettings { size: 0.4, decay: 0.7, low_ratio: 1.0, high_ratio: 0.55, predelay: 6.0, early: 0.55, modulation: 0.3, ..base },
        "plate" => ReverbSettings {
            size: 0.65,
            decay: 1.9,
            low_ratio: 0.9,
            high_ratio: 0.75,
            high_cross: 6000.0,
            predelay: 0.0,
            diffusion: 0.9,
            early: 0.0,
            modulation: 0.6,
            low_cut: 160.0,
            high_cut: 14_000.0,
            ..base
        },
        "chamber" => ReverbSettings { size: 0.65, decay: 1.3, high_ratio: 0.55, predelay: 12.0, early: 0.45, diffusion: 0.8, ..base },
        "cathedral" => ReverbSettings {
            size: 1.7,
            decay: 6.0,
            low_ratio: 1.3,
            high_ratio: 0.35,
            high_cross: 3000.0,
            predelay: 45.0,
            early: 0.25,
            modulation: 0.4,
            low_cut: 60.0,
            high_cut: 9000.0,
            ..base
        },
        "ambience" => ReverbSettings {
            size: 0.25,
            decay: 0.35,
            low_ratio: 0.9,
            high_ratio: 0.7,
            predelay: 0.0,
            early: 0.8,
            diffusion: 0.65,
            modulation: 0.2,
            low_cut: 150.0,
            ..base
        },
        _ => return None,
    })
}

/// Closest name in `names` to `s` (edit distance), for "did you mean" errors.
pub fn closest<'a>(s: &str, names: &[&'a str]) -> Option<&'a str> {
    fn lev(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut prev: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut cur = vec![i + 1];
            for (j, cb) in b.iter().enumerate() {
                cur.push((prev[j] + (ca != *cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1));
            }
            prev = cur;
        }
        prev[b.len()]
    }
    let s = s.to_ascii_lowercase();
    names.iter().map(|n| (lev(&s, n), *n)).min().map(|(_, n)| n)
}

impl ReverbParams {
    /// Resolve against the preset. Errors on an unknown preset name.
    pub fn resolve(&self) -> Result<ReverbSettings, String> {
        let name = self.preset.as_deref().unwrap_or("hall");
        let b = preset(&name.to_ascii_lowercase()).ok_or_else(|| {
            format!(
                "unknown reverb preset {name:?} (did you mean {:?}?); presets: {}",
                closest(name, REVERB_PRESETS).unwrap_or("hall"),
                REVERB_PRESETS.join(", ")
            )
        })?;
        Ok(ReverbSettings {
            size: self.size.unwrap_or(b.size).clamp(0.1, 2.5),
            decay: self.decay.unwrap_or(b.decay).clamp(0.05, 30.0),
            low_ratio: self.low_ratio.unwrap_or(b.low_ratio).clamp(0.1, 3.0),
            high_ratio: self.high_ratio.unwrap_or(b.high_ratio).clamp(0.05, 1.5),
            low_cross: self.low_cross.unwrap_or(b.low_cross).clamp(40.0, 2000.0),
            high_cross: self.high_cross.unwrap_or(b.high_cross).clamp(1000.0, 16_000.0),
            predelay: self.predelay.unwrap_or(b.predelay).clamp(0.0, 500.0),
            diffusion: self.diffusion.unwrap_or(b.diffusion).clamp(0.0, 1.0),
            early: self.early.unwrap_or(b.early).clamp(0.0, 2.0),
            modulation: self.modulation.unwrap_or(b.modulation).clamp(0.0, 2.0),
            width: self.width.unwrap_or(b.width).clamp(0.0, 1.5),
            low_cut: self.low_cut.unwrap_or(b.low_cut).max(0.0),
            high_cut: self.high_cut.unwrap_or(b.high_cut).max(0.0),
            wet: self.wet.unwrap_or(b.wet),
            dry: self.dry.unwrap_or(b.dry),
        })
    }
}

const N: usize = 16;
const ER_TAPS: usize = 18;
/// Base FDN line lengths at size 1 (ms), roughly exponentially spaced.
const LINE_MS: [f32; N] = [
    29.7, 32.3, 35.1, 37.9, 41.3, 44.1, 47.9, 51.7, 55.9, 60.7, 65.3, 70.9, 76.3, 82.7, 89.3, 96.7,
];
const DIFF_MS: [[f32; 4]; 2] = [[4.77, 3.59, 12.73, 9.30], [5.03, 3.77, 13.27, 8.77]];

fn is_prime(n: usize) -> bool {
    n >= 2 && (2..).take_while(|d| d * d <= n).all(|d| !n.is_multiple_of(d))
}

fn prime_near(x: f32) -> usize {
    let mut n = x.round().max(3.0) as usize;
    while !is_prime(n) {
        n += 1;
    }
    n
}

/// First-order shelf (TDF): DC gain `g_dc`, Nyquist gain `g_ny`.
fn shelf(rate: f32, fc: f32, low: bool, g: f32) -> (f32, f32, f32) {
    let k = (std::f32::consts::PI * fc / rate).tan();
    let d = 1.0 + k;
    if low {
        ((1.0 + g * k) / d, (g * k - 1.0) / d, (k - 1.0) / d)
    } else {
        ((g + k) / d, (k - g) / d, (k - 1.0) / d)
    }
}

/// The 16-line FDN reverb.
#[derive(Debug, Clone)]
pub struct Reverb {
    s: ReverbSettings,
    pre: [DelayLine; 3],
    pre_d: usize,
    diff: [[DelayLine; 4]; 2],
    diff_d: [[usize; 4]; 2],
    diff_g: f32,
    er: [(usize, f32, f32); ER_TAPS],
    er_lp: [f32; 2],
    er_c: f32,
    lines: Vec<DelayLine>,
    len: [f32; N],
    gain: [f32; N],
    lb0: [f32; N],
    lb1: [f32; N],
    la1: [f32; N],
    hb0: [f32; N],
    hb1: [f32; N],
    ha1: [f32; N],
    blk: usize,
    lz: [f32; N],
    hz: [f32; N],
    lfo: [(f32, f32); N],
    lfo0: [(f32, f32); N],
    rot: [(f32, f32); N],
    depth: f32,
    in_gain: f32,
    eq: [Biquad; 2],
    wet: f32,
    dry: f32,
    width: f32,
}

impl Reverb {
    /// Build from params; an unknown preset falls back to `hall` (use
    /// [`Reverb::try_new`] or [`ReverbParams::resolve`] to surface the error).
    pub fn new(rate: f32, p: &ReverbParams) -> Reverb {
        let s = p.resolve().unwrap_or_else(|_| ReverbParams { preset: None, ..p.clone() }.resolve().unwrap());
        Reverb::with_settings(rate, s)
    }

    pub fn try_new(rate: f32, p: &ReverbParams) -> Result<Reverb, String> {
        Ok(Reverb::with_settings(rate, p.resolve()?))
    }

    pub fn with_settings(rate: f32, s: ReverbSettings) -> Reverb {
        let ms = |v: f32| v * 0.001 * rate;
        let pre_d = ms(s.predelay).round() as usize;
        // Early reflections: deterministic sparse taps over 3..(70·size) ms.
        let mut rng = Rng::new(0x05EE_D0E4);
        let er_span = 70.0 * s.size.sqrt();
        let mut er = [(0usize, 0.0f32, 0.0f32); ER_TAPS];
        for (k, e) in er.iter_mut().enumerate() {
            let u = (k as f32 + rng.range(0.1, 0.9)) / ER_TAPS as f32;
            let t = 3.0 + er_span * u * u.sqrt();
            let g = 0.9 * (1.0 - 0.75 * u).powf(1.5) * if rng.f32() < 0.5 { -1.0 } else { 1.0 };
            let pan = if k % 2 == 0 { rng.range(-1.0, -0.45) } else { rng.range(0.45, 1.0) };
            let (gl, gr) = crate::voice::pan_gains(pan);
            *e = (ms(t).round() as usize, g * gl, g * gr);
        }
        let er_max = er.iter().map(|e| e.0).max().unwrap_or(0);
        let pre = [DelayLine::new(pre_d + 8), DelayLine::new(pre_d + 8), DelayLine::new(pre_d + er_max + 8)];
        let dscale = s.size.sqrt().clamp(0.5, 1.4);
        let mut diff_d = [[0usize; 4]; 2];
        for c in 0..2 {
            for k in 0..4 {
                diff_d[c][k] = prime_near(ms(DIFF_MS[c][k] * dscale));
            }
        }
        let diff = diff_d.map(|row| row.map(|d| DelayLine::new(d + 2)));
        let depth = ms(0.5) * s.modulation;
        let mut len = [0.0f32; N];
        let mut lines = Vec::with_capacity(N);
        for i in 0..N {
            len[i] = prime_near(ms(LINE_MS[i] * s.size)) as f32;
            lines.push(DelayLine::new(len[i] as usize + depth as usize + 8));
        }
        let mut gain = [0.0f32; N];
        let (mut lb0, mut lb1, mut la1) = ([1.0f32; N], [0.0f32; N], [0.0f32; N]);
        let (mut hb0, mut hb1, mut ha1) = ([1.0f32; N], [0.0f32; N], [0.0f32; N]);
        let mut lfo = [(1.0, 0.0); N];
        let mut rot = [(1.0, 0.0); N];
        let rt = |t: f32, d: f32| 10f32.powf(-3.0 * d / (t * rate));
        for i in 0..N {
            // Effective loop delay includes the write/read step.
            let d = len[i] + 1.0;
            let gm = rt(s.decay, d);
            let gl = rt(s.decay * s.low_ratio, d);
            let gh = rt(s.decay * s.high_ratio, d);
            gain[i] = gm;
            (lb0[i], lb1[i], la1[i]) = shelf(rate, s.low_cross, true, gl / gm);
            (hb0[i], hb1[i], ha1[i]) = shelf(rate, s.high_cross, false, gh / gm);
            let ph = rng.range(0.0, std::f32::consts::TAU);
            lfo[i] = (ph.cos(), ph.sin());
            let w = std::f32::consts::TAU * (0.13 + 0.07 * i as f32 + rng.range(0.0, 0.05)) / rate;
            rot[i] = (w.cos(), w.sin());
        }
        let eq = [
            if s.low_cut > 0.0 { Biquad::new(FilterKind::HighPass, rate, s.low_cut, std::f32::consts::FRAC_1_SQRT_2, 0.0) } else { Biquad::from_coefs(super::biquad::Coefs::IDENTITY) },
            if s.high_cut > 0.0 && s.high_cut < rate * 0.49 {
                Biquad::new(FilterKind::LowPass, rate, s.high_cut, std::f32::consts::FRAC_1_SQRT_2, 0.0)
            } else {
                Biquad::from_coefs(super::biquad::Coefs::IDENTITY)
            },
        ];
        // Loudness normalisation: tail energy grows with RT60 / mean line length.
        let mean = len.iter().sum::<f32>() / N as f32;
        let in_gain = 0.55 * (mean / (s.decay * rate)).sqrt().min(0.5) * 4.0;
        Reverb {
            pre,
            pre_d,
            diff,
            diff_d,
            diff_g: 0.45 + 0.3 * s.diffusion,
            er,
            er_lp: [0.0; 2],
            er_c: (-std::f32::consts::TAU * (s.high_cross * 1.5).min(rate * 0.45) / rate).exp(),
            lines,
            len,
            gain,
            lb0,
            lb1,
            la1,
            hb0,
            hb1,
            ha1,
            blk: ((len.iter().fold(f32::MAX, |a, b| a.min(*b)) - depth.ceil() - 3.0) as usize).clamp(1, BLK),
            lz: [0.0; N],
            hz: [0.0; N],
            lfo,
            lfo0: lfo,
            rot,
            depth,
            in_gain,
            eq,
            wet: s.wet,
            dry: s.dry,
            width: s.width,
            s,
        }
    }

    /// The resolved settings.
    pub fn settings(&self) -> &ReverbSettings {
        &self.s
    }

    /// Set wet and dry gains (linear).
    pub fn set_mix(&mut self, wet: f32, dry: f32) {
        self.wet = wet;
        self.dry = dry;
    }

    /// 16-point Walsh-Hadamard transform as H4 ⊗ H4 on a 4×4 view: butterflies
    /// within each row, then 4-wide vector butterflies across rows.
    #[inline(always)]
    fn fwht(v: &mut [f32; N]) {
        let mut m = [[0.0f32; 4]; 4];
        for (r, row) in m.iter_mut().enumerate() {
            let (a, b, c, d) = (v[4 * r], v[4 * r + 1], v[4 * r + 2], v[4 * r + 3]);
            let (s0, s1, d0, d1) = (a + b, c + d, a - b, c - d);
            *row = [s0 + s1, d0 + d1, s0 - s1, d0 - d1];
        }
        let [r0, r1, r2, r3] = m;
        for c in 0..4 {
            let (s0, s1, d0, d1) = (r0[c] + r1[c], r2[c] + r3[c], r0[c] - r1[c], r2[c] - r3[c]);
            v[c] = s0 + s1;
            v[4 + c] = d0 + d1;
            v[8 + c] = s0 - s1;
            v[12 + c] = d0 - d1;
        }
    }
}

/// Output tap signs (orthogonal patterns for decorrelated L/R).
const OUT_L: [f32; N] = [1., 1., -1., 1., 1., -1., 1., -1., -1., 1., 1., 1., -1., -1., 1., -1.];
const OUT_R: [f32; N] = [1., -1., 1., 1., -1., -1., 1., 1., 1., 1., -1., 1., 1., -1., -1., -1.];
const IN_SIGN: [f32; N] = [1., 1., -1., 1., 1., -1., -1., 1., -1., 1., 1., -1., 1., -1., 1., 1.];

impl Processor for Reverb {
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let mut start = 0;
        while start < n {
            let m = (n - start).min(self.blk);
            self.block(&mut l[start..start + m], &mut r[start..start + m]);
            start += m;
        }
        for p in self.lfo.iter_mut() {
            let n = 1.0 / (p.0 * p.0 + p.1 * p.1).sqrt();
            *p = (p.0 * n, p.1 * n);
        }
    }

    fn reset(&mut self) {
        self.pre.iter_mut().for_each(|d| d.reset());
        self.diff.iter_mut().flatten().for_each(|d| d.reset());
        self.lines.iter_mut().for_each(|d| d.reset());
        self.er_lp = [0.0; 2];
        self.lz = [0.0; N];
        self.hz = [0.0; N];
        self.lfo = self.lfo0;
        self.eq.iter_mut().for_each(|b| b.reset());
    }
}

/// Largest FDN block: every line is longer than this, so a whole block can be
/// read before any of it is written back.
const BLK: usize = 64;

impl Reverb {
    /// One block (≤ `self.blk` samples): input stage per sample, then the FDN
    /// reads for all lines, the per-sample matrix/filter math across lines
    /// (vectorised over the 16 lines), the writes, and the output stage.
    fn block(&mut self, l: &mut [f32], r: &mut [f32]) {
        let m = l.len();
        let (wet, dry) = (self.wet, self.dry);
        let er_g = self.s.early * 0.7;
        let out_scale = 1.0 / (N as f32).sqrt();
        let half_w = 0.5 * self.width;
        let g_ap = self.diff_g;
        let mut din = [[0.0f32; 2]; BLK];
        let mut ers = [[0.0f32; 2]; BLK];
        // Input: pre-delay, early reflections, diffusion.
        for k in 0..m {
            let (xl, xr) = (l[k], r[k]);
            self.pre[0].push(xl);
            self.pre[1].push(xr);
            self.pre[2].push(xl + xr);
            let mut d = [self.pre[0].tap(self.pre_d), self.pre[1].tap(self.pre_d)];
            let (mut el, mut erv) = (0.0f32, 0.0f32);
            if er_g > 0.0 {
                for &(dd, gl, gr) in &self.er {
                    let v = self.pre[2].tap(self.pre_d + dd);
                    el += v * gl;
                    erv += v * gr;
                }
            }
            self.er_lp[0] = flush(el + self.er_c * (self.er_lp[0] - el));
            self.er_lp[1] = flush(erv + self.er_c * (self.er_lp[1] - erv));
            ers[k] = self.er_lp;
            for c in 0..2 {
                let mut x = d[c];
                for j in 0..4 {
                    let line = &mut self.diff[c][j];
                    let z = line.tap(self.diff_d[c][j] - 1);
                    let w = x + g_ap * z;
                    x = z - g_ap * w;
                    line.push(flush(w));
                }
                d[c] = x * self.in_gain;
            }
            din[k] = d;
        }
        // FDN reads for the whole block: even lines modulated (cubic), odd
        // lines at fixed integer delays.
        let mut v = [[0.0f32; N]; BLK];
        for i in 0..N {
            let line = &self.lines[i];
            if i % 2 == 1 || self.depth == 0.0 {
                let d = self.len[i] as usize;
                for (k, row) in v.iter_mut().enumerate().take(m) {
                    row[i] = line.tap(d - k);
                }
                continue;
            }
            let (mut c, mut s) = self.lfo[i];
            let (rc, rs) = self.rot[i];
            for (k, row) in v.iter_mut().enumerate().take(m) {
                let nc = c * rc - s * rs;
                s = c * rs + s * rc;
                c = nc;
                row[i] = line.read_ahead(k, self.len[i] + self.depth * s);
            }
            self.lfo[i] = (c, s);
        }
        // Absorption, output taps, Hadamard feedback, injection.
        let mut out = [[0.0f32; 2]; BLK];
        for k in 0..m {
            let mut x = v[k];
            for i in 0..N {
                let g = x[i] * self.gain[i];
                let y = self.lb0[i] * g + self.lz[i];
                self.lz[i] = flush(self.lb1[i] * g - self.la1[i] * y);
                let z = self.hb0[i] * y + self.hz[i];
                self.hz[i] = flush(self.hb1[i] * y - self.ha1[i] * z);
                x[i] = z;
            }
            out[k] = [super::util::dot(&x, &OUT_L), super::util::dot(&x, &OUT_R)];
            Self::fwht(&mut x);
            let inj = din[k];
            for i in 0..N {
                v[k][i] = flush(x[i] * 0.25 + inj[i & 1] * IN_SIGN[i]);
            }
        }
        for i in 0..N {
            let line = &mut self.lines[i];
            for row in v.iter().take(m) {
                line.push(row[i]);
            }
        }
        // Output: wet EQ, width, mix.
        for k in 0..m {
            let wl = out[k][0] * out_scale + ers[k][0] * er_g;
            let wr = out[k][1] * out_scale + ers[k][1] * er_g;
            let wl = self.eq[0].tick(0, wl);
            let wl = self.eq[1].tick(0, wl);
            let wr = self.eq[0].tick(1, wr);
            let wr = self.eq[1].tick(1, wr);
            let mid = 0.5 * (wl + wr);
            let side = half_w * (wl - wr);
            l[k] = l[k] * dry + (mid + side) * wet;
            r[k] = r[k] * dry + (mid - side) * wet;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn impulse_response(p: &ReverbParams, secs: f32) -> (Vec<f32>, Vec<f32>) {
        let mut rv = Reverb::new(48_000.0, p);
        rv.set_mix(1.0, 0.0);
        let n = (secs * 48_000.0) as usize;
        let mut l = vec![0.0f32; n];
        l[0] = 1.0;
        let mut r = l.clone();
        rv.process(&mut l, &mut r);
        (l, r)
    }

    /// RT60 from the Schroeder energy-decay curve (T20: −5 → −25 dB, ×3).
    fn rt60(l: &[f32], r: &[f32]) -> f32 {
        let e: Vec<f64> = l.iter().zip(r).map(|(a, b)| (*a as f64).powi(2) + (*b as f64).powi(2)).collect();
        let mut edc = vec![0.0f64; e.len()];
        let mut acc = 0.0;
        for i in (0..e.len()).rev() {
            acc += e[i];
            edc[i] = acc;
        }
        let db = |i: usize| 10.0 * (edc[i] / edc[0]).log10();
        let t5 = (0..e.len()).find(|&i| db(i) < -5.0).unwrap();
        let t25 = (0..e.len()).find(|&i| db(i) < -25.0).unwrap();
        3.0 * (t25 - t5) as f32 / 48_000.0
    }

    #[test]
    fn rt60_matches_setting() {
        for decay in [0.8f32, 2.0, 4.0] {
            let p = ReverbParams {
                decay: Some(decay),
                low_ratio: Some(1.0),
                high_ratio: Some(1.0),
                low_cut: Some(0.0),
                high_cut: Some(0.0),
                early: Some(0.0),
                predelay: Some(0.0),
                ..Default::default()
            };
            let (l, r) = impulse_response(&p, decay * 2.0);
            let got = rt60(&l, &r);
            assert!((got / decay - 1.0).abs() < 0.15, "decay {decay}: measured {got}");
        }
    }

    #[test]
    fn presets_are_clean() {
        for name in REVERB_PRESETS {
            let p = ReverbParams { preset: Some(name.to_string()), ..Default::default() };
            let (l, r) = impulse_response(&p, 3.0);
            assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()), "{name}");
            let dc: f64 = l.iter().map(|x| *x as f64).sum::<f64>();
            assert!(dc.abs() < 0.05, "{name}: dc {dc}");
            // Decorrelated stereo tail.
            let (mut sl, mut sr, mut slr) = (0.0f64, 0.0f64, 0.0f64);
            for (a, b) in l.iter().zip(&r) {
                sl += (*a as f64).powi(2);
                sr += (*b as f64).powi(2);
                slr += *a as f64 * *b as f64;
            }
            let corr = slr / (sl * sr).sqrt();
            eprintln!("{name}: correlation {corr:.3}");
            assert!(corr.abs() < 0.5, "{name}: correlation {corr}");
        }
        let err = ReverbParams { preset: Some("hal".into()), ..Default::default() }.resolve().unwrap_err();
        assert!(err.contains("\"hall\""), "{err}");
    }

    #[test]
    fn darker_highs_decay_faster() {
        let p = ReverbParams { decay: Some(2.0), high_ratio: Some(0.3), low_cut: Some(0.0), high_cut: Some(0.0), ..Default::default() };
        let (l, r) = impulse_response(&p, 3.0);
        // High-pass the late tail and compare its decay with a low-passed copy.
        let late = 48_000..96_000;
        let mut hl = l.clone();
        let mut hr = r.clone();
        let mut hp = Biquad::new(FilterKind::HighPass, 48_000.0, 8000.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
        hp.process(&mut hl, &mut hr);
        let mut ll = l.clone();
        let mut lr = r.clone();
        let mut lp = Biquad::new(FilterKind::LowPass, 48_000.0, 1000.0, std::f32::consts::FRAC_1_SQRT_2, 0.0);
        lp.process(&mut ll, &mut lr);
        let e = |x: &[f32], rg: std::ops::Range<usize>| x[rg].iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
        let hi_ratio = e(&hl, late.clone()) / e(&hl, 0..48_000);
        let lo_ratio = e(&ll, late.clone()) / e(&ll, 0..48_000);
        assert!(hi_ratio < lo_ratio * 0.1, "{hi_ratio} vs {lo_ratio}");
    }
}
