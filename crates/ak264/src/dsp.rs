//! Pixel kernels: SAD, SATD, block compare. Scalar reference versions; SIMD in `simd`.

/// SATD (sum of |4×4 Hadamard| / 2) of a w×h block, contiguous rows of width `w`.
#[inline]
pub fn satd(a: &[u8], b: &[u8], w: usize, h: usize) -> u32 {
    crate::simd::satd_sum(a, w, b, w, w, h) >> 1
}

/// Is every |a − b| ≤ t over a w×h region?
#[inline]
pub fn max_abs_le(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize, t: u8) -> bool {
    (0..h).all(|y| {
        let (ra, rb) = (&a[y * sa..y * sa + w], &b[y * sb..y * sb + w]);
        ra.iter().zip(rb).fold(0u8, |m, (&x, &z)| m.max(x.abs_diff(z))) <= t
    })
}

/// Exact equality of a w×h region.
#[inline]
pub fn block_eq(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> bool {
    (0..h).all(|y| a[y * sa..y * sa + w] == b[y * sb..y * sb + w])
}
