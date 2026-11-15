//! Additive point splatting into an RGBA pixel buffer.
//!
//! Renders hundreds of thousands of projected points (from
//! [`crate::space::Projector`]) as glowing light: sub-pixel bilinear splats
//! for far points, soft discs for near ones, depth fog, an optional screen-
//! space lens around the pointer, motion trails by decaying the accumulator,
//! and filmic per-channel tone mapping over a background colour. Additive
//! light is order-independent, so no depth sort is needed.

use alloc::vec;
use alloc::vec::Vec;
use crate::math::{ceilf, sqrtf};


/// Per-draw parameters.
#[derive(Clone, Copy, Debug)]
pub struct SplatParams {
    /// Point radius in world units (scaled by perspective).
    pub size: f32,
    /// Light energy per point.
    pub intensity: f32,
    /// Depth where fog starts / where points vanish.
    pub fog_near: f32,
    pub fog_far: f32,
    /// Buffer pixels per projected unit (e.g. 0.75 renders at 75% res).
    pub scale: f32,
    /// Lens centre (projected units), radius and push strength (0 = off).
    pub lens: [f32; 4],
    /// Largest disc radius in buffer pixels.
    pub max_radius: f32,
    /// Colour multiplier applied to every point (cheap palette changes).
    pub tint: [f32; 3],
}

impl Default for SplatParams {
    fn default() -> Self {
        Self { size: 1.0, intensity: 0.3, fog_near: 0.0, fog_far: f32::INFINITY, scale: 1.0, lens: [0.0; 4], max_radius: 6.0, tint: [1.0; 3] }
    }
}

/// Light accumulator + tone-mapped RGBA output.
#[derive(Debug, Default)]
pub struct Splat {
    pub width: usize,
    pub height: usize,
    /// Interleaved light accumulator `[r, g, b] * width * height`. Interleaved
    /// (not planar) so a splat touches one cache line per row instead of
    /// three; the tone-map loop still vectorises (stride-3 interleaved
    /// access).
    acc: Vec<f32>,
    /// Tone-mapped pixels as little-endian RGBA words (`width * height`).
    pub rgba: Vec<u32>,
    /// Per-point linear RGB in [0, 1], `[r, g, b] * n`; missing entries are white.
    pub colors: Vec<f32>,
    /// Rows drawn into since the last resolve.
    touched: Vec<bool>,
    /// Rows that held any light at the last resolve (so their pixels aren't plain background).
    lit: Vec<bool>,
    last_bg: Option<u32>,
    /// Subtractive "ink" rendering: accumulated values absorb the background
    /// (`bg / (1 + c)` per channel) instead of adding light. Point colours
    /// are then absorption per channel (1, 1, 1 = black ink).
    pub ink: bool,
    last_ink: bool,
}

/// Points up to this buffer-pixel radius use the 4-tap bilinear splat.
const BILINEAR_MAX: f32 = 1.6;

/// `x.max(0.0)` as a compare + select. `f32::max` has IEEE `maxNum` semantics,
/// which wasm's `f32x4.max` doesn't implement, so LLVM scalarises it inside
/// vectorised loops; this form lowers to `f32x4.pmax`. Same result for every
/// input, NaN included (→ 0).
#[inline(always)]
fn max0(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}
/// `x.min(m)` for a non-NaN `m` as a compare + select (→ `f32x4.pmin`).
#[inline(always)]
fn min_sel(x: f32, m: f32) -> f32 {
    if x < m {
        x
    } else {
        m
    }
}

#[inline(always)]
fn pack(vr: f32, vg: f32, vb: f32) -> u32 {
    (vr as i32 as u32) | ((vg as i32 as u32) << 8) | ((vb as i32 as u32) << 16) | 0xff00_0000
}

/// Tone-map constants per channel: background, scale and headroom.
#[derive(Clone, Copy)]
struct Tone {
    bg: [f32; 3],
    k: [f32; 3],
    m: [f32; 3],
}

impl Tone {
    /// Channel `ch` with exposed light `c`.
    #[inline(always)]
    fn value<const INK: bool>(&self, c: f32, ch: usize) -> f32 {
        if INK {
            // Ink absorbs: transmittance 1 / (1 + c) per channel.
            self.bg[ch] / (1.0 + c) + 0.5
        } else {
            min_sel(self.k[ch] * c / (1.0 + c), self.m[ch]) + self.bg[ch] + 0.5
        }
    }
}

/// Tone-map one row and decay its light. Returns whether any channel held
/// light (`> 0`) before the decay.
#[inline(always)]
fn resolve_row<const INK: bool>(acc: &mut [f32], out: &mut [u32], t: &Tone, s: f32, keep: f32, floor: f32) -> bool {
    // Bits of a non-negative float order like the float, so an integer max
    // tracks "any light" without a float reduction (which wouldn't vectorise).
    let mut any = 0i32;
    let done = resolve_simd::<INK>(acc, out, t, s, keep, floor, &mut any);
    for (a, o) in acc[done * 3..].chunks_exact_mut(3).zip(out[done..].iter_mut()) {
        let (r, g, b) = (a[0], a[1], a[2]);
        any = any.max(r.to_bits() as i32).max(g.to_bits() as i32).max(b.to_bits() as i32);
        *o = pack(t.value::<INK>(r * s, 0), t.value::<INK>(g * s, 1), t.value::<INK>(b * s, 2));
        a[0] = max0(r * keep - floor);
        a[1] = max0(g * keep - floor);
        a[2] = max0(b * keep - floor);
    }
    any > 0
}

/// WebAssembly SIMD tone map: four pixels per step as three `f32x4`
/// (`[r0 g0 b0 r1] [g1 b1 r2 g2] [b2 r3 g3 b3]`) against per-lane channel
/// constants, so the interleaved layout needs no shuffles until the final
/// byte pack. Same operations in the same order as the scalar loop, so the
/// output is bit-identical for every in-range value (out-of-range bytes
/// saturate instead of spilling into the neighbouring channel). Returns the
/// number of pixels done; the caller finishes the tail.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
#[inline(always)]
fn resolve_simd<const INK: bool>(acc: &mut [f32], out: &mut [u32], t: &Tone, s: f32, keep: f32, floor: f32, any: &mut i32) -> usize {
    use core::arch::wasm32::*;
    let n = out.len().min(acc.len() / 3) / 4 * 4;
    // Lane pattern of vector `o`: channels o, o+1, o+2, o (mod 3).
    let pat = |a: [f32; 3], o: usize| f32x4(a[o], a[(o + 1) % 3], a[(o + 2) % 3], a[o]);
    let (k, m, bg) = ([pat(t.k, 0), pat(t.k, 1), pat(t.k, 2)], [pat(t.m, 0), pat(t.m, 1), pat(t.m, 2)], [pat(t.bg, 0), pat(t.bg, 1), pat(t.bg, 2)]);
    let (sv, one, half, kv, fv, zero) = (f32x4_splat(s), f32x4_splat(1.0), f32x4_splat(0.5), f32x4_splat(keep), f32x4_splat(floor), f32x4_splat(0.0));
    let alpha = u8x16_splat(0xff);
    let mut anyv = i32x4_splat(0);
    for (a, o) in acc[..n * 3].chunks_exact_mut(12).zip(out[..n].chunks_exact_mut(4)) {
        let v = [f32x4(a[0], a[1], a[2], a[3]), f32x4(a[4], a[5], a[6], a[7]), f32x4(a[8], a[9], a[10], a[11])];
        anyv = i32x4_max(anyv, i32x4_max(v[0], i32x4_max(v[1], v[2])));
        let mut q = [i32x4_splat(0); 3];
        for j in 0..3 {
            let c = f32x4_mul(v[j], sv);
            let tone = if INK {
                f32x4_add(f32x4_div(bg[j], f32x4_add(one, c)), half)
            } else {
                // pmin(m, x) = x < m ? x : m, exactly `min_sel`.
                f32x4_add(f32x4_add(f32x4_pmin(m[j], f32x4_div(f32x4_mul(k[j], c), f32x4_add(one, c))), bg[j]), half)
            };
            q[j] = i32x4_trunc_sat_f32x4(tone);
            // pmax(0, x) = 0 < x ? x : 0, exactly `max0`.
            let d = f32x4_pmax(zero, f32x4_sub(f32x4_mul(v[j], kv), fv));
            a[j * 4] = f32x4_extract_lane::<0>(d);
            a[j * 4 + 1] = f32x4_extract_lane::<1>(d);
            a[j * 4 + 2] = f32x4_extract_lane::<2>(d);
            a[j * 4 + 3] = f32x4_extract_lane::<3>(d);
        }
        // 12 channel bytes in pixel order, then an opaque alpha byte per pixel.
        let b = u8x16_narrow_i16x8(i16x8_narrow_i32x4(q[0], q[1]), i16x8_narrow_i32x4(q[2], q[2]));
        let px = i8x16_shuffle::<0, 1, 2, 16, 3, 4, 5, 16, 6, 7, 8, 16, 9, 10, 11, 16>(b, alpha);
        o[0] = u32x4_extract_lane::<0>(px);
        o[1] = u32x4_extract_lane::<1>(px);
        o[2] = u32x4_extract_lane::<2>(px);
        o[3] = u32x4_extract_lane::<3>(px);
    }
    let lanes = [i32x4_extract_lane::<0>(anyv), i32x4_extract_lane::<1>(anyv), i32x4_extract_lane::<2>(anyv), i32x4_extract_lane::<3>(anyv)];
    *any = lanes.into_iter().fold(*any, i32::max);
    n
}
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
#[inline(always)]
fn resolve_simd<const INK: bool>(_acc: &mut [f32], _out: &mut [u32], _t: &Tone, _s: f32, _keep: f32, _floor: f32, _any: &mut i32) -> usize {
    0
}

impl Splat {
    pub fn new(width: usize, height: usize) -> Self {
        let mut s = Self::default();
        s.resize(width, height);
        s
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width.max(1);
        self.height = height.max(1);
        let n = self.width * self.height;
        self.acc = vec![0.0; n * 3];
        self.rgba = vec![0; n];
        self.touched = vec![true; self.height];
        self.lit = vec![true; self.height];
        self.last_bg = None;
    }

    /// RGBA bytes of the last [`Splat::resolve`].
    pub fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
        self.rgba.iter().flat_map(|p| p.to_le_bytes())
    }

    /// Light accumulated at pixel `(x, y)` (linear RGB).
    pub fn light(&self, x: usize, y: usize) -> [f32; 3] {
        let i = (y * self.width + x) * 3;
        [self.acc[i], self.acc[i + 1], self.acc[i + 2]]
    }

    /// Multiply accumulated light by `k` (0 clears; 0.8 leaves trails).
    pub fn fade(&mut self, k: f32) {
        self.touched.iter_mut().for_each(|t| *t = true);
        if k <= 0.0 {
            self.acc.iter_mut().for_each(|v| *v = 0.0);
        } else if k < 1.0 {
            self.acc.iter_mut().for_each(|v| *v = max0(*v * k - 1e-4));
        }
    }

    /// Splat `n` projected points (`[sx, sy, depth, scale] * n`).
    pub fn draw(&mut self, proj: &[f32], n: usize, p: &SplatParams) {
        let n = n.min(proj.len() / 4);
        let Splat { acc, colors, width, height, touched, .. } = self;
        let (width, height) = (*width, *height);
        let stride = width * 3;
        let (wf, hf) = (width as f32, height as f32);
        let fog = p.fog_far.is_finite();
        let inv_fog = 1.0 / (p.fog_far - p.fog_near).max(1e-3);
        let [lx, ly, lr, ls] = p.lens;
        let lens = lr > 0.0 && ls != 0.0;
        let lr2 = lr * lr;
        let maxr = p.max_radius.max(BILINEAR_MAX);
        let has_colors = colors.len() >= n * 3;
        let size = p.size * p.scale;
        for (i, o) in proj[..n * 4].chunks_exact(4).enumerate() {
            let sc = o[3];
            // `!(sc > 0)` also rejects NaN; garbage positions are skipped too.
            if !(sc > 0.0 && sc.is_finite() && o[0].is_finite() && o[1].is_finite()) {
                continue;
            }
            let (mut x, mut y) = (o[0], o[1]);
            let mut e = p.intensity;
            if fog {
                let f = 1.0 - (o[2] - p.fog_near) * inv_fog;
                if f <= 0.0 {
                    continue;
                }
                e *= f.min(1.0);
            }
            if lens {
                let (dx, dy) = (x - lx, y - ly);
                let d2 = dx * dx + dy * dy;
                if d2 < lr2 && d2 > 1e-6 {
                    let dl = sqrtf(d2);
                    let q = 1.0 - dl / lr;
                    let push = ls * q * q / dl;
                    x += dx * push;
                    y += dy * push;
                    e *= 1.0 + q * 1.5;
                }
            }
            let (cr, cg, cb) = if has_colors { (colors[i * 3], colors[i * 3 + 1], colors[i * 3 + 2]) } else { (1.0, 1.0, 1.0) };
            let (cr, cg, cb) = (cr * p.tint[0], cg * p.tint[1], cb * p.tint[2]);
            let bx = x * p.scale - 0.5;
            let by = y * p.scale - 0.5;
            let rad = (size * sc).min(maxr);
            if rad <= BILINEAR_MAX {
                // Bilinear splat: exact sub-pixel placement, energy-conserving.
                if !(bx >= 0.0 && by >= 0.0 && bx < wf - 1.0 && by < hf - 1.0) {
                    continue;
                }
                let (ix, iy) = (bx as usize, by as usize);
                let (fx, fy) = (bx - ix as f32, by - iy as f32);
                let (w1, w3) = (fx * (1.0 - fy), fx * fy);
                let (w0, w2) = ((1.0 - fy) - w1, fy - w3);
                touched[iy] = true;
                touched[iy + 1] = true;
                let (er, eg, eb) = (cr * e, cg * e, cb * e);
                let i0 = (iy * width + ix) * 3;
                let a = &mut acc[i0..i0 + 6];
                a[0] += er * w0;
                a[1] += eg * w0;
                a[2] += eb * w0;
                a[3] += er * w1;
                a[4] += eg * w1;
                a[5] += eb * w1;
                let b = &mut acc[i0 + stride..i0 + stride + 6];
                b[0] += er * w2;
                b[1] += eg * w2;
                b[2] += eb * w2;
                b[3] += er * w3;
                b[4] += eg * w3;
                b[5] += eb * w3;
            } else {
                // Cull in float space first: casting huge coordinates to usize
                // wraps on 32-bit targets (wasm32) and broke the range checks.
                if bx + rad < 0.0 || by + rad < 0.0 || bx - rad >= wf || by - rad >= hf {
                    continue;
                }
                // Soft disc; energy grows ~linearly with radius so near points read as bokeh.
                let x0 = (bx - rad).max(0.0) as usize;
                let y0 = (by - rad).max(0.0) as usize;
                let x1 = (ceilf(bx + rad) as isize).min(width as isize - 1);
                let y1 = (ceilf(by + rad) as isize).min(height as isize - 1);
                if x1 < 0 || y1 < 0 || x0 as isize > x1 || y0 as isize > y1 {
                    continue;
                }
                let (x1, y1) = (x1 as usize, y1 as usize);
                touched[y0..=y1].iter_mut().for_each(|t| *t = true);
                let inv = 1.0 / (rad * rad);
                let k = e * 2.0 / rad;
                let (er, eg, eb) = (cr * k, cg * k, cb * k);
                for yy in y0..=y1 {
                    let dy = yy as f32 - by;
                    let dy2 = dy * dy;
                    let row = &mut acc[(yy * width + x0) * 3..(yy * width + x1 + 1) * 3];
                    // (A branch-free select here measured ~20% slower on
                    // wasm: it turns every box pixel into a store.)
                    for (xx, px) in (x0..).zip(row.chunks_exact_mut(3)) {
                        let dx = xx as f32 - bx;
                        let q = 1.0 - (dx * dx + dy2) * inv;
                        if q > 0.0 {
                            let q = q * q;
                            px[0] += er * q;
                            px[1] += eg * q;
                            px[2] += eb * q;
                        }
                    }
                }
            }
        }
    }

    /// Tone-map the accumulator over background `bg` (0xRRGGBB) into
    /// [`Splat::rgba`], then multiply the accumulator by `keep` (trails; 0
    /// clears) ready for the next frame.
    pub fn resolve_fade(&mut self, exposure: f32, bg: u32, keep: f32) {
        let ch = |shift: u32| ((bg >> shift) & 255) as f32;
        let (br, bgc, bb) = (ch(16), ch(8), ch(0));
        let tone = Tone {
            bg: [br, bgc, bb],
            k: [(255.0 - br) * 1.125, (255.0 - bgc) * 1.125, (255.0 - bb) * 1.125],
            m: [255.0 - br, 255.0 - bgc, 255.0 - bb],
        };
        let s = exposure.max(0.0);
        let keep = keep.clamp(0.0, 1.0);
        let floor = if keep < 1.0 { 1e-4 } else { 0.0 };
        // One pass: Reinhard c / (1 + c) rescaled so very bright light reaches
        // white, then decay the light for the next frame. Branch-free so it
        // vectorises (f32x4 on wasm simd128). Subtracting a floor keeps
        // decaying light out of denormal range (~100x slower arithmetic) and
        // lets trails end.
        if self.last_bg != Some(bg) || self.last_ink != self.ink {
            self.last_ink = self.ink;
            // Background changed (or first frame): every row must be written.
            self.lit.iter_mut().for_each(|l| *l = true);
            self.last_bg = Some(bg);
        }
        let w = self.width;
        let rows = self.acc.chunks_exact_mut(w * 3).zip(self.rgba.chunks_exact_mut(w));
        for ((acc, out), (touched, lit)) in rows.zip(self.touched.iter_mut().zip(self.lit.iter_mut())) {
            // A row with no light now and none at the last resolve already
            // shows plain background: skip it (most of the frame for sparse
            // shapes — this pass is memory-bound).
            if !*touched && !*lit {
                continue;
            }
            *touched = false;
            *lit = if self.ink {
                resolve_row::<true>(acc, out, &tone, s, keep, floor)
            } else {
                resolve_row::<false>(acc, out, &tone, s, keep, floor)
            };
        }
    }

    /// Tone-map without fading.
    pub fn resolve(&mut self, exposure: f32, bg: u32) {
        self.resolve_fade(exposure, bg, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The previous planar implementation, kept verbatim as the reference
    /// the interleaved accumulator must match bit for bit.
    #[allow(dead_code)]
    mod reference {
        use crate::math::{ceilf, sqrtf};
        use crate::splat::SplatParams;
        use alloc::vec;
        use alloc::vec::Vec;
        const BILINEAR_MAX: f32 = 1.6;
        #[derive(Debug, Default)]
        pub struct Planar {
            pub width: usize,
            pub height: usize,
            pub r: Vec<f32>,
            pub g: Vec<f32>,
            pub b: Vec<f32>,
            pub rgba: Vec<u32>,
            pub colors: Vec<f32>,
            touched: Vec<bool>,
            lit: Vec<bool>,
            last_bg: Option<u32>,
            pub ink: bool,
            last_ink: bool,
        }
        impl Planar {
            pub fn new(width: usize, height: usize) -> Self {
                let mut s = Self::default();
                s.resize(width, height);
                s
            }

            pub fn resize(&mut self, width: usize, height: usize) {
                self.width = width.max(1);
                self.height = height.max(1);
                let n = self.width * self.height;
                self.r = vec![0.0; n];
                self.g = vec![0.0; n];
                self.b = vec![0.0; n];
                self.rgba = vec![0; n];
                self.touched = vec![true; self.height];
                self.lit = vec![true; self.height];
                self.last_bg = None;
            }

            /// RGBA bytes of the last [`Splat::resolve`].
            pub fn bytes(&self) -> impl Iterator<Item = u8> + '_ {
                self.rgba.iter().flat_map(|p| p.to_le_bytes())
            }

            /// Light accumulated at pixel `(x, y)` (linear RGB).
            pub fn light(&self, x: usize, y: usize) -> [f32; 3] {
                let i = y * self.width + x;
                [self.r[i], self.g[i], self.b[i]]
            }

            /// Multiply accumulated light by `k` (0 clears; 0.8 leaves trails).
            pub fn fade(&mut self, k: f32) {
                self.touched.iter_mut().for_each(|t| *t = true);
                for p in [&mut self.r, &mut self.g, &mut self.b] {
                    if k <= 0.0 {
                        p.iter_mut().for_each(|v| *v = 0.0);
                    } else if k < 1.0 {
                        p.iter_mut().for_each(|v| *v = (*v * k - 1e-4).max(0.0));
                    }
                }
            }

            /// Splat `n` projected points (`[sx, sy, depth, scale] * n`).
            pub fn draw(&mut self, proj: &[f32], n: usize, p: &SplatParams) {
                let n = n.min(proj.len() / 4);
                let Planar { r: pr, g: pg, b: pb, colors, width, height, touched, .. } = self;
                let (width, height) = (*width, *height);
                let (wf, hf) = (width as f32, height as f32);
                let fog = p.fog_far.is_finite();
                let inv_fog = 1.0 / (p.fog_far - p.fog_near).max(1e-3);
                let [lx, ly, lr, ls] = p.lens;
                let lens = lr > 0.0 && ls != 0.0;
                let lr2 = lr * lr;
                let maxr = p.max_radius.max(BILINEAR_MAX);
                let has_colors = colors.len() >= n * 3;
                let size = p.size * p.scale;
                for (i, o) in proj[..n * 4].chunks_exact(4).enumerate() {
                    let sc = o[3];
                    // `!(sc > 0)` also rejects NaN; garbage positions are skipped too.
                    if !(sc > 0.0 && sc.is_finite() && o[0].is_finite() && o[1].is_finite()) {
                        continue;
                    }
                    let (mut x, mut y) = (o[0], o[1]);
                    let mut e = p.intensity;
                    if fog {
                        let f = 1.0 - (o[2] - p.fog_near) * inv_fog;
                        if f <= 0.0 {
                            continue;
                        }
                        e *= f.min(1.0);
                    }
                    if lens {
                        let (dx, dy) = (x - lx, y - ly);
                        let d2 = dx * dx + dy * dy;
                        if d2 < lr2 && d2 > 1e-6 {
                            let dl = sqrtf(d2);
                            let q = 1.0 - dl / lr;
                            let push = ls * q * q / dl;
                            x += dx * push;
                            y += dy * push;
                            e *= 1.0 + q * 1.5;
                        }
                    }
                    let (cr, cg, cb) = if has_colors { (colors[i * 3], colors[i * 3 + 1], colors[i * 3 + 2]) } else { (1.0, 1.0, 1.0) };
                    let (cr, cg, cb) = (cr * p.tint[0], cg * p.tint[1], cb * p.tint[2]);
                    let bx = x * p.scale - 0.5;
                    let by = y * p.scale - 0.5;
                    let rad = (size * sc).min(maxr);
                    if rad <= BILINEAR_MAX {
                        // Bilinear splat: exact sub-pixel placement, energy-conserving.
                        if !(bx >= 0.0 && by >= 0.0 && bx < wf - 1.0 && by < hf - 1.0) {
                            continue;
                        }
                        let (ix, iy) = (bx as usize, by as usize);
                        let (fx, fy) = (bx - ix as f32, by - iy as f32);
                        let (w1, w3) = (fx * (1.0 - fy), fx * fy);
                        let (w0, w2) = ((1.0 - fy) - w1, fy - w3);
                        touched[iy] = true;
                        touched[iy + 1] = true;
                        let i0 = iy * width + ix;
                        let i2 = i0 + width;
                        for (plane, c) in [(&mut *pr, cr * e), (&mut *pg, cg * e), (&mut *pb, cb * e)] {
                            let a = &mut plane[i0..i0 + 2];
                            a[0] += c * w0;
                            a[1] += c * w1;
                            let b = &mut plane[i2..i2 + 2];
                            b[0] += c * w2;
                            b[1] += c * w3;
                        }
                    } else {
                        // Cull in float space first: casting huge coordinates to usize
                        // wraps on 32-bit targets (wasm32) and broke the range checks.
                        if bx + rad < 0.0 || by + rad < 0.0 || bx - rad >= wf || by - rad >= hf {
                            continue;
                        }
                        // Soft disc; energy grows ~linearly with radius so near points read as bokeh.
                        let x0 = (bx - rad).max(0.0) as usize;
                        let y0 = (by - rad).max(0.0) as usize;
                        let x1 = (ceilf(bx + rad) as isize).min(width as isize - 1);
                        let y1 = (ceilf(by + rad) as isize).min(height as isize - 1);
                        if x1 < 0 || y1 < 0 || x0 as isize > x1 || y0 as isize > y1 {
                            continue;
                        }
                        touched[y0..=y1 as usize].iter_mut().for_each(|t| *t = true);
                        let inv = 1.0 / (rad * rad);
                        let k = e * 2.0 / rad;
                        let (er, eg, eb) = (cr * k, cg * k, cb * k);
                        for yy in y0..=y1 as usize {
                            let dy = yy as f32 - by;
                            let dy2 = dy * dy;
                            let row = yy * width;
                            for xx in x0..=x1 as usize {
                                let dx = xx as f32 - bx;
                                let q = 1.0 - (dx * dx + dy2) * inv;
                                if q > 0.0 {
                                    let q = q * q;
                                    pr[row + xx] += er * q;
                                    pg[row + xx] += eg * q;
                                    pb[row + xx] += eb * q;
                                }
                            }
                        }
                    }
                }
            }

            /// Tone-map the accumulator over background `bg` (0xRRGGBB) into
            /// [`Splat::rgba`], then multiply the accumulator by `keep` (trails; 0
            /// clears) ready for the next frame.
            pub fn resolve_fade(&mut self, exposure: f32, bg: u32, keep: f32) {
                let ch = |shift: u32| ((bg >> shift) & 255) as f32;
                let (br, bgc, bb) = (ch(16), ch(8), ch(0));
                let (kr, kg, kb) = ((255.0 - br) * 1.125, (255.0 - bgc) * 1.125, (255.0 - bb) * 1.125);
                let (mr, mg, mb) = (255.0 - br, 255.0 - bgc, 255.0 - bb);
                let s = exposure.max(0.0);
                let keep = keep.clamp(0.0, 1.0);
                let floor = if keep < 1.0 { 1e-4 } else { 0.0 };
                // One pass: Reinhard c / (1 + c) rescaled so very bright light reaches
                // white, then decay the light for the next frame. Branch-free so it
                // vectorises (f32x4 on wasm simd128). Subtracting a floor keeps
                // decaying light out of denormal range (~100x slower arithmetic) and
                // lets trails end.
                if self.last_bg != Some(bg) || self.last_ink != self.ink {
                    self.last_ink = self.ink;
                    // Background changed (or first frame): every row must be written.
                    self.lit.iter_mut().for_each(|l| *l = true);
                    self.last_bg = Some(bg);
                }
                let w = self.width;
                for y in 0..self.height {
                    // A row with no light now and none at the last resolve already
                    // shows plain background: skip it (most of the frame for sparse
                    // shapes — this pass is memory-bound).
                    if !self.touched[y] && !self.lit[y] {
                        continue;
                    }
                    self.touched[y] = false;
                    let span = y * w..(y + 1) * w;
                    let mut any = 0.0f32;
                    let pixels = self.rgba[span.clone()]
                        .iter_mut()
                        .zip(self.r[span.clone()].iter_mut())
                        .zip(self.g[span.clone()].iter_mut())
                        .zip(self.b[span].iter_mut());
                    if self.ink {
                        // Ink absorbs: transmittance 1 / (1 + c) per channel.
                        for (((px, r), g), b) in pixels {
                            let (cr, cg, cb) = (*r * s, *g * s, *b * s);
                            any = any.max(*r + *g + *b);
                            let vr = br / (1.0 + cr) + 0.5;
                            let vg = bgc / (1.0 + cg) + 0.5;
                            let vb = bb / (1.0 + cb) + 0.5;
                            *px = (vr as i32 as u32) | ((vg as i32 as u32) << 8) | ((vb as i32 as u32) << 16) | 0xff00_0000;
                            *r = (*r * keep - floor).max(0.0);
                            *g = (*g * keep - floor).max(0.0);
                            *b = (*b * keep - floor).max(0.0);
                        }
                    } else {
                        for (((px, r), g), b) in pixels {
                            let (cr, cg, cb) = (*r * s, *g * s, *b * s);
                            any = any.max(*r + *g + *b);
                            let vr = (kr * cr / (1.0 + cr)).min(mr) + br + 0.5;
                            let vg = (kg * cg / (1.0 + cg)).min(mg) + bgc + 0.5;
                            let vb = (kb * cb / (1.0 + cb)).min(mb) + bb + 0.5;
                            *px = (vr as i32 as u32) | ((vg as i32 as u32) << 8) | ((vb as i32 as u32) << 16) | 0xff00_0000;
                            *r = (*r * keep - floor).max(0.0);
                            *g = (*g * keep - floor).max(0.0);
                            *b = (*b * keep - floor).max(0.0);
                        }
                    }
                    self.lit[y] = any > 0.0;
                }
            }

            /// Tone-map without fading.
            pub fn resolve(&mut self, exposure: f32, bg: u32) {
                self.resolve_fade(exposure, bg, 1.0);
            }
        }
    }

    /// A fixed scene rendered through the projector + splat pipeline, hashed
    /// (FNV-1a over the RGBA words of every frame). `test/cloud.test.ts`
    /// renders the same scene through the WebAssembly build and expects the
    /// same hash, which pins the wasm SIMD tone-map path to this scalar one.
    pub(crate) fn golden_scene_hash<D, R>(mut draw: D, mut resolve: R) -> u32
    where
        D: FnMut(&[f32], usize, &SplatParams),
        R: FnMut(f32, u32, f32, bool) -> alloc::vec::Vec<u32>,
    {
        let n = 400;
        let mut pts = alloc::vec::Vec::with_capacity(n * 3);
        for i in 0..n {
            let z = 1.0 + ((i * 31) % 97) as f64 / 97.0 * 3.0;
            let x = (((i * 7919) % 997) as f64 / 997.0 * 2.4 - 1.2) * z;
            let y = (((i * 104_729) % 991) as f64 / 991.0 * 2.4 - 1.2) * z;
            pts.extend_from_slice(&[x as f32, y as f32, z as f32]);
        }
        // Clip w = z: a bare perspective divide.
        let mut vp = crate::space::IDENTITY;
        vp[11] = 1.0;
        vp[15] = 0.0;
        let mut pr = crate::space::Projector::default();
        pr.project(&pts, 3, [0, 1, 2], n, &crate::space::IDENTITY, &vp, 37.0, 23.0, false);
        let mut h = 0x811c_9dc5u32;
        for frame in 0..8 {
            let p = SplatParams {
                size: 0.25,
                intensity: 0.75,
                fog_near: 1.5,
                fog_far: if frame % 2 == 0 { 4.5 } else { f32::INFINITY },
                scale: 1.0,
                lens: [18.0, 11.0, 6.0, if frame == 1 { 2.0 } else { 0.0 }],
                max_radius: 3.0,
                tint: [1.0, 0.75, 0.5],
            };
            if frame != 3 {
                draw(&pr.out, n, &p);
            }
            let keep = [0.5, 0.0, 1.0, 0.75][frame % 4];
            let bg = if frame < 4 { 0x07080b } else { 0xeeddcc };
            for w in resolve(1.0 + frame as f32 * 0.25, bg, keep, frame >= 5) {
                h = (h ^ w).wrapping_mul(16_777_619);
            }
        }
        h
    }

    pub(crate) const GOLDEN_COLORS: fn(usize) -> f32 = |i| (((i * 104_729) % 1000) as f64 / 1000.0) as f32;

    #[test]
    fn golden_scene_matches_reference() {
        use core::cell::RefCell;
        let colors: alloc::vec::Vec<f32> = (0..400 * 3).map(GOLDEN_COLORS).collect();
        let a = RefCell::new(Splat::new(37, 23));
        a.borrow_mut().colors = colors.clone();
        let got = golden_scene_hash(
            |pr, n, p| a.borrow_mut().draw(pr, n, p),
            |e, bg, keep, ink| {
                let mut s = a.borrow_mut();
                s.ink = ink;
                s.resolve_fade(e, bg, keep);
                s.rgba.clone()
            },
        );
        let b = RefCell::new(reference::Planar::new(37, 23));
        b.borrow_mut().colors = colors;
        let want = golden_scene_hash(
            |pr, n, p| b.borrow_mut().draw(pr, n, p),
            |e, bg, keep, ink| {
                let mut s = b.borrow_mut();
                s.ink = ink;
                s.resolve_fade(e, bg, keep);
                s.rgba.clone()
            },
        );
        assert_eq!(got, want);
        assert_eq!(got, GOLDEN_HASH, "update GOLDEN_HASH here and in test/cloud.test.ts only if the output is meant to change");
    }
    const GOLDEN_HASH: u32 = 4_017_498_113;

    #[test]
    fn matches_planar_reference_bit_for_bit() {
        use reference::Planar;
        let mut rng = crate::math::Rng(12345);
        let mut f = || rng.next_f64() as f32;
        // Odd widths and heights catch any row-stride mistakes.
        for &(w, h, ink) in &[(37usize, 23usize, false), (64, 40, false), (37, 23, true)] {
            let mut a = Splat::new(w, h);
            let mut b = Planar::new(w, h);
            a.ink = ink;
            b.ink = ink;
            let n = 600;
            let colors: Vec<f32> = (0..n * 3).map(|_| f()).collect();
            a.colors = colors.clone();
            b.colors = colors;
            for frame in 0..12 {
                // Mix of bilinear points, discs (some straddling the edges),
                // off-screen, culled and fogged points.
                let mut proj = Vec::with_capacity(n * 4);
                for _ in 0..n {
                    let sx = f() * (w as f32 + 10.0) - 5.0;
                    let sy = f() * (h as f32 + 10.0) - 5.0;
                    let depth = 500.0 + f() * 2500.0;
                    let sc = if f() < 0.1 { 0.0 } else { f() * f() * 6.0 };
                    proj.extend_from_slice(&[sx, sy, depth, sc]);
                }
                let p = SplatParams {
                    size: 1.0 + frame as f32 * 0.1,
                    intensity: 0.4,
                    fog_near: 700.0,
                    fog_far: if frame % 2 == 0 { 2600.0 } else { f32::INFINITY },
                    scale: 0.9,
                    lens: [w as f32 * 0.5, h as f32 * 0.5, 8.0, if frame % 3 == 0 { 3.0 } else { 0.0 }],
                    max_radius: 5.0,
                    tint: [1.0, 0.8, 0.6],
                };
                // Draw sometimes skips a frame so untouched/lit row skipping runs.
                if frame % 4 != 3 {
                    a.draw(&proj, n, &p);
                    b.draw(&proj, n, &p);
                }
                let bg = if frame < 6 { 0x07080b } else { 0x203040 };
                let keep = [0.5, 0.0, 1.0, 0.85][frame % 4];
                a.resolve_fade(1.0 + frame as f32 * 0.25, bg, keep);
                b.resolve_fade(1.0 + frame as f32 * 0.25, bg, keep);
                assert_eq!(a.rgba, b.rgba, "rgba differs at frame {frame} ({w}x{h}, ink {ink})");
                for y in 0..h {
                    for x in 0..w {
                        let i = y * w + x;
                        assert_eq!(a.light(x, y), [b.r[i], b.g[i], b.b[i]], "light differs at frame {frame}");
                    }
                }
                if frame == 8 {
                    a.fade(0.7);
                    b.fade(0.7);
                }
            }
        }
    }

    #[test]
    fn bilinear_conserves_energy_and_tone_maps() {
        let mut s = Splat::new(16, 16);
        let proj = [8.25f32, 8.75, 10.0, 0.5];
        s.draw(&proj, 1, &SplatParams { intensity: 1.0, ..Default::default() });
        let total: f32 = s.acc.iter().step_by(3).sum();
        assert!((total - 1.0).abs() < 1e-5);
        s.resolve(1.0, 0x000000);
        let b: alloc::vec::Vec<u8> = s.bytes().collect();
        assert!(b.chunks(4).any(|p| p[0] > 0));
        assert!(b.chunks(4).all(|p| p[3] == 255));
        s.resolve_fade(1.0, 0x102030, 0.0);
        s.resolve(1.0, 0x102030);
        let b: alloc::vec::Vec<u8> = s.bytes().collect();
        assert_eq!(&b[..4], &[0x10, 0x20, 0x30, 255]);
        // Saturated light reaches white.
        s.draw(&[3.0, 3.0, 1.0, 0.5], 1, &SplatParams { intensity: 1000.0, ..Default::default() });
        s.resolve(1.0, 0x102030);
        assert_eq!(s.rgba[3 * 16 + 3] & 0xff, 255);
    }

    #[test]
    fn culls_and_clips() {
        let mut s = Splat::new(8, 8);
        // Off-screen, culled (scale 0), far off-screen disc: nothing lands.
        let proj = [-5.0f32, 3.0, 1.0, 0.5, 3.0, 3.0, 1.0, 0.0, 100.0, 100.0, 1.0, 9.0];
        s.draw(&proj, 3, &SplatParams { size: 1.0, ..Default::default() });
        assert!(s.acc.iter().all(|v| *v == 0.0));
        // A big disc straddling the edge is clipped, not dropped.
        s.draw(&[7.0, 4.0, 1.0, 9.0], 1, &SplatParams { size: 1.0, ..Default::default() });
        assert!(s.acc.iter().any(|v| *v > 0.0));
    }

    #[test]
    fn skips_non_finite_points() {
        let mut s = Splat::new(8, 8);
        let n = f32::NAN;
        s.draw(&[4.0, 4.0, 1.0, n, n, 4.0, 1.0, 9.0, 4.0, f32::INFINITY, 1.0, 9.0], 3, &SplatParams::default());
        assert!(s.acc.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn huge_coordinates_are_culled_not_wrapped() {
        // A point just in front of the camera can project billions of pixels away.
        let mut s = Splat::new(64, 48);
        let proj = [3.6e9f32, 20.0, 1.0, 9.0, 20.0, 3.6e9, 1.0, 9.0, -3.6e9, -3.6e9, 1.0, 9.0, 3.0e9, 3.0e9, 1.0, 1e6];
        s.draw(&proj, 4, &SplatParams { max_radius: 7.0, ..Default::default() });
        assert!(s.acc.iter().all(|v| *v == 0.0));
        s.resolve(1.0, 0);
    }

    #[test]
    fn ink_darkens_paper() {
        let mut s = Splat::new(8, 8);
        s.ink = true;
        s.draw(&[4.0, 4.0, 1.0, 0.5], 1, &SplatParams { intensity: 50.0, ..Default::default() });
        s.resolve(1.0, 0xeeeeee);
        let dark = s.rgba[3 * 8 + 3] & 0xff;
        assert!(dark < 0x40, "{dark:x}");
        assert_eq!(s.rgba[0] & 0xff, 0xee);
        // Tint scales the deposit.
        let mut t = Splat::new(8, 8);
        t.draw(&[4.0, 4.0, 1.0, 0.5], 1, &SplatParams { tint: [0.0, 1.0, 1.0], ..Default::default() });
        assert_eq!(t.light(3, 3)[0], 0.0);
        assert!(t.light(3, 3)[1] > 0.0);
    }

    #[test]
    fn lens_pushes_points() {
        let mut a = Splat::new(64, 8);
        let mut b = Splat::new(64, 8);
        let proj = [30.0f32, 4.0, 1.0, 0.5];
        a.draw(&proj, 1, &SplatParams::default());
        b.draw(&proj, 1, &SplatParams { lens: [26.0, 4.0, 20.0, 10.0], ..Default::default() });
        let cx = |s: &Splat| {
            let mut m = 0.0;
            let mut t = 0.0;
            for (i, v) in s.acc.iter().step_by(3).enumerate() {
                m += (i % 64) as f32 * v;
                t += v;
            }
            m / t
        };
        assert!(cx(&b) > cx(&a) + 3.0);
    }
}
