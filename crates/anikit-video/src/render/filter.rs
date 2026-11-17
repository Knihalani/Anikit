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
        Filter::Ink { amount, scale, seed, ox, oy } => ink(buf, w, amount, scale, seed, ox, oy),
    }
}

/// Gradient map / posterise through a 256-entry luma table.
#[inline(always)]
fn tone(t: &ToneLut, buf: &mut [u8]) {
    let lv = t.levels as u32;
    let q = |v: u32| if lv >= 2 { ((v * (lv - 1) + 127) / 255) * 255 / (lv - 1) } else { v };
    let amt = (t.amount.clamp(0.0, 1.0) * 256.0) as u32;
    for p in buf.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a == 0 {
            continue;
        }
        // Unpremultiply (rounded).
        let un = |v: u8| ((v as u32 * 255 + a / 2) / a).min(255);
        let (r, g, b) = (un(p[0]), un(p[1]), un(p[2]));
        let o = match &t.map {
            Some(m) => {
                let y = (54 * r + 183 * g + 19 * b + 128) >> 8;
                let c = m[q(y.min(255)) as usize];
                [c[0] as u32, c[1] as u32, c[2] as u32]
            }
            None => [q(r), q(g), q(b)],
        };
        let src = [r, g, b];
        for c in 0..3 {
            let v = (src[c] * (256 - amt) + o[c] * amt) >> 8;
            p[c] = ((v * a + 127) / 255) as u8;
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
    let mut rt = [0f32; 257];
    for (i, r) in rt.iter_mut().enumerate() {
        *r = spot_radius(h.shape, (i.min(256) as f32 / 256.0 * h.gain).min(1.0));
    }
    let (sn, cs) = h.angle.to_radians().sin_cos();
    let inv = 1.0 / h.cell;
    let src_ink = h.ink.0[3] <= 0.0;
    let paper_a = h.paper.0[3].clamp(0.0, 1.0);
    let ink = h.ink.0.map(|v| v.clamp(0.0, 1.0) * 255.0);
    let paper = h.paper.0.map(|v| v.clamp(0.0, 1.0) * 255.0);
    for (y, row) in buf.chunks_exact_mut(w * 4).enumerate() {
        let fy = y as f32 + 0.5 + h.oy;
        for (x, p) in row.chunks_exact_mut(4).enumerate() {
            let a = p[3] as u32;
            if a == 0 {
                continue;
            }
            let un = |v: u8| ((v as u32 * 255 + a / 2) / a).min(255) as f32;
            let (r, g, b) = (un(p[0]), un(p[1]), un(p[2]));
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let k = (255.0 - lum).max(0.0) * (256.0 / 255.0);
            let rad = rt[(k as usize).min(256)];
            let fx = x as f32 + 0.5 + h.ox;
            let u = (fx * cs + fy * sn) * inv;
            let v = (-fx * sn + fy * cs) * inv;
            let du = u - u.floor() - 0.5;
            let dv = v - v.floor() - 0.5;
            let d = match h.shape {
                1 => dv.abs(),
                2 => du.abs().max(dv.abs()),
                _ => (du * du + dv * dv).sqrt(),
            };
            let cov = ((rad - d) * h.cell + 0.5).clamp(0.0, 1.0);
            let inkc = if src_ink { [r, g, b] } else { [ink[0], ink[1], ink[2]] };
            let sa = a as f32 / 255.0;
            if paper_a > 0.0 {
                // Opaque print: paper under ink, keeping the layer's coverage.
                let oa = sa * (paper_a + (1.0 - paper_a) * cov);
                for c in 0..3 {
                    let col = paper[c] * paper_a * (1.0 - cov) + inkc[c] * cov;
                    p[c] = (col * sa + 0.5).min(255.0) as u8;
                }
                p[3] = (oa * 255.0 + 0.5) as u8;
            } else {
                let oa = sa * cov;
                for c in 0..3 {
                    p[c] = (inkc[c] * oa + 0.5) as u8;
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

/// Uneven ink coverage: smooth value noise (`scale` px cells, two octaves) plus a
/// fine speckle, multiplied into the layer's premultiplied pixels.
fn ink(buf: &mut [u8], w: usize, amount: f32, scale: f32, seed: u32, ox: f32, oy: f32) {
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
    for (y, row) in buf.chunks_exact_mut(w * 4).enumerate() {
        let gy = y as f32 + oy;
        for (x, p) in row.chunks_exact_mut(4).enumerate() {
            if p[3] == 0 {
                continue;
            }
            let gx = x as f32 + ox;
            // Three octaves on rotated lattices (no visible grid), thresholded so most of the
            // area is solid and the thin spots are irregular patches, plus speckle and pinholes.
            let (u, v) = (gx * inv, gy * inv);
            let n = 0.5 * vn(u, v, seed)
                + 0.3 * vn(0.8 * u - 0.6 * v + 17.3, 0.6 * u + 0.8 * v, seed ^ 0x9e37) * 1.0
                + 0.2 * vn(2.9 * (0.28 * u + 0.96 * v), 2.9 * (-0.96 * u + 0.28 * v) + 5.1, seed ^ 0x7f4a);
            let n2 = vn(u * 6.3 + 3.7, v * 6.3, seed ^ 0x2c1b);
            let speck = hash2(gx as i32, gy as i32, seed ^ 0x51ed);
            let t = ((n - 0.47) / 0.28).clamp(0.0, 1.0);
            let patch = t * t * (3.0 - 2.0 * t);
            let thin = (patch * (0.55 + 0.45 * n2) + 0.25 * (speck - 0.5).max(0.0) + if speck > 0.992 { 0.7 } else { 0.0 }).min(1.0);
            let k = (1.0 - amount * thin).clamp(0.0, 1.0);
            let m = (k * 256.0) as u32;
            for c in p.iter_mut() {
                *c = ((*c as u32 * m) >> 8) as u8;
            }
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
                let ht = Halftone { cell: 8.0, angle: 15.0, shape, ink: Rgba([0.0, 0.0, 0.0, 1.0]), paper: Rgba([1.0, 1.0, 1.0, 1.0]), gain: 1.0, ox: 0.0, oy: 0.0 };
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
