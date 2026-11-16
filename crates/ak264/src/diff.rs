//! Exact per-macroblock change detection between two source pictures.

use crate::frame::Pic;

/// Set `out[mb] = true` where any luma or chroma sample of the macroblock differs.
pub(crate) fn changed_map(a: &Pic, b: &Pic, mbw: usize, mbh: usize, out: &mut [bool]) {
    for mby in 0..mbh {
        for mbx in 0..mbw {
            let mut eq = rows_eq(&a.y.data, &b.y.data, a.y.off(mbx as isize * 16, mby as isize * 16), a.y.stride, 16, 16);
            if eq {
                let o = a.u.off(mbx as isize * 8, mby as isize * 8);
                eq = rows_eq(&a.u.data, &b.u.data, o, a.u.stride, 8, 8)
                    && rows_eq(&a.v.data, &b.v.data, o, a.v.stride, 8, 8);
            }
            out[mby * mbw + mbx] = !eq;
        }
    }
}

#[inline(always)]
fn rows_eq(a: &[u8], b: &[u8], o: usize, s: usize, w: usize, h: usize) -> bool {
    (0..h).all(|y| a[o + y * s..o + y * s + w] == b[o + y * s..o + y * s + w])
}
