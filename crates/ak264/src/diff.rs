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
