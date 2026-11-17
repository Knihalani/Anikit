//! Band-limited oscillators.
//!
//! [`Lanes`] runs up to eight oscillators side by side (unison, supersaw,
//! detuned layers) in fixed-size lane arrays so the per-sample loop
//! vectorises across lanes. Saw and pulse are PolyBLEP-corrected, triangle is
//! PolyBLAMP-corrected, sine is a polynomial. Phases are deterministic (from
//! the note seed).

use super::util::{fmin, fract, fract_pos, sin2pi_unit};
use crate::voice::Rng;

/// Lanes per oscillator bank.
pub const LANES: usize = 8;

/// Oscillator waveform.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave {
    /// Sawtooth (PolyBLEP).
    Saw,
    /// Pulse with width `pw` (two PolyBLEP saws).
    Pulse,
    /// Triangle (PolyBLAMP).
    Tri,
    /// Sine.
    Sine,
}

// Band-limiting integrates a 4-sample triweight kernel `(1 − x²/4)³`
// ("4-point PolyBLEP / PolyBLAMP"): the step residual spans two samples on
// each side of the discontinuity and is a single polynomial (no branches).
// Alias energy of a saw at the base rate: ≈ −61 dB at A4, ≈ −50 dB at C7
// (the classic 2-point PolyBLEP: ≈ −29 dB at C7). Bright patches also run
// notes above `OVERSAMPLE_ABOVE` at 2× with a 71-tap decimator (≈ −80 dB).

/// Step residual magnitude `G(a) = ∫ₐ² k(u) du` for `a = |x|` samples from a
/// step (zero for a ≥ 2). The residual is `+G` before the step, `−G` after.
#[inline(always)]
pub fn blep4(a: f32) -> f32 {
    let a = fmin(a, 2.0);
    let a2 = a * a;
    0.5 - a * (1.0 + a2 * (-0.25 + a2 * (0.0375 - a2 * (1.0 / 448.0)))) * (1.0 / 1.828_571_4)
}

/// Slope-change (BLAMP) residual `K(a) = ∫ₐ² G(v) dv` for `a = |x|` samples
/// from a corner (zero for a ≥ 2).
#[inline(always)]
pub fn blamp4(a: f32) -> f32 {
    let a = fmin(a, 2.0);
    let a2 = a * a;
    let q = a2 * (0.5 + a2 * (-0.0625 + a2 * (1.0 / 160.0 - a2 * (1.0 / 3584.0))));
    0.5 * (2.0 - a) - (1.328_571_4 - q) * (1.0 / 1.828_571_4)
}

/// Band-limited saw in [−1, 1] at phase `t` with `idt = 1/dt`.
#[inline(always)]
pub fn saw(t: f32, idt: f32) -> f32 {
    // The wrap is a step of −2: correct by +2G after it and −2G before it.
    // Below dt = 1/4 only the nearer side can be within two samples, so one
    // evaluation at the distance to the nearest wrap suffices.
    let after = t < 0.5;
    let d = if after { t } else { 1.0 - t };
    let g = 2.0 * blep4(d * idt);
    2.0 * t - 1.0 + if after { g } else { -g }
}

/// Band-limited pulse (zero mean). `pw` in (0, 1).
#[inline(always)]
pub fn pulse(t: f32, idt: f32, pw: f32) -> f32 {
    let mut t2 = t + 1.0 - pw;
    t2 -= (t2 >= 1.0) as i32 as f32;
    saw(t, idt) - saw(t2, idt)
}

/// Band-limited triangle in [−1, 1] (peak at t = 0).
#[inline(always)]
pub fn tri(t: f32, dt: f32, idt: f32) -> f32 {
    let naive = 4.0 * (t - 0.5).abs() - 1.0;
    let d0 = fmin(t, 1.0 - t);
    let d5 = (t - 0.5).abs();
    // Only the nearer corner can be within two samples (dt < 1/4).
    let near5 = d5 < d0;
    let k = 8.0 * dt * blamp4(fmin(d0, d5) * idt);
    naive + if near5 { k } else { -k }
}

/// JP-8000-style supersaw lane offsets (fraction of frequency at full detune).
pub const SUPERSAW: [f32; 7] = [
    -0.110_023_13,
    -0.062_884_39,
    -0.019_523_56,
    0.0,
    0.019_912_21,
    0.062_165_38,
    0.107_452_42,
];

/// A bank of up to [`LANES`] oscillators sharing one waveform.
#[derive(Clone, Debug)]
pub struct Lanes {
    pub wave: Wave,
    /// Pulse width (Pulse only).
    pub pw: f32,
    pub ph: [f32; LANES],
    /// Frequency of each lane relative to the voice pitch.
    pub ratio: [f32; LANES],
    pub gl: [f32; LANES],
    pub gr: [f32; LANES],
    dt: [f32; LANES],
    idt: [f32; LANES],
    /// 2× oversampling (decided at the first `set_freq`: high notes only).
    os: Option<bool>,
    hist_l: [f32; HB_HIST],
    hist_r: [f32; HB_HIST],
}

/// Notes whose highest lane is above this (Hz) render at 2× and are
/// decimated (aliases ≤ −75 dB instead of ≈ −45 dB with 4-point PolyBLEP alone).
pub const OVERSAMPLE_ABOVE: f32 = 500.0;

/// 71-tap Kaiser (β 7) low-pass for 2× decimation: flat to 18 kHz
/// (−0.02 dB), ≥ 72 dB down from 24 kHz, so nothing folds back below Nyquist.
#[rustfmt::skip]
#[allow(clippy::excessive_precision)]
const DEC: [f32; 71] = [
    -2.697060255e-05, 7.504290444e-05, 1.341568940e-04, -1.026183002e-04,
    -3.566743999e-04, -9.951835595e-19, 6.712661588e-04, 3.686570866e-04,
    -9.498621741e-04, -1.111842196e-03, 9.398982291e-04, 2.218860714e-03,
    -2.994306217e-04, -3.464882371e-03, -1.298427525e-03, 4.356076413e-03,
    4.002715670e-03, -4.157640115e-03, -7.603091166e-03, 2.027483559e-03,
    1.138755803e-02, 2.757802269e-03, -1.409199215e-02, -1.053854573e-02,
    1.395378727e-02, 2.105162315e-02, -8.792123991e-03, -3.333909623e-02,
    -4.170311985e-03, 4.584161173e-02, 2.978408063e-02, -5.667789964e-02,
    -8.381342143e-02, 6.404963799e-02, 3.105176630e-01, 4.333138180e-01,
    3.105176630e-01, 6.404963799e-02, -8.381342143e-02, -5.667789964e-02,
    2.978408063e-02, 4.584161173e-02, -4.170311985e-03, -3.333909623e-02,
    -8.792123991e-03, 2.105162315e-02, 1.395378727e-02, -1.053854573e-02,
    -1.409199215e-02, 2.757802269e-03, 1.138755803e-02, 2.027483559e-03,
    -7.603091166e-03, -4.157640115e-03, 4.002715670e-03, 4.356076413e-03,
    -1.298427525e-03, -3.464882371e-03, -2.994306217e-04, 2.218860714e-03,
    9.398982291e-04, -1.111842196e-03, -9.498621741e-04, 3.686570866e-04,
    6.712661588e-04, -9.951835595e-19, -3.566743999e-04, -1.026183002e-04,
    1.341568940e-04, 7.504290444e-05, -2.697060255e-05,
];
const HB_HIST: usize = 70;

/// Decimate `2n` new samples (after `HB_HIST` samples of history in `x`) by 2,
/// adding the result into `out`; keeps the history for the next block.
/// Polyphase: even and odd input samples are split into contiguous arrays so
/// every tap is a vector multiply-add over the block.
#[inline(always)]
fn decimate(
    x: &mut [f32; HB_HIST + 2 * crate::voice::BLOCK],
    hist: &mut [f32; HB_HIST],
    out: &mut [f32],
) {
    let n = out.len();
    x[..HB_HIST].copy_from_slice(hist);
    let mut even = [0.0f32; crate::voice::BLOCK + 36];
    let mut odd = [0.0f32; crate::voice::BLOCK + 36];
    for i in 0..n + 35 {
        even[i] = x[2 * i];
        odd[i] = x[2 * i + 1];
    }
    let mut acc = [0.0f32; crate::voice::BLOCK];
    for j in 0..36 {
        let h = DEC[2 * j];
        for m in 0..n {
            acc[m] += h * even[m + j];
        }
    }
    for j in 0..35 {
        let h = DEC[2 * j + 1];
        for m in 0..n {
            acc[m] += h * odd[m + j];
        }
    }
    for m in 0..n {
        out[m] += acc[m];
    }
    hist.copy_from_slice(&x[2 * n..2 * n + HB_HIST]);
}

impl Lanes {
    /// An empty bank (all lanes silent).
    pub fn new(wave: Wave) -> Lanes {
        Lanes {
            wave,
            pw: 0.5,
            ph: [0.0; LANES],
            ratio: [1.0; LANES],
            gl: [0.0; LANES],
            gr: [0.0; LANES],
            dt: [0.0; LANES],
            idt: [0.0; LANES],
            os: None,
            hist_l: [0.0; HB_HIST],
            hist_r: [0.0; HB_HIST],
        }
    }

    /// `voices` lanes detuned symmetrically over ±`cents` and spread across the
    /// stereo field by `width` (0 = mono centre, 1 = hard left/right), at
    /// relative pitch `ratio` and total gain `gain`. Random phases from `rng`.
    pub fn unison(
        wave: Wave,
        voices: usize,
        cents: f32,
        width: f32,
        ratio: f32,
        gain: f32,
        rng: &mut Rng,
    ) -> Lanes {
        let mut b = Lanes::new(wave);
        let n = voices.clamp(1, LANES);
        let g = gain / (n as f32).sqrt();
        for k in 0..n {
            let x = if n == 1 {
                0.0
            } else {
                k as f32 / (n - 1) as f32 * 2.0 - 1.0
            };
            // Slightly irregular spacing avoids periodic phasing between lanes.
            let irr = if n > 2 { rng.bi() * 0.08 } else { 0.0 };
            b.ratio[k] = ratio * ((x + irr * (1.0 - x.abs())) * cents / 1200.0).exp2();
            let p = if k % 2 == 0 { x.abs() } else { -x.abs() } * width;
            let (l, r) = crate::voice::pan_gains(p);
            b.gl[k] = g * l * std::f32::consts::SQRT_2;
            b.gr[k] = g * r * std::f32::consts::SQRT_2;
            b.ph[k] = rng.f32();
        }
        b
    }

    /// The classic 7-oscillator supersaw: `detune` and `mix` in 0..1.
    pub fn supersaw(detune: f32, mix: f32, width: f32, gain: f32, rng: &mut Rng) -> Lanes {
        let mut b = Lanes::new(Wave::Saw);
        // Detune curve and mix law after Szabo's analysis of the JP-8000.
        let d = detune.clamp(0.0, 1.0);
        let amt = 0.0003 + 0.035 * d + 0.12 * d.powi(4) + 0.3 * d.powi(11);
        let side = -0.737_64 * mix * mix + 1.284_1 * mix + 0.044_372;
        let centre = -0.553_66 * mix + 0.997_85;
        let norm = gain / (centre * centre + 6.0 * side * side).sqrt();
        for (k, &off) in SUPERSAW.iter().enumerate() {
            b.ratio[k] = 1.0 + off * amt / 0.11;
            let a = if k == 3 { centre } else { side } * norm;
            let p = if k == 3 {
                0.0
            } else {
                (k as f32 - 3.0) / 3.0 * width * if k % 2 == 0 { 1.0 } else { -1.0 }
            };
            let (l, r) = crate::voice::pan_gains(p);
            b.gl[k] = a * l * std::f32::consts::SQRT_2;
            b.gr[k] = a * r * std::f32::consts::SQRT_2;
            b.ph[k] = rng.f32();
        }
        b
    }

    /// Force 2× oversampling on or off (otherwise decided from the first pitch:
    /// on above [`OVERSAMPLE_ABOVE`]). Patches whose filter keeps the top
    /// octave dark gain nothing from it: aliases fold to where the filter is.
    pub fn oversample(&mut self, on: bool) {
        self.os = Some(on);
    }

    /// Set the pitch (`f` Hz at `rate`); `lane_mod` multiplies each lane (ensemble, drift).
    #[inline]
    pub fn set_freq(&mut self, f: f32, rate: f32, lane_mod: &[f32; LANES]) {
        let base = f / rate;
        let os = *self.os.get_or_insert_with(|| {
            let top = (0..LANES)
                .filter(|&k| self.gl[k] != 0.0 || self.gr[k] != 0.0)
                .map(|k| self.ratio[k])
                .fold(0.0, f32::max);
            base * top * rate > OVERSAMPLE_ABOVE
        });
        let scale = if os { 0.5 } else { 1.0 };
        for k in 0..LANES {
            let dt = (base * self.ratio[k] * lane_mod[k] * scale).clamp(1e-7, 0.45);
            self.dt[k] = dt;
            self.idt[k] = 1.0 / dt;
        }
    }

    /// Add the bank into `l`/`r`.
    #[inline(always)]
    pub fn render(&mut self, l: &mut [f32], r: &mut [f32]) {
        if self.os == Some(true) {
            let n = l.len().min(r.len());
            let mut xl = [0.0f32; HB_HIST + 2 * crate::voice::BLOCK];
            let mut xr = [0.0f32; HB_HIST + 2 * crate::voice::BLOCK];
            {
                let (a, b) = (
                    &mut xl[HB_HIST..HB_HIST + 2 * n],
                    &mut xr[HB_HIST..HB_HIST + 2 * n],
                );
                self.render_base(a, b);
            }
            decimate(&mut xl, &mut self.hist_l, &mut l[..n]);
            decimate(&mut xr, &mut self.hist_r, &mut r[..n]);
            return;
        }
        self.render_base(l, r);
    }

    #[inline(always)]
    fn render_base(&mut self, l: &mut [f32], r: &mut [f32]) {
        match self.wave {
            Wave::Saw => self.run(l, r, |t, _, idt, _| saw(t, idt)),
            Wave::Pulse => self.run(l, r, pulse_w),
            Wave::Tri => self.run(l, r, |t, dt, idt, _| tri(t, dt, idt)),
            Wave::Sine => self.run(l, r, |t, _, _, _| sin2pi_unit(t)),
        }
    }

    /// Add the bank into a mono buffer (average of the lane gains).
    #[inline(always)]
    pub fn render_mono(&mut self, x: &mut [f32]) {
        if self.os == Some(true) {
            let n = x.len();
            let mut xm = [0.0f32; HB_HIST + 2 * crate::voice::BLOCK];
            self.render_mono_base(&mut xm[HB_HIST..HB_HIST + 2 * n]);
            decimate(&mut xm, &mut self.hist_l, x);
            return;
        }
        self.render_mono_base(x);
    }

    #[inline(always)]
    fn render_mono_base(&mut self, x: &mut [f32]) {
        match self.wave {
            Wave::Saw => self.run_mono(x, |t, _, idt, _| saw(t, idt)),
            Wave::Pulse => self.run_mono(x, pulse_w),
            Wave::Tri => self.run_mono(x, |t, dt, idt, _| tri(t, dt, idt)),
            Wave::Sine => self.run_mono(x, |t, _, _, _| sin2pi_unit(t)),
        }
    }

    /// Lanes are processed one at a time with the phase computed directly as
    /// `fract(t0 + i·dt)`, so each lane is a pure map over the block and
    /// vectorises across samples (no per-sample recurrence).
    #[inline(always)]
    fn run<F: Fn(f32, f32, f32, f32) -> f32>(&mut self, l: &mut [f32], r: &mut [f32], f: F) {
        let n = l.len().min(r.len());
        let (l, r) = (&mut l[..n], &mut r[..n]);
        let pw = self.pw;
        // Two lanes per pass halves the loads/stores of the output buffers.
        for k in (0..LANES).step_by(2) {
            let (gla, gra, glb, grb) = (self.gl[k], self.gr[k], self.gl[k + 1], self.gr[k + 1]);
            let (a_on, b_on) = (gla != 0.0 || gra != 0.0, glb != 0.0 || grb != 0.0);
            let (ta, dta, idta) = (self.ph[k], self.dt[k], self.idt[k]);
            let (tb, dtb, idtb) = (self.ph[k + 1], self.dt[k + 1], self.idt[k + 1]);
            if a_on && b_on {
                for i in 0..n {
                    let fi = (i + 1) as f32;
                    let va = f(fract_pos(ta + fi * dta), dta, idta, pw);
                    let vb = f(fract_pos(tb + fi * dtb), dtb, idtb, pw);
                    l[i] = l[i] + va * gla + vb * glb;
                    r[i] = r[i] + va * gra + vb * grb;
                }
            } else if a_on {
                for i in 0..n {
                    let va = f(fract_pos(ta + (i + 1) as f32 * dta), dta, idta, pw);
                    l[i] += va * gla;
                    r[i] += va * gra;
                }
            } else if b_on {
                for i in 0..n {
                    let vb = f(fract_pos(tb + (i + 1) as f32 * dtb), dtb, idtb, pw);
                    l[i] += vb * glb;
                    r[i] += vb * grb;
                }
            }
            if a_on {
                self.ph[k] = fract_pos(ta + n as f32 * dta);
            }
            if b_on {
                self.ph[k + 1] = fract_pos(tb + n as f32 * dtb);
            }
        }
    }

    #[inline(always)]
    fn run_mono<F: Fn(f32, f32, f32, f32) -> f32>(&mut self, x: &mut [f32], f: F) {
        let n = x.len();
        let pw = self.pw;
        for k in 0..LANES {
            let g = self.gl[k] + self.gr[k];
            if g == 0.0 {
                continue;
            }
            let g = g * 0.5;
            let (t0, dt, idt) = (self.ph[k], self.dt[k], self.idt[k]);
            for (i, o) in x.iter_mut().enumerate() {
                let t = fract_pos(t0 + (i + 1) as f32 * dt);
                *o += f(t, dt, idt, pw) * g;
            }
            self.ph[k] = fract_pos(t0 + n as f32 * dt);
        }
    }
}

#[inline(always)]
fn pulse_w(t: f32, _dt: f32, idt: f32, pw: f32) -> f32 {
    pulse(t, idt, pw)
}

/// Slow random pitch wander ("analog drift"), deterministic from the seed.
#[derive(Clone, Debug)]
pub struct Drift {
    cur: [f32; LANES],
    target: [f32; LANES],
    rng: Rng,
    count: u32,
    /// Depth in cents.
    pub cents: f32,
}

impl Drift {
    pub fn new(seed: u64, cents: f32) -> Drift {
        let mut rng = Rng::new(seed ^ 0xD81F7);
        let mut cur = [0.0; LANES];
        for c in &mut cur {
            *c = rng.bi() * cents * 0.5;
        }
        Drift {
            cur,
            target: cur,
            rng,
            count: 0,
            cents,
        }
    }
    /// Advance by one control block; writes ratio multipliers (≈ 1 ± cents) into `out`.
    #[inline]
    pub fn step(&mut self, out: &mut [f32; LANES]) {
        if self.cents == 0.0 {
            *out = [1.0; LANES];
            return;
        }
        self.count += 1;
        if self.count >= 48 {
            self.count = 0;
            for t in &mut self.target {
                *t = self.rng.bi() * self.cents;
            }
        }
        for k in 0..LANES {
            self.cur[k] += (self.target[k] - self.cur[k]) * 0.02;
            out[k] = 1.0 + self.cur[k] * (std::f32::consts::LN_2 / 1200.0);
        }
    }
}

/// Phase of a unit oscillator wrapped to [0, 1) (re-exported helper).
#[inline(always)]
pub fn wrap(t: f32) -> f32 {
    fract(t)
}
