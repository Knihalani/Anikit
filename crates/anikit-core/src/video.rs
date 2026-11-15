//! Video frame processing for renderers and encoders.
//!
//! * [`FrameMixer`] accumulates sub-frames for motion blur (a film shutter):
//!   add N RGBA frames, read back their average. Integer maths, no division
//!   in the inner loop.
//! * [`rgba_to_i420`] converts RGBA/BGRA pixels to planar YUV 4:2:0
//!   (BT.709, limited range) — the format H.264/VP9 encoders consume — so an
//!   encoder pipe carries 1.5 bytes per pixel instead of 4 and skips its own
//!   colour conversion.

use alloc::vec;
use alloc::vec::Vec;

/// Averages sub-frames (motion blur).
#[derive(Debug, Default)]
pub struct FrameMixer {
    pub width: usize,
    pub height: usize,
    acc: Vec<u16>,
    /// Number of frames accumulated since the last [`FrameMixer::reset`].
    pub count: u32,
    /// The averaged frame (RGBA) after [`FrameMixer::resolve`].
    pub out: Vec<u8>,
}

impl FrameMixer {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, acc: vec![0; width * height * 4], count: 0, out: vec![0; width * height * 4] }
    }

    pub fn reset(&mut self) {
        self.acc.iter_mut().for_each(|v| *v = 0);
        self.count = 0;
    }

    /// Add one RGBA frame (`width * height * 4` bytes). Up to 257 frames.
    pub fn add(&mut self, rgba: &[u8]) {
        for (a, &p) in self.acc.iter_mut().zip(rgba) {
            *a += p as u16;
        }
        self.count += 1;
    }

    /// Average of the accumulated frames into [`FrameMixer::out`].
    pub fn resolve(&mut self) -> &[u8] {
        let n = self.count.max(1);
        // Fixed-point reciprocal: (a * inv + half) >> 16 == round(a / n).
        let inv = (65536 + n / 2) / n;
        for (o, &a) in self.out.iter_mut().zip(&self.acc) {
            *o = ((a as u32 * inv + 32768) >> 16).min(255) as u8;
        }
        &self.out
    }
}

/// Pixel byte order of the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Rgba,
    Bgra,
}

/// Bytes needed for an I420 frame of `w × h` (even dimensions).
pub fn i420_len(w: usize, h: usize) -> usize {
    w * h + 2 * ((w / 2) * (h / 2))
}

/// Convert packed RGBA/BGRA to planar I420 (Y, then U, then V), BT.709
/// limited range. `w` and `h` must be even. `out` needs [`i420_len`] bytes.
pub fn rgba_to_i420(src: &[u8], w: usize, h: usize, order: Order, out: &mut [u8]) {
    let (ri, bi) = if order == Order::Rgba { (0, 2) } else { (2, 0) };
    let (ysz, csz) = (w * h, (w / 2) * (h / 2));
    let (yp, rest) = out.split_at_mut(ysz);
    let (up, vp) = rest.split_at_mut(csz);
    // BT.709 limited-range coefficients × 256.
    const YR: i32 = 47;
    const YG: i32 = 157;
    const YB: i32 = 16;
    for y in 0..h {
        let row = &src[y * w * 4..(y + 1) * w * 4];
        let yo = &mut yp[y * w..(y + 1) * w];
        let done = luma_simd(row, yo, ri, bi);
        for (px, yv) in row[done * 4..].chunks_exact(4).zip(yo[done..].iter_mut()) {
            let (r, g, b) = (px[ri] as i32, px[1] as i32, px[bi] as i32);
            *yv = (((YR * r + YG * g + YB * b + 128) >> 8) + 16) as u8;
        }
    }
    let cw = w / 2;
    for cy in 0..h / 2 {
        let r0 = &src[(2 * cy) * w * 4..(2 * cy + 1) * w * 4];
        let r1 = &src[(2 * cy + 1) * w * 4..(2 * cy + 2) * w * 4];
        let uo = &mut up[cy * cw..(cy + 1) * cw];
        let vo = &mut vp[cy * cw..(cy + 1) * cw];
        for (((a, b), u), v) in r0.chunks_exact(8).zip(r1.chunks_exact(8)).zip(uo.iter_mut()).zip(vo.iter_mut()) {
            // Average the 2×2 block.
            let r = (a[ri] as i32 + a[4 + ri] as i32 + b[ri] as i32 + b[4 + ri] as i32 + 2) >> 2;
            let g = (a[1] as i32 + a[5] as i32 + b[1] as i32 + b[5] as i32 + 2) >> 2;
            let bl = (a[bi] as i32 + a[4 + bi] as i32 + b[bi] as i32 + b[4 + bi] as i32 + 2) >> 2;
            *u = (((-26 * r - 86 * g + 112 * bl + 128) >> 8) + 128).clamp(0, 255) as u8;
            *v = (((112 * r - 102 * g - 10 * bl + 128) >> 8) + 128).clamp(0, 255) as u8;
        }
    }
}

/// Luma for 8 pixels per step with 128-bit WebAssembly SIMD (safe intrinsics).
/// Returns how many pixels were converted; the caller finishes the tail.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn luma_simd(row: &[u8], out: &mut [u8], ri: usize, bi: usize) -> usize {
    use core::arch::wasm32::*;
    let n = out.len() / 8 * 8;
    let (sr, sb) = ((ri * 8) as u32, (bi * 8) as u32);
    let m = u32x4_splat(0xff);
    let (cr, cg, cb) = (i32x4_splat(47), i32x4_splat(157), i32x4_splat(16));
    let k = i32x4_splat(128 + (16 << 8));
    let luma4 = |v: v128| {
        let r = v128_and(u32x4_shr(v, sr), m);
        let g = v128_and(u32x4_shr(v, 8), m);
        let b = v128_and(u32x4_shr(v, sb), m);
        i32x4_shr(i32x4_add(i32x4_add(i32x4_mul(cr, r), i32x4_mul(cg, g)), i32x4_add(i32x4_mul(cb, b), k)), 8)
    };
    let mut i = 0;
    while i < n {
        let p = &row[i * 4..i * 4 + 32];
        let wd = |o: usize| u32::from_le_bytes([p[o], p[o + 1], p[o + 2], p[o + 3]]);
        let a = u32x4(wd(0), wd(4), wd(8), wd(12));
        let b = u32x4(wd(16), wd(20), wd(24), wd(28));
        let y16 = i16x8_narrow_i32x4(luma4(a), luma4(b));
        let y8 = u8x16_narrow_i16x8(y16, y16);
        out[i..i + 8].copy_from_slice(&(i64x2_extract_lane::<0>(y8) as u64).to_le_bytes());
        i += 8;
    }
    n
}
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn luma_simd(_row: &[u8], _out: &mut [u8], _ri: usize, _bi: usize) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_averages_and_rounds() {
        let mut m = FrameMixer::new(1, 1);
        m.add(&[0, 100, 255, 255]);
        m.add(&[10, 101, 255, 255]);
        m.add(&[20, 103, 0, 255]);
        assert_eq!(m.resolve(), &[10, 101, 170, 255]);
        m.reset();
        m.add(&[7, 8, 9, 10]);
        assert_eq!(m.resolve(), &[7, 8, 9, 10]);
    }

    #[test]
    fn i420_matches_bt709() {
        let (w, h) = (4, 2);
        let mut px = alloc::vec![0u8; w * h * 4];
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[255, 255, 255, 255]);
        }
        let mut out = alloc::vec![0u8; i420_len(w, h)];
        rgba_to_i420(&px, w, h, Order::Rgba, &mut out);
        assert!(out[..8].iter().all(|&y| y == 235)); // white → Y 235
        assert!(out[8..].iter().all(|&c| (127..=129).contains(&c))); // neutral chroma
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[255, 0, 0, 255]);
        }
        rgba_to_i420(&px, w, h, Order::Rgba, &mut out);
        assert_eq!(out[0], 63); // red → Y 63, V 240
        assert_eq!(out[8 + 2], 240);
        // BGRA input gives the same result for the same colour.
        for p in px.chunks_exact_mut(4) {
            p.copy_from_slice(&[0, 0, 255, 255]);
        }
        let mut out2 = alloc::vec![0u8; i420_len(w, h)];
        rgba_to_i420(&px, w, h, Order::Bgra, &mut out2);
        assert_eq!(out, out2);
    }
}
