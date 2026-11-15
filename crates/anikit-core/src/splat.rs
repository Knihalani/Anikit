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
}

impl Default for SplatParams {
    fn default() -> Self {
        Self { size: 1.0, intensity: 0.3, fog_near: 0.0, fog_far: f32::INFINITY, scale: 1.0, lens: [0.0; 4], max_radius: 6.0 }
    }
}

/// Accumulation planes + tone-mapped RGBA output.
#[derive(Debug, Default)]
pub struct Splat {
    pub width: usize,
    pub height: usize,
    /// Planar light accumulators (R, G, B), `width * height` each — planar so
    /// fade and tone mapping vectorise.
    r: Vec<f32>,
    g: Vec<f32>,
    b: Vec<f32>,
    /// Tone-mapped pixels as little-endian RGBA words (`width * height`).
    pub rgba: Vec<u32>,
    /// Per-point linear RGB in [0, 1], `[r, g, b] * n`; missing entries are white.
    pub colors: Vec<f32>,
    /// Rows drawn into since the last resolve.
    touched: Vec<bool>,
    /// Rows that held any light at the last resolve (so their pixels aren't plain background).
    lit: Vec<bool>,
    last_bg: Option<u32>,
}

/// Points up to this buffer-pixel radius use the 4-tap bilinear splat.
const BILINEAR_MAX: f32 = 1.6;

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
        let Splat { r: pr, g: pg, b: pb, colors, width, height, touched, .. } = self;
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
        if self.last_bg != Some(bg) {
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
            self.lit[y] = any > 0.0;
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

    #[test]
    fn bilinear_conserves_energy_and_tone_maps() {
        let mut s = Splat::new(16, 16);
        let proj = [8.25f32, 8.75, 10.0, 0.5];
        s.draw(&proj, 1, &SplatParams { intensity: 1.0, ..Default::default() });
        let total: f32 = s.r.iter().sum();
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
        assert!(s.r.iter().all(|v| *v == 0.0));
        // A big disc straddling the edge is clipped, not dropped.
        s.draw(&[7.0, 4.0, 1.0, 9.0], 1, &SplatParams { size: 1.0, ..Default::default() });
        assert!(s.r.iter().any(|v| *v > 0.0));
    }

    #[test]
    fn skips_non_finite_points() {
        let mut s = Splat::new(8, 8);
        let n = f32::NAN;
        s.draw(&[4.0, 4.0, 1.0, n, n, 4.0, 1.0, 9.0, 4.0, f32::INFINITY, 1.0, 9.0], 3, &SplatParams::default());
        assert!(s.r.iter().all(|v| *v == 0.0));
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
            for (i, v) in s.r.iter().enumerate() {
                m += (i % 64) as f32 * v;
                t += v;
            }
            m / t
        };
        assert!(cx(&b) > cx(&a) + 3.0);
    }
}
