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
    /// God rays: highlights smeared radially from `rays_center` (0…1 of the
    /// frame, e.g. a light behind the scene) over `rays_length` of the way (0…1).
    pub rays: f32,
    pub rays_center: [f32; 2],
    pub rays_length: f32,
    /// Edge antialiasing (FXAA-style, 0…1): smooths stair-stepped geometry edges.
    pub aa: f32,
    /// Lens flares (0…2): ghosts of bright lights mirrored through the frame
    /// centre at several scales with a chromatic tint, plus a halo ring.
    pub flares: f32,
    /// Shockwave: a refracting ring `[centre x, centre y (0…1 of the frame),
    /// radius px, half-width px, strength px]`; strength 0 = off. Pixels in
    /// the ring are displaced radially (red and blue a little apart) and the
    /// wavefront brightens. Animate the radius for an expanding blast.
    pub shock: [f32; 5],
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
            rays: 0.0,
            rays_center: [0.5, 0.4],
            rays_length: 0.6,
            aa: 0.0,
            flares: 0.0,
            shock: [0.5, 0.5, 0.0, 0.0, 0.0],
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
    rays: Vec<f32>,
    luma: Vec<u8>,
    flare: Vec<f32>,
    copy: Vec<u8>,
    /// Zoom blur: half-res packed RGB and its copy for the second pass.
    half: Vec<u32>,
    half2: Vec<u32>,
}

impl PostFx {
    /// Apply `p` to an RGBA frame in place. Order: zoom blur, aberration,
    /// bloom + streaks (from the same bright pass), grade, vignette + grain.
    pub fn apply(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        if p.aa > 0.0 {
            self.antialias(rgba, w, h, p.aa);
        }
        if p.zoom_blur > 0.0 {
            self.zoom(rgba, w, h, p);
        }
        if p.shock[4] != 0.0 && p.shock[3] > 0.0 {
            self.shockwave(rgba, w, h, p.shock);
        }
        let glow = p.bloom > 0.0 || p.streaks > 0.0 || p.rays > 0.0 || p.flares > 0.0;
        if glow {
            self.glow(rgba, w, h, p);
        }
        if glow || p.aberration > 0.0 || p.grading() || p.vignette > 0.0 || p.grain > 0.0 {
            self.composite(rgba, w, h, p, glow);
        }
    }

    /// FXAA-style edge antialiasing: where local luma contrast is high, blend
    /// each pixel towards its neighbour across the edge, by how much the
    /// centre differs from its 3×3 neighbourhood (sub-pixel aliasing).
    #[allow(clippy::needless_range_loop)]
    fn antialias(&mut self, rgba: &mut [u8], w: usize, h: usize, strength: f32) {
        if w < 3 || h < 3 {
            return;
        }
        self.luma.clear();
        self.luma.extend(rgba.chunks_exact(4).map(|p| ((54 * p[0] as u32 + 183 * p[1] as u32 + 19 * p[2] as u32) >> 8) as u8));
        // Blends read the original colours. Rows below aren't written yet and the
        // row above was saved before it changed, so two row copies replace a
        // copy of the whole frame: `copy` holds [row y − 1 | row y], originals.
        let rw = w * 4;
        self.copy.clear();
        self.copy.extend_from_slice(&rgba[..2 * rw]);
        let l = &self.luma;
        let k = strength.clamp(0.0, 1.0);
        for y in 1..h - 1 {
            if y > 1 {
                // Shift: the previous "row y" becomes "row y − 1"; save the new row y.
                self.copy.copy_within(rw..2 * rw, 0);
                self.copy[rw..2 * rw].copy_from_slice(&rgba[y * rw..(y + 1) * rw]);
            }
            // Row slices: the neighbourhood reads below need no bounds checks.
            let (up, row, dn) = (&l[(y - 1) * w..y * w], &l[y * w..(y + 1) * w], &l[(y + 1) * w..(y + 2) * w]);
            for x in 1..w - 1 {
                let (c, n, s, e, wv) = (row[x] as i32, up[x] as i32, dn[x] as i32, row[x + 1] as i32, row[x - 1] as i32);
                let hi = c.max(n).max(s).max(e).max(wv);
                let lo = c.min(n).min(s).min(e).min(wv);
                let range = hi - lo;
                if range < 16.max(hi >> 3) {
                    continue;
                }
                let i = y * w + x;
                let (nw, ne, sw, se) = (up[x - 1] as i32, up[x + 1] as i32, dn[x - 1] as i32, dn[x + 1] as i32);
                // Sub-pixel blend amount from the centre vs. its neighbourhood.
                let avg = (2 * (n + s + e + wv) + nw + ne + sw + se) as f32 / 12.0;
                let sub = ((avg - c as f32).abs() / range as f32).clamp(0.0, 1.0);
                let sub = sub * sub * (3.0 - 2.0 * sub);
                let amt = sub * sub * 0.75 * k;
                if amt < 0.02 {
                    continue;
                }
                // Edge orientation: horizontal edges blend vertically, and vice versa.
                let horiz = (n + s - 2 * c).abs() * 2 + (nw + sw - 2 * wv).abs() + (ne + se - 2 * e).abs();
                let vert = (e + wv - 2 * c).abs() * 2 + (nw + ne - 2 * n).abs() + (sw + se - 2 * s).abs();
                let j = if horiz >= vert {
                    if (n - c).abs() >= (s - c).abs() { i - w } else { i + w }
                } else if (wv - c).abs() >= (e - c).abs() {
                    i - 1
                } else {
                    i + 1
                };
                let ai = (amt * 256.0) as i32;
                // Original colours: row y and y − 1 from the saved rows; row y + 1 is untouched.
                let pa: [u8; 3] = [self.copy[rw + x * 4], self.copy[rw + x * 4 + 1], self.copy[rw + x * 4 + 2]];
                let src = if j + w == i { &self.copy[x * 4..x * 4 + 3] } else if j == i + w { &rgba[j * 4..j * 4 + 3] } else { &self.copy[rw + (j - y * w) * 4..rw + (j - y * w) * 4 + 3] };
                let pb: [u8; 3] = [src[0], src[1], src[2]];
                for ch in 0..3 {
                    let (a, b) = (pa[ch] as i32, pb[ch] as i32);
                    rgba[i * 4 + ch] = (a + (((b - a) * ai + 128) >> 8)) as u8;
                }
            }
        }
    }

    /// Radial zoom blur (speed lines): two passes of four jittered taps along
    /// each pixel's ray towards the centre, the second with a 4× longer step,
    /// so 8 reads per pixel give 16 evenly spread samples without ghosting.
    fn zoom(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post) {
        // Blur at half resolution (radial smear is smooth along the ray), then
        // blend over the sharp frame by how long each pixel's ray is, so the
        // centre stays full-resolution sharp.
        if w < 4 || h < 4 {
            return;
        }
        let (bw, bh) = (w / 2, h / 2);
        const RB: u32 = 0x00ff_00ff;
        // 2×2 mean into packed RGB words (red+blue and green summed in 16-bit lanes).
        self.half.clear();
        self.half.resize(bw * bh, 0);
        let word = |i: usize| u32::from_le_bytes([rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]);
        for by in 0..bh {
            for bx in 0..bw {
                let i = (by * 2 * w + bx * 2) * 4;
                let q = [word(i), word(i + 4), word(i + w * 4), word(i + w * 4 + 4)];
                let rb = (q.iter().map(|v| v & RB).sum::<u32>() + 0x0002_0002) >> 2 & RB;
                let g = (q.iter().map(|v| (v >> 8) & 255).sum::<u32>() + 2) >> 2;
                self.half[by * bw + bx] = rb | g << 8;
            }
        }
        let (cx, cy) = (p.zoom_center[0] * bw as f32, p.zoom_center[1] * bh as f32);
        let k = p.zoom_blur.min(0.9) / 16.0;
        // Taps lie between the pixel and the centre (step·t < 1), so with the
        // centre on screen they never leave it: no clamping per tap.
        let inside = cx >= 0.0 && cy >= 0.0 && cx <= (bw - 1) as f32 && cy <= (bh - 1) as f32;
        let (wi, hi) = (bw as i32 - 1, bh as i32 - 1);
        for pass in 0..2u32 {
            core::mem::swap(&mut self.half, &mut self.half2);
            self.half.clear();
            self.half.extend_from_slice(&self.half2);
            let src = &self.half2;
            let step = if pass == 0 { k } else { k * 4.0 };
            for by in 0..bh {
                let dy = (cy - by as f32) * step;
                for bx in 0..bw {
                    let dx = (cx - bx as f32) * step;
                    if dx * dx + dy * dy < 0.0625 {
                        continue;
                    }
                    let mut hs = (bx as u32).wrapping_mul(0x9E37_79B1) ^ (by as u32).wrapping_mul(0x85EB_CA77) ^ pass;
                    hs ^= hs >> 15;
                    hs = hs.wrapping_mul(0x2C1B_3C6D);
                    let jit = (hs >> 8) as f32 * (1.0 / 16_777_216.0);
                    // 16.16 fixed point along the ray, starting at t = jit.
                    let mut fx = ((bx as f32 + dx * jit + 0.5) * 65536.0) as i32;
                    let mut fy = ((by as f32 + dy * jit + 0.5) * 65536.0) as i32;
                    let (ddx, ddy) = ((dx * 65536.0) as i32, (dy * 65536.0) as i32);
                    let (mut arb, mut ag) = (0u32, 0u32);
                    for _ in 0..4 {
                        let (sx, sy) = (fx >> 16, fy >> 16);
                        let (sx, sy) = if inside { (sx, sy) } else { (sx.clamp(0, wi), sy.clamp(0, hi)) };
                        let v = src[sy as usize * bw + sx as usize];
                        arb += v & RB;
                        ag += (v >> 8) & 255;
                        fx += ddx;
                        fy += ddy;
                    }
                    self.half[by * bw + bx] = ((arb + 0x0002_0002) >> 2 & RB) | ((ag + 2) >> 2) << 8;
                }
            }
        }
        // Blend: weight by ray length in full-res px (sharp below ~1.5 px, blurred from ~4 px).
        let (fcx, fcy) = (p.zoom_center[0] * w as f32, p.zoom_center[1] * h as f32);
        let kk = p.zoom_blur.min(0.9) * 1.25; // ≈ total smear length per px of radius
        for y in 0..h {
            let by = (y / 2).min(bh - 1);
            let dy = y as f32 - fcy;
            let hrow = &self.half[by * bw..(by + 1) * bw];
            let row = &mut rgba[y * w * 4..(y + 1) * w * 4];
            // Beyond radius 4 / kk the blur is total: copy the blurred pixel. Per row
            // that is everything outside [xa, xb); only that span blends by distance.
            let full = 4.0 / kk.max(1e-6);
            let span = full * full - dy * dy;
            let (xa, xb) = if span > 0.0 {
                let r = crate::math::sqrtf(span);
                ((crate::math::floorf(fcx - r).max(0.0) as usize).min(w), (crate::math::ceilf(fcx + r).max(0.0) as usize).min(w))
            } else {
                (w, w)
            };
            let copy_run = |row: &mut [u8], x0: usize, x1: usize| {
                for (k, px) in row[x0 * 4..x1 * 4].chunks_exact_mut(4).enumerate() {
                    let b = hrow[((x0 + k) / 2).min(bw - 1)];
                    px[0] = b as u8;
                    px[1] = (b >> 8) as u8;
                    px[2] = (b >> 16) as u8;
                }
            };
            copy_run(row, 0, xa);
            copy_run(row, xb.max(xa), w);
            for x in xa..xb {
                let px = &mut row[x * 4..x * 4 + 4];
                let dx = x as f32 - fcx;
                let len = crate::math::sqrtf(dx * dx + dy * dy) * kk;
                let a = ((len - 1.5) * 0.4).clamp(0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let ai = (a * 256.0) as u32;
                let b = hrow[(x / 2).min(bw - 1)];
                let c = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                let o_rb = (((c & RB) * (256 - ai) + (b & RB) * ai + 0x0080_0080) >> 8) & RB;
                let o_g = ((((c >> 8) & 255) * (256 - ai) + ((b >> 8) & 255) * ai + 128) >> 8) & 255;
                px.copy_from_slice(&((c & 0xff00_0000) | o_rb | o_g << 8).to_le_bytes());
            }
        }
    }

    /// Bloom and streaks at quarter resolution into `self.small` (already
    /// scaled by their strengths): bright pass, blurs.
    #[allow(clippy::needless_range_loop)]
    fn glow(&mut self, rgba: &[u8], w: usize, h: usize, p: &Post) {
        let (sw, sh) = (w.div_ceil(4), h.div_ceil(4));
        self.small.resize(sw * sh * 3, 0.0);
        self.tmp.resize(sw * sh * 3, 0.0);
        // Weight per 8-bit luminance (a LUT instead of a divide per pixel).
        let th = p.bloom_threshold * 255.0;
        let inv = 1.0 / (255.0 - th + 1.0);
        let lut: [f32; 256] = core::array::from_fn(|l| ((l as f32 - th) * inv).max(0.0) * (1.0 / 16.0));
        self.small.iter_mut().for_each(|v| *v = 0.0);
        for y in 0..h {
            let sy = y >> 2;
            let row = &rgba[y * w * 4..(y + 1) * w * 4];
            let out = &mut self.small[sy * sw * 3..(sy + 1) * sw * 3];
            for (x, px) in row.chunks_exact(4).enumerate() {
                let l = (54 * px[0] as u32 + 183 * px[1] as u32 + 19 * px[2] as u32) >> 8;
                let k = lut[l as usize];
                if k > 0.0 {
                    let o = (x >> 2) * 3;
                    out[o] += px[0] as f32 * k;
                    out[o + 1] += px[1] as f32 * k;
                    out[o + 2] += px[2] as f32 * k;
                }
            }
        }
        let r = (crate::math::floorf(p.bloom_radius / 4.0 + 0.5) as usize).max(1);
        if p.rays > 0.0 {
            self.rays.clear();
            self.rays.extend_from_slice(&self.small);
        }
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
        if p.flares > 0.0 {
            // From the sharp bright pass: each ghost is a (softened) image of the lights.
            self.lens_flares(sw, sh);
        }
        for _ in 0..2 {
            box_blur(&self.small, &mut self.tmp, sw, sh, r, true);
            box_blur(&self.tmp, &mut self.small, sw, sh, r, false);
        }
        if p.rays > 0.0 {
            self.god_rays(sw, sh, p);
        }
        let (bk, st) = (p.bloom, p.streaks);
        if p.rays > 0.0 {
            for (v, r) in self.small.iter_mut().zip(&self.rays) {
                *v = *v * bk + r * p.rays;
            }
            if st > 0.0 {
                let tint = p.streak_tint;
                for (i, v) in self.small.iter_mut().enumerate() {
                    *v += self.streak[i] * st * tint[i % 3];
                }
            }
        } else if st > 0.0 {
            let tint = p.streak_tint;
            for (i, v) in self.small.iter_mut().enumerate() {
                *v = *v * bk + self.streak[i] * st * tint[i % 3];
            }
        } else {
            for v in self.small.iter_mut() {
                *v *= bk;
            }
        }
        if p.flares > 0.0 {
            let k = p.flares;
            for (v, f) in self.small.iter_mut().zip(&self.flare) {
                *v += f * k;
            }
        }
    }

    /// A refracting ring (see [`Post::shock`]): only the rows and columns the
    /// annulus touches are visited, so it costs little while it is small.
    fn shockwave(&mut self, rgba: &mut [u8], w: usize, h: usize, s: [f32; 5]) {
        let [ux, uy, rad, half, amt] = s;
        let (cx, cy) = (ux * w as f32, uy * h as f32);
        let outer = rad + half;
        let y0 = crate::math::floorf(cy - outer).max(0.0) as usize;
        let y1 = (crate::math::ceilf(cy + outer).max(0.0) as usize).min(h);
        if y0 >= y1 {
            return;
        }
        // Only the band of rows the ring covers is read back.
        self.copy.clear();
        let (r0, r1) = (y0.saturating_sub(amt.abs() as usize + 2), (y1 + amt.abs() as usize + 2).min(h));
        self.copy.extend_from_slice(&rgba[r0 * w * 4..r1 * w * 4]);
        let wi = w as i32 - 1;
        let src = &self.copy;
        let at = |x: f32, y: f32| {
            let (xi, yi) = ((x + 0.5) as i32, ((y + 0.5) as i32).clamp(r0 as i32, r1 as i32 - 1));
            ((yi as usize - r0) * w + xi.clamp(0, wi) as usize) * 4
        };
        let inv_half = 1.0 / half;
        for y in y0..y1 {
            let dy = y as f32 + 0.5 - cy;
            let span = outer * outer - dy * dy;
            if span <= 0.0 {
                continue;
            }
            let sx = crate::math::sqrtf(span);
            let x0 = crate::math::floorf(cx - sx).max(0.0) as usize;
            let x1 = (crate::math::ceilf(cx + sx).max(0.0) as usize).min(w);
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let d = crate::math::sqrtf(dx * dx + dy * dy).max(1e-3);
                let u = (d - rad) * inv_half;
                if !(-1.0..1.0).contains(&u) {
                    continue;
                }
                // Push out ahead of the front, pull in behind it; zero at the edges.
                let env = 1.0 - u * u;
                let disp = amt * libm::sinf(core::f32::consts::PI * u) * env;
                let (nx, ny) = (dx / d, dy / d);
                let (px, py) = (x as f32, y as f32);
                let ir = at(px - nx * disp * 1.2, py - ny * disp * 1.2);
                let ig = at(px - nx * disp, py - ny * disp);
                let ib = at(px - nx * disp * 0.8, py - ny * disp * 0.8);
                let lift = 1.0 + 0.18 * env * env * (amt.abs() / 24.0).min(1.0);
                let o = (y * w + x) * 4;
                rgba[o] = (src[ir] as f32 * lift).min(255.0) as u8;
                rgba[o + 1] = (src[ig + 1] as f32 * lift).min(255.0) as u8;
                rgba[o + 2] = (src[ib + 2] as f32 * lift).min(255.0) as u8;
            }
        }
    }

    /// Pseudo lens flares from the bright pass (`self.small`, quarter
    /// resolution) into `self.flare`: each pixel gathers the bright pass seen
    /// through a few "ghosts" (the frame mirrored through its centre at
    /// different scales, tinted like lens coatings, fading towards the edges)
    /// and a halo ring around the centre.
    #[allow(clippy::needless_range_loop)]
    fn lens_flares(&mut self, sw: usize, sh: usize) {
        // (scale, r, g, b): negative scales mirror through the centre.
        const GHOSTS: [(f32, f32, f32, f32); 5] = [(-0.5, 0.9, 0.5, 1.2), (-1.0, 0.4, 0.8, 1.1), (-1.6, 1.1, 0.6, 0.4), (0.45, 0.5, 1.0, 0.7), (-2.4, 0.7, 0.4, 1.0)];
        self.flare.clear();
        self.flare.resize(sw * sh * 3, 0.0);
        let (fw, fh) = (sw as f32, sh as f32);
        let aspect = fw / fh;
        // Only the hottest lights cast ghosts: a knee well above the bloom
        // threshold (a sun does, a lit sign doesn't).
        self.tmp.clear();
        self.tmp.extend(self.small.iter().map(|v| (v - 110.0).max(0.0) * 2.0));
        let src = &self.tmp;
        let sample = |u: f32, v: f32| -> Option<usize> {
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                return None;
            }
            Some(((v * fh) as usize).min(sh - 1) * sw + ((u * fw) as usize).min(sw - 1))
        };
        for y in 0..sh {
            let v = (y as f32 + 0.5) / fh;
            for x in 0..sw {
                let u = (x as f32 + 0.5) / fw;
                let (cu, cv) = (u - 0.5, v - 0.5);
                let mut acc = [0.0f32; 3];
                for &(s, r, g, b) in &GHOSTS {
                    let (gu, gv) = (0.5 + cu * s, 0.5 + cv * s);
                    if let Some(i) = sample(gu, gv) {
                        // Ghosts fade towards the edge of the frame they come from.
                        let d = ((gu - 0.5) * (gu - 0.5) + (gv - 0.5) * (gv - 0.5)) * 2.0;
                        let f = (1.0 - d).max(0.0);
                        let f = f * f;
                        acc[0] += src[i * 3] * f * r;
                        acc[1] += src[i * 3 + 1] * f * g;
                        acc[2] += src[i * 3 + 2] * f * b;
                    }
                }
                // Halo: a ring at a fixed radius, sampled outward along the ray from the centre.
                let len = crate::math::sqrtf(cu * cu * aspect * aspect + cv * cv).max(1e-4);
                let (hu, hv) = (u + cu / len * 0.32 / aspect, v + cv / len * 0.32);
                if let Some(i) = sample(hu, hv) {
                    let ring = 1.0 - ((len - 0.32).abs() * 6.0).min(1.0);
                    let f = ring * ring * 0.8;
                    acc[0] += src[i * 3] * f;
                    acc[1] += src[i * 3 + 1] * f * 0.8;
                    acc[2] += src[i * 3 + 2] * f * 1.1;
                }
                let o = (y * sw + x) * 3;
                self.flare[o..o + 3].copy_from_slice(&acc);
            }
        }
        // Soften: ghosts are out-of-focus images.
        let r = (sw / 120).max(2);
        self.tmp.resize(sw * sh * 3, 0.0);
        box_blur(&self.flare, &mut self.tmp, sw, sh, r, true);
        box_blur(&self.tmp, &mut self.flare, sw, sh, r, false);
    }

    /// Radial blur of the quarter-resolution bright pass towards the light
    /// (`self.rays` in, rays out): 24 decaying taps per pixel, cheap at ¼ res.
    #[allow(clippy::needless_range_loop)]
    fn god_rays(&mut self, sw: usize, sh: usize, p: &Post) {
        const N: usize = 14;
        let (cx, cy) = (p.rays_center[0] * sw as f32, p.rays_center[1] * sh as f32);
        let k = p.rays_length.clamp(0.0, 1.0) / 24.0; // taps are spread over 24 steps of the ray
        self.tmp.resize(sw * sh * 3, 0.0);
        // Decaying tap weights, normalised once (×2: rays are a glow, not an average).
        let mut wts = [0.0f32; N];
        let mut wgt = 1.0f32;
        for v in wts.iter_mut() {
            *v = wgt;
            wgt *= 0.93;
        }
        let scale = 2.0 / wts.iter().sum::<f32>();
        wts.iter_mut().for_each(|v| *v *= scale);
        // Taps run from the pixel towards the centre and stop short of it, so
        // with the centre on screen they never leave it: no clamps per tap.
        let inside = cx >= 0.0 && cy >= 0.0 && cx <= (sw - 1) as f32 && cy <= (sh - 1) as f32;
        let (wi, hi) = (sw as i32 - 1, sh as i32 - 1);
        let st = 24.0 / N as f32;
        for y in 0..sh {
            for x in 0..sw {
                let (dx, dy) = ((cx - x as f32) * k, (cy - y as f32) * k);
                let mut acc = [0.0f32; 3];
                // Jittered start (per pixel hash) hides the coarser tap spacing.
                let mut hs = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
                hs ^= hs >> 15;
                hs = hs.wrapping_mul(0x2C1B_3C6D);
                let jit = (hs >> 8) as f32 * (1.0 / 16_777_216.0);
                // 16.16 fixed point along the ray.
                let mut fx = ((x as f32 + dx * jit * st + 0.5) * 65536.0) as i32;
                let mut fy = ((y as f32 + dy * jit * st + 0.5) * 65536.0) as i32;
                let (ddx, ddy) = ((dx * st * 65536.0) as i32, (dy * st * 65536.0) as i32);
                for &wv in &wts {
                    let (sx, sy) = (fx >> 16, fy >> 16);
                    let (sx, sy) = if inside { (sx, sy) } else { (sx.clamp(0, wi), sy.clamp(0, hi)) };
                    let o = (sy as usize * sw + sx as usize) * 3;
                    acc[0] += self.rays[o] * wv;
                    acc[1] += self.rays[o + 1] * wv;
                    acc[2] += self.rays[o + 2] * wv;
                    fx += ddx;
                    fy += ddy;
                }
                let o = (y * sw + x) * 3;
                self.tmp[o..o + 3].copy_from_slice(&acc);
            }
        }
        core::mem::swap(&mut self.rays, &mut self.tmp);
    }

    /// One fused full-resolution pass, four pixels at a time in SIMD:
    /// chromatic aberration (red/blue sampled radially apart), bloom/streaks
    /// added back with a bilinear 4× upsample, the grade (exposure, contrast,
    /// saturation, split toning), vignette and grain.
    #[allow(clippy::needless_range_loop)]
    fn composite(&mut self, rgba: &mut [u8], w: usize, h: usize, p: &Post, glow: bool) {
        use crate::simd::V4;
        let ab = p.aberration > 0.0;
        if ab {
            self.copy.clear();
            self.copy.extend_from_slice(rgba);
        }
        let (sw, sh) = (w.div_ceil(4), h.div_ceil(4));
        if glow {
            self.tmp.resize(sw * 3, 0.0);
        }
        // Exposure + contrast around mid-grey is affine on 0…1 values.
        let (ga, gb) = (p.exposure * p.contrast / 255.0, 0.5 - 0.5 * p.contrast);
        let tint = p.shadows.iter().any(|v| *v != 0.0) || p.highlights.iter().any(|v| *v != 0.0);
        let sat = p.saturation != 1.0;
        let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
        let inv_r2 = 1.0 / (cx * cx + cy * cy);
        let norm = p.aberration / crate::math::sqrtf(cx * cx + cy * cy).max(1.0);
        let vig = p.vignette * 1.4;
        let grain = p.grain * 2.0 / 255.0;
        let seed = p.seed.wrapping_mul(0x9E37_79B9);
        let (wi, hi) = (w as isize - 1, h as isize - 1);
        let phase = V4::new(0.375, 0.125, 0.125, 0.375);
        let lanes = V4::new(0.0, 1.0, 2.0, 3.0);
        for y in 0..h {
            if glow {
                let fy = ((y as f32 + 0.5) * 0.25 - 0.5).clamp(0.0, (sh - 1) as f32);
                let y0 = fy as usize;
                let y1 = (y0 + 1).min(sh - 1);
                let ty = fy - y0 as f32;
                let (r0, r1) = (&self.small[y0 * sw * 3..(y0 + 1) * sw * 3], &self.small[y1 * sw * 3..(y1 + 1) * sw * 3]);
                for ((o, a), b) in self.tmp[..sw * 3].iter_mut().zip(r0).zip(r1) {
                    *o = a + (b - a) * ty;
                }
            }
            let dy = y as f32 - cy;
            let dy2 = V4::splat(dy * dy);
            // Aberration sources: red at x − (x − cx)·norm on row y − dy·norm, blue
            // mirrored. Rows are constant and x is affine, so step x in 16.16.
            let (ry, by_) = (((y as f32 - dy * norm + 0.5) as isize).clamp(0, hi) as usize * w, ((y as f32 + dy * norm + 0.5) as isize).clamp(0, hi) as usize * w);
            let (rx0, bx0) = ((((cx * norm + 0.5) * 65536.0) as i64), (((-cx * norm + 0.5) * 65536.0) as i64));
            let (rdx, bdx) = (((1.0 - norm) * 65536.0) as i64, ((1.0 + norm) * 65536.0) as i64);
            let row = &mut rgba[y * w * 4..(y + 1) * w * 4];
            let mut x0 = 0;
            while x0 < w {
                let nl = (w - x0).min(4);
                // Four pixels as packed words (the last group repeats its edge pixel).
                let words: [u32; 4] = core::array::from_fn(|l| {
                    let o = (x0 + l.min(nl - 1)) * 4;
                    u32::from_le_bytes([row[o], row[o + 1], row[o + 2], row[o + 3]])
                });
                let [mut r, g, mut b] = V4::unpack_rgb(words);
                if ab {
                    // Red pushed outward, blue pulled inward, proportional to radius.
                    let mut rr = [0f32; 4];
                    let mut bb = [0f32; 4];
                    for l in 0..4 {
                        let x = (x0 + l.min(nl - 1)) as i64;
                        let sxr = ((rx0 + x * rdx) >> 16).clamp(0, wi as i64) as usize;
                        let sxb = ((bx0 + x * bdx) >> 16).clamp(0, wi as i64) as usize;
                        rr[l] = self.copy[(ry + sxr) * 4] as f32;
                        bb[l] = self.copy[(by_ + sxb) * 4 + 2] as f32;
                    }
                    r = V4::from_array(rr);
                    b = V4::from_array(bb);
                }
                let mut c = [r, g, b];
                if glow {
                    // Lanes x0..x0+3 are exactly the four upsample phases of source pixel x0/4.
                    let sx = x0 >> 2;
                    let (lft, rgt) = (sx.saturating_sub(1), (sx + 1).min(sw - 1));
                    let rb = &self.tmp;
                    for ch in 0..3 {
                        let a = rb[sx * 3 + ch];
                        let nb = V4::new(rb[lft * 3 + ch], rb[lft * 3 + ch], rb[rgt * 3 + ch], rb[rgt * 3 + ch]);
                        c[ch] = c[ch] + a + (nb - a) * phase;
                    }
                }
                for v in c.iter_mut() {
                    *v = *v * ga + gb;
                }
                if sat || tint {
                    let l = c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722;
                    if sat {
                        for v in c.iter_mut() {
                            *v = l + (*v - l) * p.saturation;
                        }
                    }
                    if tint {
                        let lc = l.clamp(0.0, 1.0);
                        let (shd, hil) = ((1.0 - lc) * (1.0 - lc), lc * lc);
                        for k in 0..3 {
                            c[k] = c[k] + shd * p.shadows[k] + c[k] * hil * p.highlights[k];
                        }
                    }
                }
                let dx = lanes + (x0 as f32 - cx);
                let r2 = (dx * dx + dy2) * inv_r2;
                let k = 1.0 - r2 * r2 * vig;
                let n = if grain > 0.0 {
                    // Grain: an independent hash per pixel (no serial RNG chain).
                    let g4: [f32; 4] = core::array::from_fn(|l| {
                        let mut hs = ((x0 + l) as u32).wrapping_mul(0x85EB_CA77) ^ (y as u32).wrapping_mul(0xC2B2_AE3D) ^ seed;
                        hs ^= hs >> 15;
                        hs = hs.wrapping_mul(0x2C1B_3C6D);
                        hs ^= hs >> 12;
                        ((hs >> 24) as f32 / 255.0 - 0.5) * grain
                    });
                    V4::from_array(g4)
                } else {
                    V4::splat(0.0)
                };
                let out = V4::pack_rgb(((c[0] + n) * k).maxf(0.0), ((c[1] + n) * k).maxf(0.0), ((c[2] + n) * k).maxf(0.0));
                for l in 0..nl {
                    let o = (x0 + l) * 4;
                    // RGB from the grade, alpha kept.
                    let v = (out[l] & 0x00ff_ffff) | (words[l] & 0xff00_0000);
                    row[o..o + 4].copy_from_slice(&v.to_le_bytes());
                }
                x0 += 4;
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

    #[test]
    fn shockwave_refracts_only_inside_its_ring() {
        // Vertical stripes, so any horizontal displacement shows.
        let (w, h) = (200usize, 120usize);
        let px: Vec<u8> = (0..w * h).flat_map(|i| { let v = if (i % w / 4) % 2 == 0 { 220 } else { 30 }; [v, v, v, 255] }).collect();
        let mut a = px.clone();
        let mut fx = PostFx::default();
        fx.apply(&mut a, w, h, &Post { shock: [0.5, 0.5, 40.0, 10.0, 6.0], ..Default::default() });
        let changed = |x: usize, y: usize| a[(y * w + x) * 4..(y * w + x) * 4 + 3] != px[(y * w + x) * 4..(y * w + x) * 4 + 3];
        let ring = (0..w).filter(|&x| changed(x, 60) && { let d = (x as f32 + 0.5 - 100.0).abs(); (30.0..50.0).contains(&d) }).count();
        assert!(ring > 6, "pixels in the ring move: {ring}");
        assert!((0..w).filter(|&x| { let d = (x as f32 + 0.5 - 100.0).abs(); !(29.0..51.0).contains(&d) }).all(|x| !changed(x, 60)), "nothing outside it");
        assert!((0..w).all(|x| !changed(x, 5)), "rows the ring misses are untouched");
    }

    #[test]
    fn lens_flares_mirror_bright_lights_through_the_centre() {
        // One hot spot in the upper-left quadrant of a dark frame.
        let (w, h) = (256usize, 128usize);
        let mut px = vec![0u8; w * h * 4];
        for y in 20..32 {
            for x in 40..56 {
                px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let mut a = px.clone();
        let mut fx = PostFx::default();
        fx.apply(&mut a, w, h, &Post { flares: 2.0, bloom_threshold: 0.5, ..Default::default() });
        // Mirrored through the centre: around (w − 48, h − 26), lower right.
        let lum = |b: &[u8], x0: usize, y0: usize| (y0 - 6..y0 + 6).flat_map(|y| (x0 - 6..x0 + 6).map(move |x| (y, x))).map(|(y, x)| b[(y * w + x) * 4] as u32 + b[(y * w + x) * 4 + 2] as u32).sum::<u32>();
        let ghost = lum(&a, w - 48, h - 26);
        let before = lum(&px, w - 48, h - 26);
        assert!(ghost > before + 200, "a ghost appears opposite the light: {before} -> {ghost}");
        // Off: nothing changes.
        let mut b = px.clone();
        fx.apply(&mut b, w, h, &Post { flares: 0.0, ..Default::default() });
        assert_eq!(b, px);
    }

    #[test]
    fn antialias_softens_stair_steps_only() {
        let (w, h) = (16usize, 16usize);
        // A hard diagonal edge (white above y = x).
        let mut img = alloc::vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let v = if y < x { 255 } else { 0 };
                img[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let before = img.clone();
        PostFx::default().apply(&mut img, w, h, &Post { aa: 1.0, ..Default::default() });
        let mids = img.chunks_exact(4).filter(|p| p[0] > 20 && p[0] < 235).count();
        assert!(mids > 5, "edge pixels blended: {mids}");
        // Flat regions are untouched.
        assert_eq!(img[(2 * w + 13) * 4], before[(2 * w + 13) * 4]);
        assert_eq!(img[(13 * w + 2) * 4], before[(13 * w + 2) * 4]);
    }
}
