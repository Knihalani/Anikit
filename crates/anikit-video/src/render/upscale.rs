//! Fast bilinear upscale of a (low-res) premultiplied image straight into a
//! frame buffer: used for full-frame backgrounds (mesh / aurora fields,
//! photos) so vello neither clears nor samples them.
//!
//! Separable: each needed source row is interpolated horizontally once into a
//! 16-bit row (value · 256), then every output row is a vertical lerp of two
//! such rows — a straight, vectorised loop over bytes.

use kurbo::Rect;

#[derive(Default, Debug)]
pub struct Upscaler {
    /// Use the smooth, dithered cubic filter instead of bilinear.
    pub cubic: bool,
    /// Cubic kernel: `None` picks from the scale (B-spline for ≥ 12× enlargement,
    /// Mitchell otherwise); `Some(true)` forces B-spline, `Some(false)` Mitchell.
    pub bspline: Option<bool>,
    hrows: Vec<i32>,
    /// Rounding offsets (dither + ½) for the 64 dither rows, spread over channels.
    zrows: Vec<i32>,
    xtab: Vec<(u32, u32, u32)>,
    rows: [Vec<u8>; 2],
    row_ids: [usize; 2],
    /// What `xtab` and the cached rows were built for.
    key: Option<(usize, usize, usize, [u64; 4], usize, i64)>,
    /// Column table as structure of arrays (source index, weight) for the gather path.
    gx: (Vec<i32>, Vec<i32>),
}

/// One source row of a single-channel plane, interpolated to `r.len()` columns
/// with AVX2 gathers (8 columns per step). `s` must extend ≥ 4 bytes past the
/// highest index read.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn gather_row(s: &[u8], (idx, wts): &(Vec<i32>, Vec<i32>), r: &mut [u8]) {
    use std::arch::x86_64::*;
    let n = r.len();
    let mask = _mm256_set1_epi32(0xff);
    let k256 = _mm256_set1_epi32(256);
    let k128 = _mm256_set1_epi32(128);
    let mut x = 0;
    while x + 8 <= n {
        let i = _mm256_loadu_si256(idx.as_ptr().add(x) as *const __m256i);
        let w = _mm256_loadu_si256(wts.as_ptr().add(x) as *const __m256i);
        // 4 bytes from each index: byte 0 is a, byte 1 is b (= s[i + 1]).
        let g = _mm256_i32gather_epi32::<1>(s.as_ptr() as *const i32, i);
        let a = _mm256_and_si256(g, mask);
        let b = _mm256_and_si256(_mm256_srli_epi32::<8>(g), mask);
        let v = _mm256_add_epi32(_mm256_add_epi32(_mm256_mullo_epi32(a, _mm256_sub_epi32(k256, w)), _mm256_mullo_epi32(b, w)), k128);
        let v = _mm256_srli_epi32::<8>(v);
        // 8 × i32 → 8 × u8.
        let p16 = _mm256_packus_epi32(v, v); // [v0..3 v0..3 | v4..7 v4..7]
        let p8 = _mm256_packus_epi16(p16, p16);
        let lo = _mm256_castsi256_si128(p8);
        let hi = _mm256_extracti128_si256::<1>(p8);
        let out = _mm_unpacklo_epi32(lo, hi);
        _mm_storel_epi64(r.as_mut_ptr().add(x) as *mut __m128i, out);
        x += 8;
    }
    while x < n {
        let (i, w) = (idx[x] as usize, wts[x] as u32);
        let (a, b) = (s[i] as u32, s[i + 1] as u32);
        r[x] = ((a * (256 - w) + b * w + 128) >> 8) as u8;
        x += 1;
    }
}

impl Upscaler {
    /// A smooth (cubic B-spline) upscaler.
    pub fn cubic() -> Self {
        Self { cubic: true, ..Default::default() }
    }

    /// Forget cached rows (the source pixels changed in place).
    pub fn invalidate(&mut self) {
        self.key = None;
    }

    /// Write `src` (`sw × sh` premultiplied RGBA8), placed at device rect `dev`,
    /// into `out` (`ow × oh` pixels, tightly packed), whose pixel (0,0) is
    /// frame pixel (`ox`, `oy`). Samples are clamped at the image edges.
    #[allow(clippy::too_many_arguments)]
    pub fn run(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], ow: usize, oh: usize, ox: i64, oy: i64) {
        self.run_c::<4>(src, sw, sh, dev, out, ow * 4, ow, oh, ox, oy)
    }

    /// As [`Self::run`] for a single 8-bit plane, with output row stride `stride` bytes.
    #[allow(clippy::too_many_arguments)]
    pub fn run_plane(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], stride: usize, ow: usize, oh: usize, ox: i64, oy: i64) {
        self.run_c::<1>(src, sw, sh, dev, out, stride, ow, oh, ox, oy)
    }

    #[allow(clippy::too_many_arguments)]
    fn run_c<const C: usize>(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], stride: usize, ow: usize, oh: usize, ox: i64, oy: i64) {
        // 1:1 at an integer offset: a plain copy (any filter would only blur).
        if dev.width() == sw as f64 && dev.height() == sh as f64 && dev.x0.fract() == 0.0 && dev.y0.fract() == 0.0 {
            let (bx, by) = (ox - dev.x0 as i64, oy - dev.y0 as i64);
            if bx >= 0 && by >= 0 && bx as usize + ow <= sw && by as usize + oh <= sh {
                for y in 0..oh {
                    let s = ((by as usize + y) * sw + bx as usize) * C;
                    out[y * stride..y * stride + ow * C].copy_from_slice(&src[s..s + ow * C]);
                }
                return;
            }
        }
        if self.cubic {
            return self.run_cubic::<C>(src, sw, sh, dev, out, stride, ow, oh, ox, oy);
        }
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: feature checked above.
            return unsafe { self.run_avx2::<C>(src, sw, sh, dev, out, stride, ow, oh, ox, oy) };
        }
        self.run_inner::<C>(src, sw, sh, dev, out, stride, ow, oh, ox, oy, false)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    #[allow(clippy::too_many_arguments)]
    unsafe fn run_avx2<const C: usize>(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], stride: usize, ow: usize, oh: usize, ox: i64, oy: i64) {
        self.run_inner::<C>(src, sw, sh, dev, out, stride, ow, oh, ox, oy, true)
    }

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn run_inner<const C: usize>(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], stride: usize, ow: usize, oh: usize, ox: i64, oy: i64, gather: bool) {
        let (sx, sy) = (sw as f64 / dev.width(), sh as f64 / dev.height());
        let lerp_tab = |o: f64, s: f64, n: usize| {
            let u = (o + 0.5) * s - 0.5;
            let u = u.clamp(0.0, (n - 1) as f64);
            let i0 = u.floor() as u32;
            let i1 = (i0 + 1).min(n as u32 - 1);
            let w = ((u - i0 as f64) * 256.0).round() as u32;
            (i0, i1, w)
        };
        // Repeated calls on the same image and columns (row streaming) keep
        // the column table and the cached source rows.
        let key = (src.as_ptr() as usize ^ C, sw, sh, [dev.x0, dev.y0, dev.x1, dev.y1].map(f64::to_bits), ow, ox);
        if self.key != Some(key) {
            self.xtab.clear();
            self.xtab.extend((0..ow).map(|x| lerp_tab((ox + x as i64) as f64 - dev.x0, sx, sw)));
            self.gx.0.clear();
            self.gx.1.clear();
            self.gx.0.extend(self.xtab.iter().map(|t| t.0 as i32));
            // Weight in the low half, (256 - w) in the high half; i1 = i0 + (w > 0) is implied.
            self.gx.1.extend(self.xtab.iter().map(|t| if t.1 == t.0 { 0 } else { t.2 as i32 }));
            for r in &mut self.rows {
                r.resize(ow * C, 0);
            }
            self.row_ids = [usize::MAX; 2];
            self.key = Some(key);
        }
        let srow = sw * C;
        for y in 0..oh {
            let (j0, j1, wy) = lerp_tab((oy + y as i64) as f64 - dev.y0, sy, sh);
            let (j0, j1) = (j0 as usize, j1 as usize);
            // Moving down one source row: the old second row becomes the first.
            if self.row_ids[0] != j0 && self.row_ids[1] == j0 {
                self.rows.swap(0, 1);
                self.row_ids.swap(0, 1);
            }
            for (slot, j) in [(0usize, j0), (1, j1)] {
                if self.row_ids[slot] == j {
                    continue;
                }
                let s = &src[j * srow..(j + 1) * srow];
                let r = &mut self.rows[slot];
                debug_assert!(r.len() >= self.xtab.len() * C);
                #[cfg(target_arch = "x86_64")]
                if C == 1 && gather && src.len() >= (j + 1) * srow + 4 {
                    // SAFETY: AVX2 checked by the caller (`gather`); reads stay
                    // within `src` (≥ 4 bytes past this row, asserted above).
                    unsafe { gather_row(&src[j * srow..], &self.gx, r) };
                    self.row_ids[slot] = j;
                    continue;
                }
                let (sp, rp) = (s.as_ptr(), r.as_mut_ptr());
                for (x, &(i0, i1, w)) in self.xtab.iter().enumerate() {
                    // SAFETY: i0, i1 < sw (clamped in `lerp_tab`); x < ow and r.len() == ow * C.
                    unsafe {
                        let a = std::ptr::read_unaligned(sp.add(i0 as usize * C) as *const [u8; C]);
                        let b = std::ptr::read_unaligned(sp.add(i1 as usize * C) as *const [u8; C]);
                        let o: [u8; C] = std::array::from_fn(|c| ((a[c] as u32 * (256 - w) + b[c] as u32 * w + 128) >> 8) as u8);
                        std::ptr::write_unaligned(rp.add(x * C) as *mut [u8; C], o);
                    }
                }
                self.row_ids[slot] = j;
            }
            let (a, b) = (&self.rows[0][..ow * C], &self.rows[1][..ow * C]);
            let d = &mut out[y * stride..y * stride + ow * C];
            if wy == 0 {
                d.copy_from_slice(a);
            } else {
                let (iw, wy) = ((256 - wy) as u16, wy as u16);
                for ((d, &a), &b) in d.iter_mut().zip(a.iter()).zip(b.iter()) {
                    *d = ((a as u16 * iw + b as u16 * wy + 128) >> 8) as u8;
                }
            }
        }
    }
}

/// Mitchell–Netravali (B = C = 1/3) weights for fractional offset `t`, taps at −1, 0, 1, 2:
/// sharp enough for photos and screenshots, without visible ringing.
#[inline(always)]
fn mitchell(t: f32) -> [f32; 4] {
    let k = |x: f32| -> f32 {
        let x = x.abs();
        let (b, c) = (1.0 / 3.0, 1.0 / 3.0);
        if x < 1.0 {
            ((12.0 - 9.0 * b - 6.0 * c) * x * x * x + (-18.0 + 12.0 * b + 6.0 * c) * x * x + (6.0 - 2.0 * b)) / 6.0
        } else if x < 2.0 {
            ((-b - 6.0 * c) * x * x * x + (6.0 * b + 30.0 * c) * x * x + (-12.0 * b - 48.0 * c) * x + (8.0 * b + 24.0 * c)) / 6.0
        } else {
            0.0
        }
    };
    [k(t + 1.0), k(t), k(1.0 - t), k(2.0 - t)]
}

/// Cubic B-spline weights (sum 1) for fractional offset `t`, taps at −1, 0, 1, 2.
#[inline(always)]
fn bspline(t: f32) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [(1.0 - t).powi(3) / 6.0, (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0, (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0, t3 / 6.0]
}

impl Upscaler {
    /// Smooth upscale: separable cubic B-spline (C²-continuous, no ringing),
    /// so low-res fields show no bilinear creases, plus a fixed per-pixel
    /// triangular dither when rounding to 8 bits so smooth ramps don't band.
    #[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
    fn run_cubic<const C: usize>(&mut self, src: &[u8], sw: usize, sh: usize, dev: Rect, out: &mut [u8], stride: usize, ow: usize, oh: usize, ox: i64, oy: i64) {
        let (sx, sy) = (sw as f64 / dev.width(), sh as f64 / dev.height());
        let clampi = |i: i64, n: usize| i.clamp(0, n as i64 - 1) as usize;
        // Very low-res sources (fields, ≥ 12× enlargement) get the smoothest
        // filter; ordinary images the sharper Mitchell kernel.
        let smooth = self.bspline.unwrap_or(sx <= 1.0 / 12.0 && sy <= 1.0 / 12.0);
        let kernel: fn(f32) -> [f32; 4] = if smooth { bspline } else { mitchell };
        // Column taps: 4 source indices and weights (fixed point, sum 4096).
        let cols: Vec<([usize; 4], [i32; 4])> = (0..ow)
            .map(|x| {
                let u = ((ox + x as i64) as f64 + 0.5 - dev.x0) * sx - 0.5;
                let i = u.floor();
                let w = kernel((u - i) as f32);
                let i = i as i64;
                let idx = [clampi(i - 1, sw), clampi(i, sw), clampi(i + 1, sw), clampi(i + 2, sw)];
                let mut wi = w.map(|v| (v * 4096.0).round() as i32);
                wi[1] += 4096 - wi.iter().sum::<i32>();
                (idx, wi)
            })
            .collect();
        // Bands of output rows, each filtering only the source rows it needs
        // (horizontally, into a band-local buffer that stays in cache), then
        // the vertical pass: fixed point (rows ×4096, weights ×256 → ×2^20),
        // with a tiled triangular dither (±0.5 LSB) in the rounding.
        let srow = sw * C;
        let row_len = ow * C;
        let dtab = dither_table();
        let x0 = ox as usize;
        let mut zrows = std::mem::take(&mut self.zrows);
        zrows.clear();
        for ty in 0..64usize {
            let dt = &dtab[ty * 64..ty * 64 + 64];
            zrows.extend((0..row_len).map(|k| if C == 4 && k % 4 == 3 { 1 << 19 } else { dt[(x0 + k / C) & 63] }));
        }
        let zrows_ref = &zrows;
        #[cfg(target_arch = "x86_64")]
        let avx2 = std::arch::is_x86_feature_detected!("avx2");
        // Output row y: its 4 source rows and fixed-point weights.
        let taps = |y: usize| -> ([usize; 4], [i32; 4]) {
            let v = ((oy + y as i64) as f64 + 0.5 - dev.y0) * sy - 0.5;
            let j = v.floor();
            let wy = kernel((v - j) as f32);
            let j = j as i64;
            let rs = [clampi(j - 1, sh), clampi(j, sh), clampi(j + 1, sh), clampi(j + 2, sh)];
            let mut wq = wy.map(|v| (v * 256.0).round() as i32);
            wq[1] += 256 - wq.iter().sum::<i32>();
            (rs, wq)
        };
        // Exact 2× (the usual ½-res field): the taps repeat every 2 outputs,
        // one source pixel apart, so the interior needs no index tables.
        let (p0, p1) = (4usize, ow.saturating_sub(4) & !1);
        let periodic = C == 1
            && p1 > p0 + 2
            && (p0..p1).all(|x| {
                let (a, b) = (&cols[x], &cols[p0 + (x - p0) % 2]);
                a.1 == b.1 && a.0[0] == b.0[0] + (x - p0) / 2 && a.0[3] == a.0[0] + 3
            });
        let hfilter = |r: usize, hrow: &mut [i32]| {
            let s = &src[r * srow..(r + 1) * srow];
            if periodic {
                let ((ia, wa), (ib, wb)) = (cols[p0], cols[p0 + 1]);
                let (ia, ib) = (ia[0], ib[0]);
                let pairs = (p1 - p0) / 2;
                let (sa, sb) = (&s[ia..ia + pairs + 3], &s[ib..ib + pairs + 3]);
                for (k, o) in hrow[p0..p1].chunks_exact_mut(2).enumerate() {
                    o[0] = wa[0] * sa[k] as i32 + wa[1] * sa[k + 1] as i32 + wa[2] * sa[k + 2] as i32 + wa[3] * sa[k + 3] as i32;
                    o[1] = wb[0] * sb[k] as i32 + wb[1] * sb[k + 1] as i32 + wb[2] * sb[k + 2] as i32 + wb[3] * sb[k + 3] as i32;
                }
                for x in (0..p0).chain(p1..ow) {
                    let (idx, wi) = &cols[x];
                    hrow[x] = wi[0] * s[idx[0]] as i32 + wi[1] * s[idx[1]] as i32 + wi[2] * s[idx[2]] as i32 + wi[3] * s[idx[3]] as i32;
                }
                return;
            }
            for (x, (idx, wi)) in cols.iter().enumerate() {
                for c in 0..C {
                    hrow[x * C + c] = wi[0] * s[idx[0] * C + c] as i32 + wi[1] * s[idx[1] * C + c] as i32 + wi[2] * s[idx[2] * C + c] as i32 + wi[3] * s[idx[3] * C + c] as i32;
                }
            }
        };
        const BAND: usize = 32;
        let band = |(b, oband): (usize, &mut [u8])| {
            let y0b = b * BAND;
            let y1b = (y0b + BAND).min(oh);
            // Source rows this band reads (monotonic in y, so a contiguous range).
            let (lo, hi) = (0..4).fold((usize::MAX, 0), |(lo, hi), k| {
                let (a, _) = taps(y0b);
                let (z, _) = taps(y1b - 1);
                (lo.min(a[k]).min(z[k]), hi.max(a[k]).max(z[k]))
            });
            HROWS.with(|cell| {
                let mut h = cell.borrow_mut();
                h.clear();
                h.resize((hi - lo + 1) * row_len, 0);
                for r in lo..=hi {
                    hfilter(r, &mut h[(r - lo) * row_len..(r - lo + 1) * row_len]);
                }
                for (k, orow) in oband.chunks_mut(stride).enumerate().take(y1b - y0b) {
                    let y = y0b + k;
                    let (rs, wq) = taps(y);
                    let row = |i: usize| &h[(rs[i] - lo) * row_len..(rs[i] - lo + 1) * row_len];
                    let yy = (oy + y as i64) as usize;
                    // The rounding offset (dither + ½) for this row, spread over channels
                    // (no dither on alpha): one of 64 rows, built once per call.
                    let z = &zrows_ref[(yy & 63) * row_len..(yy & 63) * row_len + row_len];
                    let d = &mut orow[..row_len];
                    #[cfg(target_arch = "x86_64")]
                    if avx2 {
                        // SAFETY: feature checked above.
                        unsafe { vrow_avx2::<C>([row(0), row(1), row(2), row(3)], wq, z, d) };
                        continue;
                    }
                    vrow::<C>([row(0), row(1), row(2), row(3)], wq, z, d);
                }
            });
        };
        use rayon::prelude::*;
        let bands = (oh.div_ceil(BAND)).max(1);
        out.par_chunks_mut(BAND * stride).take(bands).enumerate().for_each(band);
        let hrows = std::mem::take(&mut self.hrows);
        self.hrows = hrows;
        self.zrows = zrows;
    }
}

/// 64×64 tile of rounding offsets in 1/2^20 units: 0.5 (round) plus a
/// triangular dither of ±0.5 LSB, fixed per pixel position.
/// One output row of the vertical pass: flat element-wise loops (they vectorise;
/// a per-pixel loop over `chunks_exact(C)` didn't). `z` is the rounding offset
/// (dither + ½, ×2²⁰) per element.
#[inline(always)]
fn vrow<const C: usize>(r: [&[i32]; 4], wq: [i32; 4], z: &[i32], d: &mut [u8]) {
    let n = d.len();
    let (r0, r1, r2, r3, z) = (&r[0][..n], &r[1][..n], &r[2][..n], &r[3][..n], &z[..n]);
    for k in 0..n {
        let v = wq[0] * r0[k] + wq[1] * r1[k] + wq[2] * r2[k] + wq[3] * r3[k];
        d[k] = ((v + z[k]) >> 20).clamp(0, 255) as u8;
    }
    // Premultiplied: overshoot (Mitchell's lobes) must not exceed alpha.
    if C == 4 {
        for o in d.chunks_exact_mut(4) {
            let al = o[3];
            o[0] = o[0].min(al);
            o[1] = o[1].min(al);
            o[2] = o[2].min(al);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn vrow_avx2<const C: usize>(r: [&[i32]; 4], wq: [i32; 4], z: &[i32], d: &mut [u8]) {
    if C != 4 {
        return vrow::<C>(r, wq, z, d);
    }
    use std::arch::x86_64::*;
    let n = d.len();
    let (r0, r1, r2, r3, z) = (&r[0][..n], &r[1][..n], &r[2][..n], &r[3][..n], &z[..n]);
    for k in 0..n {
        let v = wq[0] * r0[k] + wq[1] * r1[k] + wq[2] * r2[k] + wq[3] * r3[k];
        d[k] = ((v + z[k]) >> 20).clamp(0, 255) as u8;
    }
    // min(colour, alpha) per pixel, 8 pixels at a time: broadcast each pixel's
    // alpha byte over its 4 bytes and take the byte-wise minimum (alpha stays).
    let bcast = _mm256_setr_epi8(3, 3, 3, 3, 7, 7, 7, 7, 11, 11, 11, 11, 15, 15, 15, 15, 3, 3, 3, 3, 7, 7, 7, 7, 11, 11, 11, 11, 15, 15, 15, 15);
    let mut i = 0;
    while i + 32 <= n {
        let p = _mm256_loadu_si256(d.as_ptr().add(i) as *const __m256i);
        let a = _mm256_shuffle_epi8(p, bcast);
        _mm256_storeu_si256(d.as_mut_ptr().add(i) as *mut __m256i, _mm256_min_epu8(p, a));
        i += 32;
    }
    for o in d[i..].chunks_exact_mut(4) {
        let al = o[3];
        o[0] = o[0].min(al);
        o[1] = o[1].min(al);
        o[2] = o[2].min(al);
    }
}

thread_local! {
    /// Band-local horizontally filtered rows (per thread: bands run in parallel).
    static HROWS: std::cell::RefCell<Vec<i32>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn dither_table() -> &'static [i32] {
    static T: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        (0..64 * 64)
            .map(|i| {
                let d = super::post::tri_dither((i % 64) as u32, (i / 64) as u32) * 0.5;
                ((0.5 + d) * (1 << 20) as f32) as i32
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upscale_flat_and_ramp() {
        // 2×1 image black→white stretched to 8 px: a monotone ramp with clamped ends.
        let src = [0, 0, 0, 255, 255, 255, 255, 255];
        let mut out = vec![0u8; 8 * 2 * 4];
        Upscaler::default().run(&src, 2, 1, Rect::new(0.0, 0.0, 8.0, 2.0), &mut out, 8, 2, 0, 0);
        let r: Vec<u8> = out[..32].chunks(4).map(|p| p[0]).collect();
        assert_eq!(r[0], 0);
        assert_eq!(r[1], 0);
        assert_eq!(r[7], 255);
        assert!(r.windows(2).all(|w| w[0] <= w[1]), "{r:?}");
        assert!(out.chunks(4).all(|p| p[3] == 255));
        assert_eq!(&out[..32], &out[32..]);
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    #[test]
    #[ignore]
    fn upscale_timing() {
        let (sw, sh) = (481usize, 271usize);
        let src: Vec<u8> = (0..sw * sh * 4).map(|i| (i % 251) as u8).collect();
        let mut out = vec![0u8; 1920 * 1080 * 4];
        let mut u = Upscaler::default();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            u.run(&src, sw, sh, Rect::new(0.0, 0.0, 1924.0, 1084.0), &mut out, 1920, 1080, 0, 0);
            eprintln!("upscale {:?}", t.elapsed());
        }
        let plane: Vec<u8> = (0..sw * sh + 4).map(|i| (i % 251) as u8).collect();
        let mut y = vec![0u8; 1920 * 1080];
        for _ in 0..3 {
            let t = std::time::Instant::now();
            u.run_plane(&plane, sw, sh, Rect::new(0.0, 0.0, 1924.0, 1084.0), &mut y, 1920, 1920, 1080, 0, 0);
            eprintln!("plane {:?}", t.elapsed());
        }
    }
}
