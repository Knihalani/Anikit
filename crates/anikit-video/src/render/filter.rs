//! Layer filters on premultiplied RGBA8 pixels: blur (3× box ≈ Gaussian),
//! drop shadow, glow and colour matrix. All loops are branch-free over rows
//! so they vectorise; scratch memory is reused across frames.

use crate::dl::{Filter, Rgba};

/// Reusable scratch buffers.
#[derive(Default, Debug)]
pub struct FilterScratch {
    a: Vec<u8>,
    b: Vec<u8>,
    sums: Vec<u32>,
}

/// Extent a filter adds around its input, in pixels: (left, top, right, bottom).
pub fn margin(f: &Filter) -> [f32; 4] {
    match *f {
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

/// Box radii approximating a Gaussian of std-dev `sigma` with three passes.
fn box_radii(sigma: f32) -> [usize; 3] {
    let s = sigma as f64;
    let n = 3.0;
    let w_ideal = (12.0 * s * s / n + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let wlf = wl as f64;
    let m = ((12.0 * s * s - n * wlf * wlf - 4.0 * n * wlf - 3.0 * n) / (-4.0 * wlf - 4.0)).round() as i64;
    let mut r = [0usize; 3];
    for (i, ri) in r.iter_mut().enumerate() {
        let w = if (i as i64) < m { wl } else { wu };
        *ri = ((w - 1) / 2) as usize;
    }
    r
}

/// Horizontal box blur (zero outside), `C` interleaved channels.
fn box_h<const C: usize>(src: &[u8], dst: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst[..w * h * C].copy_from_slice(&src[..w * h * C]);
        return;
    }
    let inv = (1u32 << 24) / (2 * r as u32 + 1);
    for y in 0..h {
        let s = &src[y * w * C..(y + 1) * w * C];
        let d = &mut dst[y * w * C..(y + 1) * w * C];
        let mut sum = [0u32; C];
        for x in 0..=r.min(w - 1) {
            for c in 0..C {
                sum[c] += s[x * C + c] as u32;
            }
        }
        for x in 0..w {
            for c in 0..C {
                d[x * C + c] = ((sum[c] * inv + (1 << 23)) >> 24) as u8;
            }
            let add = x + r + 1;
            if add < w {
                for c in 0..C {
                    sum[c] += s[add * C + c] as u32;
                }
            }
            if x >= r {
                for c in 0..C {
                    sum[c] -= s[(x - r) * C + c] as u32;
                }
            }
        }
    }
}

/// Vertical box blur (zero outside) over rows of `row` bytes.
fn box_v(src: &[u8], dst: &mut [u8], row: usize, h: usize, r: usize, sums: &mut Vec<u32>) {
    if r == 0 {
        dst[..row * h].copy_from_slice(&src[..row * h]);
        return;
    }
    let inv = (1u32 << 24) / (2 * r as u32 + 1);
    sums.clear();
    sums.resize(row, 0);
    for y in 0..=r.min(h - 1) {
        for (s, &v) in sums.iter_mut().zip(&src[y * row..(y + 1) * row]) {
            *s += v as u32;
        }
    }
    for y in 0..h {
        for (d, &s) in dst[y * row..(y + 1) * row].iter_mut().zip(sums.iter()) {
            *d = ((s * inv + (1 << 23)) >> 24) as u8;
        }
        let add = y + r + 1;
        if add < h {
            for (s, &v) in sums.iter_mut().zip(&src[add * row..(add + 1) * row]) {
                *s += v as u32;
            }
        }
        if y >= r {
            let sub = y - r;
            for (s, &v) in sums.iter_mut().zip(&src[sub * row..(sub + 1) * row]) {
                *s -= v as u32;
            }
        }
    }
}

/// Gaussian-ish blur of `buf` (`C` channels) in place.
fn blur<const C: usize>(buf: &mut [u8], tmp: &mut Vec<u8>, sums: &mut Vec<u32>, w: usize, h: usize, sigma: f32) {
    if sigma <= 0.0 || w == 0 || h == 0 {
        return;
    }
    tmp.resize(w * h * C, 0);
    for r in box_radii(sigma) {
        box_h::<C>(buf, tmp, w, h, r);
        box_v(tmp, buf, w * C, h, r, sums);
    }
}

fn premul(c: Rgba) -> [u32; 4] {
    let a = c.0[3].clamp(0.0, 1.0);
    let q = |v: f32| (v.clamp(0.0, 1.0) * a * 255.0 + 0.5) as u32;
    [q(c.0[0]), q(c.0[1]), q(c.0[2]), (a * 255.0 + 0.5) as u32]
}

/// Draw `color × shifted, blurred alpha × gain` *behind* the pixels in `buf`.
#[allow(clippy::too_many_arguments)]
fn behind(buf: &mut [u8], s: &mut FilterScratch, w: usize, h: usize, dx: i32, dy: i32, sigma: f32, color: Rgba, gain: f32) {
    let n = w * h;
    s.a.clear();
    s.a.extend(buf.chunks_exact(4).map(|p| p[3]));
    blur::<1>(&mut s.a, &mut s.b, &mut s.sums, w, h, sigma);
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
        for x in x0.min(x1)..x1 {
            let sa = arow[(x as i32 - dx) as usize] as u32;
            let sa = ((sa * g) >> 8).min(255);
            let p = &mut row[x * 4..x * 4 + 4];
            let inv = 255 - p[3] as u32;
            for c in 0..4 {
                let sh = (col[c] * sa + 127) / 255;
                p[c] = (p[c] as u32 + (sh * inv + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// Apply one filter to premultiplied RGBA8 `buf` (`w × h`) in place.
pub fn apply(f: &Filter, buf: &mut [u8], w: usize, h: usize, s: &mut FilterScratch) {
    match *f {
        Filter::Blur(sigma) => blur::<4>(buf, &mut s.b, &mut s.sums, w, h, sigma),
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
    fn box_radii_cover_sigma() {
        for &s in &[0.5f32, 1.0, 3.0, 10.0, 40.0] {
            let r = box_radii(s);
            // variance of three boxes ≈ sigma²
            let var: f64 = r.iter().map(|&r| ((2 * r + 1) as f64).powi(2) - 1.0).sum::<f64>() / 12.0;
            assert!((var.sqrt() - s as f64).abs() < 0.8, "{s} {r:?} {var}");
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
