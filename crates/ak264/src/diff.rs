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

/// Compare-and-copy a whole macroblock row, setting `changed[mbx]`. Uses the
/// per-macroblock path at the right/bottom picture edges.
pub(crate) fn load_row(
    y: &mut Plane,
    u: &mut Plane,
    v: &mut Plane,
    f: &YuvFrame,
    mby: usize,
    mbw: usize,
    changed: &mut [bool],
) {
    let (w, h) = (f.width as usize, f.height as usize);
    let full_rows = (mby + 1) * 16 <= h && (mby + 1) * 8 <= h / 2;
    let full_cols = w / 16; // macroblocks fully inside horizontally (luma and chroma)
    let full_cols = full_cols.min((w / 2) / 8);
    let mut start = 0;
    if full_rows && f.y.len() >= (mby * 16 + 15) * f.y_stride + full_cols * 16 {
        #[cfg(target_arch = "x86_64")]
        {
            for mbx in 0..full_cols {
                // SAFETY: the macroblock lies inside both pictures (checked above; planes
                // are at least MB-aligned in size); SSE2 is baseline.
                changed[mbx] = unsafe { mb_diff_copy(y, u, v, f, mbx, mby) };
            }
            start = full_cols;
        }
    }
    for mbx in start..mbw {
        changed[mbx] = load_mb(y, u, v, f, mbx, mby, true);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn mb_diff_copy(y: &mut Plane, u: &mut Plane, v: &mut Plane, f: &YuvFrame, mbx: usize, mby: usize) -> bool {
    use std::arch::x86_64::*;
    let ys = f.y_stride;
    let sy = f.y.as_ptr().add(mby * 16 * ys + mbx * 16);
    let dy = y.data.as_mut_ptr().add((mby * 16 + y.pad) * y.stride + y.pad + mbx * 16);
    let cs = f.c_stride;
    let co = mby * 8 * cs + mbx * 8;
    let su = f.u.as_ptr().add(co);
    let sv = f.v.as_ptr().add(co);
    let cdo = (mby * 8 + u.pad) * u.stride + u.pad + mbx * 8;
    let du = u.data.as_mut_ptr().add(cdo);
    let dv = v.data.as_mut_ptr().add(cdo);
    let (ds, dcs) = (y.stride, u.stride);
    let mut acc = _mm_setzero_si128();
    for r in 0..16 {
        let a = _mm_loadu_si128(sy.add(r * ys) as *const __m128i);
        let b = _mm_loadu_si128(dy.add(r * ds) as *const __m128i);
        acc = _mm_or_si128(acc, _mm_xor_si128(a, b));
    }
    for r in 0..8 {
        let a = _mm_unpacklo_epi64(
            _mm_loadl_epi64(su.add(r * cs) as *const __m128i),
            _mm_loadl_epi64(sv.add(r * cs) as *const __m128i),
        );
        let b = _mm_unpacklo_epi64(
            _mm_loadl_epi64(du.add(r * dcs) as *const __m128i),
            _mm_loadl_epi64(dv.add(r * dcs) as *const __m128i),
        );
        acc = _mm_or_si128(acc, _mm_xor_si128(a, b));
    }
    if _mm_movemask_epi8(_mm_cmpeq_epi8(acc, _mm_setzero_si128())) == 0xffff {
        return false;
    }
    for r in 0..16 {
        std::ptr::copy_nonoverlapping(sy.add(r * ys), dy.add(r * ds), 16);
    }
    for r in 0..8 {
        std::ptr::copy_nonoverlapping(su.add(r * cs), du.add(r * dcs), 8);
        std::ptr::copy_nonoverlapping(sv.add(r * cs), dv.add(r * dcs), 8);
    }
    true
}
