//! Direct pixel evaluation for effect components whose look is cheaper to
//! compute per pixel than to rasterise as hundreds of vector ops (god-ray
//! fans, additive particle splats). The result is one straight-alpha RGBA8
//! image per frame (see [`super::Assets::image`]), composited with
//! source-over exactly where the vector ops would have been.
//!
//! Colours follow the renderer's conventions: sRGB components, gradients
//! interpolated in premultiplied sRGB, pixels sampled at their centres.

use crate::dl::Rgba;

/// Premultiplied sRGB colour `[r·a, g·a, b·a, a]`, clamped like the renderer's gradient stops.
#[inline]
pub fn premul(c: Rgba) -> [f32; 4] {
    let a = c.0[3].clamp(0.0, 1.0);
    [c.0[0].clamp(0.0, 1.0) * a, c.0[1].clamp(0.0, 1.0) * a, c.0[2].clamp(0.0, 1.0) * a, a]
}

#[inline(always)]
pub fn lerp4(a: [f32; 4], b: [f32; 4], f: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, a[3] + (b[3] - a[3]) * f]
}

/// `src` over `dst`, both premultiplied.
#[inline(always)]
pub fn over(src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    let k = 1.0 - src[3];
    [src[0] + dst[0] * k, src[1] + dst[1] * k, src[2] + dst[2] * k, src[3] + dst[3] * k]
}

/// A gradient's premultiplied colour at `x` (stops sorted by offset; padded at both ends).
pub fn stops_at(stops: &[(f32, Rgba)], x: f32) -> [f32; 4] {
    let Some(first) = stops.first() else { return [0.0; 4] };
    if x <= first.0 {
        return premul(first.1);
    }
    for w in stops.windows(2) {
        let ((o0, c0), (o1, c1)) = (w[0], w[1]);
        if x <= o1 {
            let f = if o1 > o0 { (x - o0) / (o1 - o0) } else { 1.0 };
            return lerp4(premul(c0), premul(c1), f);
        }
    }
    premul(stops[stops.len() - 1].1)
}

/// A radial gradient (premultiplied) tabulated over distance from its
/// centre, read with linear interpolation: exact for the piecewise-linear
/// stop ramp up to the table step. Zero beyond `radius`.
pub struct RadialLut {
    inv_step: f32,
    max: f32,
    v: Vec<[f32; 4]>,
}

impl RadialLut {
    /// `stops` over `0..radius` px, sampled every `step` px.
    pub fn new(radius: f32, stops: &[(f32, Rgba)], step: f32) -> RadialLut {
        let radius = radius.max(1e-3);
        let n = (radius / step).ceil() as usize + 1;
        let mut v: Vec<[f32; 4]> = (0..n).map(|i| stops_at(stops, (i as f32 * step / radius).min(1.0))).collect();
        // The fill is the disc of `radius`: nothing outside it.
        v.push([0.0; 4]);
        v.push([0.0; 4]);
        RadialLut { inv_step: 1.0 / step, max: (v.len() - 2) as f32, v }
    }

    /// An empty table (transparent everywhere).
    pub fn none() -> RadialLut {
        RadialLut { inv_step: 0.0, max: 0.0, v: vec![[0.0; 4]; 2] }
    }

    /// Radius beyond which the table is transparent.
    pub fn reach(&self) -> f32 {
        if self.inv_step > 0.0 {
            (self.max - 1.0) / self.inv_step
        } else {
            0.0
        }
    }

    #[inline(always)]
    pub fn at(&self, r: f32) -> [f32; 4] {
        let x = (r * self.inv_step).min(self.max);
        let i = x as usize;
        let f = x - i as f32;
        lerp4(self.v[i], self.v[i + 1], f)
    }
}

/// `atan2(y, x)` in `[-π, π]`, branch-free (vectorises), max error ≈ 2e-6 rad.
#[inline(always)]
pub fn atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (ax, ay) = (x.abs(), y.abs());
    let mx = ax.max(ay).max(1e-30);
    let a = ax.min(ay) / mx;
    let s = a * a;
    let p = a * (0.999_979_8 + s * (-0.332_655_4 + s * (0.193_669_9 + s * (-0.116_65 + s * (0.052_822_2 + s * -0.011_769_97)))));
    let p = if ay > ax { FRAC_PI_2 - p } else { p };
    let p = if x < 0.0 { PI - p } else { p };
    if y < 0.0 {
        -p
    } else {
        p
    }
}

/// `max` as a plain compare-and-select (vectorises; no NaN handling).
#[inline(always)]
pub fn mx(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

/// `min` as a plain compare-and-select (vectorises; no NaN handling).
#[inline(always)]
pub fn mn(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// Round `v` (clamped to 0..255) to the nearest integer as a byte, branch-free:
/// adding 2²³ leaves the rounded integer in the low mantissa bits.
#[inline(always)]
pub fn byte(v: f32) -> u8 {
    (mn(mx(v, 0.0), 255.0) + 8_388_608.0).to_bits() as u8
}

/// Premultiplied float RGBA (planar rows) to straight RGBA8, rounded (vectorises).
#[inline(always)]
pub fn pack_row(r: &[f32], g: &[f32], b: &[f32], a: &[f32], out: &mut [u8]) {
    let n = out.len() / 4;
    let (r, g, b, a, out) = (&r[..n], &g[..n], &b[..n], &a[..n], &mut out[..n * 4]);
    for i in 0..n {
        let al = mn(mx(a[i], 0.0), 1.0);
        let k = 255.0 / mx(al, 1e-6);
        out[i * 4] = byte(r[i] * k);
        out[i * 4 + 1] = byte(g[i] * k);
        out[i * 4 + 2] = byte(b[i] * k);
        out[i * 4 + 3] = byte(al * 255.0);
    }
}

/// Fast 64-bit hash of `f32` slices (image cache keys from computed tables).
pub struct Hash(u64);

impl Default for Hash {
    fn default() -> Self {
        Self::new()
    }
}

impl Hash {
    pub fn new() -> Hash {
        Hash(0xcbf2_9ce4_8422_2325)
    }
    #[inline]
    pub fn u32(&mut self, v: u32) {
        self.0 = (self.0.rotate_left(5) ^ v as u64).wrapping_mul(0x51_7cc1_b727_220a_95);
    }
    pub fn f32s(&mut self, v: &[f32]) {
        for x in v {
            self.u32(x.to_bits());
        }
    }
    pub fn bytes(&mut self, v: &[u8]) {
        for x in v {
            self.u32(*x as u32);
        }
    }
    pub fn finish(&self) -> u64 {
        let mut h = self.0;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
        h ^ (h >> 33)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atan2_is_accurate() {
        let mut worst = 0.0f32;
        for i in 0..20000 {
            let a = i as f32 * 0.000_314_2;
            for r in [0.5f32, 3.0, 1700.0] {
                let (y, x) = ((a - 3.14).sin() * r, (a - 3.14).cos() * r);
                worst = worst.max((atan2(y, x) - y.atan2(x)).abs());
            }
        }
        assert!(worst < 5e-6, "{worst}");
        assert_eq!(atan2(0.0, 0.0), 0.0);
    }

    #[test]
    fn radial_lut_matches_stops() {
        let c = Rgba([1.0, 0.5, 0.25, 1.0]);
        let stops = vec![(0.0, Rgba([1.0, 0.5, 0.25, 0.8])), (0.1, Rgba([1.0, 0.5, 0.25, 0.3])), (1.0, Rgba([1.0, 0.5, 0.25, 0.0]))];
        let l = RadialLut::new(200.0, &stops, 0.5);
        for r in [0.0f32, 7.3, 20.0, 133.3, 199.9] {
            let e = stops_at(&stops, r / 200.0);
            let g = l.at(r);
            for k in 0..4 {
                assert!((e[k] - g[k]).abs() < 2e-3, "{r} {e:?} {g:?}");
            }
        }
        assert_eq!(l.at(250.0), [0.0; 4]);
        let _ = c;
    }
}
