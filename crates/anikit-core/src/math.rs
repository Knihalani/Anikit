//! Thin, inlinable math layer.
//!
//! The core is `no_std`, so transcendental functions come from `libm`. On
//! `wasm32` the cheap operations (`sqrt`, `floor`, `ceil`) call the single
//! WebAssembly instructions directly.

pub const PI: f64 = core::f64::consts::PI;
pub const TAU: f64 = core::f64::consts::TAU;
pub const DEG: f64 = 180.0 / PI;
pub const RAD: f64 = PI / 180.0;

// `libm` only uses hardware instructions with nightly-only features, and the
// scalar `core::arch::wasm32::f32_sqrt`-style intrinsics are unstable too. The
// SIMD intrinsics are stable, so on wasm32+simd128 the cheap operations run
// one lane of the vector instruction (which the backend lowers to the scalar
// `f32.sqrt` / `f64.floor` / ... instructions). Elsewhere libm inlines to the
// platform instructions.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod hw {
    use core::arch::wasm32 as w;
    #[inline(always)]
    pub fn sqrt(x: f64) -> f64 {
        w::f64x2_extract_lane::<0>(w::f64x2_sqrt(w::f64x2_splat(x)))
    }
    #[inline(always)]
    pub fn floor(x: f64) -> f64 {
        w::f64x2_extract_lane::<0>(w::f64x2_floor(w::f64x2_splat(x)))
    }
    #[inline(always)]
    pub fn ceil(x: f64) -> f64 {
        w::f64x2_extract_lane::<0>(w::f64x2_ceil(w::f64x2_splat(x)))
    }
    #[inline(always)]
    pub fn sqrtf(x: f32) -> f32 {
        w::f32x4_extract_lane::<0>(w::f32x4_sqrt(w::f32x4_splat(x)))
    }
    #[inline(always)]
    pub fn floorf(x: f32) -> f32 {
        w::f32x4_extract_lane::<0>(w::f32x4_floor(w::f32x4_splat(x)))
    }
    #[inline(always)]
    pub fn ceilf(x: f32) -> f32 {
        w::f32x4_extract_lane::<0>(w::f32x4_ceil(w::f32x4_splat(x)))
    }
}
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
mod hw {
    pub use libm::{ceil, ceilf, floor, floorf, sqrt, sqrtf};
}
pub use hw::{ceilf, floorf, sqrtf};

#[inline(always)]
pub fn sqrt(x: f64) -> f64 {
    hw::sqrt(x)
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
/// `(sin x, cos x)` sharing one argument reduction; bit-identical to calling
/// [`sin`] and [`cos`] separately (same libm kernels).
#[inline(always)]
pub fn sincos(x: f64) -> (f64, f64) {
    libm::sincos(x)
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
    hw::floor(x)
}
#[inline(always)]
pub fn ceil(x: f64) -> f64 {
    hw::ceil(x)
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

/// `x^(5/12)` (= `x^(1/2.4)`, the sRGB encode exponent) for positive normal
/// `x`, relative error < 5e-9 — about 20× tighter than [`fast_pow`] and
/// several times cheaper: no log/exp, no division, no floor. Splits
/// `x = m·2^e`, so `x^(5/12) = m^(5/12) · 2^(5e/12)`; with `5e = 12q + r`
/// the second factor is `2^q` (exponent bits) times a 12-entry table, and
/// `m^(5/12)` on `[1, 2)` is a degree-8 Chebyshev fit evaluated with
/// Estrin's scheme (short dependency chains).
#[inline(always)]
pub fn pow_5_12(x: f64) -> f64 {
    const TWELFTHS: [f64; 12] = [
        1.0,
        1.0594630943592953,
        1.122462048309373,
        1.189207115002721,
        1.2599210498948732,
        1.3348398541700344,
        core::f64::consts::SQRT_2,
        1.4983070768766815,
        1.5874010519681994,
        1.681792830507429,
        1.7817974362806785,
        1.8877486253633868,
    ];
    const C: [f64; 9] = [
        1.184053586836625,
        0.3289037087059427,
        -0.0639534049887947,
        0.022505612795755314,
        -0.009691795145395442,
        0.004580285364930706,
        -0.0023205301721254943,
        0.0015018739497418565,
        -0.0008479898563599124,
    ];
    let bits = x.to_bits();
    let e = ((bits >> 52) & 0x7ff) as i32 - 1023;
    let u = f64::from_bits((bits & 0x000F_FFFF_FFFF_FFFF) | 0x3FF0_0000_0000_0000) - 1.5;
    let k = 5 * e;
    let (q, r) = (k.div_euclid(12), k.rem_euclid(12) as usize);
    let u2 = u * u;
    let u4 = u2 * u2;
    let p = (C[0] + C[1] * u) + u2 * (C[2] + C[3] * u) + u4 * ((C[4] + C[5] * u) + u2 * (C[6] + C[7] * u) + u4 * C[8]);
    p * TWELFTHS[r] * f64::from_bits(((q + 1023) as u64) << 52)
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

    #[test]
    fn pow_5_12_accuracy() {
        // The sRGB encode range (and well beyond, for out-of-gamut values).
        let mut worst: f64 = 0.0;
        let mut x = 0.0031308;
        while x < 1e6 {
            let exact = libm::pow(x, 5.0 / 12.0);
            worst = worst.max((pow_5_12(x) - exact).abs() / exact);
            x *= 1.000_37;
        }
        for i in 0..100_000 {
            let x = 0.0031308 + i as f64 / 100_000.0;
            let exact = libm::pow(x, 1.0 / 2.4);
            worst = worst.max((pow_5_12(x) - exact).abs() / exact);
        }
        assert!(worst < 5e-9, "{worst}");
        assert!((pow_5_12(1.0) - 1.0).abs() < 5e-9);
        assert!(pow_5_12(f64::INFINITY).is_finite());
    }

    #[test]
    fn sincos_matches_sin_and_cos_bit_for_bit() {
        let mut rng = Rng(99);
        for i in 0..200_000 {
            // Small, medium (several reductions) and huge arguments, plus specials.
            let x = match i % 4 {
                0 => (rng.next_f64() - 0.5) * 2.0,
                1 => (rng.next_f64() - 0.5) * 200.0,
                2 => (rng.next_f64() - 0.5) * 1e9,
                _ => [0.0, -0.0, 1e-300, PI, -PI / 4.0, f64::INFINITY, f64::NAN][i % 7],
            };
            let (s, c) = sincos(x);
            assert_eq!(s.to_bits(), sin(x).to_bits(), "sin {x}");
            assert_eq!(c.to_bits(), cos(x).to_bits(), "cos {x}");
        }
    }
}
