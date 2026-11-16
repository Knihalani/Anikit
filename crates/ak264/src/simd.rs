//! SIMD kernels (x86_64 SSE4.1 / AVX2 via `std::arch`) with scalar fallbacks.
//! All kernels return exactly what the scalar versions return.

use std::sync::atomic::{AtomicU8, Ordering};

static LEVEL: AtomicU8 = AtomicU8::new(255);

/// 0 = scalar, 1 = SSE4.1, 2 = AVX2.
#[inline(always)]
pub fn level() -> u8 {
    let l = LEVEL.load(Ordering::Relaxed);
    if l != 255 {
        return l;
    }
    detect()
}

#[cold]
fn detect() -> u8 {
    let mut l = 0;
    #[cfg(target_arch = "x86_64")]
    {
        if std::env::var("AK264_SIMD").map(|v| v == "0").unwrap_or(false) {
            l = 0;
        } else if is_x86_feature_detected!("avx2") {
            l = 2;
        } else if is_x86_feature_detected!("sse4.1") {
            l = 1;
        }
    }
    LEVEL.store(l, Ordering::Relaxed);
    l
}

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

/// Sum of |4×4 Hadamard| over a w×h block (w multiple of 8, or 4), not halved.
#[inline]
pub fn satd_sum(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        assert!(a.len() >= (h - 1) * sa + w && b.len() >= (h - 1) * sb + w && h % 4 == 0);
        let l = level();
        if l == 2 && w == 16 {
            // SAFETY: AVX2 detected; bounds checked.
            return unsafe { satd16_avx2(a.as_ptr(), sa, b.as_ptr(), sb, h) };
        }
        if l >= 1 && w % 8 == 0 {
            let mut s = 0;
            for x in (0..w).step_by(8) {
                // SAFETY: SSE4.1 detected; bounds checked.
                s += unsafe { satd8_sse41(a.as_ptr().add(x), sa, b.as_ptr().add(x), sb, h) };
            }
            return s;
        }
    }
    satd_sum_scalar(a, sa, b, sb, w, h)
}

pub fn satd_sum_scalar(a: &[u8], sa: usize, b: &[u8], sb: usize, w: usize, h: usize) -> u32 {
    let mut s = 0;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let mut d = [0i32; 16];
            for y in 0..4 {
                for x in 0..4 {
                    d[y * 4 + x] = a[(by + y) * sa + bx + x] as i32 - b[(by + y) * sb + bx + x] as i32;
                }
            }
            let hd = crate::transform::hadamard4(&d);
            s += hd.iter().map(|v| v.unsigned_abs()).sum::<u32>();
        }
    }
    s
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn satd8_sse41(a: *const u8, sa: usize, b: *const u8, sb: usize, h: usize) -> u32 {
    use std::arch::x86_64::*;
    let z = _mm_setzero_si128();
    let ones = _mm_set1_epi16(1);
    let mut acc = _mm_setzero_si128();
    let mut y = 0;
    while y < h {
        let mut r = [z; 4];
        for (k, rk) in r.iter_mut().enumerate() {
            let va = _mm_unpacklo_epi8(_mm_loadl_epi64(a.add((y + k) * sa) as *const __m128i), z);
            let vb = _mm_unpacklo_epi8(_mm_loadl_epi64(b.add((y + k) * sb) as *const __m128i), z);
            *rk = _mm_sub_epi16(va, vb);
        }
        // vertical butterflies
        let a0 = _mm_add_epi16(r[0], r[1]);
        let a1 = _mm_sub_epi16(r[0], r[1]);
        let a2 = _mm_add_epi16(r[2], r[3]);
        let a3 = _mm_sub_epi16(r[2], r[3]);
        let v = [_mm_add_epi16(a0, a2), _mm_sub_epi16(a0, a2), _mm_add_epi16(a1, a3), _mm_sub_epi16(a1, a3)];
        for x in v {
            // horizontal butterflies within groups of 4 lanes
            let s1 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(x, 0b10_11_00_01), 0b10_11_00_01);
            let e = _mm_add_epi16(x, s1);
            let f = _mm_sub_epi16(s1, x);
            let x1 = _mm_blend_epi16(e, f, 0b1010_1010);
            let s2 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(x1, 0b01_00_11_10), 0b01_00_11_10);
            let e = _mm_add_epi16(x1, s2);
            let f = _mm_sub_epi16(s2, x1);
            let x2 = _mm_blend_epi16(e, f, 0b1100_1100);
            acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_abs_epi16(x2), ones));
        }
        y += 4;
    }
    let t = _mm_add_epi32(acc, _mm_shuffle_epi32(acc, 0b01_00_11_10));
    let t = _mm_add_epi32(t, _mm_shuffle_epi32(t, 0b10_11_00_01));
    _mm_cvtsi128_si32(t) as u32
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn satd16_avx2(a: *const u8, sa: usize, b: *const u8, sb: usize, h: usize) -> u32 {
    use std::arch::x86_64::*;
    let ones = _mm256_set1_epi16(1);
    let mut acc = _mm256_setzero_si256();
    let mut y = 0;
    while y < h {
        let mut r = [_mm256_setzero_si256(); 4];
        for (k, rk) in r.iter_mut().enumerate() {
            let va = _mm256_cvtepu8_epi16(_mm_loadu_si128(a.add((y + k) * sa) as *const __m128i));
            let vb = _mm256_cvtepu8_epi16(_mm_loadu_si128(b.add((y + k) * sb) as *const __m128i));
            *rk = _mm256_sub_epi16(va, vb);
        }
        let a0 = _mm256_add_epi16(r[0], r[1]);
        let a1 = _mm256_sub_epi16(r[0], r[1]);
        let a2 = _mm256_add_epi16(r[2], r[3]);
        let a3 = _mm256_sub_epi16(r[2], r[3]);
        let v = [_mm256_add_epi16(a0, a2), _mm256_sub_epi16(a0, a2), _mm256_add_epi16(a1, a3), _mm256_sub_epi16(a1, a3)];
        for x in v {
            let s1 = _mm256_shufflelo_epi16(_mm256_shufflehi_epi16(x, 0b10_11_00_01), 0b10_11_00_01);
            let e = _mm256_add_epi16(x, s1);
            let f = _mm256_sub_epi16(s1, x);
            let x1 = _mm256_blend_epi16(e, f, 0b1010_1010);
            let s2 = _mm256_shufflelo_epi16(_mm256_shufflehi_epi16(x1, 0b01_00_11_10), 0b01_00_11_10);
            let e = _mm256_add_epi16(x1, s2);
            let f = _mm256_sub_epi16(s2, x1);
            let x2 = _mm256_blend_epi16(e, f, 0b1100_1100);
            acc = _mm256_add_epi32(acc, _mm256_madd_epi16(_mm256_abs_epi16(x2), ones));
        }
        y += 4;
    }
    let t = _mm_add_epi32(_mm256_castsi256_si128(acc), _mm256_extracti128_si256(acc, 1));
    let t = _mm_add_epi32(t, _mm_shuffle_epi32(t, 0b01_00_11_10));
    let t = _mm_add_epi32(t, _mm_shuffle_epi32(t, 0b10_11_00_01));
    _mm_cvtsi128_si32(t) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn satd_matches_scalar() {
        let a: Vec<u8> = (0..32 * 16).map(|i| ((i * 37 + i / 7 * 11) % 256) as u8).collect();
        let b: Vec<u8> = (0..32 * 16).map(|i| ((i * 91 + 13) % 256) as u8).collect();
        for (w, h) in [(16, 16), (8, 8), (16, 8), (8, 16)] {
            assert_eq!(satd_sum(&a, 32, &b, 32, w, h), satd_sum_scalar(&a, 32, &b, 32, w, h), "{w}x{h}");
        }
    }
}
