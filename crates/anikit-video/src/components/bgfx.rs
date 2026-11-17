//! Pixel helpers for the backgrounds: colour spaces (sRGB, linear, OKLab),
//! fast 1D/2D value noise, Catmull-Rom upsampling and dithered 8-bit output.
//!
//! Fields are computed in linear light as `f32` and only quantised once, at
//! the very end, with an ordered (interleaved-gradient) dither, so smooth
//! gradients never band.

use crate::dl::Rgba;
use std::sync::OnceLock;

// ---------------------------------------------------------------- colour

/// sRGB component (0..1) to linear light.
#[inline]
pub fn to_lin(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light (0..1) to an sRGB component.
#[inline]
pub fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// An sRGB colour as linear RGB.
pub fn lin3(c: Rgba) -> [f32; 3] {
    [to_lin(c.0[0]), to_lin(c.0[1]), to_lin(c.0[2])]
}

/// Linear RGB to sRGB `Rgba` (opaque).
pub fn from_lin3(c: [f32; 3]) -> Rgba {
    Rgba([to_srgb(c[0]), to_srgb(c[1]), to_srgb(c[2]), 1.0])
}

/// Linear sRGB → OKLab.
pub fn oklab(c: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = c;
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_99 * b).max(0.0).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).max(0.0).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).max(0.0).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab → linear sRGB (may leave gamut; clamp after).
#[inline]
pub fn oklab_inv(c: [f32; 3]) -> [f32; 3] {
    let [ll, a, b] = c;
    let l = ll + 0.396_337_78 * a + 0.215_803_76 * b;
    let m = ll - 0.105_561_346 * a - 0.063_854_17 * b;
    let s = ll - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    [
        (4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s).max(0.0),
        (-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s).max(0.0),
        (-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s).max(0.0),
    ]
}

/// Rotate a colour's hue by `deg` in OKLCh, scaling chroma by `chroma` and
/// adding `dl` to lightness.
pub fn hue_shift(c: Rgba, deg: f32, chroma: f32, dl: f32) -> Rgba {
    let [l, a, b] = oklab(lin3(c));
    let (s, co) = deg.to_radians().sin_cos();
    let (a, b) = ((a * co - b * s) * chroma, (a * s + b * co) * chroma);
    let mut o = from_lin3(oklab_inv([(l + dl).clamp(0.0, 1.0), a, b]));
    o.0[3] = c.0[3];
    o
}

/// Interpolate a list of colours (evenly spaced) in OKLab: `n` linear-RGB samples.
pub fn ramp(cols: &[Rgba], n: usize) -> Vec<[f32; 3]> {
    let labs: Vec<[f32; 3]> = cols.iter().map(|c| oklab(lin3(*c))).collect();
    (0..n)
        .map(|i| {
            let x = i as f32 / (n - 1).max(1) as f32 * (labs.len() - 1) as f32;
            let k = (x.floor() as usize).min(labs.len().saturating_sub(2));
            let f = if labs.len() < 2 { 0.0 } else { x - k as f32 };
            let (a, b) = (labs[k], labs[(k + 1).min(labs.len() - 1)]);
            oklab_inv([0, 1, 2].map(|j| a[j] + (b[j] - a[j]) * f))
        })
        .collect()
}

// ---------------------------------------------------------------- noise

#[inline]
pub fn hash_u(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Hash of an integer lattice point to `[-1, 1)`.
#[inline]
pub fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x27d4_eb2d) ^ (y as u32).wrapping_mul(0x1656_67b1) ^ seed.wrapping_mul(0x9E37_79B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 * (2.0 / (1u32 << 24) as f32) - 1.0
}

#[inline]
fn quintic(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Smooth 2D value noise in `[-1, 1]` (C² continuous).
#[inline]
pub fn vnoise2(x: f32, y: f32, seed: u32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i32, yf as i32);
    let (u, v) = (quintic(x - xf), quintic(y - yf));
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    let ab = a + (b - a) * u;
    let cd = c + (d - c) * u;
    ab + (cd - ab) * v
}

/// Gradient-free 1D noise that evolves with time `t`: value noise over (x, t).
#[inline]
pub fn vnoise1t(x: f32, t: f32, seed: u32) -> f32 {
    vnoise2(x, t, seed)
}

/// Fractal sum of [`vnoise2`], normalised to about `[-1, 1]`.
pub fn fbm2(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut s, mut a, mut f, mut n) = (0.0, 1.0, 1.0, 0.0);
    for o in 0..octaves {
        // Rotate each octave a little so lattice axes don't line up.
        let (rx, ry) = (x * f * 0.96 - y * f * 0.28, x * f * 0.28 + y * f * 0.96);
        s += a * vnoise2(rx + o as f32 * 17.13, ry - o as f32 * 7.7, seed.wrapping_add(o * 101));
        n += a;
        a *= 0.5;
        f *= 2.03;
    }
    s / n
}

#[inline]
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------- images

/// A linear-light RGB float image.
pub struct Lin {
    pub w: usize,
    pub h: usize,
    pub px: Vec<f32>,
}

impl Lin {
    pub fn new(w: usize, h: usize) -> Lin {
        Lin { w, h, px: vec![0.0; w * h * 3] }
    }

    /// Catmull-Rom (separable, interpolating) resample to `dw × dh`.
    pub fn upsample(&self, dw: usize, dh: usize) -> Lin {
        let tx = cubic_taps(self.w, dw);
        let ty = cubic_taps(self.h, dh);
        // Horizontal pass: every source row → dw columns.
        let mut mid = vec![0.0f32; dw * self.h * 3];
        for y in 0..self.h {
            let src = &self.px[y * self.w * 3..(y + 1) * self.w * 3];
            let dst = &mut mid[y * dw * 3..(y + 1) * dw * 3];
            for (x, (i, w)) in tx.iter().enumerate() {
                let mut acc = [0.0f32; 3];
                for k in 0..4 {
                    let s = &src[i[k] * 3..i[k] * 3 + 3];
                    acc[0] += s[0] * w[k];
                    acc[1] += s[1] * w[k];
                    acc[2] += s[2] * w[k];
                }
                dst[x * 3..x * 3 + 3].copy_from_slice(&acc);
            }
        }
        // Vertical pass.
        let mut out = Lin::new(dw, dh);
        let row = dw * 3;
        for (y, (i, w)) in ty.iter().enumerate() {
            let dst = &mut out.px[y * row..(y + 1) * row];
            let (r0, r1, r2, r3) = (&mid[i[0] * row..][..row], &mid[i[1] * row..][..row], &mid[i[2] * row..][..row], &mid[i[3] * row..][..row]);
            for x in 0..row {
                dst[x] = (r0[x] * w[0] + r1[x] * w[1] + r2[x] * w[2] + r3[x] * w[3]).max(0.0);
            }
        }
        out
    }

    /// Quantise to straight RGBA8 (opaque) with an ordered dither of ±½ LSB
    /// in sRGB, so no gradient bands at 8 bits.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let lut = srgb_lut();
        let tile = dither_tile();
        let mut out = vec![255u8; self.w * self.h * 4];
        let lut = &lut[..=LUT_N];
        for y in 0..self.h {
            let src = &self.px[y * self.w * 3..(y + 1) * self.w * 3];
            let dst = &mut out[y * self.w * 4..(y + 1) * self.w * 4];
            let trow = &tile[(y % DT) * DT..(y % DT + 1) * DT];
            for (x, (s, d)) in src.chunks_exact(3).zip(dst.chunks_exact_mut(4)).enumerate() {
                // TPDF offset around ½: `floor` below then rounds to nearest on average.
                let dz = trow[x % DT];
                for k in 0..3 {
                    // Nearest entry: the table is dense enough (≤ 0.05 LSB per step).
                    let i = ((s[k] * LUT_N as f32 + 0.5) as usize).min(LUT_N);
                    d[k] = (lut[i] + dz).clamp(0.0, 255.0) as u8;
                }
            }
        }
        out
    }
}

/// Interleaved gradient noise in `[0, 1)`: a cheap, blue-ish ordered dither.
#[inline]
pub fn ign(x: f32, y: f32) -> f32 {
    let f = 0.067_110_56 * x + 0.005_837_15 * y;
    (52.982_918 * (f - f.floor())).fract()
}

const DT: usize = 64;

/// A 64×64 tile of triangular (TPDF) white noise, offset to `[0, 1)` on
/// average ±1 LSB: no lattice pattern even when the image is upscaled, and
/// no noise modulation with the signal level.
fn dither_tile() -> &'static [f32] {
    static T: OnceLock<Vec<f32>> = OnceLock::new();
    T.get_or_init(|| {
        (0..DT * DT)
            .map(|i| {
                let a = (hash_u(i as u32 ^ 0xA511_E9B3) >> 8) as f32 / (1u32 << 24) as f32;
                let b = (hash_u(i as u32 ^ 0x63D8_3595) >> 8) as f32 / (1u32 << 24) as f32;
                a + b - 0.5
            })
            .collect()
    })
}

const LUT_N: usize = 65536;

/// Linear light → sRGB·255, sampled densely enough (≤ 0.05 LSB per step)
/// to read without interpolation.
fn srgb_lut() -> &'static [f32] {
    static L: OnceLock<Vec<f32>> = OnceLock::new();
    L.get_or_init(|| (0..=LUT_N).map(|i| to_srgb(i as f32 / LUT_N as f32) * 255.0).collect())
}

/// `1 - e^(-x)` for `x ≥ 0`, accurate to ~1e-4 and vectorisable.
#[inline]
pub fn one_minus_exp_neg(x: f32) -> f32 {
    // e^-x = 2^(-x·log2 e); split into integer and fractional parts.
    let y = (-x * std::f32::consts::LOG2_E).max(-126.0);
    let fl = y.floor();
    let f = y - fl;
    // 2^f on [0, 1): degree-4 polynomial.
    let p = 1.0 + f * (std::f32::consts::LN_2 + f * (0.240_226_5 + f * (0.055_504_11 + f * 0.009_618_13)));
    let scale = f32::from_bits(((fl as i32 + 127) as u32) << 23);
    1.0 - p * scale
}

/// Catmull-Rom taps (indices clamped at the borders) mapping `n` sources to `m` outputs.
fn cubic_taps(n: usize, m: usize) -> Vec<([usize; 4], [f32; 4])> {
    let sc = n as f32 / m as f32;
    (0..m)
        .map(|i| {
            let c = (i as f32 + 0.5) * sc - 0.5;
            let f0 = c.floor();
            let t = c - f0;
            let b = f0 as i32;
            let idx = [b - 1, b, b + 1, b + 2].map(|k| k.clamp(0, n as i32 - 1) as usize);
            let (t2, t3) = (t * t, t * t * t);
            let w = [
                0.5 * (-t3 + 2.0 * t2 - t),
                0.5 * (3.0 * t3 - 5.0 * t2 + 2.0),
                0.5 * (-3.0 * t3 + 4.0 * t2 + t),
                0.5 * (t3 - t2),
            ];
            (idx, w)
        })
        .collect()
}

/// Sample a 1D array at a fractional index (linear, clamped).
#[inline]
pub fn sample1(a: &[f32], x: f32) -> f32 {
    let n = a.len();
    let x = x.clamp(0.0, (n - 1) as f32);
    let i = (x as usize).min(n - 2);
    let f = x - i as f32;
    a[i] + (a[i + 1] - a[i]) * f
}

/// Box blur a 1D array in place (`passes` times, radius `r`), clamped edges.
pub fn blur1(a: &mut [f32], r: usize, passes: usize) {
    if r == 0 || a.len() < 2 {
        return;
    }
    let n = a.len();
    let mut tmp = vec![0.0f32; n];
    let k = 1.0 / (2 * r + 1) as f32;
    for _ in 0..passes {
        let mut s: f32 = (0..=2 * r).map(|j| a[(j as isize - r as isize).clamp(0, n as isize - 1) as usize]).sum();
        for i in 0..n {
            tmp[i] = s * k;
            let add = (i + r + 1).min(n - 1);
            let sub = (i as isize - r as isize).clamp(0, n as isize - 1) as usize;
            s += a[add] - a[sub];
        }
        a.copy_from_slice(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oklab_roundtrip() {
        for c in [[0.2, 0.5, 0.9], [1.0, 0.0, 0.0], [0.01, 0.02, 0.0]] {
            let r = oklab_inv(oklab(c));
            for k in 0..3 {
                assert!((r[k] - c[k]).abs() < 1e-3, "{c:?} -> {r:?}");
            }
        }
    }
    #[test]
    fn dither_preserves_mean() {
        // A flat 10.3/255 sRGB field averages to ~10.3 after dithering.
        let v = to_lin(10.3 / 255.0);
        let mut l = Lin::new(64, 64);
        l.px.iter_mut().for_each(|p| *p = v);
        let px = l.to_rgba8();
        let mean = px.chunks(4).map(|p| p[0] as f32).sum::<f32>() / 4096.0;
        assert!((mean - 10.3).abs() < 0.08, "{mean}");
    }
}
