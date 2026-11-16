//! Fused change detection and source update, one macroblock at a time.

use crate::frame::{Plane, YuvFrame};

/// Compare (if `compare`) the macroblock of `f` against `dst` and copy it in when it
/// differs (or always, without `compare`). Samples past the picture edge replicate
/// the last column/row. Returns whether the macroblock changed.
pub(crate) fn load_mb(y: &mut Plane, u: &mut Plane, v: &mut Plane, f: &YuvFrame, mbx: usize, mby: usize, compare: bool) -> bool {
    let (w, h) = (f.width as usize, f.height as usize);
    let mut changed = !compare;
    changed |= block(y, f.y, f.y_stride, w, h, mbx * 16, mby * 16, 16, changed);
    let (cw, ch) = (w / 2, h / 2);
    changed |= block(u, f.u, f.c_stride, cw, ch, mbx * 8, mby * 8, 8, changed);
    changed |= block(v, f.v, f.c_stride, cw, ch, mbx * 8, mby * 8, 8, changed);
    changed
}

/// Returns true if the block differed (and was copied). With `force`, copies unconditionally.
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn block(p: &mut Plane, src: &[u8], ss: usize, w: usize, h: usize, x0: usize, y0: usize, n: usize, force: bool) -> bool {
    let full = x0 + n <= w;
    if full && y0 + n <= h {
        // Fast path: compare the whole block first, copy only if it differs.
        let d0 = (y0 + p.pad) * p.stride + p.pad + x0;
        if !force && blocks_equal(&p.data[d0..], p.stride, &src[y0 * ss + x0..], ss, n) {
            return false;
        }
        for yy in 0..n {
            let d = d0 + yy * p.stride;
            let s = (y0 + yy) * ss + x0;
            p.data[d..d + n].copy_from_slice(&src[s..s + n]);
        }
        return true;
    }
    let mut changed = false;
    for yy in 0..n {
        let sy = (y0 + yy).min(h - 1);
        let d = (y0 + yy + p.pad) * p.stride + p.pad + x0;
        if full {
            let s = &src[sy * ss + x0..sy * ss + x0 + n];
            let dr = &mut p.data[d..d + n];
            if force || dr != s {
                dr.copy_from_slice(s);
                changed = true;
            }
        } else {
            for xx in 0..n {
                let sx = (x0 + xx).min(w - 1);
                let v = src[sy * ss + sx];
                if p.data[d + xx] != v {
                    p.data[d + xx] = v;
                    changed = true;
                }
            }
            changed |= force;
        }
    }
    changed
}

#[inline(always)]
fn blocks_equal(a: &[u8], sa: usize, b: &[u8], sb: usize, n: usize) -> bool {
    assert!(a.len() >= (n - 1) * sa + n && b.len() >= (n - 1) * sb + n);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds asserted above; SSE2 is baseline on x86_64.
    unsafe {
        use std::arch::x86_64::*;
        let (pa, pb) = (a.as_ptr(), b.as_ptr());
        let mut acc = _mm_setzero_si128();
        if n == 16 {
            for y in 0..16 {
                let x = _mm_loadu_si128(pa.add(y * sa) as *const __m128i);
                let z = _mm_loadu_si128(pb.add(y * sb) as *const __m128i);
                acc = _mm_or_si128(acc, _mm_xor_si128(x, z));
            }
        } else {
            for y in 0..n {
                let x = _mm_loadl_epi64(pa.add(y * sa) as *const __m128i);
                let z = _mm_loadl_epi64(pb.add(y * sb) as *const __m128i);
                acc = _mm_or_si128(acc, _mm_xor_si128(x, z));
            }
        }
        _mm_movemask_epi8(_mm_cmpeq_epi8(acc, _mm_setzero_si128())) == 0xffff
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        (0..n).all(|y| a[y * sa..y * sa + n] == b[y * sb..y * sb + n])
    }
}

/// Streaming compare of one macroblock row: sets `changed[0..n]` (no copying).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn row_diff_avx2(y: &Plane, u: &Plane, v: &Plane, f: &YuvFrame, mby: usize, n: usize, changed: &mut [bool]) {
    use std::arch::x86_64::*;
    let mut acc = [_mm256_setzero_si256(); 256]; // per pair of macroblocks
    let pairs = n / 2;
    let ys = f.y_stride;
    for r in 0..16 {
        let s = f.y.as_ptr().add((mby * 16 + r) * ys);
        let d = y.data.as_ptr().add((mby * 16 + r + y.pad) * y.stride + y.pad);
        for p in 0..pairs {
            let a = _mm256_loadu_si256(s.add(p * 32) as *const __m256i);
            let b = _mm256_loadu_si256(d.add(p * 32) as *const __m256i);
            acc[p] = _mm256_or_si256(acc[p], _mm256_xor_si256(a, b));
        }
    }
    let mut cacc = [_mm256_setzero_si256(); 128]; // per 4 macroblocks (32 chroma bytes)
    let quads = n / 4;
    let cs = f.c_stride;
    for (pl, src) in [(u, f.u), (v, f.v)] {
        for r in 0..8 {
            let s = src.as_ptr().add((mby * 8 + r) * cs);
            let d = pl.data.as_ptr().add((mby * 8 + r + pl.pad) * pl.stride + pl.pad);
            for q in 0..quads {
                let a = _mm256_loadu_si256(s.add(q * 32) as *const __m256i);
                let b = _mm256_loadu_si256(d.add(q * 32) as *const __m256i);
                cacc[q] = _mm256_or_si256(cacc[q], _mm256_xor_si256(a, b));
            }
        }
    }
    let z = _mm256_setzero_si256();
    for p in 0..pairs {
        let m = _mm256_movemask_epi8(_mm256_cmpeq_epi8(acc[p], z)) as u32;
        changed[2 * p] = m & 0xffff != 0xffff;
        changed[2 * p + 1] = m >> 16 != 0xffff;
    }
    for q in 0..quads {
        let m = _mm256_movemask_epi8(_mm256_cmpeq_epi8(cacc[q], z)) as u32;
        for k in 0..4 {
            changed[4 * q + k] |= (m >> (8 * k)) & 0xff != 0xff;
        }
    }
    // leftovers (odd counts): exact per-macroblock compare
    for mbx in (pairs * 2).min(quads * 4)..n {
        changed[mbx] = mb_differs(y, u, v, f, mbx, mby);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn mb_differs(y: &Plane, u: &Plane, v: &Plane, f: &YuvFrame, mbx: usize, mby: usize) -> bool {
    let ys = f.y_stride;
    for r in 0..16 {
        let s = std::slice::from_raw_parts(f.y.as_ptr().add((mby * 16 + r) * ys + mbx * 16), 16);
        let o = (mby * 16 + r + y.pad) * y.stride + y.pad + mbx * 16;
        if s != &y.data[o..o + 16] {
            return true;
        }
    }
    let cs = f.c_stride;
    for (pl, src) in [(u, f.u), (v, f.v)] {
        for r in 0..8 {
            let s = std::slice::from_raw_parts(src.as_ptr().add((mby * 8 + r) * cs + mbx * 8), 8);
            let o = (mby * 8 + r + pl.pad) * pl.stride + pl.pad + mbx * 8;
            if s != &pl.data[o..o + 8] {
                return true;
            }
        }
    }
    false
}


/// Read-only change flags for one macroblock row. Exact for macroblocks fully
/// inside the picture; macroblocks crossing the right/bottom edge are flagged
/// (callers re-check flagged ones with [`load_mb`]).
pub(crate) fn flag_row(y: &Plane, u: &Plane, v: &Plane, f: &YuvFrame, mby: usize, out: &mut [bool]) {
    let (w, h) = (f.width as usize, f.height as usize);
    let full_rows = (mby + 1) * 16 <= h && (mby + 1) * 8 <= h / 2;
    let full_cols = (w / 16).min((w / 2) / 8);
    out.fill(true);
    if !full_rows {
        return;
    }
    #[cfg(target_arch = "x86_64")]
    {
        if crate::simd::level() >= 2 && full_cols <= 512 {
            // SAFETY: AVX2 detected; full macroblocks lie inside both pictures.
            unsafe { row_diff_avx2(y, u, v, f, mby, full_cols, out) };
            return;
        }
        for mbx in 0..full_cols {
            // SAFETY: the macroblock lies inside both pictures.
            out[mbx] = unsafe { mb_differs(y, u, v, f, mbx, mby) };
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = (y, u, v, full_cols);
}
