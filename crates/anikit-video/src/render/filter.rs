//! Layer filters on premultiplied RGBA8 pixels: blur (3× box ≈ Gaussian),
//! drop shadow, glow and colour matrix. All loops are branch-free over rows
//! so they vectorise; scratch memory is reused across frames.

use super::blur::{blur, BlurScratch};
use crate::dl::{Filter, Halftone, Rgba, ToneLut};

/// Reusable scratch buffers.
#[derive(Default, Debug)]
pub struct FilterScratch {
    a: Vec<u8>,
    blur: BlurScratch,
    /// Ink texture thin-map for the last region: (seed, scale bits, ox, oy, w, h) → coverage multipliers' inputs.
    ink_key: Option<(u32, u32, i32, i32, usize, usize)>,
    ink_map: Vec<u8>,
}

impl FilterScratch {
    /// Reference quality: blurs without decimation.
    pub fn set_exact(&mut self, on: bool) {
        self.blur.exact = on;
    }
}

/// Finite and within ±`max` (NaN → 0).
pub fn sane(v: f32, max: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(-max, max)
    }
}

/// The filter with every parameter finite and in range.
pub fn sanitize(f: &Filter) -> Filter {
    let s = |v: f32| sane(v, 1000.0).max(0.0);
    let c = |c: Rgba| Rgba(c.0.map(|v| sane(v, 1.0).max(0.0)));
    match *f {
        Filter::Blur(v) => Filter::Blur(s(v)),
        Filter::Shadow { dx, dy, blur, color } => Filter::Shadow { dx: sane(dx, 4096.0), dy: sane(dy, 4096.0), blur: s(blur), color: c(color) },
        Filter::Glow { radius, intensity, color } => Filter::Glow { radius: s(radius), intensity: sane(intensity, 64.0).max(0.0), color: c(color) },
        Filter::ColorMatrix(m) => Filter::ColorMatrix(m.map(|v| sane(v, 1e6))),
        Filter::Tone(ref t) => Filter::Tone(t.clone()),
        Filter::Ink { amount, scale, seed, ox, oy } => Filter::Ink { amount: sane(amount, 1.0).max(0.0), scale: sane(scale, 512.0).max(1.0), seed, ox: sane(ox, 1e7), oy: sane(oy, 1e7) },
        Filter::Halftone(h) => Filter::Halftone(Halftone {
            cell: sane(h.cell, 512.0).max(2.0),
            angle: sane(h.angle, 3600.0),
            gain: sane(h.gain, 8.0).max(0.05),
            ink: c(h.ink),
            paper: c(h.paper),
            ox: sane(h.ox, 1e7),
            oy: sane(h.oy, 1e7),
            ..h
        }),
    }
}

/// Extent a filter adds around its input, in pixels: (left, top, right, bottom).
pub fn margin(f: &Filter) -> [f32; 4] {
    match sanitize(f) {
        Filter::Blur(s) => [blur_extent(s); 4],
        Filter::Shadow { dx, dy, blur, .. } => {
            let e = blur_extent(blur);
            [e + (-dx).max(0.0), e + (-dy).max(0.0), e + dx.max(0.0), e + dy.max(0.0)]
        }
        Filter::Glow { radius, .. } => [blur_extent(radius); 4],
        Filter::ColorMatrix(_) | Filter::Tone(_) | Filter::Halftone(_) | Filter::Ink { .. } => [0.0; 4],
    }
}

fn blur_extent(sigma: f32) -> f32 {
    if sigma > 0.0 {
        (sigma * 3.0).ceil() + 1.0
    } else {
        0.0
    }
}

fn premul(c: Rgba) -> [u32; 4] {
    let a = c.0[3].clamp(0.0, 1.0);
    let q = |v: f32| (v.clamp(0.0, 1.0) * a * 255.0 + 0.5) as u32;
    [q(c.0[0]), q(c.0[1]), q(c.0[2]), (a * 255.0 + 0.5) as u32]
}

/// Draw `color × shifted, blurred alpha × gain` *behind* the pixels in `buf`.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn behind(buf: &mut [u8], s: &mut FilterScratch, w: usize, h: usize, dx: i32, dy: i32, sigma: f32, color: Rgba, gain: f32) {
    let n = w * h;
    s.a.clear();
    s.a.extend(buf.chunks_exact(4).map(|p| p[3]));
    blur::<1>(&mut s.a, w, h, sigma, &mut s.blur);
    let col = premul(color);
    let g = (gain.max(0.0) * 256.0) as u32;
    debug_assert_eq!(buf.len(), n * 4);
    for y in 0..h as i32 {
        let sy = y - dy;
        let row = &mut buf[y as usize * w * 4..(y as usize + 1) * w * 4];
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        let arow = &s.a[sy as usize * w..(sy as usize + 1) * w];
        let x0 = dx.max(0) as usize;
        let x1 = (w as i32 + dx.min(0)).max(0) as usize;
        if x0 >= x1 {
            continue;
        }
        let (sx0, n) = ((x0 as i32 - dx) as usize, x1 - x0);
        let d255 = |v: u32| (v + 128 + ((v + 128) >> 8)) >> 8;
        for (p, &a) in row[x0 * 4..x1 * 4].chunks_exact_mut(4).zip(&arow[sx0..sx0 + n]) {
            let sa = ((a as u32 * g) >> 8).min(255);
            let inv = 255 - p[3] as u32;
            for c in 0..4 {
                let sh = d255(col[c] * sa);
                p[c] = (p[c] as u32 + d255(sh * inv)).min(255) as u8;
            }
        }
    }
}

/// Apply one filter to premultiplied RGBA8 `buf` (`w × h`) in place.
pub fn apply(f: &Filter, buf: &mut [u8], w: usize, h: usize, s: &mut FilterScratch) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: feature checked above.
        return unsafe { apply_avx2(f, buf, w, h, s) };
    }
    apply_inner(f, buf, w, h, s)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn apply_avx2(f: &Filter, buf: &mut [u8], w: usize, h: usize, s: &mut FilterScratch) {
    apply_inner(f, buf, w, h, s)
}

#[inline(always)]
fn apply_inner(f: &Filter, buf: &mut [u8], w: usize, h: usize, s: &mut FilterScratch) {
    match sanitize(f) {
        Filter::Blur(sigma) => blur::<4>(buf, w, h, sigma, &mut s.blur),
        Filter::Shadow { dx, dy, blur, color } => {
            behind(buf, s, w, h, dx.round() as i32, dy.round() as i32, blur, color, 1.0)
        }
        Filter::Glow { radius, intensity, color } => behind(buf, s, w, h, 0, 0, radius, color, intensity),
        Filter::ColorMatrix(m) => {
            for p in buf.chunks_exact_mut(4) {
                let a = p[3] as f32 / 255.0;
                let ia = if a > 0.0 { 1.0 / (a * 255.0) } else { 0.0 };
                let v = [p[0] as f32 * ia, p[1] as f32 * ia, p[2] as f32 * ia, a];
                let mut o = [0f32; 4];
                for (r, oi) in o.iter_mut().enumerate() {
                    let k = &m[r * 5..r * 5 + 5];
                    *oi = (k[0] * v[0] + k[1] * v[1] + k[2] * v[2] + k[3] * v[3] + k[4]).clamp(0.0, 1.0);
                }
                let na = o[3];
                p[0] = (o[0] * na * 255.0 + 0.5) as u8;
                p[1] = (o[1] * na * 255.0 + 0.5) as u8;
                p[2] = (o[2] * na * 255.0 + 0.5) as u8;
                p[3] = (na * 255.0 + 0.5) as u8;
            }
        }
        Filter::Tone(t) => tone(&t, buf),
        Filter::Halftone(ht) => halftone(&ht, buf, w),
        Filter::Ink { amount, scale, seed, ox, oy } => ink(buf, w, amount, scale, seed, ox, oy, s),
    }
}

/// 255 / a for a in 0..=255 (0 → 0), for unpremultiplying without divides.
#[allow(dead_code)]
fn recip255() -> &'static [f32; 256] {
    static T: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|a| if a == 0 { 0.0 } else { 255.0 / a as f32 }))
}

/// round(255 × 65536 / a): `(v × R[a]) >> 16` ≈ v × 255 / a.
fn recip255_q16() -> &'static [u32; 256] {
    static T: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|a| if a == 0 { 0 } else { ((255u64 * 65536 + a as u64 / 2) / a as u64) as u32 }))
}

/// round(v × a / 255) for v, a in 0..=255, exact, without a divide.
#[inline(always)]
fn mul255(v: u32, a: u32) -> u32 {
    let t = v * a + 128;
    (t + (t >> 8)) >> 8
}

/// Gradient map / posterise through a 256-entry luma table.
#[inline(always)]
fn tone(t: &ToneLut, buf: &mut [u8]) {
    let lv = t.levels as u32;
    let q = |v: u32| if lv >= 2 { ((v * (lv - 1) + 127) / 255) * 255 / (lv - 1) } else { v };
    // Per-channel posterise as a table (no divides per pixel).
    let qt: [u8; 256] = std::array::from_fn(|v| q(v as u32) as u8);
    let amt = (t.amount.clamp(0.0, 1.0) * 256.0) as u32;
    if let (Some(m), 256) = (&t.map, amt) {
        // Full-strength gradient map: integer luma straight from premultiplied pixels.
        let rq = recip255_q16();
        let lut: [[u8; 3]; 256] = std::array::from_fn(|y| m[qt[y] as usize]);
        for p in buf.chunks_exact_mut(4) {
            let a = p[3] as u32;
            let y = (54 * p[0] as u32 + 183 * p[1] as u32 + 19 * p[2] as u32 + 128) >> 8;
            let yu = ((y * rq[a as usize] + 32768) >> 16).min(255);
            let c = lut[yu as usize];
            p[0] = mul255(c[0] as u32, a) as u8;
            p[1] = mul255(c[1] as u32, a) as u8;
            p[2] = mul255(c[2] as u32, a) as u8;
        }
        return;
    }
    let rc = recip255();
    for p in buf.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a == 0 {
            continue;
        }
        let ia = rc[a as usize];
        let un = |v: u8| ((v as f32 * ia + 0.5) as u32).min(255);
        let (r, g, b) = (un(p[0]), un(p[1]), un(p[2]));
        let o = match &t.map {
            Some(m) => {
                let y = (54 * r + 183 * g + 19 * b + 128) >> 8;
                let c = m[qt[y.min(255) as usize] as usize];
                [c[0] as u32, c[1] as u32, c[2] as u32]
            }
            None => [qt[r as usize] as u32, qt[g as usize] as u32, qt[b as usize] as u32],
        };
        let src = [r, g, b];
        for c in 0..3 {
            let v = (src[c] * (256 - amt) + o[c] * amt) >> 8;
            p[c] = mul255(v, a) as u8;
        }
    }
}

/// Radius (in cell units) at which a spot of `shape` covers fraction `k` of its cell.
fn spot_radius(shape: u8, k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    match shape {
        1 => k * 0.5,
        2 => k.sqrt() * 0.5,
        _ => {
            // Round dot: area of {d < r} inside the unit cell, inverted by bisection.
            let area = |r: f32| {
                let pi = std::f32::consts::PI;
                if r <= 0.5 {
                    pi * r * r
                } else if r >= std::f32::consts::FRAC_1_SQRT_2 {
                    1.0
                } else {
                    pi * r * r - 4.0 * (r * r * (0.5 / r).acos() - 0.5 * (r * r - 0.25).sqrt())
                }
            };
            let (mut lo, mut hi) = (0.0f32, std::f32::consts::FRAC_1_SQRT_2);
            for _ in 0..24 {
                let m = 0.5 * (lo + hi);
                if area(m) < k {
                    lo = m
                } else {
                    hi = m
                }
            }
            0.5 * (lo + hi)
        }
    }
}

/// Amplitude-modulated halftone: each pixel's darkness is compared with the
/// spot function of its screen cell (as a RIP does), so dot edges follow the
/// image and the screen stays fixed to the frame.
#[inline(always)]
fn halftone(h: &Halftone, buf: &mut [u8], w: usize) {
    if h.cmyk {
        return halftone_cmyk(h, buf, w);
    }
    match h.shape {
        1 => halftone_s::<1>(h, buf, w),
        2 => halftone_s::<2>(h, buf, w),
        _ => halftone_s::<0>(h, buf, w),
    }
}

/// CMYK separation: under-colour removal to K, four screens at the press angles, the inks multiplied
/// on the paper (subtractive), the layer's coverage kept.
fn halftone_cmyk(h: &Halftone, buf: &mut [u8], w: usize) {
    let rt: [f32; 257] = std::array::from_fn(|i| spot_radius(h.shape, (i as f32 / 256.0 * h.gain).min(1.0)));
    const INKS: [[f32; 3]; 4] = [[0.0, 0.68, 0.94], [0.93, 0.0, 0.55], [1.0, 0.95, 0.0], [0.14, 0.12, 0.13]];
    const ANG: [f32; 4] = [15.0, 75.0, 0.0, 45.0];
    let paper = if h.paper.0[3] > 0.0 { [h.paper.0[0], h.paper.0[1], h.paper.0[2]] } else { [1.0, 1.0, 1.0] };
    let inv = 1.0 / h.cell;
    let rot: [(f32, f32); 4] = std::array::from_fn(|k| (h.angle + ANG[k]).to_radians().sin_cos());
    for (y, row) in buf.chunks_exact_mut(w * 4).enumerate() {
        let fy = y as f32 + 0.5 + h.oy;
        for (x, p) in row.chunks_exact_mut(4).enumerate() {
            let a = p[3] as u32;
            if a == 0 {
                continue;
            }
            let sa = a as f32 / 255.0;
            let rgb = [p[0] as f32 / 255.0 / sa, p[1] as f32 / 255.0 / sa, p[2] as f32 / 255.0 / sa].map(|v| v.min(1.0));
            // Skeleton black: K only takes over past 25% darkness; CMY carry the midtones.
            let k = ((1.0 - rgb[0].max(rgb[1]).max(rgb[2]) - 0.25) / 0.75).max(0.0);
            let d = (1.0 - k).max(1e-4);
            let tones = [((1.0 - rgb[0] - k) / d).max(0.0), ((1.0 - rgb[1] - k) / d).max(0.0), ((1.0 - rgb[2] - k) / d).max(0.0), k];
            let fx = x as f32 + 0.5 + h.ox;
            let mut col = paper;
            for pl in 0..4 {
                let (sn, cs) = rot[pl];
                let u = (fx * cs + fy * sn) * inv + 1024.0;
                let v = (-fx * sn + fy * cs) * inv + 1024.0;
                let (du, dv) = (u - u.floor() - 0.5, v - v.floor() - 0.5);
                let dist = match h.shape {
                    1 => dv.abs(),
                    2 => du.abs().max(dv.abs()),
                    _ => (du * du + dv * dv).sqrt(),
                };
                let rad = rt[((tones[pl].clamp(0.0, 1.0) * 256.0) as usize).min(256)];
                let cov = ((rad - dist) * h.cell + 0.5).clamp(0.0, 1.0);
                for c in 0..3 {
                    col[c] *= 1.0 - cov * (1.0 - INKS[pl][c]);
                }
            }
            for c in 0..3 {
                p[c] = (col[c] * sa * 255.0 + 0.5) as u8;
            }
        }
    }
}

#[inline(always)]
fn halftone_s<const SHAPE: u8>(h: &Halftone, buf: &mut [u8], w: usize) {
    // Spot radius by unpremultiplied luma (0..255).
    let rt: [f32; 256] = std::array::from_fn(|y| spot_radius(h.shape, ((255 - y) as f32 / 255.0 * h.gain).min(1.0)));
    let (sn, cs) = h.angle.to_radians().sin_cos();
    let inv = 1.0 / h.cell;
    let src_ink = h.ink.0[3] <= 0.0;
    let paper_a = h.paper.0[3].clamp(0.0, 1.0);
    let ink = h.ink.0.map(|v| v.clamp(0.0, 1.0) * 255.0);
    let paper = h.paper.0.map(|v| v.clamp(0.0, 1.0) * 255.0);
    // Fixed inks: the output (before the layer's own alpha) as a table of coverage.
    let lut: [[u8; 4]; 256] = std::array::from_fn(|i| {
        let cov = i as f32 / 255.0;
        let q = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;
        if paper_a > 0.0 {
            let oa = paper_a + (1.0 - paper_a) * cov;
            [0, 1, 2].map(|c| q(paper[c] * paper_a * (1.0 - cov) + ink[c] * cov)).into_iter().chain([q(oa * 255.0)]).collect::<Vec<u8>>().try_into().unwrap()
        } else {
            [q(ink[0] * cov), q(ink[1] * cov), q(ink[2] * cov), q(cov * 255.0)]
        }
    });
    let rq = recip255_q16();
    let cell = h.cell;
    let mut covs = vec![0u8; w];
    let mut rads = vec![0f32; w];
    for (y, row) in buf.chunks_exact_mut(w * 4).enumerate() {
        let fy = y as f32 + 0.5 + h.oy;
        let fx0 = 0.5 + h.ox;
        // Positive offset so truncation is floor; positions as base + i × step (no drift).
        let u0 = (fx0 * cs + fy * sn) * inv + 1024.0;
        let v0 = (-fx0 * sn + fy * cs) * inv + 1024.0;
        let (du_dx, dv_dx) = (cs * inv, -sn * inv);
        // Pass 1: spot radius from unpremultiplied luma.
        for (r, p) in rads.iter_mut().zip(row.chunks_exact(4)) {
            let a = p[3] as u32;
            let yl = (54 * p[0] as u32 + 183 * p[1] as u32 + 19 * p[2] as u32 + 128) >> 8;
            let yu = ((yl * rq[a as usize] + 32768) >> 16).min(255) as usize;
            *r = rt[yu];
        }
        // Pass 2: coverage, branch-free (vectorises).
        for (i, (c, &rad)) in covs.iter_mut().zip(&rads).enumerate() {
            let uu = u0 + i as f32 * du_dx;
            let vv = v0 + i as f32 * dv_dx;
            let du = uu - uu.floor() - 0.5;
            let dv = vv - vv.floor() - 0.5;
            let d = match SHAPE {
                1 => dv.abs(),
                2 => du.abs().max(dv.abs()),
                _ => (du * du + dv * dv).sqrt(),
            };
            let cov = ((rad - d) * cell + 0.5).clamp(0.0, 1.0);
            *c = (cov * 255.0 + 0.5) as u8;
        }
        // Pass 3: colour.
        for (p, &c8) in row.chunks_exact_mut(4).zip(&covs) {
            let a = p[3] as u32;
            if !src_ink {
                let o = lut[c8 as usize];
                p[0] = mul255(o[0] as u32, a) as u8;
                p[1] = mul255(o[1] as u32, a) as u8;
                p[2] = mul255(o[2] as u32, a) as u8;
                p[3] = mul255(o[3] as u32, a) as u8;
                continue;
            }
            if a == 0 {
                continue;
            }
            // Each dot in the layer's own colour there.
            let cov = c8 as f32 / 255.0;
            let sa = a as f32 / 255.0;
            let ia = 1.0 / sa;
            let inkc = [p[0] as f32 * ia, p[1] as f32 * ia, p[2] as f32 * ia];
            if paper_a > 0.0 {
                let oa = sa * (paper_a + (1.0 - paper_a) * cov);
                for c in 0..3 {
                    let col = paper[c] * paper_a * (1.0 - cov) + inkc[c].min(255.0) * cov;
                    p[c] = (col * sa + 0.5).min(255.0) as u8;
                }
                p[3] = (oa * 255.0 + 0.5) as u8;
            } else {
                let oa = sa * cov;
                for c in 0..3 {
                    p[c] = (inkc[c].min(255.0) * oa + 0.5) as u8;
                }
                p[3] = (oa * 255.0 + 0.5) as u8;
            }
        }
    }
}

#[inline(always)]
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// The ink thin-map of a region: 0 (full ink) … 255 (thinnest), frame-anchored. The smooth part
/// is evaluated on a 2 px lattice and interpolated; speckle and pinholes per pixel.
fn ink_map(map: &mut Vec<u8>, w: usize, h: usize, scale: f32, seed: u32, ox: f32, oy: f32) {
    let inv = 1.0 / scale;
    let vn = |x: f32, y: f32, s: u32| {
        let (fx, fy) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx, y - fy);
        let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let (ix, iy) = (fx as i32, fy as i32);
        let a = hash2(ix, iy, s);
        let b = hash2(ix + 1, iy, s);
        let c = hash2(ix, iy + 1, s);
        let d = hash2(ix + 1, iy + 1, s);
        a + (b - a) * sx + (c - a) * sy + (a - b - c + d) * sx * sy
    };
    // Smooth field on a 2 px lattice aligned to the frame (so regions agree).
    let (gx0, gy0) = (((ox / 2.0).floor()) as i32, ((oy / 2.0).floor()) as i32);
    let gw = (w as f32 / 2.0).ceil() as usize + 2;
    let gh = (h as f32 / 2.0).ceil() as usize + 2;
    let mut grid = vec![0f32; gw * gh];
    for j in 0..gh {
        for i in 0..gw {
            let (gx, gy) = (((gx0 + i as i32) * 2) as f32, ((gy0 + j as i32) * 2) as f32);
            let (u, v) = (gx * inv, gy * inv);
            let n = 0.5 * vn(u, v, seed) + 0.3 * vn(0.8 * u - 0.6 * v + 17.3, 0.6 * u + 0.8 * v, seed ^ 0x9e37) + 0.2 * vn(2.9 * (0.28 * u + 0.96 * v), 2.9 * (-0.96 * u + 0.28 * v) + 5.1, seed ^ 0x7f4a);
            let n2 = vn(u * 6.3 + 3.7, v * 6.3, seed ^ 0x2c1b);
            let t = ((n - 0.47) / 0.28).clamp(0.0, 1.0);
            grid[j * gw + i] = t * t * (3.0 - 2.0 * t) * (0.55 + 0.45 * n2);
        }
    }
    map.clear();
    map.resize(w * h, 0);
    for y in 0..h {
        let fy = y as f32 + oy;
        let gy = fy / 2.0 - gy0 as f32;
        let (j, ty) = ((gy.floor() as usize).min(gh - 2), gy - gy.floor());
        for x in 0..w {
            let fx = x as f32 + ox;
            let gx = fx / 2.0 - gx0 as f32;
            let (i, tx) = ((gx.floor() as usize).min(gw - 2), gx - gx.floor());
            let g = |i: usize, j: usize| grid[j * gw + i];
            let patch = (g(i, j) * (1.0 - tx) + g(i + 1, j) * tx) * (1.0 - ty) + (g(i, j + 1) * (1.0 - tx) + g(i + 1, j + 1) * tx) * ty;
            let speck = hash2(fx as i32, fy as i32, seed ^ 0x51ed);
            let thin = (patch + 0.25 * (speck - 0.5).max(0.0) + if speck > 0.992 { 0.7 } else { 0.0 }).min(1.0);
            map[y * w + x] = (thin * 255.0 + 0.5) as u8;
        }
    }
}

/// Uneven ink coverage: multiply the layer by (1 − amount × thin-map). The map of the
/// last region is cached, so a static ink layer costs one multiply per pixel per frame.
#[allow(clippy::too_many_arguments)]
fn ink(buf: &mut [u8], w: usize, amount: f32, scale: f32, seed: u32, ox: f32, oy: f32, s: &mut FilterScratch) {
    let h = buf.len() / (4 * w.max(1));
    let key = (seed, scale.to_bits(), ox.round() as i32, oy.round() as i32, w, h);
    if s.ink_key != Some(key) {
        ink_map(&mut s.ink_map, w, h, scale, seed, ox.round(), oy.round());
        s.ink_key = Some(key);
    }
    let am = (amount.clamp(0.0, 1.0) * 256.0) as u32;
    for (p, &t) in buf.chunks_exact_mut(4).zip(&s.ink_map) {
        let m = 256 - ((t as u32 * am) >> 8);
        for c in p.iter_mut() {
            *c = ((*c as u32 * m) >> 8) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duotone_maps_black_and_white_to_the_pair() {
        let mut m = Box::new([[0u8; 3]; 256]);
        for (i, c) in m.iter_mut().enumerate() {
            *c = [i as u8, 0, 255 - i as u8];
        }
        let t = ToneLut { map: Some(m), levels: 0, amount: 1.0 };
        let mut buf = vec![0, 0, 0, 255, 255, 255, 255, 255, 64, 64, 64, 128];
        apply(&Filter::Tone(std::sync::Arc::new(t.clone())), &mut buf, 3, 1, &mut FilterScratch::default());
        assert_eq!(&buf[0..4], &[0, 0, 255, 255]);
        assert_eq!(&buf[4..8], &[255, 0, 0, 255]);
        // Half-alpha grey stays half-alpha, mapped to the middle.
        assert_eq!(buf[11], 128);
        assert!((buf[8] as i32 - 64).abs() <= 2 && (buf[10] as i32 - 64).abs() <= 2, "{:?}", &buf[8..12]);
        // Posterise to 2 levels on luma.
        let t2 = ToneLut { levels: 2, ..t };
        let mut b2 = vec![100, 100, 100, 255, 160, 160, 160, 255];
        apply(&Filter::Tone(std::sync::Arc::new(t2)), &mut b2, 2, 1, &mut FilterScratch::default());
        assert_eq!(&b2[..4], &[0, 0, 255, 255]);
        assert_eq!(&b2[4..], &[255, 0, 0, 255]);
    }

    #[test]
    fn halftone_preserves_mean_tone() {
        for shape in 0..3u8 {
            for grey in [40u8, 128, 210] {
                let (w, h) = (96, 96);
                let mut buf: Vec<u8> = (0..w * h).flat_map(|_| [grey, grey, grey, 255]).collect();
                let ht = Halftone { cell: 8.0, angle: 15.0, shape, ink: Rgba([0.0, 0.0, 0.0, 1.0]), paper: Rgba([1.0, 1.0, 1.0, 1.0]), gain: 1.0, ox: 0.0, oy: 0.0, cmyk: false };
                apply(&Filter::Halftone(ht), &mut buf, w, h, &mut FilterScratch::default());
                let mean = buf.chunks(4).map(|p| p[0] as f64).sum::<f64>() / (w * h) as f64;
                assert!((mean - grey as f64).abs() < 14.0, "shape {shape} grey {grey}: mean {mean}");
                // Binary-ish: most pixels are near ink or paper.
                let mid = buf.chunks(4).filter(|p| p[0] > 40 && p[0] < 215).count();
                assert!(mid < w * h / 3, "shape {shape} grey {grey}: {mid} mid-tones");
            }
        }
    }

    #[test]
    fn blur_preserves_mass() {
        let (w, h) = (64, 64);
        let mut buf = vec![0u8; w * h * 4];
        for y in 24..40 {
            for x in 24..40 {
                buf[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[200, 100, 50, 200]);
            }
        }
        let before: u64 = buf.iter().map(|&v| v as u64).sum();
        apply(&Filter::Blur(3.0), &mut buf, w, h, &mut FilterScratch::default());
        let after: u64 = buf.iter().map(|&v| v as u64).sum();
        assert!((before as f64 - after as f64).abs() / (before as f64) < 0.02);
        assert!(buf[(32 * w + 32) * 4 + 3] > 150 && buf[(32 * w + 20) * 4 + 3] > 0);
    }
}


#[cfg(test)]
mod perf {
    use super::*;
    #[test]
    #[ignore]
    fn blur_timing() {
        let (w, h) = (2042usize, 1202usize);
        let mut buf = vec![100u8; w * h * 4];
        let mut s = FilterScratch::default();
        let (gw, gh) = (900usize, 300usize);
        let mut g = vec![0u8; gw * gh * 4];
        for (i, p) in g.chunks_mut(4).enumerate() {
            if (i % gw) % 37 < 20 && (i / gw) % 50 < 30 {
                p.copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        for _ in 0..3 {
            let t = std::time::Instant::now();
            apply(&Filter::Glow { radius: 32.4, intensity: 0.6, color: Rgba([0.4, 0.3, 1.0, 1.0]) }, &mut g, gw, gh, &mut s);
            eprintln!("glow 900x300: {:?}", t.elapsed());
        }
        for sigma in [2.0f32, 5.0, 8.0, 20.0, 2.0, 5.0, 8.0, 20.0] {
            let t = std::time::Instant::now();
            apply(&Filter::Blur(sigma), &mut buf, w, h, &mut s);
            eprintln!("blur4 sigma {sigma}: {:?}", t.elapsed());
        }
    }
}

#[cfg(test)]
mod ink_tests {
    use super::*;

    #[test]
    fn ink_thins_coverage_deterministically_and_only_darkens_alpha() {
        let (w, h) = (128, 64);
        let src: Vec<u8> = (0..w * h).flat_map(|_| [200u8, 40, 120, 255]).collect();
        let f = Filter::Ink { amount: 0.5, scale: 16.0, seed: 3, ox: 0.0, oy: 0.0 };
        let mut a = src.clone();
        apply(&f, &mut a, w, h, &mut FilterScratch::default());
        let mut b = src.clone();
        apply(&f, &mut b, w, h, &mut FilterScratch::default());
        assert_eq!(a, b);
        let mean = a.chunks(4).map(|p| p[3] as f64).sum::<f64>() / (w * h) as f64;
        assert!(mean < 250.0 && mean > 150.0, "{mean}");
        assert!(a.chunks(4).all(|p| p[0] <= p[3]));
        // Frame-anchored: shifting the origin by a whole tile row moves the texture with it.
        let mut c = src[w * 4 * 8..].to_vec();
        apply(&Filter::Ink { amount: 0.5, scale: 16.0, seed: 3, ox: 0.0, oy: 8.0 }, &mut c, w, h - 8, &mut FilterScratch::default());
        assert_eq!(&c[..], &a[w * 4 * 8..]);
    }
}

#[cfg(test)]
mod graphic_perf {
    use super::*;
    #[test]
    #[ignore]
    fn treatment_timing() {
        let (w, h) = (1920usize, 1080usize);
        let src: Vec<u8> = (0..w * h).flat_map(|i| { let v = ((i % w) * 255 / w) as u8; [v, v / 2, 255 - v, 255] }).collect();
        let mut s = FilterScratch::default();
        let ht = Halftone { cell: 10.0, angle: 15.0, shape: std::env::var("HS").ok().and_then(|v| v.parse().ok()).unwrap_or(0), ink: Rgba([0.0, 0.0, 0.0, 1.0]), paper: Rgba([1.0, 1.0, 1.0, 1.0]), gain: 1.0, ox: 0.0, oy: 0.0, cmyk: std::env::var("CMYK").is_ok() };
        for _ in 0..3 {
            let mut b = src.clone();
            let t = std::time::Instant::now();
            apply(&Filter::Halftone(ht), &mut b, w, h, &mut s);
            eprintln!("halftone 1080p: {:?}", t.elapsed());
        }
    }
}

#[cfg(test)]
mod cmyk_tests {
    use super::*;

    #[test]
    fn cmyk_keeps_tone_and_hue() {
        let (w, h) = (96, 96);
        for col in [[200u8, 60, 40], [90, 90, 90], [40, 120, 200]] {
            let mut b: Vec<u8> = (0..w * h).flat_map(|_| [col[0], col[1], col[2], 255]).collect();
            let ht = Halftone { cell: 8.0, angle: 0.0, shape: 0, ink: Rgba([0.0; 4]), paper: Rgba([1.0, 1.0, 1.0, 1.0]), gain: 1.0, ox: 0.0, oy: 0.0, cmyk: true };
            apply(&Filter::Halftone(ht), &mut b, w, h, &mut FilterScratch::default());
            let mean = |c: usize| b.chunks(4).map(|p| p[c] as f64).sum::<f64>() / (w * h) as f64;
            let m = [mean(0), mean(1), mean(2)];
            // The dominant channel stays dominant, and the overall lightness is within reach.
            let src_max = (0..3).max_by_key(|&i| col[i]).unwrap();
            let out_max = (0..3).max_by(|&a, &b| m[a].total_cmp(&m[b])).unwrap();
            assert_eq!(src_max == out_max || col.iter().all(|&c| c == col[0]), true, "{col:?} → {m:?}");
            let ls = col.iter().map(|&c| c as f64).sum::<f64>() / 3.0;
            let lo = (m[0] + m[1] + m[2]) / 3.0;
            assert!((ls - lo).abs() < 45.0, "{col:?} → {m:?}");
        }
    }
}
