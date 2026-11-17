//! Luma sub-pixel interpolation (8.4.2.2.1): 6-tap half samples, bilinear quarters.
//!
//! [`Hpel`] caches the integer, half and centre samples of a 16×16 block's
//! neighbourhood so that every quarter-pel position within ±3/4 of the base can
//! be formed cheaply (motion refinement), bit-exactly as a decoder does.

use crate::frame::Plane;

const W: usize = 18; // positions -1..=16
const L: usize = 32; // padded lane count

pub struct Hpel {
    g: [[u8; L]; W], // G(x, y)          [y+1][x+1]
    b: [[u8; L]; W], // half between x and x+1 at row y
    h: [[u8; L]; W], // half between y and y+1 at column x
    j: [[u8; L]; W], // centre
}

impl Hpel {
    /// Neighbourhood of the block whose integer top-left sample is (x, y) in `p`.
    pub fn new(p: &Plane, x: isize, y: isize) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if crate::simd::level() >= 2 {
                #[target_feature(enable = "avx2")]
                unsafe fn f(p: &Plane, x: isize, y: isize) -> Hpel {
                    Hpel::new_impl(p, x, y)
                }
                // SAFETY: AVX2 detected at run time.
                return unsafe { f(p, x, y) };
            }
        }
        Self::new_impl(p, x, y)
    }

    #[inline(always)]
    fn new_impl(p: &Plane, x: isize, y: isize) -> Self {
        let mut hp = Hpel { g: [[0; L]; W], b: [[0; L]; W], h: [[0; L]; W], j: [[0; L]; W] };
        // 23 source rows (y-3 ..= y+19); columns x-3 .. x-3+L+5 as i16
        let mut r = [[0i16; L + 8]; 23];
        let avail = p.data.len();
        for (k, row) in r.iter_mut().enumerate() {
            let o = p.off(x - 3, y - 3 + k as isize);
            let n = (L + 8).min(avail - o);
            let src = &p.data[o..o + n];
            for i in 0..n {
                row[i] = src[i] as i16;
            }
        }
        for yy in 0..W {
            let src = &r[yy + 2];
            let dst = &mut hp.g[yy];
            for c in 0..L {
                dst[c] = src[c + 2] as u8;
            }
        }
        // unrounded horizontal halves b1 for rows -3..=18 (22), lane c = column c-1
        let mut b1 = [[0i16; L]; 22];
        for k in 0..22 {
            let row = &r[k];
            let out = &mut b1[k];
            for c in 0..L {
                out[c] = row[c] - 5 * row[c + 1] + 20 * row[c + 2] + 20 * row[c + 3] - 5 * row[c + 4] + row[c + 5];
            }
        }
        for yy in 0..W {
            let src = &b1[yy + 2];
            let dst = &mut hp.b[yy];
            for c in 0..L {
                dst[c] = ((src[c] + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for yy in 0..W {
            let k = yy + 2;
            let (r0, r1, r2, r3, r4, r5) = (&r[k - 2], &r[k - 1], &r[k], &r[k + 1], &r[k + 2], &r[k + 3]);
            let dst = &mut hp.h[yy];
            for c in 0..L {
                let i = c + 2;
                let v = r0[i] - 5 * r1[i] + 20 * r2[i] + 20 * r3[i] - 5 * r4[i] + r5[i];
                dst[c] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for yy in 0..W - 1 {
            let k = yy + 2;
            let (r0, r1, r2, r3, r4, r5) = (&b1[k - 2], &b1[k - 1], &b1[k], &b1[k + 1], &b1[k + 2], &b1[k + 3]);
            let dst = &mut hp.j[yy];
            for c in 0..L {
                let v = r0[c] as i32 - 5 * r1[c] as i32 + 20 * r2[c] as i32 + 20 * r3[c] as i32 - 5 * r4[c] as i32
                    + r5[c] as i32;
                dst[c] = ((v + 512) >> 10).clamp(0, 255) as u8;
            }
        }
        hp
    }

    /// Prediction at quarter-pel offset (dx, dy) ∈ [-3, 3]² from the base.
    pub fn pred(&self, dx: i32, dy: i32, out: &mut [u8; 256]) {
        let (ix, fx) = (dx >> 2, dx & 3);
        let (iy, fy) = (dy >> 2, dy & 3);
        let ox = (ix + 1) as usize;
        let oy = (iy + 1) as usize;
        let (g, b, h, j) = (&self.g, &self.b, &self.h, &self.j);
        type P<'a> = (&'a [[u8; L]; W], usize, usize);
        let (p1, p2): (P, Option<P>) = match (fx, fy) {
            (0, 0) => ((g, 0, 0), None),
            (1, 0) => ((g, 0, 0), Some((b, 0, 0))),
            (2, 0) => ((b, 0, 0), None),
            (3, 0) => ((b, 0, 0), Some((g, 0, 1))),
            (0, 1) => ((g, 0, 0), Some((h, 0, 0))),
            (0, 2) => ((h, 0, 0), None),
            (0, 3) => ((h, 0, 0), Some((g, 1, 0))),
            (1, 1) => ((b, 0, 0), Some((h, 0, 0))),
            (3, 1) => ((b, 0, 0), Some((h, 0, 1))),
            (1, 3) => ((h, 0, 0), Some((b, 1, 0))),
            (3, 3) => ((h, 0, 1), Some((b, 1, 0))),
            (2, 1) => ((b, 0, 0), Some((j, 0, 0))),
            (2, 3) => ((j, 0, 0), Some((b, 1, 0))),
            (1, 2) => ((h, 0, 0), Some((j, 0, 0))),
            (3, 2) => ((j, 0, 0), Some((h, 0, 1))),
            _ => ((j, 0, 0), None),
        };
        for y in 0..16 {
            let a: [u8; 16] = p1.0[oy + y + p1.1][ox + p1.2..ox + p1.2 + 16].try_into().unwrap();
            let o: &mut [u8; 16] = (&mut out[y * 16..y * 16 + 16]).try_into().unwrap();
            match p2 {
                None => *o = a,
                Some(q) => {
                    let c: [u8; 16] = q.0[oy + y + q.1][ox + q.2..ox + q.2 + 16].try_into().unwrap();
                    *o = crate::simd::avg16(&a, &c);
                }
            }
        }
    }
}

/// Luma prediction of the 16×16 block at integer (x, y) displaced by a quarter-pel `mv`.
pub fn luma_pred(p: &Plane, x: isize, y: isize, mv: (i32, i32), out: &mut [u8; 256]) {
    let bx = x + (mv.0 >> 2) as isize;
    let by = y + (mv.1 >> 2) as isize;
    if mv.0 & 3 == 0 && mv.1 & 3 == 0 {
        let o = p.off(bx, by);
        for r in 0..16 {
            out[r * 16..r * 16 + 16].copy_from_slice(&p.data[o + r * p.stride..o + r * p.stride + 16]);
        }
        return;
    }
    Hpel::new(p, bx, by).pred(mv.0 & 3, mv.1 & 3, out);
}

/// Half-sample planes of a whole reference picture (b: horizontal, h: vertical,
/// j: centre), same geometry as the luma plane. Built once per reference when
/// many macroblocks refine; predictions are identical to [`Hpel`].
pub struct HpelPlanes {
    pub b: Plane,
    pub h: Plane,
    pub j: Plane,
    b1: Vec<i16>,
}

impl HpelPlanes {
    pub fn new(r: &Plane) -> Self {
        HpelPlanes { b: r.clone(), h: r.clone(), j: r.clone(), b1: vec![0; r.data.len()] }
    }

    /// Recompute from the (border-extended) reference luma plane.
    #[cfg(test)]
    pub fn compute(&mut self, r: &Plane) {
        let rows = r.data.len() / r.stride;
        self.compute_rect(r, 0, r.stride, 0, rows)
    }

    /// Update after the reference samples in columns `[xa, xb)` × rows `[a, b)`
    /// (plane coordinates, border included) changed.
    #[cfg(test)]
    pub fn compute_rect(&mut self, r: &Plane, xa: usize, xb: usize, a: usize, b: usize) {
        self.filter(r, xa, xb, a, b, a.saturating_sub(3), b + 2)
    }

    /// Columns affected by a change in `[xa, xb)`: horizontal filter on rows
    /// `[a, b)`, vertical and centre filters on rows `[va, vb)` (clipped).
    pub fn filter(&mut self, r: &Plane, xa: usize, xb: usize, a: usize, b: usize, va: usize, vb: usize) {
        #[cfg(target_arch = "x86_64")]
        {
            if crate::simd::level() >= 2 {
                #[target_feature(enable = "avx2")]
                unsafe fn f(s: &mut HpelPlanes, r: &Plane, xa: usize, xb: usize, a: usize, b: usize, va: usize, vb: usize) {
                    s.compute_avx2(r, xa, xb, a, b, va, vb)
                }
                // SAFETY: AVX2 detected at run time.
                return unsafe { f(self, r, xa, xb, a, b, va, vb) };
            }
        }
        self.compute_impl(r, xa, xb, a, b, va, vb)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn compute_avx2(&mut self, r: &Plane, xa: usize, xb: usize, a: usize, b: usize, va: usize, vb: usize) {
        use std::arch::x86_64::*;
        let s = r.stride;
        let rows = r.data.len() / s;
        let b = b.min(rows);
        self.b1.resize(rows * s, 0);
        let d = r.data.as_ptr();
        let b1 = self.b1.as_mut_ptr();
        let (bo, ho, jo) = (self.b.data.as_mut_ptr(), self.h.data.as_mut_ptr(), self.j.data.as_mut_ptr());
        let (c5, c20, c16) = (_mm256_set1_epi16(5), _mm256_set1_epi16(20), _mm256_set1_epi16(16));
        #[inline]
        #[target_feature(enable = "avx2")]
        unsafe fn ld(p: *const u8) -> __m256i {
            _mm256_cvtepu8_epi16(_mm_loadu_si128(p as *const __m128i))
        }
        #[inline]
        #[target_feature(enable = "avx2")]
        unsafe fn tap(a: __m256i, b: __m256i, c: __m256i, d: __m256i, e: __m256i, f: __m256i, c5: __m256i, c20: __m256i) -> __m256i {
            let t = _mm256_add_epi16(a, f);
            let t = _mm256_sub_epi16(t, _mm256_mullo_epi16(_mm256_add_epi16(b, e), c5));
            _mm256_add_epi16(t, _mm256_mullo_epi16(_mm256_add_epi16(c, d), c20))
        }
        #[inline]
        #[target_feature(enable = "avx2")]
        unsafe fn st8(p: *mut u8, v: __m256i, c16: __m256i) {
            let v = _mm256_srai_epi16(_mm256_add_epi16(v, c16), 5);
            let pk = _mm256_packus_epi16(v, v);
            let pk = _mm256_permute4x64_epi64(pk, 0b1000);
            _mm_storeu_si128(p as *mut __m128i, _mm256_castsi256_si128(pk));
        }
        // horizontal: affected columns within 2..s-3, in blocks of 16 (last block aligned to the end)
        let hi = (xb + 2).min(s - 3);
        let xlast = hi - 16;
        let lo = xa.saturating_sub(3).max(2).min(xlast);
        for y in a..b {
            let row = d.add(y * s);
            let mut x = lo;
            loop {
                let xx = x.min(xlast);
                let p = row.add(xx);
                let v = tap(ld(p.sub(2)), ld(p.sub(1)), ld(p), ld(p.add(1)), ld(p.add(2)), ld(p.add(3)), c5, c20);
                _mm256_storeu_si256(b1.add(y * s + xx) as *mut __m256i, v);
                st8(bo.add(y * s + xx), v, c16);
                if xx == xlast {
                    break;
                }
                x += 16;
            }
        }
        let hi = (xb + 2).min(s);
        let xlast = hi - 16;
        let lo = xa.saturating_sub(3).min(xlast);
        for y in va.max(2)..vb.min(rows - 3) {
            let mut x0 = lo;
            loop {
                let x = x0.min(xlast);
                let p = d.add((y - 2) * s + x);
                let v = tap(ld(p), ld(p.add(s)), ld(p.add(2 * s)), ld(p.add(3 * s)), ld(p.add(4 * s)), ld(p.add(5 * s)), c5, c20);
                st8(ho.add(y * s + x), v, c16);
                // j = (a - 5b + 20c + 512) >> 10 in 16 bits, exactly (floor shifts compose):
                // ((((a - b) >> 2) - b + c) >> 2) + c == (a - 5b + 20c) >> 4, and with the
                // 6-tap row sums in [-2550, 10710] no step leaves the i16 range.
                let q = b1.add((y - 2) * s + x) as *const i16;
                let l = |k: usize| _mm256_loadu_si256(q.add(k * s) as *const __m256i);
                let (ta, tb, tc) = (_mm256_add_epi16(l(0), l(5)), _mm256_add_epi16(l(1), l(4)), _mm256_add_epi16(l(2), l(3)));
                let t = _mm256_srai_epi16(_mm256_sub_epi16(ta, tb), 2);
                // (x + c) >> 2 == ((x + (c & 3)) >> 2) + (c >> 2), which stays inside i16
                let x3 = _mm256_add_epi16(_mm256_sub_epi16(t, tb), _mm256_and_si256(tc, _mm256_set1_epi16(3)));
                let t = _mm256_add_epi16(_mm256_srai_epi16(x3, 2), _mm256_srai_epi16(tc, 2));
                let t = _mm256_add_epi16(t, tc);
                let t = _mm256_srai_epi16(_mm256_add_epi16(t, _mm256_set1_epi16(32)), 6);
                let pk = _mm256_packus_epi16(t, t);
                let pk = _mm256_permute4x64_epi64(pk, 0b1000);
                _mm_storeu_si128(jo.add(y * s + x) as *mut __m128i, _mm256_castsi256_si128(pk));
                if x == xlast {
                    break;
                }
                x0 += 16;
            }
        }
    }

    #[inline(always)]
    fn compute_impl(&mut self, r: &Plane, xa: usize, xb: usize, a: usize, b: usize, va: usize, vb: usize) {
        let s = r.stride;
        let rows = r.data.len() / s;
        let d = &r.data;
        // columns 2..s-3 of every row get b; rows 2..rows-3 get h and j
        let mut b1 = std::mem::take(&mut self.b1);
        b1.resize(rows * s, 0);
        let b = b.min(rows);
        for y in a..b {
            let row = &d[y * s..y * s + s];
            let o = &mut b1[y * s..y * s + s];
            let bo = &mut self.b.data[y * s..y * s + s];
            for x in xa.saturating_sub(3).max(2)..(xb + 2).min(s - 3) {
                let v = row[x - 2] as i16 - 5 * row[x - 1] as i16 + 20 * row[x] as i16 + 20 * row[x + 1] as i16
                    - 5 * row[x + 2] as i16
                    + row[x + 3] as i16;
                o[x] = v;
                bo[x] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for y in va.max(2)..vb.min(rows - 3) {
            let r0 = &d[(y - 2) * s..(y - 1) * s];
            let r1 = &d[(y - 1) * s..y * s];
            let r2 = &d[y * s..(y + 1) * s];
            let r3 = &d[(y + 1) * s..(y + 2) * s];
            let r4 = &d[(y + 2) * s..(y + 3) * s];
            let r5 = &d[(y + 3) * s..(y + 4) * s];
            let ho = &mut self.h.data[y * s..(y + 1) * s];
            for x in xa.saturating_sub(3)..(xb + 2).min(s) {
                let v = r0[x] as i16 - 5 * r1[x] as i16 + 20 * r2[x] as i16 + 20 * r3[x] as i16 - 5 * r4[x] as i16
                    + r5[x] as i16;
                ho[x] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
            let (c0, c1, c2) = (&b1[(y - 2) * s..(y - 1) * s], &b1[(y - 1) * s..y * s], &b1[y * s..(y + 1) * s]);
            let (c3, c4, c5) = (&b1[(y + 1) * s..(y + 2) * s], &b1[(y + 2) * s..(y + 3) * s], &b1[(y + 3) * s..(y + 4) * s]);
            let jo = &mut self.j.data[y * s..(y + 1) * s];
            for x in xa.saturating_sub(3)..(xb + 2).min(s) {
                let v = c0[x] as i32 - 5 * c1[x] as i32 + 20 * c2[x] as i32 + 20 * c3[x] as i32 - 5 * c4[x] as i32
                    + c5[x] as i32;
                jo[x] = ((v + 512) >> 10).clamp(0, 255) as u8;
            }
        }
        self.b1 = b1;
    }

    /// Prediction of the block whose integer top-left sample is (x, y) at the
    /// quarter-pel fraction (fx, fy) ∈ [0, 3]².
    /// SAD of the prediction [`Self::pred`] would produce against `src` (16×16).
    /// SADs of the 16×16 predictions at quarter-pel offsets `cands` (each within
    /// ±3, i.e. ±1 sample incl. phase) from the integer position (x, y).
    pub fn sad_cands(&self, g: &Plane, x: isize, y: isize, cands: &[(i32, i32)], src: &[u8; 256], out: &mut [u32]) {
        #[cfg(target_arch = "x86_64")]
        {
            if crate::simd::level() >= 2 {
                let s = g.stride;
                let o = g.off(x, y);
                let n = g.data.len();
                // reads reach (x−1, y−1) .. (x+1+1+15, y+1+1+15) (offset +1/+stride included)
                assert!(o >= s + 1 && o + 17 * s + 18 <= n);
                assert!(cands.len() <= out.len() && cands.iter().all(|c| c.0.abs() <= 3 && c.1.abs() <= 3));
                let planes = [g.data.as_ptr(), self.b.data.as_ptr(), self.h.data.as_ptr(), self.j.data.as_ptr()];
                // SAFETY: the asserts bound every read; AVX2 detected.
                unsafe { sad_cands_avx2(&planes, o, s, cands, src, out) };
                return;
            }
        }
        for (c, r) in cands.iter().zip(out.iter_mut()) {
            *r = self.sad(g, x + (c.0 >> 2) as isize, y + (c.1 >> 2) as isize, c.0 & 3, c.1 & 3, src);
        }
    }

    pub fn sad(&self, g: &Plane, x: isize, y: isize, fx: i32, fy: i32, src: &[u8; 256]) -> u32 {
        #[cfg(target_arch = "x86_64")]
        {
            const T: [[u8; 4]; 16] = PHASE;
            let s = g.stride;
            let o = g.off(x, y);
            let n = g.data.len();
            debug_assert!(self.b.data.len() == n && self.h.data.len() == n && self.j.data.len() == n);
            // every read is at o + {0, 1, s} + r·s + 0..16, r < 16
            assert!(o + 16 * s + 16 <= n);
            let t = T[((fy & 3) * 4 + (fx & 3)) as usize];
            let planes = [g.data.as_ptr(), self.b.data.as_ptr(), self.h.data.as_ptr(), self.j.data.as_ptr()];
            let off = |k: u8| [0, 1, s][k as usize];
            // SAFETY: the assert bounds every read (all planes have the luma plane's
            // length); AVX2 is only used when detected.
            unsafe {
                let a = planes[t[0] as usize].add(o + off(t[1]));
                let b = if t[2] < 4 { Some(planes[t[2] as usize].add(o + off(t[3]))) } else { None };
                if crate::simd::level() >= 2 {
                    return sad_avx2(a, b, s, src);
                }
                use std::arch::x86_64::*;
                let mut acc = _mm_setzero_si128();
                for r in 0..16 {
                    let mut v = _mm_loadu_si128(a.add(r * s) as *const __m128i);
                    if let Some(b) = b {
                        v = _mm_avg_epu8(v, _mm_loadu_si128(b.add(r * s) as *const __m128i));
                    }
                    let t = _mm_loadu_si128(src.as_ptr().add(r * 16) as *const __m128i);
                    acc = _mm_add_epi64(acc, _mm_sad_epu8(v, t));
                }
                (_mm_cvtsi128_si64(acc) + _mm_extract_epi64::<1>(acc)) as u32
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            let mut p = [0u8; 256];
            self.pred(g, x, y, fx, fy, &mut p);
            crate::simd::sad16(src, 16, &p, 16)
        }
    }

    #[allow(clippy::type_complexity)]
    fn sources<'s>(&'s self, g: &'s Plane, fx: i32, fy: i32) -> ((&'s [u8], usize), Option<(&'s [u8], usize)>) {
        let s = g.stride;
        let (gd, b, h, j) = (&g.data, &self.b.data, &self.h.data, &self.j.data);
        type P<'a> = (&'a [u8], usize);
        let r: (P, Option<P>) = match (fx, fy) {
            (0, 0) => ((gd, 0), None),
            (1, 0) => ((gd, 0), Some((b, 0))),
            (2, 0) => ((b, 0), None),
            (3, 0) => ((b, 0), Some((gd, 1))),
            (0, 1) => ((gd, 0), Some((h, 0))),
            (0, 2) => ((h, 0), None),
            (0, 3) => ((h, 0), Some((gd, s))),
            (1, 1) => ((b, 0), Some((h, 0))),
            (3, 1) => ((b, 0), Some((h, 1))),
            (1, 3) => ((h, 0), Some((b, s))),
            (3, 3) => ((h, 1), Some((b, s))),
            (2, 1) => ((b, 0), Some((j, 0))),
            (2, 3) => ((j, 0), Some((b, s))),
            (1, 2) => ((h, 0), Some((j, 0))),
            (3, 2) => ((j, 0), Some((h, 1))),
            _ => ((j, 0), None),
        };
        r
    }

    pub fn pred(&self, g: &Plane, x: isize, y: isize, fx: i32, fy: i32, out: &mut [u8; 256]) {
        let s = g.stride;
        let o = g.off(x, y);
        let (p1, p2) = self.sources(g, fx, fy);
        let (q1, end1) = (o + p1.1, o + p1.1 + 15 * s + 16);
        assert!(end1 <= p1.0.len());
        if let Some(q) = p2 {
            assert!(o + q.1 + 15 * s + 16 <= q.0.len());
        }
        #[cfg(target_arch = "x86_64")]
        // SAFETY: the asserts above bound every 16-byte row read; `out` has 256 bytes.
        unsafe {
            use std::arch::x86_64::*;
            let a = p1.0.as_ptr().add(q1);
            let d = out.as_mut_ptr();
            match p2 {
                None => {
                    for r in 0..16 {
                        let v = _mm_loadu_si128(a.add(r * s) as *const __m128i);
                        _mm_storeu_si128(d.add(r * 16) as *mut __m128i, v);
                    }
                }
                Some(q) => {
                    let b = q.0.as_ptr().add(o + q.1);
                    for r in 0..16 {
                        let v = _mm_avg_epu8(
                            _mm_loadu_si128(a.add(r * s) as *const __m128i),
                            _mm_loadu_si128(b.add(r * s) as *const __m128i),
                        );
                        _mm_storeu_si128(d.add(r * 16) as *mut __m128i, v);
                    }
                }
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        for r in 0..16 {
            let i = o + r * s;
            let a: [u8; 16] = p1.0[i + p1.1..i + p1.1 + 16].try_into().unwrap();
            let dst: &mut [u8; 16] = (&mut out[r * 16..r * 16 + 16]).try_into().unwrap();
            *dst = match p2 {
                None => a,
                Some(q) => crate::simd::avg16(&a, q.0[i + q.1..i + q.1 + 16].try_into().unwrap()),
            };
        }
    }
}

#[cfg(test)]
mod planes_test {
    use super::*;
    #[test]
    fn centre_filter_i16_identity() {
        // same operations as the AVX2 centre filter, in wrapping i16
        let f16 = |c: [i16; 6]| -> i32 {
            let (ta, tb, tc) = (c[0].wrapping_add(c[5]), c[1].wrapping_add(c[4]), c[2].wrapping_add(c[3]));
            let t = ta.wrapping_sub(tb) >> 2;
            let x3 = t.wrapping_sub(tb).wrapping_add(tc & 3);
            let t = (x3 >> 2).wrapping_add(tc >> 2).wrapping_add(tc);
            (t.wrapping_add(32) >> 6) as i32
        };
        let vals = [-2550i16, -2549, -1000, -1, 0, 1, 3, 4000, 10709, 10710];
        let n = vals.len();
        for m in 0..n.pow(6) {
            let mut c = [0i16; 6];
            let mut k = m;
            for v in c.iter_mut() {
                *v = vals[k % n];
                k /= n;
            }
            let v: i32 = c[0] as i32 - 5 * c[1] as i32 + 20 * c[2] as i32 + 20 * c[3] as i32 - 5 * c[4] as i32 + c[5] as i32;
            assert_eq!(f16(c), (v + 512) >> 10, "{c:?}");
        }
    }

    #[test]
    fn planes_extremes() {
        // rows/columns of 0 and 255 that drive the centre filter to its limits
        let mut p = Plane::new(64, 48, 32);
        let s = p.stride;
        for (i, v) in p.data.iter_mut().enumerate() {
            let (x, y) = (i % s, i / s);
            let f = |k: usize| matches!(k % 6, 0 | 3);
            *v = if f(x) ^ f(y) { 255 } else { 0 };
            if (x / 7 + y / 5) % 3 == 0 {
                *v = 255 - *v;
            }
        }
        let rows = p.data.len() / s;
        let (mut a, mut b) = (HpelPlanes::new(&p), HpelPlanes::new(&p));
        a.compute(&p);
        b.compute_impl(&p, 0, s, 0, rows, 0, rows);
        assert!(a.j.data == b.j.data && a.b.data == b.b.data && a.h.data == b.h.data);
    }

    #[test]
    fn planes_match_window() {
        let mut p = Plane::new(64, 48, 32);
        for (i, v) in p.data.iter_mut().enumerate() {
            *v = ((i * 37 + i / 97 * 13) % 251) as u8;
        }
        p.extend();
        let mut hp = HpelPlanes::new(&p);
        hp.compute(&p);
        let mut sc = HpelPlanes::new(&p);
        let rows = p.data.len() / p.stride;
        sc.compute_impl(&p, 0, p.stride, 0, rows, 0, rows);
        // partial update after a local change equals a full recompute
        let mut q = p.clone();
        for y in 40..60 {
            for x in 50..70 {
                q.data[y * q.stride + x] ^= 0x5a;
            }
        }
        hp.compute_rect(&q, 50, 70, 40, 60);
        sc.compute(&q);
        assert!(sc.b.data == hp.b.data && sc.h.data == hp.h.data && sc.j.data == hp.j.data);
        hp.compute(&p);
        sc.compute_impl(&p, 0, p.stride, 0, rows, 0, rows);
        assert!(sc.b.data == hp.b.data && sc.h.data == hp.h.data && sc.j.data == hp.j.data);
        for (x, y) in [(0isize, 0isize), (-17, -17), (40, 20), (48, 32), (-5, 30)] {
            let w = Hpel::new(&p, x, y);
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let (mut a, mut b) = ([0u8; 256], [0u8; 256]);
                    w.pred(dx, dy, &mut a);
                    let (gx, gy) = (x + (dx >> 2) as isize, y + (dy >> 2) as isize);
                    hp.pred(&p, gx, gy, dx & 3, dy & 3, &mut b);
                    assert_eq!(a, b, "({x},{y}) d=({dx},{dy})");
                    let src: [u8; 256] = core::array::from_fn(|i| (i * 7 % 256) as u8);
                    assert_eq!(hp.sad(&p, gx, gy, dx & 3, dy & 3, &src), crate::simd::sad16(&src, 16, &b, 16));
                }
            }
        }
    }
}

/// Sources per quarter-pel phase (index fy·4 + fx): (plane, offset) and an optional
/// second (plane, offset) to average; planes 0..4 = full, b, h, j; offsets
/// 0 = +0, 1 = +1, 2 = +stride; plane 4 = none.
const PHASE: [[u8; 4]; 16] = [
    [0, 0, 4, 0], // (0,0)
    [0, 0, 1, 0], // (1,0)
    [1, 0, 4, 0], // (2,0)
    [1, 0, 0, 1], // (3,0)
    [0, 0, 2, 0], // (0,1)
    [1, 0, 2, 0], // (1,1)
    [1, 0, 3, 0], // (2,1)
    [1, 0, 2, 1], // (3,1)
    [2, 0, 4, 0], // (0,2)
    [2, 0, 3, 0], // (1,2)
    [3, 0, 4, 0], // (2,2)
    [3, 0, 2, 1], // (3,2)
    [2, 0, 0, 2], // (0,3)
    [2, 0, 1, 2], // (1,3)
    [3, 0, 1, 2], // (2,3)
    [2, 1, 1, 2], // (3,3)
];

/// SAD of 16 rows of 16 bytes at `a` (averaged with `b` when given), stride `s`,
/// against `src`; two rows per 256-bit step.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sad_avx2(a: *const u8, b: Option<*const u8>, s: usize, src: &[u8; 256]) -> u32 {
    use std::arch::x86_64::*;
    let row2 = |p: *const u8, r: usize| {
        let lo = _mm_loadu_si128(p.add(r * s) as *const __m128i);
        let hi = _mm_loadu_si128(p.add(r * s + s) as *const __m128i);
        _mm256_inserti128_si256::<1>(_mm256_castsi128_si256(lo), hi)
    };
    let mut acc = _mm256_setzero_si256();
    match b {
        None => {
            for r in (0..16).step_by(2) {
                let t = _mm256_loadu_si256(src.as_ptr().add(r * 16) as *const __m256i);
                acc = _mm256_add_epi64(acc, _mm256_sad_epu8(row2(a, r), t));
            }
        }
        Some(b) => {
            for r in (0..16).step_by(2) {
                let v = _mm256_avg_epu8(row2(a, r), row2(b, r));
                let t = _mm256_loadu_si256(src.as_ptr().add(r * 16) as *const __m256i);
                acc = _mm256_add_epi64(acc, _mm256_sad_epu8(v, t));
            }
        }
    }
    let x = _mm_add_epi64(_mm256_castsi256_si128(acc), _mm256_extracti128_si256::<1>(acc));
    (_mm_cvtsi128_si64(x) + _mm_extract_epi64::<1>(x)) as u32
}

/// [`HpelPlanes::sad_cands`]: one call for a whole candidate pattern.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn sad_cands_avx2(planes: &[*const u8; 4], o: usize, s: usize, cands: &[(i32, i32)], src: &[u8; 256], out: &mut [u32]) {
    for (c, r) in cands.iter().zip(out.iter_mut()) {
        let t = PHASE[((c.1 & 3) * 4 + (c.0 & 3)) as usize];
        let base = (o as isize + (c.1 >> 2) as isize * s as isize + (c.0 >> 2) as isize) as usize;
        let off = |k: u8| [0, 1, s][k as usize];
        let a = planes[t[0] as usize].add(base + off(t[1]));
        let b = if t[2] < 4 { Some(planes[t[2] as usize].add(base + off(t[3]))) } else { None };
        *r = sad_avx2(a, b, s, src);
    }
}
