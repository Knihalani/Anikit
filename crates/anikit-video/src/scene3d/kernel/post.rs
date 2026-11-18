//! The `scene3d` post stack, all in linear HDR until the very end:
//! floor-reflection blur → AO/shadow denoise → depth of field → motion blur →
//! bloom → tone map → vignette → sRGB with dithering.
//!
//! Performance layout: the per-pixel records from the primary pass are
//! unpacked once into planes (structure of arrays), so every stage streams
//! only the channels it needs; every pass is row-parallel ([`super::par`]),
//! and the inner loops run over contiguous x with no branches so LLVM
//! vectorises them. The maths is the original pipeline's.

use super::super::math::{hash2, smoothstep, u01};
use super::super::trace::Px;
use super::par;
use super::pool::{Buf, Pod};

pub struct Post {
    /// Focus distance (view depth) and blur radius in pixels for points at infinity.
    pub focus: f32,
    pub aperture: f32,
    /// Shutter as a fraction of the frame interval (0.5 = 180°).
    pub shutter: f32,
    pub bloom: f32,
    pub exposure: f32,
    pub vignette: f32,
    /// `aces` (filmic, contrasty) or `neutral` (Khronos PBR Neutral: whites stay white).
    pub tonemap: Tonemap,
    /// Denoiser passes (0 = off).
    pub denoise: u32,
}

/// Half-resolution light added before depth of field (volumetric shafts).
pub struct Volume<'a> {
    pub px: &'a [[f32; 3]],
    pub w: usize,
    pub h: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tonemap {
    Aces,
    Neutral,
}

type Img = Buf<[f32; 3]>;

/// A raw pointer that may be written from several threads at disjoint indices.
#[derive(Clone, Copy)]
struct Out<T>(*mut T);
unsafe impl<T> Send for Out<T> {}
unsafe impl<T> Sync for Out<T> {}
impl<T> Out<T> {
    #[inline(always)]
    fn at(self, i: usize) -> *mut T {
        self.0.wrapping_add(i)
    }
    /// SAFETY: `i` is in bounds and no other thread writes index `i`.
    #[inline(always)]
    unsafe fn put(self, i: usize, v: T) {
        unsafe { self.0.add(i).write(v) }
    }
}

/// The primary pass's output as planes (structure of arrays): the trace
/// writes them through [`PlanesOut`], post reads only what each stage needs.
pub struct Planes {
    pub col: Img,
    /// Floor reflection (rgb) and the coverage of samples that traced it.
    pub refl: Buf<[f32; 4]>,
    pub cov: Buf<f32>,
    pub rad: Buf<f32>,
    pub rf: Buf<f32>,
    /// Diffuse albedo × ambient, albedo × key, and AO-occluded specular.
    pub da: Img,
    pub dk: Img,
    pub sp: Img,
    pub ao: Buf<f32>,
    pub vk: Buf<f32>,
    /// View depth (distance along the camera axis).
    pub depth: Buf<f32>,
    pub nx: Buf<f32>,
    pub ny: Buf<f32>,
    pub nz: Buf<f32>,
    pub id: Buf<u32>,
    /// 1 where the pixel has diffuse albedo or occludable specular (else 0).
    pub lit: Buf<f32>,
    /// Weight of the traced visibility (0: checkerboard gap, filled by the denoiser).
    pub vw: Buf<f32>,
    pub vel: Buf<[f32; 2]>,
}

/// Writes pixels into [`Planes`] from several threads (disjoint pixels).
#[derive(Clone, Copy)]
pub struct PlanesOut {
    col: Out<[f32; 3]>,
    refl: Out<[f32; 4]>,
    cov: Out<f32>,
    rad: Out<f32>,
    rf: Out<f32>,
    da: Out<[f32; 3]>,
    dk: Out<[f32; 3]>,
    sp: Out<[f32; 3]>,
    ao: Out<f32>,
    vk: Out<f32>,
    depth: Out<f32>,
    nx: Out<f32>,
    ny: Out<f32>,
    nz: Out<f32>,
    id: Out<u32>,
    lit: Out<f32>,
    vw: Out<f32>,
    vel: Out<[f32; 2]>,
}

impl PlanesOut {
    /// Store pixel `i`.
    ///
    /// # Safety
    /// `i` is inside the planes, no other thread writes pixel `i`, and the
    /// planes outlive every write (nothing else touches them meanwhile).
    #[inline]
    pub unsafe fn put(&self, i: usize, q: &Px) {
        unsafe {
            self.col.put(i, q.col);
            self.refl.put(i, [q.refl[0], q.refl[1], q.refl[2], q.rcov]);
            self.cov.put(i, q.cov);
            self.rad.put(i, q.rad);
            self.rf.put(i, q.rf);
            self.da.put(i, [q.alb[0] * q.amb[0], q.alb[1] * q.amb[1], q.alb[2] * q.amb[2]]);
            self.dk.put(i, [q.alb[0] * q.key[0], q.alb[1] * q.key[1], q.alb[2] * q.key[2]]);
            self.sp.put(i, q.spec);
            self.ao.put(i, q.ao);
            self.vk.put(i, q.vk);
            self.depth.put(i, q.depth);
            self.nx.put(i, q.nrm[0]);
            self.ny.put(i, q.nrm[1]);
            self.nz.put(i, q.nrm[2]);
            self.id.put(i, q.id);
            let l = q.alb[0] + q.alb[1] + q.alb[2] + q.spec[0] + q.spec[1] + q.spec[2] > 1e-5;
            self.lit.put(i, if l { 1.0 } else { 0.0 });
            self.vel.put(i, q.vel);
            self.vw.put(i, q.vw);
        }
    }
}

impl Planes {
    /// `n` pixels, contents unspecified until written (pooled storage).
    pub fn new(n: usize) -> Planes {
        Planes {
            col: Buf::uninit(n),
            refl: Buf::uninit(n),
            cov: Buf::uninit(n),
            rad: Buf::uninit(n),
            rf: Buf::uninit(n),
            da: Buf::uninit(n),
            dk: Buf::uninit(n),
            sp: Buf::uninit(n),
            ao: Buf::uninit(n),
            vk: Buf::uninit(n),
            depth: Buf::uninit(n),
            nx: Buf::uninit(n),
            ny: Buf::uninit(n),
            nz: Buf::uninit(n),
            id: Buf::uninit(n),
            lit: Buf::uninit(n),
            vw: Buf::uninit(n),
            vel: Buf::uninit(n),
        }
    }

    pub fn len(&self) -> usize {
        self.col.len()
    }

    pub fn is_empty(&self) -> bool {
        self.col.is_empty()
    }

    /// A writer for filling the planes from parallel tasks.
    pub fn writer(&mut self) -> PlanesOut {
        PlanesOut {
            col: Out(self.col.as_mut_ptr()),
            refl: Out(self.refl.as_mut_ptr()),
            cov: Out(self.cov.as_mut_ptr()),
            rad: Out(self.rad.as_mut_ptr()),
            rf: Out(self.rf.as_mut_ptr()),
            da: Out(self.da.as_mut_ptr()),
            dk: Out(self.dk.as_mut_ptr()),
            sp: Out(self.sp.as_mut_ptr()),
            ao: Out(self.ao.as_mut_ptr()),
            vk: Out(self.vk.as_mut_ptr()),
            depth: Out(self.depth.as_mut_ptr()),
            nx: Out(self.nx.as_mut_ptr()),
            ny: Out(self.ny.as_mut_ptr()),
            nz: Out(self.nz.as_mut_ptr()),
            id: Out(self.id.as_mut_ptr()),
            lit: Out(self.lit.as_mut_ptr()),
            vw: Out(self.vw.as_mut_ptr()),
            vel: Out(self.vel.as_mut_ptr()),
        }
    }

    /// From per-pixel records.
    pub fn unpack(px: &[Px]) -> Planes {
        let mut p = Planes::new(px.len());
        let o = p.writer();
        let body = |i0: usize, chunk: &[Px]| {
            for (j, q) in chunk.iter().enumerate() {
                // SAFETY: each index is written by exactly one chunk; `p` outlives this.
                unsafe { o.put(i0 + j, q) };
            }
        };
        const CH: usize = 8192;
        if par::parallel() {
            use rayon::prelude::*;
            px.par_chunks(CH).enumerate().for_each(|(k, c)| body(k * CH, c));
        } else {
            body(0, px);
        }
        p
    }
}

// ---------------------------------------------------------------- blurs

/// Box blur of radius `r` along rows; the window shrinks at the borders
/// (normalised by the samples inside).
fn box_rows<const C: usize>(src: &[[f32; C]], w: usize, h: usize, r: usize) -> Buf<[f32; C]> {
    let mut out = Buf::uninit(w * h);
    par::rows(&mut out, w, |y, row: &mut [[f32; C]]| {
        let s = &src[y * w..y * w + w];
        for (x, o) in row.iter_mut().enumerate() {
            let (a, b) = (x.saturating_sub(r), (x + r).min(w - 1));
            let mut acc = [0.0f32; C];
            for v in &s[a..=b] {
                for c in 0..C {
                    acc[c] += v[c];
                }
            }
            let inv = 1.0 / (b - a + 1) as f32;
            *o = acc.map(|v| v * inv);
        }
    });
    out
}

/// Box blur of radius `r` along columns (row-parallel: each output row sums
/// its input rows, vectorised over x).
fn box_cols<const C: usize>(src: &[[f32; C]], w: usize, h: usize, r: usize) -> Buf<[f32; C]> {
    let mut out = Buf::uninit(w * h);
    par::rows(&mut out, w, |y, row: &mut [[f32; C]]| {
        let (a, b) = (y.saturating_sub(r), (y + r).min(h - 1));
        row.copy_from_slice(&src[a * w..a * w + w]);
        for yy in a + 1..=b {
            let s = &src[yy * w..yy * w + w];
            for (o, v) in row.iter_mut().zip(s) {
                for c in 0..C {
                    o[c] += v[c];
                }
            }
        }
        let inv = 1.0 / (b - a + 1) as f32;
        for o in row.iter_mut() {
            for c in 0..C {
                o[c] *= inv;
            }
        }
    });
    out
}

/// 2×2 average (edge-clamped) to `ceil(w/2) × ceil(h/2)`.
fn half4(src: &[[f32; 4]], w: usize, h: usize) -> (Buf<[f32; 4]>, usize, usize) {
    let (nw, nh) = (w.div_ceil(2).max(1), h.div_ceil(2).max(1));
    let d = par::image(nw, nh, |y, row: &mut [[f32; 4]]| {
        let (y0, y1) = ((2 * y).min(h - 1), (2 * y + 1).min(h - 1));
        for (x, o) in row.iter_mut().enumerate() {
            let (x0, x1) = ((2 * x).min(w - 1), (2 * x + 1).min(w - 1));
            let (a, b, c, e) = (src[y0 * w + x0], src[y0 * w + x1], src[y1 * w + x0], src[y1 * w + x1]);
            *o = std::array::from_fn(|k| a[k] * 0.25 + b[k] * 0.25 + c[k] * 0.25 + e[k] * 0.25);
        }
    });
    (d, nw, nh)
}

#[inline(always)]
fn bilerp<const C: usize>(img: &[[f32; C]], w: usize, ix: usize, iy: usize, ix1: usize, iy1: usize, tx: f32, ty: f32) -> [f32; C] {
    let (a, b, c, d) = (img[iy * w + ix], img[iy * w + ix1], img[iy1 * w + ix], img[iy1 * w + ix1]);
    std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty)
}

#[inline]
fn fetch(img: &[[f32; 3]], w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    // Bilinear, clamped.
    let x = x.clamp(0.0, w as f32 - 1.001);
    let y = y.clamp(0.0, h as f32 - 1.001);
    let (ix, iy) = (x as usize, y as usize);
    bilerp(img, w, ix, iy, ix + 1, iy + 1, x - ix as f32, y - iy as f32)
}

// ---------------------------------------------------------------- stages

/// Glossy floor reflection: a pyramid of blurred levels; each floor pixel
/// interpolates between the two levels around its own blur σ. Returns the
/// reflection to add (fade and haze applied).
fn floor_reflection(pl: &Planes, w: usize, h: usize, s: f32) -> Option<Img> {
    if !par::any(&pl.cov, |&c| c > 0.0) {
        return None;
    }
    // The radius field comes from one ray per pixel and can alternate
    // between rows (hit / miss); a 9×9 box average keeps the blur coherent.
    let rad0: Buf<[f32; 1]> = par::map(w * h, |i| [if pl.cov[i] > 0.0 { pl.rad[i] * s } else { 0.0 }]);
    let rad = box_cols(&box_rows(&rad0, w, h, 4), w, h, 4);
    let gap = par::any_index(w * h, |i| pl.refl[i][3] < pl.cov[i] * 0.5);
    let maxr = (par::max(&rad, |v| v[0]) * 0.5 * 1.35).max(if gap { 2.6 } else { 0.0 });
    struct Lvl {
        sig: f32,
        w: usize,
        h: usize,
        f: f32,
        px: Buf<[f32; 4]>,
    }
    let mut levels: Vec<Lvl> = Vec::new();
    let noblur = std::env::var_os("ANIKIT_SCENE3D_NOBLUR").is_some();
    let (mut lw, mut lh, mut f, mut sig) = (w, h, 1.0f32, 0.0f32);
    while !noblur && levels.len() < 6 && sig < maxr {
        let src = levels.last().map(|l| &l.px[..]).unwrap_or(&pl.refl);
        let (d, nw, nh) = half4(src, lw, lh);
        let b = box_rows(&box_cols(&box_cols(&d, nw, nh, 1), nw, nh, 1), nw, nh, 1);
        f *= 2.0;
        sig = 1.1 * f;
        levels.push(Lvl { sig, w: nw, h: nh, f, px: b });
        (lw, lh) = (nw, nh);
    }
    let sample = |l: &Lvl, x: usize, y: usize| -> [f32; 4] {
        let fx = ((x as f32 + 0.5) / l.f - 0.5).clamp(0.0, l.w as f32 - 1.0);
        let fy = ((y as f32 + 0.5) / l.f - 0.5).clamp(0.0, l.h as f32 - 1.0);
        let (ix, iy) = (fx as usize, fy as usize);
        bilerp(&l.px, l.w, ix, iy, (ix + 1).min(l.w - 1), (iy + 1).min(l.h - 1), fx - ix as f32, fy - iy as f32)
    };
    Some(par::image(w, h, |y, row: &mut [[f32; 3]]| {
        for (x, o) in row.iter_mut().enumerate() {
            let i = y * w + x;
            let own = pl.cov[i];
            if own <= 0.0 {
                continue;
            }
            let c = pl.refl[i];
            let mut sg = rad[i][0] * 0.5 * 1.35;
            if c[3] < own * 0.5 {
                // A checkerboard gap: borrow from the neighbours.
                sg = sg.max(2.5);
            }
            // Level 0 is the unblurred input (σ 0); levels[k - 1] has σ 1.1·2^k.
            let mut k = 0;
            while k < levels.len() && levels[k].sig < sg {
                k += 1;
            }
            let a = if k == 0 { c } else { sample(&levels[k - 1], x, y) };
            let m = if k < levels.len() {
                let b = sample(&levels[k], x, y);
                let s0 = if k == 0 { 0.0 } else { levels[k - 1].sig };
                let t = ((sg - s0) / (levels[k].sig - s0)).clamp(0.0, 1.0);
                std::array::from_fn::<f32, 4, _>(|j| a[j] + (b[j] - a[j]) * t)
            } else {
                a
            };
            let r = pl.rf[i];
            // Colour per unit coverage, times this pixel's own coverage.
            *o = if m[3] > 1e-6 {
                let f = own / m[3];
                [m[0] * f * r, m[1] * f * r, m[2] * f * r]
            } else {
                [c[0] * r, c[1] * r, c[2] * r]
            };
        }
    }))
}

/// Edge-aware à-trous filter over (AO, key visibility): passes at steps 1,
/// 2, 4, … with a 3×3 tent kernel, stopped by depth, normal and id.
fn denoise_vis(pl: &Planes, w: usize, h: usize, passes: u32) -> (Buf<f32>, Buf<f32>) {
    let mut ao = pl.ao.clone();
    let mut vk = pl.vk.clone();
    // Checkerboard gaps need at least one pass to be filled.
    let gaps = par::any(&pl.vw, |&v| v < 1.0);
    let passes = if gaps { passes.max(1) } else { passes };
    if passes == 0 {
        return (ao, vk);
    }
    // Per-centre depth tolerance (reciprocal).
    let inv_tol: Buf<f32> = par::map(w * h, |i| 1.0 / (pl.depth[i] * 0.02 + 1e-3));
    // Rows with nothing lit pass through untouched.
    let row_lit: Vec<bool> = par::rows_any(&pl.lit, w, |&l| l > 0.0);
    // lit × validity, for the first pass when there are gaps.
    let gw: Buf<f32> = if gaps { par::map(w * h, |i| pl.lit[i] * pl.vw[i]) } else { Buf::uninit(0) };
    let k = [0.25f32, 0.5, 0.25];
    let mut ao2: Buf<f32> = Buf::uninit(w * h);
    let mut vk2: Buf<f32> = Buf::uninit(w * h);
    for pass in 0..passes {
        let step = 1usize << pass;
        let (sa, sv) = (&ao, &vk);
        // Untraced pixels start with no weight and take their neighbours'
        // values in the first pass.
        let first_gaps = pass == 0 && gaps;
        let vo = Out(vk2.as_mut_ptr());
        par::rows(&mut ao2, w, |y, row| {
            let c0 = y * w;
            // SAFETY: each row of `vk2` is written by the task that owns that row of `ao2`.
            let vrow = unsafe { std::slice::from_raw_parts_mut(vo.at(c0), w) };
            if !row_lit[y] {
                row.copy_from_slice(&sa[c0..c0 + w]);
                vrow.copy_from_slice(&sv[c0..c0 + w]);
                return;
            }
            let (cd, ct, cid, clit) = (&pl.depth[c0..c0 + w], &inv_tol[c0..c0 + w], &pl.id[c0..c0 + w], &pl.lit[c0..c0 + w]);
            let (cnx, cny, cnz) = (&pl.nx[c0..c0 + w], &pl.ny[c0..c0 + w], &pl.nz[c0..c0 + w]);
            let mut acc0 = [0.0f32; 4096];
            let mut acc1 = [0.0f32; 4096];
            let mut wsum = [0.0f32; 4096];
            assert!(w <= 4096, "scene3d: frames wider than 4096 px");
            for (jy, ky) in k.iter().enumerate() {
                let sy = y as isize + (jy as isize - 1) * step as isize;
                if sy < 0 || sy >= h as isize || !row_lit[sy as usize] {
                    continue;
                }
                let r0 = sy as usize * w;
                for (jx, kx) in k.iter().enumerate() {
                    let dx = (jx as isize - 1) * step as isize;
                    // Centre x range whose tap lands inside the row.
                    let xa = (-dx).max(0) as usize;
                    let xb = (w as isize - dx.max(0)).max(0) as usize;
                    if xa >= xb {
                        continue;
                    }
                    let kk = ky * kx;
                    let n = xb - xa;
                    let t0 = (r0 as isize + dx + xa as isize) as usize;
                    let tlit = if first_gaps { &gw[t0..t0 + n] } else { &pl.lit[t0..t0 + n] };
                    let t = (&pl.depth[t0..t0 + n], &pl.id[t0..t0 + n], tlit, &pl.nx[t0..t0 + n], &pl.ny[t0..t0 + n], &pl.nz[t0..t0 + n], &sa[t0..t0 + n], &sv[t0..t0 + n]);
                    let c = (&cd[xa..xb], &ct[xa..xb], &cid[xa..xb], &cnx[xa..xb], &cny[xa..xb], &cnz[xa..xb]);
                    denoise_taps(kk, t, c, (&mut acc0[xa..xb], &mut acc1[xa..xb], &mut wsum[xa..xb]));
                }
            }
            for x in 0..w {
                let i = c0 + x;
                let keep = !(clit[x] > 0.0 && wsum[x] > 1e-6);
                row[x] = if keep { sa[i] } else { acc0[x] / wsum[x] };
                vrow[x] = if keep { sv[i] } else { acc1[x] / wsum[x] };
            }
        });
        std::mem::swap(&mut ao, &mut ao2);
        std::mem::swap(&mut vk, &mut vk2);
    }
    (ao, vk)
}

type Taps<'a> = (&'a [f32], &'a [u32], &'a [f32], &'a [f32], &'a [f32], &'a [f32], &'a [f32], &'a [f32]);
type Centres<'a> = (&'a [f32], &'a [f32], &'a [u32], &'a [f32], &'a [f32], &'a [f32]);
type Accs<'a> = (&'a mut [f32], &'a mut [f32], &'a mut [f32]);

simd_fn! {
    /// One à-trous tap for a run of centre pixels: tap (depth, id, lit,
    /// normal, AO, key visibility), centre (depth, 1/tolerance, id, normal),
    /// accumulators (AO, visibility, weight). Branch-free, so it vectorises.
    fn denoise_taps(kk: f32, t: Taps, c: Centres, a: Accs) {
        let (td, tid, tlit, tnx, tny, tnz, ta, tv) = t;
        let (cd, ct, cid, cnx, cny, cnz) = c;
        let (a0, a1, ws) = a;
        let n = a0.len();
        let (td, tid, tlit, tnx, tny, tnz, ta, tv) = (&td[..n], &tid[..n], &tlit[..n], &tnx[..n], &tny[..n], &tnz[..n], &ta[..n], &tv[..n]);
        let (cd, ct, cid, cnx, cny, cnz) = (&cd[..n], &ct[..n], &cid[..n], &cnx[..n], &cny[..n], &cnz[..n]);
        let (a1, ws) = (&mut a1[..n], &mut ws[..n]);
        for x in 0..n {
            // Different objects only blend at the floor (id MAX).
            let same = (tid[x] == cid[x]) | (tid[x] == u32::MAX) | (cid[x] == u32::MAX);
            let dz = (td[x] - cd[x]).abs() * ct[x];
            let mut nd = (cnx[x] * tnx[x] + cny[x] * tny[x] + cnz[x] * tnz[x]).max(0.0);
            nd *= nd;
            nd *= nd;
            nd *= nd;
            let wt = kk * nd * nd / (1.0 + dz * dz) * tlit[x];
            let wt = if same { wt } else { 0.0 };
            a0[x] += ta[x] * wt;
            a1[x] += tv[x] * wt;
            ws[x] += wt;
        }
    }
}

/// Vogel (golden-angle) disc: `n` points with radius ≤ 1.
fn vogel(n: usize) -> Vec<(f32, f32)> {
    // (Points keep their original order and values.)
    (0..n)
        .map(|i| {
            let r = ((i as f32 + 0.5) / n as f32).sqrt();
            let a = i as f32 * 2.399_963;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// Circle-of-confusion radius (pixels). The backdrop (no geometry) blurs
/// like something about 8× the focus distance away: it is smooth anyway, and
/// a huge radius there starves the gather at object edges.
#[inline]
fn coc_of(p: &Post, s: f32, depth: f32) -> f32 {
    let d = depth.min(p.focus * 8.0).max(1e-3);
    (p.aperture * s * (1.0 - p.focus / d).abs()).min(28.0 * s)
}

/// Depth of field, gathered at half resolution (blur is low-frequency; the
/// 2×2 prefilter also turns sparse highlights into smooth discs) and blended
/// over the sharp image by how defocused each pixel is.
fn depth_of_field(hdr: &mut Img, depth: &[f32], w: usize, h: usize, p: &Post, s: f32) {
    let prof = std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some();
    let clock = std::time::Instant::now();
    let lap = |n: &str| {
        if prof {
            eprintln!("    dof {n}: {:.1}", clock.elapsed().as_secs_f64() * 1e3);
        }
    };
    let (hw, hh) = (w.div_ceil(2), h.div_ceil(2));
    let mut hd: Buf<f32> = Buf::uninit(hw * hh);
    let hc: Img = {
        let hdp = Out(hd.as_mut_ptr());
        let src = &*hdr;
        par::image(hw, hh, |y, row: &mut [[f32; 3]]| {
            let (y0, y1) = ((2 * y).min(h - 1), (2 * y + 1).min(h - 1));
            for (x, o) in row.iter_mut().enumerate() {
                let (x0, x1) = ((2 * x).min(w - 1), (2 * x + 1).min(w - 1));
                let mut acc = [0.0f32; 3];
                let mut d = f32::MAX;
                for (sx, sy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                    let c = src[sy * w + sx];
                    for k in 0..3 {
                        acc[k] += c[k] * 0.25;
                    }
                    d = d.min(depth[sy * w + sx]);
                }
                *o = acc;
                // SAFETY: one writer per half-res pixel.
                unsafe { hdp.put(y * hw + x, d) };
            }
        })
    };
    lap("prefilter");
    // CoC in half-res pixels.
    let coc: Buf<f32> = par::map(hw * hh, |i| coc_of(p, s, hd[i]) * 0.5);
    // Max CoC per 8×8 tile, dilated by two tiles, bounds the gather radius.
    let (tw, th) = (hw.div_ceil(8), hh.div_ceil(8));
    let tiles: Buf<[f32; 2]> = par::image(tw, th, |ty, row: &mut [[f32; 2]]| {
        for y in ty * 8..(ty * 8 + 8).min(hh) {
            for x in 0..hw {
                let t = &mut row[x / 8];
                t[0] = t[0].max(coc[y * hw + x]);
                if hd[y * hw + x] < 1e3 {
                    t[1] = 1.0;
                }
            }
        }
    });
    let mut dil = vec![0.0f32; tw * th];
    for ty in 0..th {
        for tx in 0..tw {
            let mut m = 0.0f32;
            let mut g = false;
            for b in ty.saturating_sub(2)..(ty + 3).min(th) {
                for a in tx.saturating_sub(2)..(tx + 3).min(tw) {
                    let t = tiles[b * tw + a];
                    m = m.max(t[0]);
                    g |= t[1] > 0.0;
                }
            }
            dil[ty * tw + tx] = if g { m } else { 0.0 };
        }
    }
    lap("tiles");
    // Vogel sets with each point's radius (rotation keeps it).
    let sets: Vec<Vec<(f32, f32, f32)>> = [24usize, 48, 96, 160].iter().map(|&n| vogel(n).into_iter().map(|(x, y)| (x, y, (x * x + y * y).sqrt())).collect()).collect();
    // Everything a gather tap reads, packed in one 32-byte record: colour,
    // depth, CoC, its blur alpha smoothstep(0.4, 1.2, coc) and 1 / max(coc², 1).
    // (For a tap behind the centre, ct = min(coc, cc), and both derived
    // values are monotonic in ct, so they are min/max of the precomputed ones.)
    let taps: Buf<[f32; 8]> = par::map(hw * hh, |i| {
        let c = hc[i];
        let cc = coc[i];
        [c[0], c[1], c[2], hd[i], cc, smoothstep(0.4, 1.2, cc), 1.0 / (cc * cc).max(1.0), 0.0]
    });
    // The same sets as lanes (dx, dy, radius), for the AVX2 gather.
    let soa: Vec<[Vec<f32>; 3]> = sets.iter().map(|v| [v.iter().map(|p| p.0).collect(), v.iter().map(|p| p.1).collect(), v.iter().map(|p| p.2).collect()]).collect();
    let avx = super::wide::avx2();
    // Half-res result: colour plus how much of it is blur (alpha).
    let out4: Buf<[f32; 4]> = par::image(hw, hh, |y, row: &mut [[f32; 4]]| {
        for x in 0..hw {
            let i0 = y * hw + x;
            let rmax = dil[(y / 8) * tw + x / 8];
            let c0 = taps[i0];
            if rmax < 0.5 {
                row[x] = [c0[0], c0[1], c0[2], 0.0];
                continue;
            }
            let (dc, cc, sc, ic) = (c0[3], c0[4], c0[5], c0[6]);
            let set = &sets[if rmax < 2.5 {
                0
            } else if rmax < 5.0 {
                1
            } else if rmax < 9.0 {
                2
            } else {
                3
            }];
            let ang = u01(hash2(x as u32, y as u32 ^ 0xdead)) * std::f32::consts::TAU;
            let (ca, sa) = (ang.cos(), ang.sin());
            #[cfg(target_arch = "x86_64")]
            if avx {
                let k = if rmax < 2.5 {
                    0
                } else if rmax < 5.0 {
                    1
                } else if rmax < 9.0 {
                    2
                } else {
                    3
                };
                // SAFETY: the CPU has AVX2; taps holds hw × hh records.
                row[x] = unsafe { dof_gather_avx2(&taps, hw, hh, x, y, &soa[k], rmax, ca, sa, c0) };
                continue;
            }
            let wt0 = ic;
            let mut acc = [c0[0] * wt0, c0[1] * wt0, c0[2] * wt0, sc * wt0];
            let mut wsum = wt0;
            let (xm, ym) = (hw as f32 - 1.0, hh as f32 - 1.0);
            for &(dx, dy, r) in set {
                let (fx, fy) = ((dx * ca - dy * sa) * rmax, (dx * sa + dy * ca) * rmax);
                // Round to nearest, clamped (non-negative after the clamp, so +0.5 and truncate).
                let sx = (x as f32 + fx + 0.5).clamp(0.0, xm + 0.5) as usize;
                let sy = (y as f32 + fy + 0.5).clamp(0.0, ym + 0.5) as usize;
                // SAFETY: sx < hw and sy < hh after the clamps.
                let t = unsafe { taps.get_unchecked(sy * hw + sx) };
                let dist = r * rmax;
                // A sample behind the centre can't blur over it more than the
                // centre is blurred. Branch-free: a tap that doesn't cover the
                // centre adds exactly zero.
                let behind = t[3] > dc;
                let ct = if behind { t[4].min(cc) } else { t[4] };
                let cover = ((ct - dist) * 0.5 + 0.5).clamp(0.0, 1.0);
                let st = if behind { t[5].min(sc) } else { t[5] };
                let it = if behind { t[6].max(ic) } else { t[6] };
                let wt = cover * it;
                acc[0] += t[0] * wt;
                acc[1] += t[1] * wt;
                acc[2] += t[2] * wt;
                acc[3] += st * wt;
                wsum += wt;
            }
            row[x] = acc.map(|v| v / wsum);
        }
    });
    lap("gather");
    // A half-res tent smooths the gather's grain before upsampling; in-focus
    // half-res pixels stay exact.
    let o4: Buf<[f32; 4]> = par::image(hw, hh, |y, row: &mut [[f32; 4]]| {
        for x in 0..hw {
            let own = out4[y * hw + x];
            let a = smoothstep(0.0, 0.5, own[3]);
            if a <= 0.0 {
                row[x] = own;
                continue;
            }
            let mut acc = [0.0f32; 4];
            for (dy, wy) in [(-1i32, 0.25f32), (0, 0.5), (1, 0.25)] {
                for (dx, wx) in [(-1i32, 0.25f32), (0, 0.5), (1, 0.25)] {
                    let sx = (x as i32 + dx).clamp(0, hw as i32 - 1) as usize;
                    let sy = (y as i32 + dy).clamp(0, hh as i32 - 1) as usize;
                    let t = out4[sy * hw + sx];
                    for k in 0..4 {
                        acc[k] += t[k] * wx * wy;
                    }
                }
            }
            row[x] = std::array::from_fn(|k| own[k] * (1.0 - a) + acc[k] * a);
        }
    });
    lap("tent");
    // Upsample and blend.
    par::rows(hdr, w, |y, row| {
        let fy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, hh as f32 - 1.0);
        let (iy, ty) = (fy as usize, fy - (fy as usize) as f32);
        let iy1 = (iy + 1).min(hh - 1);
        for x in 0..w {
            let fx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, hw as f32 - 1.0);
            let (ix, tx) = (fx as usize, fx - (fx as usize) as f32);
            let up = bilerp(&o4, hw, ix, iy, (ix + 1).min(hw - 1), iy1, tx, ty);
            let own = smoothstep(0.8, 2.2, coc_of(p, s, depth[y * w + x]));
            let al = up[3].max(own).clamp(0.0, 1.0);
            if al > 0.0 {
                for k in 0..3 {
                    row[x][k] = row[x][k] * (1.0 - al) + up[k] * al;
                }
            }
        }
    });
}

/// The depth-of-field gather for one half-res pixel, eight taps at a time
/// with AVX2 gathers: the scalar loop's arithmetic per tap (accumulated in
/// eight partial sums, so the last bits can differ from the scalar order).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::too_many_arguments)]
unsafe fn dof_gather_avx2(taps: &[[f32; 8]], hw: usize, hh: usize, x: usize, y: usize, set: &[Vec<f32>; 3], rmax: f32, ca: f32, sa: f32, c0: [f32; 8]) -> [f32; 4] {
    use std::arch::x86_64::*;
    let (dc, cc, sc, ic) = (c0[3], c0[4], c0[5], c0[6]);
    let n = set[0].len();
    debug_assert!(n.is_multiple_of(8) && taps.len() >= hw * hh);
    let base = taps.as_ptr() as *const f32;
    // SAFETY: (caller) AVX2 is available; every gathered index is clamped into the image.
    unsafe {
        let (vca, vsa, vr) = (_mm256_set1_ps(ca), _mm256_set1_ps(sa), _mm256_set1_ps(rmax));
        let (vx, vy) = (_mm256_set1_ps(x as f32 + 0.5), _mm256_set1_ps(y as f32 + 0.5));
        let zero = _mm256_setzero_ps();
        let (xm, ym) = (_mm256_set1_ps(hw as f32 - 0.5), _mm256_set1_ps(hh as f32 - 0.5));
        let (vdc, vcc, vsc, vic) = (_mm256_set1_ps(dc), _mm256_set1_ps(cc), _mm256_set1_ps(sc), _mm256_set1_ps(ic));
        let (half, one) = (_mm256_set1_ps(0.5), _mm256_set1_ps(1.0));
        let vhw = _mm256_set1_epi32(hw as i32);
        let mut a = [zero; 5];
        let mut k = 0;
        while k < n {
            let dx = _mm256_loadu_ps(set[0].as_ptr().add(k));
            let dy = _mm256_loadu_ps(set[1].as_ptr().add(k));
            let r = _mm256_loadu_ps(set[2].as_ptr().add(k));
            let fx = _mm256_mul_ps(_mm256_sub_ps(_mm256_mul_ps(dx, vca), _mm256_mul_ps(dy, vsa)), vr);
            let fy = _mm256_mul_ps(_mm256_add_ps(_mm256_mul_ps(dx, vsa), _mm256_mul_ps(dy, vca)), vr);
            let sx = _mm256_cvttps_epi32(_mm256_min_ps(_mm256_max_ps(_mm256_add_ps(vx, fx), zero), xm));
            let sy = _mm256_cvttps_epi32(_mm256_min_ps(_mm256_max_ps(_mm256_add_ps(vy, fy), zero), ym));
            // Record index × 8 floats.
            let idx = _mm256_slli_epi32::<3>(_mm256_add_epi32(_mm256_mullo_epi32(sy, vhw), sx));
            let g = |f: i32| _mm256_i32gather_ps::<4>(base.add(f as usize), idx);
            let (t0, t1, t2, t3, t4, t5, t6) = (g(0), g(1), g(2), g(3), g(4), g(5), g(6));
            let dist = _mm256_mul_ps(r, vr);
            let behind = _mm256_cmp_ps::<_CMP_GT_OQ>(t3, vdc);
            let ct = _mm256_blendv_ps(t4, _mm256_min_ps(t4, vcc), behind);
            let cover = _mm256_min_ps(_mm256_max_ps(_mm256_add_ps(_mm256_mul_ps(_mm256_sub_ps(ct, dist), half), half), zero), one);
            let st = _mm256_blendv_ps(t5, _mm256_min_ps(t5, vsc), behind);
            let it = _mm256_blendv_ps(t6, _mm256_max_ps(t6, vic), behind);
            let wt = _mm256_mul_ps(cover, it);
            a[0] = _mm256_add_ps(a[0], _mm256_mul_ps(t0, wt));
            a[1] = _mm256_add_ps(a[1], _mm256_mul_ps(t1, wt));
            a[2] = _mm256_add_ps(a[2], _mm256_mul_ps(t2, wt));
            a[3] = _mm256_add_ps(a[3], _mm256_mul_ps(st, wt));
            a[4] = _mm256_add_ps(a[4], wt);
            k += 8;
        }
        let hsum = |v: __m256| {
            let mut l = [0.0f32; 8];
            _mm256_storeu_ps(l.as_mut_ptr(), v);
            ((l[0] + l[1]) + (l[2] + l[3])) + ((l[4] + l[5]) + (l[6] + l[7]))
        };
        let wsum = ic + hsum(a[4]);
        [(c0[0] * ic + hsum(a[0])) / wsum, (c0[1] * ic + hsum(a[1])) / wsum, (c0[2] * ic + hsum(a[2])) / wsum, (sc * ic + hsum(a[3])) / wsum]
    }
}

/// Motion blur along each pixel's motion vector.
fn motion_blur(hdr: &mut Img, vel: &[[f32; 2]], w: usize, h: usize, shutter: f32) {
    let moving = |v: &[f32; 2]| {
        let (vx, vy) = (v[0] * shutter, v[1] * shutter);
        vx * vx + vy * vy >= 0.35 * 0.35
    };
    if !par::any(vel, moving) {
        return;
    }
    let src = hdr.clone();
    let avx = super::wide::avx2();
    par::rows(hdr, w, |y, row| {
        for x in 0..w {
            let v = vel[y * w + x];
            let (vx, vy) = (v[0] * shutter, v[1] * shutter);
            let l = (vx * vx + vy * vy).sqrt();
            if l < 0.35 {
                continue;
            }
            let n = super::super::math::fceil(l * 1.2).clamp(3.0, 16.0) as usize;
            #[cfg(target_arch = "x86_64")]
            if avx {
                // SAFETY: the CPU has AVX2; `src` is w × h.
                row[x] = unsafe { mblur_avx2(&src, w, h, x, y, vx, vy, n) };
                continue;
            }
            let mut acc = [0.0f32; 3];
            for i in 0..n {
                let t = (i as f32 + 0.5) / n as f32 - 0.5;
                let c = fetch(&src, w, h, x as f32 - vx * t, y as f32 - vy * t);
                for k in 0..3 {
                    acc[k] += c[k];
                }
            }
            row[x] = acc.map(|a| a / n as f32);
        }
    });
}

/// [`motion_blur`]'s taps for one pixel, eight at a time with AVX2 gathers
/// (the same clamped bilinear fetch per tap; summed in lanes).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::too_many_arguments)]
unsafe fn mblur_avx2(src: &[[f32; 3]], w: usize, h: usize, x: usize, y: usize, vx: f32, vy: f32, n: usize) -> [f32; 3] {
    use std::arch::x86_64::*;
    debug_assert!(src.len() >= w * h && n <= 16);
    let base = src.as_ptr() as *const f32;
    // SAFETY: (caller) AVX2; every gathered index is clamped into the image.
    unsafe {
        let lane = _mm256_setr_ps(0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0);
        let (fx0, fy0) = (_mm256_set1_ps(x as f32), _mm256_set1_ps(y as f32));
        let (vvx, vvy) = (_mm256_set1_ps(vx), _mm256_set1_ps(vy));
        let (xm, ym) = (_mm256_set1_ps(w as f32 - 1.001), _mm256_set1_ps(h as f32 - 1.001));
        let zero = _mm256_setzero_ps();
        let one = _mm256_set1_ps(1.0);
        let (half, nf) = (_mm256_set1_ps(0.5), _mm256_set1_ps(n as f32));
        let w3 = _mm256_set1_epi32(w as i32 * 3);
        let three = _mm256_set1_epi32(3);
        let mut acc = [zero; 3];
        let mut k = 0usize;
        while k < n {
            let i = _mm256_add_ps(lane, _mm256_set1_ps(k as f32));
            let valid = _mm256_cmp_ps::<_CMP_LT_OQ>(i, nf);
            let t = _mm256_sub_ps(_mm256_div_ps(_mm256_add_ps(i, half), nf), half);
            let px = _mm256_min_ps(_mm256_max_ps(_mm256_sub_ps(fx0, _mm256_mul_ps(vvx, t)), zero), xm);
            let py = _mm256_min_ps(_mm256_max_ps(_mm256_sub_ps(fy0, _mm256_mul_ps(vvy, t)), zero), ym);
            let ix = _mm256_cvttps_epi32(px);
            let iy = _mm256_cvttps_epi32(py);
            let tx = _mm256_sub_ps(px, _mm256_cvtepi32_ps(ix));
            let ty = _mm256_sub_ps(py, _mm256_cvtepi32_ps(iy));
            let (ux, uy) = (_mm256_sub_ps(one, tx), _mm256_sub_ps(one, ty));
            // Float offsets of the four corners (× 3 channels).
            let i00 = _mm256_add_epi32(_mm256_mullo_epi32(iy, w3), _mm256_mullo_epi32(ix, three));
            let i01 = _mm256_add_epi32(i00, three);
            let i10 = _mm256_add_epi32(i00, w3);
            let i11 = _mm256_add_epi32(i10, three);
            for c in 0..3 {
                let b = base.add(c);
                let (a, bb, cc, d) = (_mm256_i32gather_ps::<4>(b, i00), _mm256_i32gather_ps::<4>(b, i01), _mm256_i32gather_ps::<4>(b, i10), _mm256_i32gather_ps::<4>(b, i11));
                let top = _mm256_add_ps(_mm256_mul_ps(a, ux), _mm256_mul_ps(bb, tx));
                let bot = _mm256_add_ps(_mm256_mul_ps(cc, ux), _mm256_mul_ps(d, tx));
                let v = _mm256_add_ps(_mm256_mul_ps(top, uy), _mm256_mul_ps(bot, ty));
                acc[c] = _mm256_add_ps(acc[c], _mm256_and_ps(v, valid));
            }
            k += 8;
        }
        let hsum = |v: __m256| {
            let mut l = [0.0f32; 8];
            _mm256_storeu_ps(l.as_mut_ptr(), v);
            ((l[0] + l[1]) + (l[2] + l[3])) + ((l[4] + l[5]) + (l[6] + l[7]))
        };
        let inv = n as f32;
        [hsum(acc[0]) / inv, hsum(acc[1]) / inv, hsum(acc[2]) / inv]
    }
}

/// Bloom: a pyramid of blurred levels, mostly from highlights, summed back
/// up level by level (each level adds its weight and is upsampled 2× into
/// the next finer one), then upsampled once onto the image.
fn bloom(hdr: &mut Img, w: usize, h: usize, strength: f32) {
    // Soft threshold: only what's above ~0.8 contributes strongly.
    let thr: Img = {
        let src = &*hdr;
        par::image(w, h, |y, row: &mut [[f32; 3]]| {
            for (x, o) in row.iter_mut().enumerate() {
                let c = src[y * w + x];
                let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
                let k = ((l - 0.6) / (l + 1e-4)).clamp(0.0, 1.0) * 0.85 + 0.15;
                *o = c.map(|v| v * k);
            }
        })
    };
    let mut levels: Vec<(Img, usize, usize)> = Vec::new();
    let (mut cw, mut ch) = (w, h);
    for _ in 0..7 {
        if cw < 8 || ch < 8 {
            break;
        }
        let (nw, nh) = (cw / 2, ch / 2);
        let src: &[[f32; 3]] = levels.last().map(|l| &l.0[..]).unwrap_or(&thr[..]);
        let next: Img = par::image(nw, nh, |y, row: &mut [[f32; 3]]| {
            let ys = [-1i32, 0, 1, 2].map(|dy| (2 * y as i32 + dy).clamp(0, ch as i32 - 1) as usize * cw);
            for x in 0..nw {
                // 13-tap-ish: 4×4 tent over the 2×2 block and its ring.
                let xs = [-1i32, 0, 1, 2].map(|dx| (2 * x as i32 + dx).clamp(0, cw as i32 - 1) as usize);
                let mut acc = [0.0f32; 3];
                for (j, wy) in [1.0f32, 3.0, 3.0, 1.0].iter().enumerate() {
                    for (i, wx) in [1.0f32, 3.0, 3.0, 1.0].iter().enumerate() {
                        let c = src[ys[j] + xs[i]];
                        let wt = wx * wy;
                        for k in 0..3 {
                            acc[k] += c[k] * wt;
                        }
                    }
                }
                row[x] = acc.map(|a| a / 64.0);
            }
        });
        levels.push((next, nw, nh));
        cw = nw;
        ch = nh;
    }
    drop(thr);
    if levels.is_empty() {
        return;
    }
    // Wider levels weighted more gently.
    let weights = [0.2, 0.22, 0.2, 0.16, 0.12, 0.06, 0.04];
    let total: f32 = weights.iter().take(levels.len()).sum();
    // Sum coarse to fine: acc_l = w_l · L_l + up2(acc_{l+1}).
    let mut acc: Option<(Img, usize, usize)> = None;
    for li in (0..levels.len()).rev() {
        let (img, lw, lh) = (&levels[li].0, levels[li].1, levels[li].2);
        let wl = weights[li];
        let up = acc.as_ref();
        let next: Img = par::image(lw, lh, |y, row: &mut [[f32; 3]]| {
            for x in 0..lw {
                let c = img[y * lw + x];
                let mut v = [c[0] * wl, c[1] * wl, c[2] * wl];
                if let Some((u, uw, uh)) = up {
                    let f = fetch(u, *uw, *uh, (x as f32 + 0.5) / 2.0 - 0.5, (y as f32 + 0.5) / 2.0 - 0.5);
                    for k in 0..3 {
                        v[k] += f[k];
                    }
                }
                row[x] = v;
            }
        });
        acc = Some((next, lw, lh));
    }
    let (b0, bw, bh) = acc.unwrap();
    let k = strength * 1.6 / total;
    par::rows(hdr, w, |y, row| {
        let fy = (y as f32 + 0.5) / 2.0 - 0.5;
        for x in 0..w {
            let b = fetch(&b0, bw, bh, (x as f32 + 0.5) / 2.0 - 0.5, fy);
            for kk in 0..3 {
                row[x][kk] = row[x][kk] * (1.0 - strength) + b[kk] * k;
            }
        }
    });
}

/// sRGB encode table over [0, 1] (linear interpolation; error < 2e-5).
fn srgb_lut() -> &'static [f32; SRGB_N + 2] {
    static L: std::sync::OnceLock<[f32; SRGB_N + 2]> = std::sync::OnceLock::new();
    L.get_or_init(|| std::array::from_fn(|i| srgb((i as f32 / SRGB_N as f32).min(1.0))))
}
const SRGB_N: usize = 4096;

/// Khronos PBR Neutral, branch-free (both branches computed, one selected).
#[inline(always)]
fn neutral_bf(c: [f32; 3]) -> [f32; 3] {
    const START: f32 = 0.8 - 0.04;
    const DESAT: f32 = 0.15;
    let x = c[0].min(c[1]).min(c[2]);
    let off = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    let c = c.map(|v| v - off);
    let peak = c[0].max(c[1]).max(c[2]);
    let d = 1.0 - START;
    let np = 1.0 - d * d / (peak + d - START);
    let g = 1.0 - 1.0 / (DESAT * (peak - np) + 1.0);
    let low = peak < START;
    c.map(|v| {
        let hi = v * np / peak;
        let r = if low { v } else { hi * (1.0 - g) + np * g };
        r.clamp(0.0, 1.0)
    })
}

simd_fn! {
    /// One output row: exposure × vignette, tone map, sRGB (table), dither,
    /// quantise. Written as lane loops over the row so each step vectorises.
    #[allow(clippy::too_many_arguments)]
    fn finish_row(src: &[[f32; 3]], row: &mut [u8], y: usize, cx: f32, cy: f32, inv_d: f32, exposure: f32, vignette: f32, neutral_tm: bool, lut: &[f32; SRGB_N + 2]) {
        let w = src.len();
        let mut m = [[0.0f32; 3]; 2048];
        let dy = (y as f32 - cy) * inv_d;
        for (x0, chunk) in src.chunks(2048).enumerate() {
            let x0 = x0 * 2048;
            let n = chunk.len();
            let m = &mut m[..n];
            for i in 0..n {
                let dx = ((x0 + i) as f32 - cx) * inv_d;
                let r2 = dx * dx + dy * dy;
                let e = exposure * (1.0 - vignette * r2 * r2.sqrt().sqrt() * 1.6);
                let c = chunk[i];
                let e = [c[0] * e, c[1] * e, c[2] * e];
                m[i] = if neutral_tm { neutral_bf(e) } else { aces(e) };
            }
            let out = &mut row[x0 * 4..(x0 + n) * 4];
            for i in 0..n {
                let x = x0 + i;
                let hs = hash2(x as u32 ^ 0x51ed, y as u32 ^ 0x27d4);
                let dith = (u01(hs) + u01(hash2(hs, 77)) - 1.0) / 255.0;
                for k in 0..3 {
                    let f = m[i][k] * SRGB_N as f32;
                    let j = (f as usize).min(SRGB_N);
                    let s = lut[j] + (lut[j + 1] - lut[j]) * (f - j as f32);
                    out[i * 4 + k] = ((s + dith) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                }
                out[i * 4 + 3] = 255;
            }
        }
        let _ = w;
    }
}

/// Tone map, vignette, sRGB, dither.
fn finish(hdr: &Img, w: usize, h: usize, p: &Post) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let inv_d = 1.0 / (cx * cx + cy * cy).sqrt();
    let lut = srgb_lut();
    let neutral_tm = p.tonemap == Tonemap::Neutral;
    par::rows(&mut out, w * 4, |y, row| finish_row(&hdr[y * w..y * w + w], row, y, cx, cy, inv_d, p.exposure, p.vignette, neutral_tm, lut));
    out
}

/// `ANIKIT_SCENE3D_DEBUG=refl|col|diffuse` shows one buffer (development aid).
fn debug() -> u8 {
    static D: std::sync::OnceLock<u8> = std::sync::OnceLock::new();
    *D.get_or_init(|| match std::env::var("ANIKIT_SCENE3D_DEBUG").as_deref() {
        Ok("refl") => 1,
        Ok("col") => 2,
        Ok("diffuse") => 3,
        _ => 0,
    })
}

/// Write a primary pass (planes) and its post settings to `path` (see [`load`]).
pub fn dump(path: &std::path::Path, pl: &Planes, w: usize, h: usize, p: &Post) {
    let mut b: Vec<u8> = Vec::new();
    for v in [w as f32, h as f32, p.focus, p.aperture, p.shutter, p.bloom, p.exposure, p.vignette, (p.tonemap == Tonemap::Neutral) as u32 as f32, p.denoise as f32] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fn raw<T: Pod>(b: &mut Vec<u8>, v: &[T]) {
        // SAFETY: plain data.
        b.extend_from_slice(unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) });
    }
    raw(&mut b, &pl.col);
    raw(&mut b, &pl.refl);
    raw(&mut b, &pl.cov);
    raw(&mut b, &pl.rad);
    raw(&mut b, &pl.rf);
    raw(&mut b, &pl.da);
    raw(&mut b, &pl.dk);
    raw(&mut b, &pl.sp);
    raw(&mut b, &pl.ao);
    raw(&mut b, &pl.vk);
    raw(&mut b, &pl.depth);
    raw(&mut b, &pl.nx);
    raw(&mut b, &pl.ny);
    raw(&mut b, &pl.nz);
    raw(&mut b, &pl.id);
    raw(&mut b, &pl.lit);
    raw(&mut b, &pl.vw);
    raw(&mut b, &pl.vel);
    let _ = std::fs::write(path, b);
}

/// Read what [`dump`] wrote.
pub fn load(path: &std::path::Path) -> Option<(Planes, usize, usize, Post)> {
    let b = std::fs::read(path).ok()?;
    let f = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
    let (w, h) = (f(0) as usize, f(1) as usize);
    let p = Post { focus: f(2), aperture: f(3), shutter: f(4), bloom: f(5), exposure: f(6), vignette: f(7), tonemap: if f(8) > 0.5 { Tonemap::Neutral } else { Tonemap::Aces }, denoise: f(9) as u32 };
    let mut pl = Planes::new(w * h);
    let mut at = 40usize;
    fn fill<T: Pod>(b: &[u8], at: &mut usize, v: &mut [T]) -> Option<()> {
        let n = std::mem::size_of_val(v);
        let src = b.get(*at..*at + n)?;
        // SAFETY: sizes match and `T` accepts any bits.
        unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), v.as_mut_ptr() as *mut u8, n) };
        *at += n;
        Some(())
    }
    fill(&b, &mut at, &mut pl.col)?;
    fill(&b, &mut at, &mut pl.refl)?;
    fill(&b, &mut at, &mut pl.cov)?;
    fill(&b, &mut at, &mut pl.rad)?;
    fill(&b, &mut at, &mut pl.rf)?;
    fill(&b, &mut at, &mut pl.da)?;
    fill(&b, &mut at, &mut pl.dk)?;
    fill(&b, &mut at, &mut pl.sp)?;
    fill(&b, &mut at, &mut pl.ao)?;
    fill(&b, &mut at, &mut pl.vk)?;
    fill(&b, &mut at, &mut pl.depth)?;
    fill(&b, &mut at, &mut pl.nx)?;
    fill(&b, &mut at, &mut pl.ny)?;
    fill(&b, &mut at, &mut pl.nz)?;
    fill(&b, &mut at, &mut pl.id)?;
    fill(&b, &mut at, &mut pl.lit)?;
    fill(&b, &mut at, &mut pl.vw)?;
    fill(&b, &mut at, &mut pl.vel)?;
    Some((pl, w, h, p))
}

/// The post stack on per-pixel records (see [`process_planes`]).
pub fn process(px: &[Px], w: usize, h: usize, p: &Post, vol: Option<Volume>, parts: Option<&super::super::particles::Layer>) -> Vec<u8> {
    process_planes(&Planes::unpack(px), w, h, p, vol, parts)
}

/// The post stack: floor-reflection blur, denoise, compose, depth of field,
/// motion blur, particles, bloom, tone mapping.
pub fn process_planes(pl: &Planes, w: usize, h: usize, p: &Post, vol: Option<Volume>, parts: Option<&super::super::particles::Layer>) -> Vec<u8> {
    let s = h as f32 / 1080.0;
    let mut clock = std::time::Instant::now();
    let mut stages: Vec<(&str, f64)> = Vec::new();
    let mut tick = |name: &'static str| {
        stages.push((name, clock.elapsed().as_secs_f64() * 1e3));
        clock = std::time::Instant::now();
    };
    tick("unpack");
    // 1. Glossy floor reflection.
    let rv = floor_reflection(pl, w, h, s);
    tick("refl");
    // 1b. Denoise AO and key visibility, then light the diffuse albedo with them.
    let (ao, vk) = denoise_vis(pl, w, h, p.denoise);
    tick("denoise");
    let (dc, dr, dd) = match debug() {
        1 => (0.0, 1.0, 0.0),
        2 => (1.0, 0.0, 0.0),
        3 => (0.0, 0.0, 1.0),
        _ => (1.0, 1.0, 1.0),
    };
    let mut hdr: Img = par::image(w, h, |y, row: &mut [[f32; 3]]| {
        for x in 0..w {
            let i = y * w + x;
            let (c, a, kk, sp) = (pl.col[i], pl.da[i], pl.dk[i], pl.sp[i]);
            let r = rv.as_ref().map(|r| r[i]).unwrap_or([0.0; 3]);
            let (v0, v1) = (ao[i], vk[i]);
            row[x] = std::array::from_fn(|k| c[k] * dc + r[k] * dr + (a[k] * v0 + kk[k] * v1 + sp[k] * v0) * dd);
        }
    });
    drop(rv);
    if let Some(v) = &vol {
        par::rows(&mut hdr, w, |y, row| {
            let fy = (y as f32 + 0.5) * 0.5 - 0.5;
            for x in 0..w {
                let c = fetch(v.px, v.w, v.h, (x as f32 + 0.5) * 0.5 - 0.5, fy);
                for k in 0..3 {
                    row[x][k] += c[k];
                }
            }
        });
    }
    tick("compose");
    // 2. Depth of field.
    if p.aperture > 0.05 {
        depth_of_field(&mut hdr, &pl.depth, w, h, p, s);
    }
    tick("dof");
    // 3. Motion blur along each pixel's motion vector.
    if p.shutter > 0.0 {
        motion_blur(&mut hdr, &pl.vel, w, h, p.shutter);
    }
    tick("mblur");
    // 3b. Particles (already blurred and streaked by their splatter) go in
    // before bloom, so bright ones glow.
    if let Some(l) = parts {
        par::rows(&mut hdr, w, |y, row| {
            for (x, c) in row.iter_mut().enumerate() {
                *c = l.apply(y * w + x, *c);
            }
        });
    }
    // 4. Bloom.
    if p.bloom > 0.0 {
        bloom(&mut hdr, w, h, p.bloom);
    }
    tick("bloom");
    let out = finish(&hdr, w, h, p);
    tick("tonemap");
    if std::env::var_os("ANIKIT_SCENE3D_PROFILE").is_some() {
        eprintln!("  post: {}", stages.iter().map(|(n, ms)| format!("{n} {ms:.1}")).collect::<Vec<_>>().join(", "));
    }
    out
}

/// ACES filmic (Stephen Hill's RRT + ODT fit), sRGB primaries in and out.
#[inline]
fn aces(c: [f32; 3]) -> [f32; 3] {
    let i = [[0.59719, 0.35458, 0.04823], [0.07600, 0.90834, 0.01566], [0.02840, 0.13383, 0.83777]];
    let o = [[1.60475, -0.53108, -0.07367], [-0.10208, 1.10813, -0.00605], [-0.00327, -0.07276, 1.07602]];
    let a: [f32; 3] = std::array::from_fn(|r| i[r][0] * c[0] + i[r][1] * c[1] + i[r][2] * c[2]);
    let b = a.map(|v| {
        let n = v * (v + 0.0245786) - 0.000090537;
        let d = v * (0.983729 * v + 0.432951) + 0.238081;
        n / d
    });
    std::array::from_fn(|r| (o[r][0] * b[0] + o[r][1] * b[1] + o[r][2] * b[2]).clamp(0.0, 1.0))
}

fn srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}
