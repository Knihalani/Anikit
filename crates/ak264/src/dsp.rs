//! Pixel kernels: SAD, SATD, block compare. Scalar reference versions; SIMD in `simd`.

/// SAD of a 16×16 block: `a` (stride sa) vs `b` (stride sb).
#[inline]
pub fn sad16(a: &[u8], sa: usize, b: &[u8], sb: usize) -> u32 {
    crate::simd::sad16(a, sa, b, sb)
}

/// SATD (sum of |4×4 Hadamard| / 2) of a w×h block, contiguous rows of width `w`.
#[inline]
pub fn satd(a: &[u8], b: &[u8], w: usize, h: usize) -> u32 {
    crate::simd::satd_sum(a, w, b, w, w, h) >> 1
}

/// Exact equality of a w×h region.
#[inline]
pub fn block_eq(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> bool {
    (0..h).all(|y| a[y * sa..y * sa + w] == b[y * sb..y * sb + w])
}
