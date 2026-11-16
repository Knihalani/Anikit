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
fn kernel(src: &[u8], w: usize, h: usize, out: &mut YuvFrame) {
    let ys = out.y_stride;
    let cs = out.c_stride;
    let cw = w.div_ceil(2);
    for cy in 0..h.div_ceil(2) {
        let y0 = cy * 2;
        let y1 = (y0 + 1).min(h - 1);
        let r0 = &src[y0 * w * 4..(y0 + 1) * w * 4];
        let r1 = &src[y1 * w * 4..(y1 + 1) * w * 4];
        {
            let (ya, yb) = out.y.split_at_mut((y0 + 1) * ys);
            let ya = &mut ya[y0 * ys..y0 * ys + w];
            for (o, p) in ya.iter_mut().zip(r0.chunks_exact(4)) {
                let s = YR * p[0] as i32 + YG * p[1] as i32 + YB * p[2] as i32;
                *o = ((s + (16 << 15) + (1 << 14)) >> 15) as u8;
            }
            if y0 + 1 < h {
                let yb = &mut yb[..w];
                for (o, p) in yb.iter_mut().zip(r1.chunks_exact(4)) {
                    let s = YR * p[0] as i32 + YG * p[1] as i32 + YB * p[2] as i32;
                    *o = ((s + (16 << 15) + (1 << 14)) >> 15) as u8;
                }
            }
        }
        let u = &mut out.u[cy * cs..cy * cs + cw];
        let v = &mut out.v[cy * cs..cy * cs + cw];
        let pairs = w / 2;
        for i in 0..pairs {
            let a = &r0[i * 8..i * 8 + 8];
            let b = &r1[i * 8..i * 8 + 8];
            let r = a[0] as i32 + a[4] as i32 + b[0] as i32 + b[4] as i32;
            let g = a[1] as i32 + a[5] as i32 + b[1] as i32 + b[5] as i32;
            let bl = a[2] as i32 + a[6] as i32 + b[2] as i32 + b[6] as i32;
            u[i] = ((UR * r + UG * g + UB * bl + (128 << 15) + (1 << 14)) >> 15) as u8;
            v[i] = ((VR * r + VG * g + VB * bl + (128 << 15) + (1 << 14)) >> 15) as u8;
        }
        if w % 2 == 1 {
            let x = w - 1;
            let a = &r0[x * 4..x * 4 + 4];
            let b = &r1[x * 4..x * 4 + 4];
            let r = 2 * (a[0] as i32 + b[0] as i32);
            let g = 2 * (a[1] as i32 + b[1] as i32);
            let bl = 2 * (a[2] as i32 + b[2] as i32);
            u[pairs] = ((UR * r + UG * g + UB * bl + (128 << 15) + (1 << 14)) >> 15) as u8;
            v[pairs] = ((VR * r + VG * g + VB * bl + (128 << 15) + (1 << 14)) >> 15) as u8;
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn kernel_avx2(src: &[u8], w: usize, h: usize, out: &mut YuvFrame) {
    kernel(src, w, h, out)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn kernel_sse41(src: &[u8], w: usize, h: usize, out: &mut YuvFrame) {
    kernel(src, w, h, out)
}

/// Replicate the right and bottom edges into the macroblock padding.
fn pad(out: &mut YuvFrame) {
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
    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: feature checked above.
            unsafe { kernel_avx2(src, w, h, out) };
            return pad(out);
        }
        if std::arch::is_x86_feature_detected!("sse4.1") {
            // SAFETY: feature checked above.
            unsafe { kernel_sse41(src, w, h, out) };
            return pad(out);
        }
    }
    kernel(src, w, h, out);
    pad(out)
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
        kernel(&src, w, h, &mut b);
        pad(&mut b);
        assert_eq!(a, b);
    }
}
