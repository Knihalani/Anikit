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

/// SATD (sum of |4×4 Hadamard| / 2) of a w×h block, contiguous rows of width `w`.
#[inline]
pub fn satd(a: &[u8], b: &[u8], w: usize, h: usize) -> u32 {
    crate::simd::satd_sum(a, w, b, w, w, h) >> 1
}

/// SATD of one 4×4 block.
#[inline(always)]
pub fn satd4(a: &[u8], b: &[u8], w: usize, bx: usize, by: usize) -> u32 {
    let o = by * w + bx;
    crate::simd::satd_sum_scalar(&a[o..], w, &b[o..], w, 4, 4) >> 1
}

/// Exact equality of a w×h region.
#[inline]
pub fn block_eq(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> bool {
    (0..h).all(|y| a[y * sa..y * sa + w] == b[y * sb..y * sb + w])
}
