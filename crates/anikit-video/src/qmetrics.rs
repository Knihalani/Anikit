//! Image-quality metrics shared by the `qbench` harness and `critique`.
//!
//! All operate on 8-bit planes. Full-reference metrics (`psnr`, `ssim`,
//! [`de_pooled`], [`flicker`]) compare a frame with a reference; the
//! reference-free detectors ([`temporal_flicker`], [`banding`], [`cut_pop`])
//! look at a rendered sequence alone.
//!
//! Thresholds used by qbench (see `docs/video/perf.md`): SSIM ≥ 0.995,
//! PSNR-Y ≥ 42 dB, pooled ΔE76 p99.9 ≤ 2.3 (one JND; max > 5 flagged), flicker
//! p99.9 ≤ 4 levels.

/// A tight 4:2:0 BT.709 limited-range frame.
#[derive(Clone, Default, Debug)]
pub struct Yuv420 {
    pub w: usize,
    pub h: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// PSNR of two equal-length 8-bit planes (99 when identical).
pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let se: u64 = a.iter().zip(b).map(|(x, y)| ((*x as i32 - *y as i32).pow(2)) as u64).sum();
    mse_to_psnr(se as f64 / a.len().max(1) as f64)
}

/// PSNR from a mean squared error (capped at 99 dB).
pub fn mse_to_psnr(mse: f64) -> f64 {
    if mse <= 0.0 {
        99.0
    } else {
        (10.0 * (255.0f64 * 255.0 / mse).log10()).min(99.0)
    }
}

/// Luma SSIM over 8×8 windows with stride 4 (as x264 and libvpx measure it).
pub fn ssim(a: &[u8], b: &[u8], w: usize, h: usize) -> f64 {
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (mut sum, mut n) = (0.0, 0usize);
    let mut y = 0;
    while y + 8 <= h {
        let mut x = 0;
        while x + 8 <= w {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0u32, 0u32, 0u64, 0u64, 0u64);
            for yy in y..y + 8 {
                let (ra, rb) = (&a[yy * w + x..yy * w + x + 8], &b[yy * w + x..yy * w + x + 8]);
                for k in 0..8 {
                    let (p, q) = (ra[k] as u32, rb[k] as u32);
                    sa += p;
                    sb += q;
                    saa += (p * p) as u64;
                    sbb += (q * q) as u64;
                    sab += (p * q) as u64;
                }
            }
            let m = 64.0;
            let (ma, mb) = (sa as f64 / m, sb as f64 / m);
            let va = saa as f64 / m - ma * ma;
            let vb = sbb as f64 / m - mb * mb;
            let cov = sab as f64 / m - ma * mb;
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
            x += 4;
        }
        y += 4;
    }
    if n == 0 {
        1.0
    } else {
        sum / n as f64
    }
}

/// Separable Gaussian blur of a `w × h` plane (σ in pixels, clamped edges).
pub fn gauss(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as isize;
    let k: Vec<f32> = (-r..=r).map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f32 = k.iter().sum();
    let k: Vec<f32> = k.iter().map(|v| v / sum).collect();
    let mut tmp = vec![0f32; w * h];
    for y in 0..h {
        let row = &src[y * w..y * w + w];
        for x in 0..w {
            let mut a = 0.0;
            for (j, kv) in k.iter().enumerate() {
                let xx = (x as isize + j as isize - r).clamp(0, w as isize - 1) as usize;
                a += kv * row[xx];
            }
            tmp[y * w + x] = a;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for (j, kv) in k.iter().enumerate() {
            let yy = (y as isize + j as isize - r).clamp(0, h as isize - 1) as usize;
            let (o, t) = (&mut out[y * w..y * w + w], &tmp[yy * w..yy * w + w]);
            for x in 0..w {
                o[x] += kv * t[x];
            }
        }
    }
    out
}

/// BT.709 limited Y'CbCr (fractional allowed) → CIELAB (D65).
pub fn lab(y: f32, u: f32, v: f32) -> [f32; 3] {
    let (y, u, v) = ((y - 16.0) * (255.0 / 219.0), (u - 128.0) * (255.0 / 224.0), (v - 128.0) * (255.0 / 224.0));
    let lin = |x: f32| {
        let c = (x / 255.0).clamp(0.0, 1.0);
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(y + 1.5748 * v), lin(y - 0.187324 * u - 0.468124 * v), lin(y + 1.8556 * u));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let yy = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(yy), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Pooled planes (S-CIELAB-style: σ = 1 px luma, σ = 1 chroma px).
pub fn pooled(f: &Yuv420) -> [Vec<f32>; 3] {
    let (cw, ch) = (f.w.div_ceil(2), f.h.div_ceil(2));
    let c = |p: &[u8]| p.iter().map(|v| *v as f32).collect::<Vec<f32>>();
    [gauss(&c(&f.y), f.w, f.h, 1.0), gauss(&c(&f.u), cw, ch, 1.0), gauss(&c(&f.v), cw, ch, 1.0)]
}

/// Pooled CIE76 colour-difference statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeStats {
    /// 99.9th percentile.
    pub p999: f64,
    pub max: f64,
    /// Pixels above one JND (2.3).
    pub over_jnd: u64,
}

/// Pooled ΔE76 of `test` against `refr` (same size).
pub fn de_pooled(test: &Yuv420, refr: &Yuv420) -> DeStats {
    if test.y == refr.y && test.u == refr.u && test.v == refr.v {
        return DeStats::default();
    }
    let (st, sr) = (pooled(test), pooled(refr));
    let (w, h, cw) = (refr.w, refr.h, refr.w.div_ceil(2));
    let mut hist = vec![0u64; 10001];
    let mut s = DeStats::default();
    for y in 0..h {
        for x in 0..w {
            let (i, c) = (y * w + x, (y / 2) * cw + x / 2);
            let a = lab(st[0][i], st[1][c], st[2][c]);
            let b = lab(sr[0][i], sr[1][c], sr[2][c]);
            let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() as f64;
            hist[((d * 100.0) as usize).min(10000)] += 1;
            s.max = s.max.max(d);
            s.over_jnd += (d > 2.3) as u64;
        }
    }
    s.p999 = percentile(&hist, 0.01, 0.999);
    s
}

/// Percentile of a histogram whose bins are `step` wide.
pub fn percentile(hist: &[u64], step: f64, p: f64) -> f64 {
    let total: u64 = hist.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let target = ((total as f64) * p).ceil() as u64;
    let mut acc = 0;
    for (i, c) in hist.iter().enumerate() {
        acc += c;
        if acc >= target {
            return i as f64 * step;
        }
    }
    (hist.len() - 1) as f64 * step
}

fn abs_p999(d: &[f32]) -> f64 {
    let mut hist = vec![0u64; 2561];
    for v in d {
        hist[((v.abs() * 10.0) as usize).min(2560)] += 1;
    }
    percentile(&hist, 0.1, 0.999)
}

/// Full-reference temporal stability: p99.9 of the frame-to-frame change of the
/// signed luma error (`test − reference`), pooled σ = 1 px, in 8-bit levels.
pub fn flicker(err_prev: &[f32], err_cur: &[f32], w: usize, h: usize) -> f64 {
    let d: Vec<f32> = err_prev.iter().zip(err_cur).map(|(a, b)| a - b).collect();
    if d.iter().all(|v| *v == 0.0) {
        return 0.0;
    }
    abs_p999(&gauss(&d, w, h, 1.0))
}

/// Reference-free flicker/strobing for three consecutive luma frames: p99.9 of
/// the pooled second difference `a − 2b + c`. Steady motion and fades have a
/// small second difference; flicker, strobing and one-frame pops don't.
/// Fail above 4 levels, warn above 2 (at 1080p; scale-independent).
pub fn temporal_flicker(a: &[u8], b: &[u8], c: &[u8], w: usize, h: usize) -> f64 {
    let d: Vec<f32> = a.iter().zip(b).zip(c).map(|((x, y), z)| *x as f32 - 2.0 * *y as f32 + *z as f32).collect();
    abs_p999(&gauss(&d, w, h, 1.0))
}

/// Gradient banding in a luma plane: the share of smooth 16×16 blocks (low
/// variance, no edges) whose values form flat runs with 1-level steps. 0..1;
/// above ~0.05 is visible banding on a large gradient.
pub fn banding(y: &[u8], w: usize, h: usize) -> f64 {
    let (mut smooth, mut banded) = (0u64, 0u64);
    let mut by = 0;
    while by + 16 <= h {
        let mut bx = 0;
        while bx + 16 <= w {
            let (mut lo, mut hi, mut steps, mut runs) = (255u8, 0u8, 0u32, 0u32);
            for yy in by..by + 16 {
                let row = &y[yy * w + bx..yy * w + bx + 16];
                let mut run = 1;
                for k in 0..16 {
                    lo = lo.min(row[k]);
                    hi = hi.max(row[k]);
                    if k > 0 {
                        match row[k].abs_diff(row[k - 1]) {
                            0 => run += 1,
                            1 => {
                                steps += 1;
                                runs += (run >= 4) as u32;
                                run = 1;
                            }
                            _ => run = 1,
                        }
                    }
                }
            }
            if hi - lo <= 6 && hi > lo {
                smooth += 1;
                if steps >= 8 && runs >= 6 {
                    banded += 1;
                }
            }
            bx += 16;
        }
        by += 16;
    }
    if smooth == 0 {
        0.0
    } else {
        banded as f64 / smooth as f64
    }
}

/// A pop between consecutive frames (reference-free): pooled ΔE76 p99.9 of
/// `cur` against `prev`. Use it at cuts and transitions, where a hard jump in
/// part of the frame that should be continuous shows up as a high value.
pub fn cut_pop(prev: &Yuv420, cur: &Yuv420) -> DeStats {
    de_pooled(cur, prev)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: usize, h: usize, v: u8) -> Yuv420 {
        Yuv420 { w, h, y: vec![v; w * h], u: vec![128; w.div_ceil(2) * h.div_ceil(2)], v: vec![128; w.div_ceil(2) * h.div_ceil(2)] }
    }

    #[test]
    fn identical_frames_score_perfectly() {
        let a = flat(64, 48, 100);
        assert_eq!(psnr(&a.y, &a.y), 99.0);
        assert!((ssim(&a.y, &a.y, 64, 48) - 1.0).abs() < 1e-12);
        assert_eq!(de_pooled(&a, &a).p999, 0.0);
    }

    #[test]
    fn flicker_ignores_steady_change_and_sees_alternation() {
        let (w, h) = (32, 32);
        let f = |v: u8| vec![v; w * h];
        assert!(temporal_flicker(&f(100), &f(105), &f(110), w, h) < 0.5);
        assert!(temporal_flicker(&f(100), &f(110), &f(100), w, h) > 4.0);
    }

    #[test]
    fn banding_sees_steps_not_noise() {
        let (w, h) = (256, 64);
        // A slow ramp quantised into wide flat steps.
        let ramp: Vec<u8> = (0..w * h).map(|i| (100 + (i % w + 8) / 64) as u8).collect();
        assert!(banding(&ramp, w, h) > 0.05);
        let flat = vec![100u8; w * h];
        assert_eq!(banding(&flat, w, h), 0.0);
    }
}
