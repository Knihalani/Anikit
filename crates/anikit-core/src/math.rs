//! Thin, inlinable math layer.
//!
//! The core is `no_std`, so transcendental functions come from `libm`. On
//! `wasm32` the cheap operations (`sqrt`, `floor`, `abs`, ...) lower to single
//! WebAssembly instructions.

pub const PI: f64 = core::f64::consts::PI;
pub const TAU: f64 = core::f64::consts::TAU;
pub const DEG: f64 = 180.0 / PI;
pub const RAD: f64 = PI / 180.0;

#[inline(always)]
pub fn sqrt(x: f64) -> f64 {
    libm::sqrt(x)
}
#[inline(always)]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}
#[inline(always)]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}
#[inline(always)]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}
#[inline(always)]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}
#[inline(always)]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}
#[inline(always)]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
#[inline(always)]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}
#[inline(always)]
pub fn exp2(x: f64) -> f64 {
    libm::exp2(x)
}
#[inline(always)]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}
#[inline(always)]
pub fn pow(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}
#[inline(always)]
pub fn floor(x: f64) -> f64 {
    libm::floor(x)
}
#[inline(always)]
pub fn ceil(x: f64) -> f64 {
    libm::ceil(x)
}
#[inline(always)]
pub fn round(x: f64) -> f64 {
    libm::round(x)
}
#[inline(always)]
pub fn abs(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & !(1u64 << 63))
}
#[inline(always)]
pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}
#[inline(always)]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
#[inline(always)]
pub fn fmod_pos(x: f64, m: f64) -> f64 {
    let r = x - floor(x / m) * m;
    if r >= m {
        0.0
    } else {
        r
    }
}

/// Interpolate two angles (degrees) along the shortest arc.
#[inline]
pub fn lerp_angle_deg(a: f64, b: f64, t: f64) -> f64 {
    let mut d = fmod_pos(b - a, 360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    a + d * t
}

/// Small deterministic PRNG (mulberry32) for seeded randomness (stagger, wiggle).
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);
impl Rng {
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        t ^ (t >> 14)
    }
    /// Uniform float in `[0, 1)`.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4_294_967_296.0
    }
}

/// Hash a float and an integer into a pseudo random value in `[-1, 1]`.
#[inline]
pub fn hash11(i: i64, seed: u32) -> f64 {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (seed as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

/// 1D gradient-free value noise with smooth (quintic) interpolation, range ~[-1, 1].
#[inline]
pub fn noise1(x: f64, seed: u32) -> f64 {
    let i = floor(x);
    let f = x - i;
    let i = i as i64;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    lerp(hash11(i, seed), hash11(i + 1, seed), u)
}

/// Fast `log2` for positive finite `x` (≈4e-8 absolute error): exponent from
/// the bits, mantissa via an odd atanh series around √2.
#[inline(always)]
pub fn fast_log2(x: f64) -> f64 {
    let bits = x.to_bits();
    let mut e = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mut m = f64::from_bits((bits & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000);
    if m > core::f64::consts::SQRT_2 {
        m *= 0.5;
        e += 1;
    }
    let z = (m - 1.0) / (m + 1.0);
    let z2 = z * z;
    const L: f64 = 2.0 / core::f64::consts::LN_2;
    let p = z * (L + z2 * (L / 3.0 + z2 * (L / 5.0 + z2 * (L / 7.0 + z2 * (L / 9.0)))));
    e as f64 + p
}

/// Fast `2^y` (≈2e-9 relative error for |y| < 1000).
#[inline(always)]
pub fn fast_exp2(y: f64) -> f64 {
    let y = clamp(y, -1000.0, 1000.0);
    let n = floor(y);
    let f = (y - n) * core::f64::consts::LN_2;
    // e^f on [0, ln 2): degree-9 Taylor (truncation < 3e-10).
    let p = 1.0
        + f * (1.0
            + f * (0.5
                + f * (1.0 / 6.0
                    + f * (1.0 / 24.0
                        + f * (1.0 / 120.0
                            + f * (1.0 / 720.0 + f * (1.0 / 5040.0 + f * (1.0 / 40320.0 + f * (1.0 / 362880.0)))))))));
    p * f64::from_bits(((n as i64 + 1023) as u64) << 52)
}

/// Fast `x^y` for `x > 0`.
#[inline(always)]
pub fn fast_pow(x: f64, y: f64) -> f64 {
    fast_exp2(y * fast_log2(x))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fast_math_accuracy() {
        let mut worst: f64 = 0.0;
        for i in 1..20000 {
            let x = i as f64 / 10000.0;
            let exact = libm::pow(x, 1.0 / 2.4);
            worst = worst.max((fast_pow(x, 1.0 / 2.4) - exact).abs() / exact);
            assert!((fast_log2(x) - libm::log2(x)).abs() < 1e-7);
        }
        assert!(worst < 1e-7, "{worst}");
        assert!((fast_exp2(-3.5) - libm::exp2(-3.5)).abs() < 1e-12);
    }
}
