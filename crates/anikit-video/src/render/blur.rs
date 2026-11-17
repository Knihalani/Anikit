//! Fast Gaussian-approximating blur on interleaved u8 pixels (1 or 4 bytes).
//!
//! Every pass is a *row* operation over contiguous bytes (so it vectorises
//! regardless of channel layout): the horizontal passes run on a transposed
//! copy. Large blurs run decimated by `f` (row averaging, then linear
//! upsampling), which a blur of σ ≥ 2f hides. Three box passes approximate
//! the Gaussian (variance-matched radii).

/// Scratch buffers, reused across calls.
#[derive(Default, Debug)]
pub struct BlurScratch {
    a: Vec<u8>,
    b: Vec<u8>,
    c: Vec<u8>,
    sums16: Vec<u16>,
    sums32: Vec<u32>,
    zero: Vec<u8>,
    /// Reference quality: no decimation (only as much as the u16 box sums need).
    pub exact: bool,
}

/// Box radii approximating a Gaussian of std-dev `sigma` with three passes.
pub fn box_radii(sigma: f32) -> [usize; 3] {
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

/// Extended box (Gwosdek et al. 2011): integer radius `r` plus edge taps of
/// weight `a` ∈ [0, 1), whose variance is exactly `var` — continuous in σ, so
/// an animated blur grows smoothly instead of in whole-pixel radius steps.
pub fn ext_box(var: f64) -> (usize, f32) {
    let var = var.max(0.0);
    let l = ((12.0 * var + 1.0).sqrt() - 1.0) / 2.0;
    let r = l.floor().max(0.0);
    let k = r * (r + 1.0) / 3.0;
    let a = ((2.0 * r + 1.0) * (var - k) / (2.0 * ((r + 1.0) * (r + 1.0) - var))).clamp(0.0, 1.0);
    (r as usize, a as f32)
}

/// Vertical extended-box blur of `n` rows × `row` bytes, zero outside: radius
/// `r ≤ 126` plus edge rows at weight `a`. Fixed point (weights × 2¹⁶; the sum
/// stays below 2³²: inner ≤ 255·2¹⁶, edges ≤ 510·2¹⁶), one fused pass per row.
#[inline(always)]
fn ext_box_rows(src: &[u8], dst: &mut [u8], row: usize, n: usize, r: usize, a: f32, sums: &mut Vec<u16>, zero: &mut Vec<u8>) {
    if a <= 0.0 {
        return box_rows(src, dst, row, n, r, sums);
    }
    debug_assert!(r <= 126);
    let den = 2.0 * r as f64 + 1.0 + 2.0 * a as f64;
    let k1 = (65536.0 / den).round() as u32;
    let k2 = (a as f64 * 65536.0 / den).round() as u32;
    sums.clear();
    sums.resize(row, 0);
    for y in 0..=r.min(n - 1) {
        for (s, &v) in sums.iter_mut().zip(&src[y * row..(y + 1) * row]) {
            *s += v as u16;
        }
    }
    zero.clear();
    zero.resize(row, 0);
    for y in 0..n {
        let d = &mut dst[y * row..(y + 1) * row];
        // Edge rows y−r−1 and y+r+1 (zero outside); the window then slides:
        // + row y+r+1, − row y−r.
        let lo = if y > r { &src[(y - r - 1) * row..(y - r) * row] } else { &zero[..] };
        let hi = if y + r + 1 < n { &src[(y + r + 1) * row..(y + r + 2) * row] } else { &zero[..] };
        let sub = if y >= r { &src[(y - r) * row..(y - r + 1) * row] } else { &zero[..] };
        for ((((d, s), &e0), &e1), &b) in d.iter_mut().zip(sums.iter_mut()).zip(lo).zip(hi).zip(sub) {
            let v = (*s as u32 * k1 + (e0 as u32 + e1 as u32) * k2 + 32768) >> 16;
            *d = v.min(255) as u8;
            *s = s.wrapping_add(e1 as u16).wrapping_sub(b as u16);
        }
    }
}

/// Vertical box blur of `n` rows × `row` bytes, zero outside. `r ≤ 127`.
#[inline(always)]
fn box_rows(src: &[u8], dst: &mut [u8], row: usize, n: usize, r: usize, sums: &mut Vec<u16>) {
    debug_assert!(r <= 127);
    if r == 0 {
        dst[..row * n].copy_from_slice(&src[..row * n]);
        return;
    }
    let inv = 65536 / (2 * r as u32 + 1);
    let rr = r as u16;
    sums.clear();
    sums.resize(row, 0);
    for y in 0..=r.min(n - 1) {
        for (s, &v) in sums.iter_mut().zip(&src[y * row..(y + 1) * row]) {
            *s += v as u16;
        }
    }
    let norm = |s: u16| (((s + rr) as u32 * inv) >> 16) as u8;
    for y in 0..n {
        let d = &mut dst[y * row..(y + 1) * row];
        let add = y + r + 1 < n;
        let sub = y >= r;
        match (add, sub) {
            (true, true) => {
                let a = &src[(y + r + 1) * row..(y + r + 2) * row];
                let b = &src[(y - r) * row..(y - r + 1) * row];
                for (((d, s), &a), &b) in d.iter_mut().zip(sums.iter_mut()).zip(a).zip(b) {
                    *d = norm(*s);
                    *s = *s + a as u16 - b as u16;
                }
            }
            (true, false) => {
                let a = &src[(y + r + 1) * row..(y + r + 2) * row];
                for ((d, s), &a) in d.iter_mut().zip(sums.iter_mut()).zip(a) {
                    *d = norm(*s);
                    *s += a as u16;
                }
            }
            (false, true) => {
                let b = &src[(y - r) * row..(y - r + 1) * row];
                for ((d, s), &b) in d.iter_mut().zip(sums.iter_mut()).zip(b) {
                    *d = norm(*s);
                    *s -= b as u16;
                }
            }
            (false, false) => {
                for (d, &s) in d.iter_mut().zip(sums.iter()) {
                    *d = norm(s);
                }
            }
        }
    }
}

/// Average groups of `f` rows: `n` rows → `ceil(n / f)` rows.
#[inline(always)]
fn down_rows(src: &[u8], dst: &mut Vec<u8>, row: usize, n: usize, f: usize, sums: &mut Vec<u32>) -> usize {
    let m = n.div_ceil(f);
    dst.resize(row * m, 0);
    if f == 1 {
        dst.copy_from_slice(&src[..row * n]);
        return m;
    }
    sums.resize(row, 0);
    for o in 0..m {
        let (y0, y1) = (o * f, ((o + 1) * f).min(n));
        sums.fill(0);
        for y in y0..y1 {
            for (s, &v) in sums.iter_mut().zip(&src[y * row..(y + 1) * row]) {
                *s += v as u32;
            }
        }
        let inv = 65536 / (y1 - y0) as u32;
        for (d, &s) in dst[o * row..(o + 1) * row].iter_mut().zip(sums.iter()) {
            *d = ((s * inv + (1 << 15)) >> 16) as u8;
        }
    }
    m
}

/// Linear upsample of `m` rows (decimated by `f`) back to `n` rows.
#[inline(always)]
fn up_rows(src: &[u8], dst: &mut [u8], row: usize, m: usize, n: usize, f: usize) {
    if f == 1 {
        dst[..row * n].copy_from_slice(&src[..row * n]);
        return;
    }
    for y in 0..n {
        let p = ((y as f32 + 0.5) / f as f32 - 0.5).clamp(0.0, (m - 1) as f32);
        let y0 = p as usize;
        let y1 = (y0 + 1).min(m - 1);
        let t = ((p - y0 as f32) * 256.0 + 0.5) as u16;
        let (a, b) = (&src[y0 * row..(y0 + 1) * row], &src[y1 * row..(y1 + 1) * row]);
        let d = &mut dst[y * row..(y + 1) * row];
        if t == 0 {
            d.copy_from_slice(a);
            continue;
        }
        let it = 256 - t;
        for ((d, &a), &b) in d.iter_mut().zip(a).zip(b) {
            *d = ((a as u16 * it + b as u16 * t + 128) >> 8) as u8;
        }
    }
}

/// Transpose a `w × h` grid of `B`-byte elements into `h × w`.
#[inline(always)]
fn transpose<const B: usize>(src: &[u8], dst: &mut Vec<u8>, w: usize, h: usize) {
    assert!(src.len() >= w * h * B);
    dst.resize(w * h * B, 0);
    const T: usize = 32;
    let (sp, dp) = (src.as_ptr(), dst.as_mut_ptr());
    for by in (0..h).step_by(T) {
        let ye = (by + T).min(h);
        for bx in (0..w).step_by(T) {
            let xe = (bx + T).min(w);
            for x in bx..xe {
                for y in by..ye {
                    // SAFETY: x < w, y < h; both buffers hold w·h elements.
                    unsafe {
                        let v = std::ptr::read_unaligned(sp.add((y * w + x) * B) as *const [u8; B]);
                        std::ptr::write_unaligned(dp.add((x * h + y) * B) as *mut [u8; B], v);
                    }
                }
            }
        }
    }
}

/// Decimation factor for a blur of std-dev `sigma`.
fn factor(sigma: f32, exact: bool) -> usize {
    let mut f = if sigma < 4.0 || exact {
        1
    } else if sigma < 8.0 {
        2
    } else if sigma < 24.0 {
        4
    } else {
        8
    };
    // Keep box radii ≤ 126 (u16 sums).
    while ext_box(pass_var(sigma, f)).0 > 126 {
        f *= 2;
    }
    f
}

/// Variance of each of the three box passes, in decimated pixels, so that
/// decimation by `f` (an `f`-box, variance (f²−1)/12), the passes and the
/// linear upsample (a tent of half-width f, variance f²/6) add up to σ².
fn pass_var(sigma: f32, f: usize) -> f64 {
    let (s, f) = (sigma as f64, f as f64);
    let resample = if f > 1.0 { (f * f - 1.0) / 12.0 + f * f / 6.0 } else { 0.0 };
    ((s * s - resample).max(0.0) / (f * f)) / 3.0
}

/// Blur `buf` (`w × h` elements of `B` bytes) in place.
#[inline(always)]
pub fn blur<const B: usize>(buf: &mut [u8], w: usize, h: usize, sigma: f32, s: &mut BlurScratch) {
    if sigma.is_nan() || sigma <= 0.0 || w == 0 || h == 0 {
        return;
    }
    let sigma = sigma.min(1000.0);
    let f = factor(sigma, s.exact).min(w.max(1)).min(h.max(1));
    let (r, a) = ext_box(pass_var(sigma, f));
    let r = r.min(126);
    // 1. decimate rows (y).
    let hs = down_rows(buf, &mut s.a, w * B, h, f, &mut s.sums32);
    transpose::<B>(&s.a, &mut s.b, w, hs);
    let ws = down_rows(&s.b, &mut s.c, hs * B, w, f, &mut s.sums32);
    // 4. blur along original x.
    s.a.resize(ws * hs * B, 0);
    for i in 0..3 {
        let (src, dst) = if i % 2 == 0 { (&s.c, &mut s.a) } else { (&s.a, &mut s.c) };
        ext_box_rows(src, dst, hs * B, ws, r, a, &mut s.sums16, &mut s.zero);
    }
    // 5. upsample back to w rows.
    s.b.resize(w * hs * B, 0);
    up_rows(&s.a, &mut s.b, hs * B, ws, w, f);
    transpose::<B>(&s.b, &mut s.c, hs, w);
    s.a.resize(w * hs * B, 0);
    for i in 0..3 {
        let (src, dst) = if i % 2 == 0 { (&s.c, &mut s.a) } else { (&s.a, &mut s.c) };
        ext_box_rows(src, dst, w * B, hs, r, a, &mut s.sums16, &mut s.zero);
    }
    // 8. upsample to h rows, into `buf`.
    up_rows(&s.a, buf, w * B, hs, h, f);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radii_cover_sigma() {
        for &s in &[0.5f32, 1.0, 3.0, 10.0, 40.0] {
            let r = box_radii(s);
            let var: f64 = r.iter().map(|&r| ((2 * r + 1) as f64).powi(2) - 1.0).sum::<f64>() / 12.0;
            assert!((var.sqrt() - s as f64).abs() < 0.8, "{s} {r:?} {var}");
        }
    }

    #[test]
    fn ext_box_variance_is_exact_and_continuous() {
        let mut prev = (0usize, 0f32);
        for i in 0..400 {
            let var = i as f64 * 0.37;
            let (r, a) = ext_box(var);
            let (rf, af) = (r as f64, a as f64);
            let got = ((2.0 * rf + 1.0) * rf * (rf + 1.0) / 3.0 + 2.0 * af * (rf + 1.0) * (rf + 1.0)) / (2.0 * rf + 1.0 + 2.0 * af);
            assert!((got - var).abs() < 1e-4 * var.max(1.0), "var {var}: r {r} a {a} -> {got}");
            // Monotone: the radius never shrinks as the variance grows.
            assert!(r as f32 + a >= prev.0 as f32 + prev.1 - 1e-6);
            prev = (r, a);
        }
    }

    #[test]
    fn transpose_roundtrip() {
        let (w, h) = (37, 19);
        let src: Vec<u8> = (0..w * h * 4).map(|i| i as u8).collect();
        let (mut t, mut back) = (Vec::new(), Vec::new());
        transpose::<4>(&src, &mut t, w, h);
        transpose::<4>(&t, &mut back, h, w);
        assert_eq!(src, back);
        assert_eq!(&t[4..8], &src[w * 4..w * 4 + 4]);
    }

    #[test]
    fn blur_matches_gaussian_profile() {
        // A 1-px line blurred with σ: compare the profile to a Gaussian.
        for &sigma in &[2.0f32, 6.0, 12.0, 30.0] {
            let (w, h) = (256usize, 256usize);
            let mut buf = vec![0u8; w * h];
            let mut s = BlurScratch::default();
            // Wide bar so decimation doesn't lose it: 16 px wide, 255.
            for y in 0..h {
                for x in 120..136 {
                    buf[y * w + x] = 255;
                }
            }
            blur::<1>(&mut buf, w, h, sigma, &mut s);
            let total: f64 = buf[128 * w..129 * w].iter().map(|&v| v as f64).sum();
            assert!((total - 16.0 * 255.0).abs() / (16.0 * 255.0) < 0.05, "σ {sigma}: mass {total}");
            // Reference: bar convolved with a Gaussian, at the centre and 2σ out.
            let g = |x: f64| -> f64 {
                (120..136).map(|k| {
                    let d = x - k as f64;
                    255.0 * (-d * d / (2.0 * (sigma as f64).powi(2))).exp() / ((2.0 * std::f64::consts::PI).sqrt() * sigma as f64)
                }).sum()
            };
            for x in [128.0, 128.0 + 8.0 + 2.0 * sigma as f64] {
                let got = buf[128 * w + x as usize] as f64;
                assert!((got - g(x + 0.5)).abs() < 12.0, "σ {sigma} x {x}: {got} vs {}", g(x + 0.5));
            }
        }
    }
}
