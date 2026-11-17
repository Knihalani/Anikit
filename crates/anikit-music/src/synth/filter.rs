//! Zero-delay-feedback (topology-preserving transform) filters.
//!
//! Coefficients are computed at control rate by the caller (`prewarp`) and
//! may be interpolated per sample; the `tick` functions are branch-free.

use super::util::soft;

/// ZDF state-variable filter (Simper/Zavalishin). Gives LP, BP and HP at once.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

/// Precomputed SVF coefficients.
#[derive(Clone, Copy, Debug, Default)]
pub struct SvfCoef {
    pub a1: f32,
    pub a2: f32,
    pub a3: f32,
    pub k: f32,
}

impl SvfCoef {
    /// `g = prewarp(fc)`, `q` resonance (0.5 = none, 0.707 = Butterworth).
    #[inline]
    pub fn new(g: f32, q: f32) -> SvfCoef {
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        SvfCoef {
            a1,
            a2,
            a3: g * a2,
            k,
        }
    }
    /// Linear interpolation between two coefficient sets (same `k` assumed).
    #[inline(always)]
    pub fn lerp(&self, o: &SvfCoef, t: f32) -> SvfCoef {
        SvfCoef {
            a1: self.a1 + (o.a1 - self.a1) * t,
            a2: self.a2 + (o.a2 - self.a2) * t,
            a3: self.a3 + (o.a3 - self.a3) * t,
            k: self.k + (o.k - self.k) * t,
        }
    }
}

impl Svf {
    /// One sample → (low, band, high).
    #[inline(always)]
    pub fn tick(&mut self, v0: f32, c: &SvfCoef) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, v0 - c.k * v1 - v2)
    }
    #[inline(always)]
    pub fn lp(&mut self, v0: f32, c: &SvfCoef) -> f32 {
        self.tick(v0, c).0
    }
    #[inline(always)]
    pub fn bp(&mut self, v0: f32, c: &SvfCoef) -> f32 {
        self.tick(v0, c).1
    }
    #[inline(always)]
    pub fn hp(&mut self, v0: f32, c: &SvfCoef) -> f32 {
        self.tick(v0, c).2
    }
    /// Flush denormal-range state.
    #[inline]
    pub fn flush(&mut self) {
        if self.ic1.abs() < 1e-15 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-15 {
            self.ic2 = 0.0;
        }
    }
}

/// 4-pole transistor-ladder low-pass (ZDF). The linear zero-delay feedback
/// loop is solved exactly each sample; the input stage saturates with a gain
/// taken from the previous sample (drive + resonance compression without
/// lengthening the per-sample dependency chain).
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    s: [f32; 4],
    sat_gain: f32,
}

impl Default for Ladder {
    fn default() -> Ladder {
        Ladder {
            s: [0.0; 4],
            sat_gain: 1.0,
        }
    }
}

/// Precomputed ladder coefficients: `big_g = g/(1+g)`, `inv = 1/(1 + k·G⁴)`.
#[derive(Clone, Copy, Debug, Default)]
pub struct LadderCoef {
    pub big_g: f32,
    pub inv: f32,
    pub k: f32,
}

impl LadderCoef {
    /// `g = prewarp(fc)`, `res` in 0..1 (self-oscillation near 1).
    #[inline]
    pub fn new(g: f32, res: f32) -> LadderCoef {
        let big_g = g / (1.0 + g);
        let k = 3.9 * res.clamp(0.0, 1.0);
        let g4 = big_g * big_g * big_g * big_g;
        LadderCoef {
            big_g,
            inv: 1.0 / (1.0 + k * g4),
            k,
        }
    }
    /// Linear interpolation between two coefficient sets.
    #[inline(always)]
    pub fn lerp(&self, o: &LadderCoef, t: f32) -> LadderCoef {
        LadderCoef {
            big_g: self.big_g + (o.big_g - self.big_g) * t,
            inv: self.inv + (o.inv - self.inv) * t,
            k: self.k,
        }
    }
}

impl Ladder {
    /// One sample. `drive` ≥ 0.1 scales into the saturating input stage.
    #[inline(always)]
    pub fn tick(&mut self, x: f32, c: &LadderCoef, drive: f32) -> f32 {
        let g = c.big_g;
        let h = 1.0 - g;
        let (g2, g3) = (g * g, g * g * g);
        let g4 = g2 * g2;
        let [s1, s2, s3, s4] = self.s;
        // Stage i output = Gⁱ·u + cᵢ, where cᵢ depends only on the states.
        let (hg, hg2, hg3) = (h * g, h * g2, h * g3);
        let c1 = h * s1;
        let c2 = hg * s1 + h * s2;
        let c3 = (hg2 * s1 + hg * s2) + h * s3;
        let c4 = (hg3 * s1 + hg2 * s2) + (hg * s3 + h * s4);
        // Bass-compensated input (keeps low end when resonance rises). Solving
        // y4 = (G⁴·u_lin + c4)·inv with u_lin = xin − k·y4 gives
        // u_lin = xin·(1 − k·G⁴·inv) − k·inv·c4: one multiply-add after c4.
        let xin = x * (1.0 + 0.5 * c.k);
        let ki = c.k * c.inv;
        let lin = xin * (1.0 - ki * g4) - ki * c4;
        let u = lin * self.sat_gain;
        // Saturation gain for the next sample: soft(v)/v, off the critical path.
        let v = lin * drive;
        self.sat_gain = if v.abs() > 1e-4 { soft(v) / v } else { 1.0 };
        // New states 2·yᵢ − sᵢ = 2Gⁱ·u + (2cᵢ − sᵢ).
        let y = g4 * u + c4;
        self.s = [
            (2.0 * g) * u + (2.0 * c1 - s1),
            (2.0 * g2) * u + (2.0 * c2 - s2),
            (2.0 * g3) * u + (2.0 * c3 - s3),
            (2.0 * g4) * u + (2.0 * c4 - s4),
        ];
        y
    }
    /// Flush denormal-range state.
    #[inline]
    pub fn flush(&mut self) {
        for s in &mut self.s {
            if s.abs() < 1e-15 {
                *s = 0.0;
            }
        }
    }
}

/// One-pole low-pass / high-pass (TPT).
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    s: f32,
}

impl OnePole {
    /// Coefficient `G = g/(1+g)` for `g = prewarp(fc)`.
    #[inline]
    pub fn coef(g: f32) -> f32 {
        g / (1.0 + g)
    }
    #[inline(always)]
    pub fn lp(&mut self, x: f32, big_g: f32) -> f32 {
        let v = (x - self.s) * big_g;
        let y = v + self.s;
        self.s = y + v;
        y
    }
    #[inline(always)]
    pub fn hp(&mut self, x: f32, big_g: f32) -> f32 {
        x - self.lp(x, big_g)
    }
    #[inline]
    pub fn flush(&mut self) {
        if self.s.abs() < 1e-15 {
            self.s = 0.0;
        }
    }
}
