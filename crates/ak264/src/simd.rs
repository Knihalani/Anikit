//! SIMD dispatch (x86 SSE2/AVX2 via `std::arch`), scalar fallback.

#[inline]
pub fn sad16(a: &[u8], sa: usize, b: &[u8], sb: usize) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        assert!(a.len() >= 15 * sa + 16 && b.len() >= 15 * sb + 16);
        // SAFETY: bounds checked above; SSE2 is baseline on x86_64.
        unsafe { sad16_sse2(a.as_ptr(), sa, b.as_ptr(), sb) }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        crate::dsp::sad16_scalar(a, sa, b, sb)
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn sad16_sse2(a: *const u8, sa: usize, b: *const u8, sb: usize) -> u32 {
    use std::arch::x86_64::*;
    let mut acc = _mm_setzero_si128();
    for y in 0..16 {
        let va = _mm_loadu_si128(a.add(y * sa) as *const __m128i);
        let vb = _mm_loadu_si128(b.add(y * sb) as *const __m128i);
        acc = _mm_add_epi64(acc, _mm_sad_epu8(va, vb));
    }
    (_mm_cvtsi128_si64(acc) + _mm_extract_epi16(acc, 4) as i64) as u32
}
