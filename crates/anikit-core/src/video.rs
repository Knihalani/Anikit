//! Video frame processing for renderers and encoders.
//!
//! * [`FrameMixer`] accumulates sub-frames for motion blur (a film shutter):
//!   add N RGBA frames, read back their average. Integer maths, no division
//!   in the inner loop.
//! * [`rgba_to_i420`] converts RGBA/BGRA pixels to planar YUV 4:2:0
//!   (BT.709, limited range) — the format H.264/VP9 encoders consume — so an
//!   encoder pipe carries 1.5 bytes per pixel instead of 4 and skips its own
//!   colour conversion.

use alloc::vec;
use alloc::vec::Vec;

/// Averages sub-frames (motion blur).
#[derive(Debug, Default)]
pub struct FrameMixer {
    pub width: usize,
    pub height: usize,
    acc: Vec<u16>,
    /// Number of frames accumulated since the last [`FrameMixer::reset`].
    pub count: u32,
    /// The averaged frame (RGBA) after [`FrameMixer::resolve`].
    pub out: Vec<u8>,
}

impl FrameMixer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, acc: vec![0; width * height * 4], count: 0, out: vec![0; width * height * 4] }
    }

    pub fn reset(&mut self) {
        self.acc.iter_mut().for_each(|v| *v = 0);
        self.count = 0;
    }

    /// Add one RGBA frame (`width * height * 4` bytes). Up to 257 frames.
    pub fn add(&mut self, rgba: &[u8]) {
        for (a, &p) in self.acc.iter_mut().zip(rgba) {
            *a += p as u16;
        }
        self.count += 1;
    }

    /// Average of the accumulated frames into [`FrameMixer::out`].
    pub fn resolve(&mut self) -> &[u8] {
        let n = self.count.max(1);
        // Fixed-point reciprocal: (a * inv + half) >> 16 == round(a / n).
        let inv = (65536 + n / 2) / n;
        for (o, &a) in self.out.iter_mut().zip(&self.acc) {
            *o = ((a as u32 * inv + 32768) >> 16).min(255) as u8;
        }
        &self.out
    }
}

/// Pixel byte order of the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Rgba,
    Bgra,
}

/// Bytes needed for an I420 frame of `w × h` (even dimensions).
pub fn i420_len(w: usize, h: usize) -> usize {
    w * h + 2 * ((w / 2) * (h / 2))
}

/// Convert packed RGBA/BGRA to planar I420 (Y, then U, then V), BT.709
/// limited range. `w` and `h` must be even. `out` needs [`i420_len`] bytes.
pub fn rgba_to_i420(src: &[u8], w: usize, h: usize, order: Order, out: &mut [u8]) {
    let (ri, bi) = if order == Order::Rgba { (0, 2) } else { (2, 0) };
    let (ysz, csz) = (w * h, (w / 2) * (h / 2));
    let (yp, rest) = out.split_at_mut(ysz);
    let (up, vp) = rest.split_at_mut(csz);
    // BT.709 limited-range coefficients × 256.
    const YR: i32 = 47;
    const YG: i32 = 157;
    const YB: i32 = 16;
    for y in 0..h {
        let row = &src[y * w * 4..(y + 1) * w * 4];
        let yo = &mut yp[y * w..(y + 1) * w];
        let done = luma_simd(row, yo, ri, bi);
        for (px, yv) in row[done * 4..].chunks_exact(4).zip(yo[done..].iter_mut()) {
            let (r, g, b) = (px[ri] as i32, px[1] as i32, px[bi] as i32);
            *yv = (((YR * r + YG * g + YB * b + 128) >> 8) + 16) as u8;
        }
    }
    let cw = w / 2;
    for cy in 0..h / 2 {
        let r0 = &src[(2 * cy) * w * 4..(2 * cy + 1) * w * 4];
        let r1 = &src[(2 * cy + 1) * w * 4..(2 * cy + 2) * w * 4];
        let uo = &mut up[cy * cw..(cy + 1) * cw];
        let vo = &mut vp[cy * cw..(cy + 1) * cw];
        for (((a, b), u), v) in r0.chunks_exact(8).zip(r1.chunks_exact(8)).zip(uo.iter_mut()).zip(vo.iter_mut()) {
            // Average the 2×2 block.
            let r = (a[ri] as i32 + a[4 + ri] as i32 + b[ri] as i32 + b[4 + ri] as i32 + 2) >> 2;
            let g = (a[1] as i32 + a[5] as i32 + b[1] as i32 + b[5] as i32 + 2) >> 2;
            let bl = (a[bi] as i32 + a[4 + bi] as i32 + b[bi] as i32 + b[4 + bi] as i32 + 2) >> 2;
            *u = (((-26 * r - 86 * g + 112 * bl + 128) >> 8) + 128).clamp(0, 255) as u8;
            *v = (((112 * r - 102 * g - 10 * bl + 128) >> 8) + 128).clamp(0, 255) as u8;
        }
    }
}

/// Film-look post-processing parameters (all off at 0).
#[derive(Clone, Copy, Debug, Default)]
pub struct Post {
    /// Bloom: brightness threshold (0…1), strength, and blur radius in pixels.
    pub bloom_threshold: f32,
    pub bloom: f32,
    pub bloom_radius: f32,
    /// Radial chromatic aberration: red/blue offset in pixels at the corners.
    pub aberration: f32,
    /// Vignette strength (0…1).
    pub vignette: f32,
    /// Film grain amplitude in 8-bit levels, re-seeded per frame.
    pub grain: f32,
    pub seed: u32,
}

/// Scratch buffers for [`Post`], reused across frames.
#[derive(Debug, Default)]
pub struct PostFx {
    small: Vec<f32>,
    tmp: Vec<f32>,
    copy: Vec<u8>,
}

impl PostFx {
    /// Apply `p` to an RGBA frame in place.
    pub fn apply(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        if p.aberration > 0.0 {
            self.aberrate(rgba, w, h, p.aberration);
        }
        if p.bloom > 0.0 {
            self.bloom(rgba, w, h, p);
        }
        if p.vignette > 0.0 || p.grain > 0.0 {
            let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
            let inv = 1.0 / (cx * cx + cy * cy);
            let mut s = p.seed.wrapping_mul(2_654_435_761) | 1;
            for y in 0..h {
                let dy = y as f32 - cy;
                let row = &mut rgba[y * w * 4..(y + 1) * w * 4];
                for (x, px) in row.chunks_exact_mut(4).enumerate() {
                    let dx = x as f32 - cx;
                    let r2 = (dx * dx + dy * dy) * inv;
                    let k = 1.0 - p.vignette * r2 * r2 * 1.4;
                    s ^= s << 13;
                    s ^= s >> 17;
                    s ^= s << 5;
                    let n = ((s >> 24) as f32 / 255.0 - 0.5) * 2.0 * p.grain;
                    for c in px[..3].iter_mut() {
                        *c = (*c as f32 * k + n).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    fn aberrate(&mut self, rgba: &mut [u8], w: usize, h: usize, amount: f32) {
        self.copy.clear();
        self.copy.extend_from_slice(rgba);
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let norm = amount / crate::math::sqrtf(cx * cx + cy * cy).max(1.0);
        for y in 0..h {
            let dy = y as f32 - cy;
            for x in 0..w {
                let dx = x as f32 - cx;
                // Red pushed outward, blue pulled inward, proportional to radius.
                let (ox, oy) = (dx * norm, dy * norm);
                let sample = |sx: f32, sy: f32, c: usize| -> u8 {
                    let ix = (crate::math::floorf(sx + 0.5) as isize).clamp(0, w as isize - 1) as usize;
                    let iy = (crate::math::floorf(sy + 0.5) as isize).clamp(0, h as isize - 1) as usize;
                    self.copy[(iy * w + ix) * 4 + c]
                };
                let i = (y * w + x) * 4;
                rgba[i] = sample(x as f32 - ox, y as f32 - oy, 0);
                rgba[i + 2] = sample(x as f32 + ox, y as f32 + oy, 2);
            }
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn bloom(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        // Bright pass at quarter resolution, two box blurs (≈ gaussian), add back.
        let (sw, sh) = (w.div_ceil(4), h.div_ceil(4));
        self.small.resize(sw * sh * 3, 0.0);
        self.tmp.resize(sw * sh * 3, 0.0);
        let th = p.bloom_threshold * 255.0;
        for sy in 0..sh {
            for sx in 0..sw {
                let mut acc = [0.0f32; 3];
                let mut n = 0.0;
                for y in sy * 4..(sy * 4 + 4).min(h) {
                    for x in sx * 4..(sx * 4 + 4).min(w) {
                        let px = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 3];
                        let l = 0.2126 * px[0] as f32 + 0.7152 * px[1] as f32 + 0.0722 * px[2] as f32;
                        let k = ((l - th) / (255.0 - th + 1.0)).max(0.0);
                        for c in 0..3 {
                            acc[c] += px[c] as f32 * k;
                        }
                        n += 1.0;
                    }
                }
                let o = (sy * sw + sx) * 3;
                for c in 0..3 {
                    self.small[o + c] = acc[c] / n;
                }
            }
        }
        let r = (crate::math::floorf(p.bloom_radius / 4.0 + 0.5) as usize).max(1);
        for _ in 0..2 {
            box_blur(&self.small, &mut self.tmp, sw, sh, r, true);
            box_blur(&self.tmp, &mut self.small, sw, sh, r, false);
        }
        for y in 0..h {
            let sy = (y / 4).min(sh - 1);
            for x in 0..w {
                let sx = (x / 4).min(sw - 1);
                let o = (sy * sw + sx) * 3;
                let i = (y * w + x) * 4;
                for c in 0..3 {
                    rgba[i + c] = (rgba[i + c] as f32 + self.small[o + c] * p.bloom).min(255.0) as u8;
                }
            }
        }
    }
}

/// Running-sum box blur of an RGB f32 image along x (`horizontal`) or y.
#[allow(clippy::needless_range_loop)]
fn box_blur(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horizontal: bool) {
    let (len, lines) = if horizontal { (w, h) } else { (h, w) };
    let idx = |line: usize, k: usize| if horizontal { (line * w + k) * 3 } else { (k * w + line) * 3 };
    let norm = 1.0 / (2 * r + 1) as f32;
    for line in 0..lines {
        let mut sum = [0.0f32; 3];
        for k in 0..=r.min(len - 1) {
            for c in 0..3 {
                sum[c] += src[idx(line, k) + c];
            }
        }
        for c in 0..3 {
            sum[c] += src[idx(line, 0) + c] * r as f32;
        }
        for k in 0..len {
            let o = idx(line, k);
            for c in 0..3 {
                dst[o + c] = sum[c] * norm;
            }
            let add = idx(line, (k + r + 1).min(len - 1));
            let sub = idx(line, k.saturating_sub(r));
            for c in 0..3 {
                sum[c] += src[add + c] - src[sub + c];
            }
        }
    }
}

/// Luma for 8 pixels per step with 128-bit WebAssembly SIMD (safe intrinsics).
/// Returns how many pixels were converted; the caller finishes the tail.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn luma_simd(row: &[u8], out: &mut [u8], ri: usize, bi: usize) -> usize {
    use core::arch::wasm32::*;
    let n = out.len() / 8 * 8;
    let (sr, sb) = ((ri * 8) as u32, (bi * 8) as u32);
    let m = u32x4_splat(0xff);
    let (cr, cg, cb) = (i32x4_splat(47), i32x4_splat(157), i32x4_splat(16));
    let k = i32x4_splat(128 + (16 << 8));
    let luma4 = |v: v128| {
        let r = v128_and(u32x4_shr(v, sr), m);
        let g = v128_and(u32x4_shr(v, 8), m);
        let b = v128_and(u32x4_shr(v, sb), m);
        i32x4_shr(i32x4_add(i32x4_add(i32x4_mul(cr, r), i32x4_mul(cg, g)), i32x4_add(i32x4_mul(cb, b), k)), 8)
    };
    let mut i = 0;
    while i < n {
        let p = &row[i * 4..i * 4 + 32];
        let wd = |o: usize| u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
        let a = u32x4(wd(0), wd(4), wd(8), wd(12));
        let b = u32x4(wd(16), wd(20), wd(24), wd(28));
        let y16 = i16x8_narrow_i32x4(luma4(a), luma4(b));
        let y8 = u8x16_narrow_i16x8(y16, y16);
        out[i..i + 8].copy_from_slice(&(i64x2_extract_lane::<0>(y8) as u64).to_le_bytes());
        i += 8;
    }
    n
}
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn luma_simd(_row: &[u8], _out: &mut [u8], _ri: usize, _bi: usize) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_averages_and_rounds() {
        let mut m = FrameMixer::new(1, 1);
        m.add(&[0, 100, 255, 255]);
        m.add(&[10, 101, 255, 255]);
        m.add(&[20, 103, 0, 255]);
        assert_eq!(m.resolve(), &[10, 101, 170, 255]);
        m.reset();
        m.add(&[7, 8, 9, 10]);
        assert_eq!(m.resolve(), &[7, 8, 9, 10]);
    }

    #[test]
    fn post_effects_behave() {
        let (w, h) = (16, 8);
        let mut px = alloc::vec![0u8; w * h * 4];
        // One bright pixel in the middle; bloom spreads light to its neighbours.
        let c = (4 * w + 8) * 4;
        px[c..c + 4].copy_from_slice(&[255, 255, 255, 255]);
        let mut fx = PostFx::default();
        fx.apply(&mut px, w, h, &Post { bloom: 2.0, bloom_threshold: 0.5, bloom_radius: 8.0, ..Default::default() });
        assert!(px[c - 4 * 2] > 0, "bloom should light neighbours");
        // Vignette darkens corners more than the centre.
        let mut v = alloc::vec![200u8; w * h * 4];
        fx.apply(&mut v, w, h, &Post { vignette: 0.8, ..Default::default() });
        assert!(v[0] < v[(4 * w + 8) * 4]);
        // Grain is deterministic per seed.
        let mut a = alloc::vec![128u8; w * h * 4];
        let mut b = a.clone();
        fx.apply(&mut a, w, h, &Post { grain: 10.0, seed: 3, ..Default::default() });
        fx.apply(&mut b, w, h, &Post { grain: 10.0, seed: 3, ..Default::default() });
        assert_eq!(a, b);
        // Aberration leaves the centre column's green untouched.
        let mut ab = alloc::vec![0u8; w * h * 4];
        for (i, p) in ab.chunks_exact_mut(4).enumerate() { p[0] = (i % w * 16) as u8; p[1] = 77; }
        fx.apply(&mut ab, w, h, &Post { aberration: 4.0, ..Default::default() });
        assert!(ab.chunks_exact(4).all(|p| p[1] == 77));
    }

    #[test]
    fn i420_matches_bt709() {
        let (w, h) = (4, 2);
        let mut px = alloc::vec![0u8; w * h * 4];
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[255, 255, 255, 255]);
        }
        let mut out = alloc::vec![0u8; i420_len(w, h)];
        rgba_to_i420(&px, w, h, Order::Rgba, &mut out);
        assert!(out[..8].iter().all(|&y| y == 235)); // white → Y 235
        assert!(out[8..].iter().all(|&c| (127..=129).contains(&c))); // neutral chroma
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[255, 0, 0, 255]);
        }
        rgba_to_i420(&px, w, h, Order::Rgba, &mut out);
        assert_eq!(out[0], 63); // red → Y 63, V 240
        assert_eq!(out[8 + 2], 240);
        // BGRA input gives the same result for the same colour.
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[0, 0, 255, 255]);
        }
        let mut out2 = alloc::vec![0u8; i420_len(w, h)];
        rgba_to_i420(&px, w, h, Order::Bgra, &mut out2);
        assert_eq!(out, out2);
    }
}
