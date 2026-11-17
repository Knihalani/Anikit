//! Layer filters on premultiplied RGBA8 pixels: blur (3× box ≈ Gaussian),
//! drop shadow, glow and colour matrix. All loops are branch-free over rows
//! so they vectorise; scratch memory is reused across frames.

use super::blur::{blur, BlurScratch};
use crate::dl::{Filter, Rgba};

/// Reusable scratch buffers.
#[derive(Default, Debug)]
pub struct FilterScratch {
    a: Vec<u8>,
    blur: BlurScratch,
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
        Filter::ColorMatrix(_) => [0.0; 4],
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
