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
#[derive(Clone, Copy, Debug)]
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
    /// Anamorphic streaks: horizontal flares from bright pixels (strength,
    /// length in pixels, tint).
    pub streaks: f32,
    pub streak_length: f32,
    pub streak_tint: [f32; 3],
    /// Radial zoom blur towards `zoom_center` (0…1 of the frame): 0 = off,
    /// 0.1 = fast, 0.3 = warp speed.
    pub zoom_blur: f32,
    pub zoom_center: [f32; 2],
    /// Colour grade: exposure multiplier (1 = none), contrast (1 = none),
    /// saturation (1 = none), additive shadow tint and multiplicative
    /// highlight tint (0…1 per channel; zeros = none).
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub shadows: [f32; 3],
    pub highlights: [f32; 3],
}

impl Default for Post {
    fn default() -> Self {
        Self {
            bloom_threshold: 0.7,
            bloom: 0.0,
            bloom_radius: 24.0,
            aberration: 0.0,
            vignette: 0.0,
            grain: 0.0,
            seed: 0,
            streaks: 0.0,
            streak_length: 160.0,
            streak_tint: [0.6, 0.75, 1.0],
            zoom_blur: 0.0,
            zoom_center: [0.5, 0.5],
            exposure: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            shadows: [0.0; 3],
            highlights: [0.0; 3],
        }
    }
}

impl Post {
    fn grading(&self) -> bool {
        self.exposure != 1.0 || self.contrast != 1.0 || self.saturation != 1.0 || self.shadows.iter().any(|v| *v != 0.0) || self.highlights.iter().any(|v| *v != 0.0)
    }
}

/// Scratch buffers for [`Post`], reused across frames.
#[derive(Debug, Default)]
pub struct PostFx {
    small: Vec<f32>,
    tmp: Vec<f32>,
    streak: Vec<f32>,
    copy: Vec<u8>,
}

impl PostFx {
    /// Apply `p` to an RGBA frame in place. Order: zoom blur, aberration,
    /// bloom + streaks (from the same bright pass), grade, vignette + grain.
    pub fn apply(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        if p.zoom_blur > 0.0 {
            self.zoom(rgba, w, h, p);
        }
        if p.aberration > 0.0 {
            self.aberrate(rgba, w, h, p.aberration);
        }
        if p.bloom > 0.0 || p.streaks > 0.0 {
            self.bloom(rgba, w, h, p);
        }
        if p.grading() {
            grade(rgba, p);
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

    /// Radial zoom blur: each pixel averages 12 samples along its ray towards
    /// the centre (the ray growing with distance: speed lines). Sample
    /// positions are jittered per pixel, trading ghost copies for fine noise.
    fn zoom(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        self.copy.clear();
        self.copy.extend_from_slice(rgba);
        let (cx, cy) = (p.zoom_center[0] * w as f32, p.zoom_center[1] * h as f32);
        const N: usize = 12;
        let k = p.zoom_blur.min(0.9) / N as f32;
        let (wi, hi) = (w as isize - 1, h as isize - 1);
        for y in 0..h {
            let dy = (cy - y as f32) * k;
            for x in 0..w {
                let dx = (cx - x as f32) * k;
                // Per-pixel hash in 0…1 (stable across frames, so it reads as texture, not boil).
                let mut hs = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
                hs ^= hs >> 15;
                hs = hs.wrapping_mul(0x2C1B_3C6D);
                let jit = (hs >> 8) as f32 * (1.0 / 16_777_216.0);
                let mut acc = [0u32; 3];
                for j in 0..N {
                    let t = j as f32 + jit;
                    let sx = ((x as f32 + dx * t + 0.5) as isize).clamp(0, wi) as usize;
                    let sy = ((y as f32 + dy * t + 0.5) as isize).clamp(0, hi) as usize;
                    let o = (sy * w + sx) * 4;
                    acc[0] += self.copy[o] as u32;
                    acc[1] += self.copy[o + 1] as u32;
                    acc[2] += self.copy[o + 2] as u32;
                }
                let i = (y * w + x) * 4;
                rgba[i] = ((acc[0] + N as u32 / 2) / N as u32) as u8;
                rgba[i + 1] = ((acc[1] + N as u32 / 2) / N as u32) as u8;
                rgba[i + 2] = ((acc[2] + N as u32 / 2) / N as u32) as u8;
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
        if p.streaks > 0.0 {
            // Anamorphic streaks: the bright pass blurred along x only, three
            // passes (≈ gaussian) with a long radius.
            self.streak.clear();
            self.streak.extend_from_slice(&self.small);
            let sr = (crate::math::floorf(p.streak_length / 4.0 + 0.5) as usize).max(1);
            for _ in 0..3 {
                box_blur(&self.streak, &mut self.tmp, sw, sh, sr, true);
                core::mem::swap(&mut self.streak, &mut self.tmp);
            }
        }
        for _ in 0..2 {
            box_blur(&self.small, &mut self.tmp, sw, sh, r, true);
            box_blur(&self.tmp, &mut self.small, sw, sh, r, false);
        }
        // Combine glow (and streaks) at quarter resolution, then add back with
        // bilinear upsampling (no 4×4 blocks).
        let (bk, st) = (p.bloom, p.streaks);
        if st > 0.0 {
            let tint = p.streak_tint;
            for (i, v) in self.small.iter_mut().enumerate() {
                *v = *v * bk + self.streak[i] * st * tint[i % 3];
            }
        } else {
            for v in self.small.iter_mut() {
                *v *= bk;
            }
        }
        let small = &self.small;
        for y in 0..h {
            let fy = ((y as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (sh - 1) as f32);
            let y0 = fy as usize;
            let y1 = (y0 + 1).min(sh - 1);
            let ty = fy - y0 as f32;
            for x in 0..w {
                let fx = ((x as f32 + 0.5) / 4.0 - 0.5).clamp(0.0, (sw - 1) as f32);
                let x0 = fx as usize;
                let x1 = (x0 + 1).min(sw - 1);
                let tx = fx - x0 as f32;
                let (a, b, c, d) = ((y0 * sw + x0) * 3, (y0 * sw + x1) * 3, (y1 * sw + x0) * 3, (y1 * sw + x1) * 3);
                let i = (y * w + x) * 4;
                for ch in 0..3 {
                    let top = small[a + ch] + (small[b + ch] - small[a + ch]) * tx;
                    let bot = small[c + ch] + (small[d + ch] - small[c + ch]) * tx;
                    let g = top + (bot - top) * ty;
                    rgba[i + ch] = (rgba[i + ch] as f32 + g).min(255.0) as u8;
                }
            }
        }
    }
}

/// Exposure, contrast (around mid-grey), saturation and split toning.
#[allow(clippy::needless_range_loop)]
fn grade(rgba: &mut [u8], p: &Post) {
    // Per-channel tone curve as a LUT (exposure + contrast), then the
    // cross-channel part (saturation, tints) per pixel.
    let mut lut = [0f32; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let x = (i as f32 / 255.0) * p.exposure;
        *v = (x - 0.5) * p.contrast + 0.5;
    }
    let tint = p.shadows.iter().any(|v| *v != 0.0) || p.highlights.iter().any(|v| *v != 0.0);
    for px in rgba.chunks_exact_mut(4) {
        let mut c = [lut[px[0] as usize], lut[px[1] as usize], lut[px[2] as usize]];
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        if p.saturation != 1.0 {
            for v in c.iter_mut() {
                *v = l + (*v - l) * p.saturation;
            }
        }
        if tint {
            let lc = l.clamp(0.0, 1.0);
            let (sh, hi) = ((1.0 - lc) * (1.0 - lc), lc * lc);
            for k in 0..3 {
                c[k] = c[k] + p.shadows[k] * sh + c[k] * p.highlights[k] * hi;
            }
        }
        for k in 0..3 {
            px[k] = (c[k] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
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

    #[test]
    fn streaks_zoom_and_grade() {
        let (w, h) = (64usize, 32usize);
        // One bright dot in the middle.
        let mut base = alloc::vec![0u8; w * h * 4];
        for y in 15..17 {
            for x in 31..33 {
                base[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let mut fx = PostFx::default();
        let mut s = base.clone();
        fx.apply(&mut s, w, h, &Post { streaks: 2.0, streak_length: 64.0, bloom_threshold: 0.5, ..Default::default() });
        // Streaks reach far horizontally but not vertically.
        let at = |b: &[u8], x: usize, y: usize| b[(y * w + x) * 4 + 2];
        assert!(at(&s, 16, 16) > 0, "streak spreads along x");
        assert_eq!(at(&s, 32, 2), 0, "but not along y");
        // Zoom blur smears towards the centre: an off-centre dot spreads radially.
        let mut z = alloc::vec![0u8; w * h * 4];
        z[(16 * w + 60) * 4..(16 * w + 60) * 4 + 3].copy_from_slice(&[255, 255, 255]);
        fx.apply(&mut z, w, h, &Post { zoom_blur: 0.5, ..Default::default() });
        let smear = (61..64).map(|x| z[(16 * w + x) * 4] as u32).sum::<u32>();
        assert!(z[(16 * w + 60) * 4] < 255 && smear > 0, "smeared outward along the ray");
        // Grade: exposure brightens, saturation 0 makes grey.
        let mut g = alloc::vec![100u8, 50, 20, 255];
        fx.apply(&mut g, 1, 1, &Post { exposure: 1.5, saturation: 0.0, ..Default::default() });
        assert!(g[0] == g[1] && g[1] == g[2] && g[0] > 50, "{g:?}");
    }
}
