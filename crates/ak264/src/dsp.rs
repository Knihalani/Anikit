//! Pixel kernels: SAD, SATD, block compare. Scalar reference versions; SIMD in `simd`.

/// SAD of a 16×16 block: `a` (stride sa) vs `b` (stride sb).
#[inline]
pub fn sad16(a: &[u8], sa: usize, b: &[u8], sb: usize) -> u32 {
    crate::simd::sad16(a, sa, b, sb)
}

#[inline(always)]
pub fn sad16_scalar(a: &[u8], sa: usize, b: &[u8], sb: usize) -> u32 {
    let mut s = 0u32;
    for y in 0..16 {
        let ra = &a[y * sa..y * sa + 16];
        let rb = &b[y * sb..y * sb + 16];
        for x in 0..16 {
            s += (ra[x] as i32 - rb[x] as i32).unsigned_abs();
        }
    }
    s
}

/// SATD (4×4 Hadamard, sum |coef| / 2) of a w×h block, contiguous rows of width `w`.
#[inline]
pub fn satd(a: &[u8], b: &[u8], w: usize, h: usize) -> u32 {
    let mut s = 0;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            s += satd4(a, b, w, bx, by);
        }
    }
    s
}

#[inline(always)]
pub fn satd4(a: &[u8], b: &[u8], w: usize, bx: usize, by: usize) -> u32 {
    let mut d = [0i32; 16];
    for y in 0..4 {
        for x in 0..4 {
            let i = (by + y) * w + bx + x;
            d[y * 4 + x] = a[i] as i32 - b[i] as i32;
        }
    }
    let h = crate::transform::hadamard4(&d);
    (h.iter().map(|v| v.unsigned_abs()).sum::<u32>() + 1) >> 1
}

/// Exact equality of a w×h region.
#[inline]
pub fn block_eq(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> bool {
    (0..h).all(|y| a[y * sa..y * sa + w] == b[y * sb..y * sb + w])
}
