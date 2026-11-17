//! Premultiplied RGBA8 → planar YUV 4:2:0, BT.709 limited range.
//!
//! The frame is composited over black (the renderer already cleared to the
//! opaque background, so premultiplied RGB *is* the composited colour).
//! Planes are padded to whole 16×16 macroblocks by edge replication, with
//! strides that are multiples of 16 — ready for the H.264 encoder.
//!
//! One portable fixed-point kernel, compiled three times (AVX2, SSE4.1,
//! baseline) and picked at runtime, so every path is bit-identical.

use vello_cpu::Pixmap;

/// Planar 4:2:0 frame. Luma is `stride × padded_height`; chroma planes are
/// `c_stride × padded_height / 2`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YuvFrame {
    /// Visible size.
    pub width: u32,
    pub height: u32,
    /// Size padded to a multiple of 16 (whole macroblocks).
    pub padded_width: u32,
    pub padded_height: u32,
    pub y_stride: usize,
    pub c_stride: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

impl YuvFrame {
    pub fn new(width: u32, height: u32) -> Self {
        let mut f = Self::default();
        f.resize(width, height);
        f
    }

    /// Resize (keeps allocations when shrinking or repeating the same size).
    pub fn resize(&mut self, width: u32, height: u32) {
        let pw = width.div_ceil(16) * 16;
        let ph = height.div_ceil(16) * 16;
        self.width = width;
        self.height = height;
        self.padded_width = pw;
        self.padded_height = ph;
        self.y_stride = pw as usize;
        self.c_stride = (pw as usize / 2).div_ceil(16) * 16;
        self.y.resize(self.y_stride * ph as usize, 0);
        self.u.resize(self.c_stride * ph as usize / 2, 128);
        self.v.resize(self.c_stride * ph as usize / 2, 128);
    }

    /// Macroblock grid size.
    pub fn mb_dims(&self) -> (usize, usize) {
        (self.padded_width as usize / 16, self.padded_height as usize / 16)
    }
}

// Luma: Y = 16 + (cr·R + cg·G + cb·B) / 2^15, coefficients × 219/255.
const YR: i32 = 5983; // 0.2126 · 219/255 · 32768
const YG: i32 = 20127; // 0.7152 · …
const YB: i32 = 2032; // 0.0722 · …
// Chroma from 2×2 sums (×4): coefficients × 224/255 / 4, scaled 2^15.
const UR: i32 = -824; // -0.114572 · 224/255 · 32768 / 4
const UG: i32 = -2774; // -0.385428 · …
const UB: i32 = 3598; // 0.5 · …
const VR: i32 = 3598;
const VG: i32 = -3268; // -0.454153 · …
const VB: i32 = -330; // -0.045847 · …

#[inline(always)]
fn luma(p: &[u8]) -> u8 {
    let s = YR * p[0] as i32 + YG * p[1] as i32 + YB * p[2] as i32;
    ((s + (16 << 15) + (1 << 14)) >> 15) as u8
}

#[inline(always)]
fn chroma(r: i32, g: i32, b: i32) -> (u8, u8) {
    (
        ((UR * r + UG * g + UB * b + (128 << 15) + (1 << 14)) >> 15) as u8,
        ((VR * r + VG * g + VB * b + (128 << 15) + (1 << 14)) >> 15) as u8,
    )
}

#[allow(clippy::too_many_arguments)]
/// Scalar conversion of one row pair from pixel `x0` (even) to `w`.
/// `yb` is `None` for the last row of an odd-height frame.
#[inline(always)]
fn rows_scalar(r0: &[u8], r1: &[u8], ya: &mut [u8], yb: Option<&mut [u8]>, u: &mut [u8], v: &mut [u8], x0: usize, w: usize) {
    for x in x0..w {
        ya[x] = luma(&r0[x * 4..]);
    }
    if let Some(yb) = yb {
        for x in x0..w {
            yb[x] = luma(&r1[x * 4..]);
        }
    }
    let mut x = x0;
    while x < w {
        let x1 = (x + 1).min(w - 1);
        let (a0, a1, b0, b1) = (&r0[x * 4..], &r0[x1 * 4..], &r1[x * 4..], &r1[x1 * 4..]);
        let c = |k: usize| a0[k] as i32 + a1[k] as i32 + b0[k] as i32 + b1[k] as i32;
        let (cu, cv) = chroma(c(0), c(1), c(2));
        u[x / 2] = cu;
        v[x / 2] = cv;
        x += 2;
    }
}

type Region = [usize; 4];

#[inline(always)]
fn kernel_with(
    src: &[u8],
    w: usize,
    h: usize,
    out: &mut YuvFrame,
    [x0, y0, x1, y1]: Region,
    (dx, dy): (usize, usize),
    simd: impl Fn(&[u8], &[u8], &mut [u8], &mut [u8], &mut [u8], &mut [u8]) -> usize,
) {
    let ys = out.y_stride;
    let cs = out.c_stride;
    let n = x1 - x0;
    for cy in y0 / 2..y1.div_ceil(2) {
        let y = cy * 2;
        let r0 = &src[(y * w + x0) * 4..(y * w + x1) * 4];
        let (oc, ox) = (cy + dy / 2, x0 + dx);
        let u = &mut out.u[oc * cs + ox / 2..oc * cs + (ox + n).div_ceil(2)];
        let v = &mut out.v[oc * cs + ox / 2..oc * cs + (ox + n).div_ceil(2)];
        let (ya, rest) = out.y[(y + dy) * ys..].split_at_mut(ys);
        let ya = &mut ya[ox..ox + n];
        if y + 1 < h {
            let r1 = &src[((y + 1) * w + x0) * 4..((y + 1) * w + x1) * 4];
            let yb = &mut rest[ox..ox + n];
            let done = simd(r0, r1, ya, yb, u, v);
            rows_scalar(r0, r1, ya, Some(yb), u, v, done, n);
        } else {
            rows_scalar(r0, r0, ya, None, u, v, 0, n);
        }
    }
}

fn kernel(src: &[u8], w: usize, h: usize, out: &mut YuvFrame, r: Region, off: (usize, usize)) {
    kernel_with(src, w, h, out, r, off, |_, _, _, _, _, _| 0)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn kernel_avx2(src: &[u8], w: usize, h: usize, out: &mut YuvFrame, r: Region, off: (usize, usize)) {
    kernel_with(src, w, h, out, r, off, |r0, r1, ya, yb, u, v| unsafe { avx2::rows(r0, r1, ya, yb, u, v) })
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn kernel_sse41(src: &[u8], w: usize, h: usize, out: &mut YuvFrame, r: Region, off: (usize, usize)) {
    kernel(src, w, h, out, r, off)
}

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use super::*;
    use std::arch::x86_64::*;

    /// Convert 16-pixel groups of a row pair; returns the number of pixels done.
    /// Bit-identical to the scalar path (exact i32 `madd` arithmetic).
    #[target_feature(enable = "avx2")]
    pub unsafe fn rows(r0: &[u8], r1: &[u8], ya: &mut [u8], yb: &mut [u8], u: &mut [u8], v: &mut [u8]) -> usize {
        let w = r0.len() / 4;
        let n = w & !15;
        let cy = _mm256_setr_epi16(
            YR as i16, YG as i16, YB as i16, 0, YR as i16, YG as i16, YB as i16, 0,
            YR as i16, YG as i16, YB as i16, 0, YR as i16, YG as i16, YB as i16, 0,
        );
        let cu = _mm256_setr_epi16(
            UR as i16, UG as i16, UB as i16, 0, UR as i16, UG as i16, UB as i16, 0,
            UR as i16, UG as i16, UB as i16, 0, UR as i16, UG as i16, UB as i16, 0,
        );
        let cv = _mm256_setr_epi16(
            VR as i16, VG as i16, VB as i16, 0, VR as i16, VG as i16, VB as i16, 0,
            VR as i16, VG as i16, VB as i16, 0, VR as i16, VG as i16, VB as i16, 0,
        );
        let ky = _mm256_set1_epi32((16 << 15) + (1 << 14));
        let kc = _mm256_set1_epi32((128 << 15) + (1 << 14));
        let order = _mm256_setr_epi32(0, 4, 1, 5, 2, 6, 3, 7);
        let fix = _mm_setr_epi8(0, 1, 8, 9, 2, 3, 10, 11, 4, 5, 12, 13, 6, 7, 14, 15);
        let mut x = 0;
        while x < n {
            let p0 = r0.as_ptr().add(x * 4);
            let p1 = r1.as_ptr().add(x * 4);
            let mut a = [_mm256_setzero_si256(); 4];
            let mut b = [_mm256_setzero_si256(); 4];
            for k in 0..4 {
                a[k] = _mm256_cvtepu8_epi16(_mm_loadu_si128(p0.add(k * 16) as *const __m128i));
                b[k] = _mm256_cvtepu8_epi16(_mm_loadu_si128(p1.add(k * 16) as *const __m128i));
            }
            let luma16 = |g: &[__m256i; 4]| {
                let m: [__m256i; 4] = [
                    _mm256_madd_epi16(g[0], cy),
                    _mm256_madd_epi16(g[1], cy),
                    _mm256_madd_epi16(g[2], cy),
                    _mm256_madd_epi16(g[3], cy),
                ];
                // [p0 p1 p4 p5 | p2 p3 p6 p7], [p8 p9 p12 p13 | p10 p11 p14 p15]
                let h01 = _mm256_srai_epi32::<15>(_mm256_add_epi32(_mm256_hadd_epi32(m[0], m[1]), ky));
                let h23 = _mm256_srai_epi32::<15>(_mm256_add_epi32(_mm256_hadd_epi32(m[2], m[3]), ky));
                let w16 = _mm256_packs_epi32(h01, h23); // [p0 p1 p4 p5 p8 p9 p12 p13 | p2 p3 p6 p7 p10 p11 p14 p15]
                let b8 = _mm256_packus_epi16(w16, w16);
                let q = _mm256_permute4x64_epi64::<0b1000>(b8); // low 128: lane0 low 8 bytes, lane1 low 8 bytes
                _mm_shuffle_epi8(_mm256_castsi256_si128(q), fix)
            };
            _mm_storeu_si128(ya.as_mut_ptr().add(x) as *mut __m128i, luma16(&a));
            _mm_storeu_si128(yb.as_mut_ptr().add(x) as *mut __m128i, luma16(&b));
            let s = [
                _mm256_add_epi16(a[0], b[0]),
                _mm256_add_epi16(a[1], b[1]),
                _mm256_add_epi16(a[2], b[2]),
                _mm256_add_epi16(a[3], b[3]),
            ];
            let chroma8 = |c: __m256i, k: __m256i| {
                let m0 = _mm256_madd_epi16(s[0], c);
                let m1 = _mm256_madd_epi16(s[1], c);
                let m2 = _mm256_madd_epi16(s[2], c);
                let m3 = _mm256_madd_epi16(s[3], c);
                let h01 = _mm256_hadd_epi32(m0, m1);
                let h23 = _mm256_hadd_epi32(m2, m3);
                // [c0 c2 c4 c6 | c1 c3 c5 c7]
                let hh = _mm256_hadd_epi32(h01, h23);
                let r = _mm256_srai_epi32::<15>(_mm256_add_epi32(hh, k));
                _mm256_permutevar8x32_epi32(r, order)
            };
            let cuv = _mm256_packs_epi32(chroma8(cu, kc), chroma8(cv, kc)); // [u0..3 v0..3 | u4..7 v4..7]
            let cuv = _mm256_permute4x64_epi64::<0b11011000>(cuv); // [u0..7 | v0..7]
            let b8 = _mm256_packus_epi16(cuv, cuv);
            _mm_storel_epi64(u.as_mut_ptr().add(x / 2) as *mut __m128i, _mm256_castsi256_si128(b8));
            _mm_storel_epi64(v.as_mut_ptr().add(x / 2) as *mut __m128i, _mm256_extracti128_si256::<1>(b8));
            x += 16;
        }
        n
    }
}

/// Replicate the right and bottom edges into the macroblock padding.
pub(crate) fn pad(out: &mut YuvFrame) {
    let (w, h) = (out.width as usize, out.height as usize);
    let (pw, ph) = (out.padded_width as usize, out.padded_height as usize);
    let pad_plane = |p: &mut [u8], stride: usize, w: usize, h: usize, pw: usize, ph: usize| {
        if w < pw {
            for y in 0..h {
                let row = &mut p[y * stride..y * stride + pw];
                let e = row[w - 1];
                row[w..].fill(e);
            }
        }
        for y in h..ph {
            p.copy_within((h - 1) * stride..h * stride, y * stride);
        }
    };
    pad_plane(&mut out.y, out.y_stride, w, h, pw, ph);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    pad_plane(&mut out.u, out.c_stride, cw, ch, pw / 2, ph / 2);
    pad_plane(&mut out.v, out.c_stride, cw, ch, pw / 2, ph / 2);
}

/// Which kernel `rgba_to_yuv420` uses on this CPU.
pub fn simd_level() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return "avx2";
        }
        if std::arch::is_x86_feature_detected!("sse4.1") {
            return "sse4.1";
        }
    }
    "scalar"
}

/// Convert premultiplied RGBA8 to YUV 4:2:0 (BT.709, limited range).
pub fn rgba_to_yuv420(src: &Pixmap, out: &mut YuvFrame) {
    let (w, h) = (src.width() as usize, src.height() as usize);
    convert_rgba(src.data_as_u8_slice(), w, h, out)
}

/// As [`rgba_to_yuv420`], from a raw `w × h × 4` premultiplied buffer.
pub fn convert_rgba(src: &[u8], w: usize, h: usize, out: &mut YuvFrame) {
    assert!(src.len() >= w * h * 4 && w > 0 && h > 0);
    out.resize(w as u32, h as u32);
    convert_region(src, w, h, [0, 0, w, h], out);
}

/// Convert only `[x0, y0, x1, y1)` of a `w × h` frame into `out` (already
/// sized for `w × h`); `x0`, `y0` must be even. Re-pads edges it touches.
pub fn convert_region(src: &[u8], w: usize, h: usize, r: Region, out: &mut YuvFrame) {
    assert!(src.len() >= w * h * 4 && out.width as usize == w && out.height as usize == h);
    let r = [r[0] & !1, r[1] & !1, r[2].min(w), r[3].min(h)];
    if r[0] >= r[2] || r[1] >= r[3] {
        return;
    }
    convert_into(src, w, h, r, out, (0, 0));
    if r[2] >= w || r[3] >= h {
        pad(out);
    }
}

/// Convert region `r` of a `w × h` source buffer into `out` at `r` shifted by
/// `off` (even). The caller keeps everything inside `out`'s visible size and
/// pads afterwards (see [`pad`]).
pub(crate) fn convert_into(src: &[u8], w: usize, h: usize, r: Region, out: &mut YuvFrame, off: (usize, usize)) {
    debug_assert!([off.0, off.1, r[0], r[1]].iter().all(|v| v.is_multiple_of(2)));
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: feature checked above.
            unsafe { kernel_avx2(src, w, h, out, r, off) };
        } else if std::arch::is_x86_feature_detected!("sse4.1") {
            // SAFETY: feature checked above.
            unsafe { kernel_sse41(src, w, h, out, r, off) };
        } else {
            kernel(src, w, h, out, r, off);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    kernel(src, w, h, out, r, off);
}

/// Premultiplied RGBA8 (composited over black) → full-resolution Y, Cb, Cr
/// planes (4:4:4), same fixed-point maths as the 4:2:0 kernel.
pub(crate) fn rgba_to_yuv444(src: &[u8], y: &mut Vec<u8>, u: &mut Vec<u8>, v: &mut Vec<u8>) {
    let n = src.len() / 4;
    y.clear();
    u.clear();
    v.clear();
    y.reserve(n);
    u.reserve(n);
    v.reserve(n);
    for p in src.chunks_exact(4) {
        y.push(luma(p));
        let (cu, cv) = chroma(4 * p[0] as i32, 4 * p[1] as i32, 4 * p[2] as i32);
        u.push(cu);
        v.push(cv);
    }
}

/// Composite a premultiplied RGBA8 box (`rw × rh`) source-over onto `out` at
/// (`x0`, `y0`) (even), in YUV space: conversion is affine, so
/// `Y = (Yb − 16)(1 − α) + 16 + luma(src)` (chroma likewise, per 2×2 block).
/// Fully transparent 16-pixel spans are skipped. Opaque pixels give exactly the
/// 4:2:0 kernel's values; transparent ones leave `out` untouched.
pub(crate) fn composite_over(src: &[u8], rw: usize, rh: usize, (x0, y0): (usize, usize), out: &mut YuvFrame) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: feature checked above.
        return unsafe { composite_over_avx2(src, rw, rh, (x0, y0), out) };
    }
    composite_over_inner(src, rw, rh, (x0, y0), out)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn composite_over_avx2(src: &[u8], rw: usize, rh: usize, o: (usize, usize), out: &mut YuvFrame) {
    composite_over_inner(src, rw, rh, o, out)
}

/// `v / 255`, rounded, for `v` in `0..=65535`.
#[inline(always)]
fn div255(v: i32) -> i32 {
    let v = v + 128;
    (v + (v >> 8)) >> 8
}

#[inline(always)]
fn composite_over_inner(src: &[u8], rw: usize, rh: usize, (x0, y0): (usize, usize), out: &mut YuvFrame) {
    let (ys, cs) = (out.y_stride, out.c_stride);
    let row = rw * 4;
    // Luma.
    for y in 0..rh {
        let s = &src[y * row..(y + 1) * row];
        let d = &mut out.y[(y0 + y) * ys + x0..(y0 + y) * ys + x0 + rw];
        for (sc, dc) in s.chunks(64).zip(d.chunks_mut(16)) {
            if sc.chunks_exact(4).all(|p| p[3] == 0) {
                continue;
            }
            for (p, yb) in sc.chunks_exact(4).zip(dc.iter_mut()) {
                let a = p[3] as i32;
                let l = (YR * p[0] as i32 + YG * p[1] as i32 + YB * p[2] as i32 + (1 << 14)) >> 15;
                let bg = *yb as i32 - 16;
                *yb = (16 + l + div255(bg.max(0) * (255 - a)) - div255((-bg).max(0) * (255 - a))) as u8;
            }
        }
    }
    // Chroma, per 2×2 block (edge blocks duplicate the last row/column).
    let (cw, ch) = (rw.div_ceil(2), rh.div_ceil(2));
    for cy in 0..ch {
        let (ra, rb) = (cy * 2, (cy * 2 + 1).min(rh - 1));
        let (sa, sb) = (&src[ra * row..(ra + 1) * row], &src[rb * row..(rb + 1) * row]);
        let o = (y0 / 2 + cy) * cs + x0 / 2;
        // Skip 8-block (16-pixel) spans that are transparent in both rows.
        let clear = |x: usize, n: usize| {
            let (a, b) = (&sa[x * 4..(x + n) * 4], &sb[x * 4..(x + n) * 4]);
            a.chunks_exact(4).zip(b.chunks_exact(4)).all(|(p, q)| p[3] | q[3] == 0)
        };
        for span in (0..cw).step_by(8) {
            if clear(span * 2, 16.min(rw - span * 2)) {
                continue;
            }
            for cx in span..(span + 8).min(cw) {
                let (xa, xb) = (cx * 2 * 4, ((cx * 2 + 1).min(rw - 1)) * 4);
                let at = |k: usize| sa[xa + k] as i32 + sa[xb + k] as i32 + sb[xa + k] as i32 + sb[xb + k] as i32;
                let asum = at(3);
                if asum == 0 {
                    continue;
                }
                let (r, g, b) = (at(0), at(1), at(2));
                let cu = (UR * r + UG * g + UB * b + (1 << 14)) >> 15;
                let cv = (VR * r + VG * g + VB * b + (1 << 14)) >> 15;
                let keep = 1020 - asum;
                let mix = |c: u8, s: i32| {
                    let d = c as i32 - 128;
                    // d · keep / 1020, rounded half away from zero.
                    let m = (d.abs() * keep + 510) / 1020;
                    (128 + s + if d < 0 { -m } else { m }).clamp(0, 255) as u8
                };
                out.u[o + cx] = mix(out.u[o + cx], cu);
                out.v[o + cx] = mix(out.v[o + cx], cv);
            }
        }
    }
}

/// Scalar reference (floating point), for tests.
pub fn reference_pixel(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let y = 16.0 + 219.0 / 255.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b);
    let u = 128.0 + 224.0 / 255.0 * (-0.114572 * r - 0.385428 * g + 0.5 * b);
    let v = 128.0 + 224.0 / 255.0 * (0.5 * r - 0.454153 * g - 0.045847 * b);
    (y, u, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_float_reference() {
        let (w, h) = (37, 21);
        let mut src = vec![0u8; w * h * 4];
        let mut seed = 12345u32;
        for p in src.chunks_exact_mut(4) {
            for c in p.iter_mut().take(3) {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *c = (seed >> 24) as u8;
            }
            p[3] = 255;
        }
        let mut f = YuvFrame::default();
        convert_rgba(&src, w, h, &mut f);
        assert_eq!((f.padded_width, f.padded_height), (48, 32));
        assert_eq!(f.y_stride % 16, 0);
        assert_eq!(f.c_stride % 16, 0);
        let px = |x: usize, y: usize| {
            let p = &src[(y * w + x) * 4..];
            (p[0] as f64, p[1] as f64, p[2] as f64)
        };
        let mut max_err = 0f64;
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = px(x, y);
                let (ry, _, _) = reference_pixel(r, g, b);
                max_err = max_err.max((f.y[y * f.y_stride + x] as f64 - ry).abs());
            }
        }
        assert!(max_err <= 0.51, "luma err {max_err}");
        let mut max_c = 0f64;
        for cy in 0..h.div_ceil(2) {
            for cx in 0..w.div_ceil(2) {
                let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (x, y) = ((cx * 2 + dx).min(w - 1), (cy * 2 + dy).min(h - 1));
                    let p = px(x, y);
                    r += p.0 / 4.0;
                    g += p.1 / 4.0;
                    b += p.2 / 4.0;
                }
                let (_, ru, rv) = reference_pixel(r, g, b);
                max_c = max_c.max((f.u[cy * f.c_stride + cx] as f64 - ru).abs());
                max_c = max_c.max((f.v[cy * f.c_stride + cx] as f64 - rv).abs());
            }
        }
        assert!(max_c <= 0.51, "chroma err {max_c}");
        // Known values: white → (235, 128, 128), black → (16, 128, 128).
        let mut f2 = YuvFrame::default();
        convert_rgba(&[255, 255, 255, 255, 0, 0, 0, 255], 2, 1, &mut f2);
        assert_eq!(f2.y[0], 235);
        assert_eq!(f2.y[1], 16);
        // Padding replicates edges.
        assert_eq!(f2.y[15], 16);
        assert_eq!(f2.y[f2.y_stride * 15], 235);
    }

    #[test]
    fn all_kernels_agree() {
        let (w, h) = (64, 18);
        let src: Vec<u8> = (0..w * h * 4).map(|i| (i * 37 % 251) as u8).collect();
        let mut a = YuvFrame::default();
        convert_rgba(&src, w, h, &mut a);
        let mut b = YuvFrame::new(w as u32, h as u32);
        kernel(&src, w, h, &mut b, [0, 0, w, h], (0, 0));
        pad(&mut b);
        assert_eq!(a, b);
        // Region conversion composes to the same frame.
        let mut c = YuvFrame::new(w as u32, h as u32);
        convert_region(&src, w, h, [0, 0, w, 8], &mut c);
        convert_region(&src, w, h, [0, 8, 32, h], &mut c);
        convert_region(&src, w, h, [32, 8, w, h], &mut c);
        assert_eq!(a, c);
    }
}
