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
    if n == 16 {
        let mut acc = 0u128;
        for y in 0..16 {
            let x = u128::from_ne_bytes(a[y * sa..y * sa + 16].try_into().unwrap());
            let z = u128::from_ne_bytes(b[y * sb..y * sb + 16].try_into().unwrap());
            acc |= x ^ z;
        }
        acc == 0
    } else {
        let mut acc = 0u64;
        for y in 0..n {
            let x = u64::from_ne_bytes(a[y * sa..y * sa + 8].try_into().unwrap());
            let z = u64::from_ne_bytes(b[y * sb..y * sb + 8].try_into().unwrap());
            acc |= x ^ z;
        }
        acc == 0
    }
}
