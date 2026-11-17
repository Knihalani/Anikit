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
        {
        let mut s = 0u32;
        for y in 0..16 {
            for x in 0..16 {
                s += (a[y * sa + x] as i32 - b[y * sb + x] as i32).unsigned_abs();
            }
        }
        s
    }
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
        assert!(a.len() >= (h - 1) * sa + w && b.len() >= (h - 1) * sb + w && h.is_multiple_of(4));
        let l = level();
        if l == 2 && w == 16 {
            // SAFETY: AVX2 detected; bounds checked.
            return unsafe { satd16_avx2(a.as_ptr(), sa, b.as_ptr(), sb, h) };
        }
        if l >= 1 && w.is_multiple_of(8) {
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
            // last stage as 2·max(|p|, |q|) (see satd16_avx2)
            let ax = _mm_abs_epi16(x1);
            let s2 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(ax, 0b01_00_11_10), 0b01_00_11_10);
            acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_max_epi16(ax, s2), ones));
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
            // last stage: |p + q| + |p − q| = 2·max(|p|, |q|); every lane i holds
            // max(|x1[i]|, |x1[i ^ 2]|), so the lane sum equals Σ|stage output|.
            let ax = _mm256_abs_epi16(x1);
            let s2 = _mm256_shufflelo_epi16(_mm256_shufflehi_epi16(ax, 0b01_00_11_10), 0b01_00_11_10);
            acc = _mm256_add_epi32(acc, _mm256_madd_epi16(_mm256_max_epi16(ax, s2), ones));
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

/// Load 16 lines of 8 bytes (`d[o + l*s .. o + l*s + 8]`) transposed: `out[j][l]`.
#[inline(always)]
pub fn load_t16x8(d: &[u8], o: usize, s: usize) -> [[u8; 16]; 8] {
    assert!(o + 15 * s + 8 <= d.len());
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is baseline.
    unsafe {
        use std::arch::x86_64::*;
        let p = d.as_ptr().add(o);
        let r: [__m128i; 16] = core::array::from_fn(|i| _mm_loadl_epi64(p.add(i * s) as *const __m128i));
        let t: [__m128i; 8] = core::array::from_fn(|i| _mm_unpacklo_epi8(r[2 * i], r[2 * i + 1]));
        // u[g*2 + h]: 4 rows (g) x cols 4h..4h+4, 32-bit units = one column's 4 rows
        let u: [__m128i; 8] = core::array::from_fn(|i| {
            let (g, h) = (i / 2, i % 2);
            if h == 0 {
                _mm_unpacklo_epi16(t[2 * g], t[2 * g + 1])
            } else {
                _mm_unpackhi_epi16(t[2 * g], t[2 * g + 1])
            }
        });
        // w[k*2 + h]: rows 8k..8k+8, two columns per register (64-bit units)
        let mut out = [[0u8; 16]; 8];
        for h in 0..2 {
            let a0 = _mm_unpacklo_epi32(u[h], u[2 + h]); // rows 0-7, cols 4h, 4h+1
            let a1 = _mm_unpackhi_epi32(u[h], u[2 + h]); // rows 0-7, cols 4h+2, 4h+3
            let b0 = _mm_unpacklo_epi32(u[4 + h], u[6 + h]); // rows 8-15
            let b1 = _mm_unpackhi_epi32(u[4 + h], u[6 + h]);
            let c = [
                _mm_unpacklo_epi64(a0, b0),
                _mm_unpackhi_epi64(a0, b0),
                _mm_unpacklo_epi64(a1, b1),
                _mm_unpackhi_epi64(a1, b1),
            ];
            for k in 0..4 {
                _mm_storeu_si128(out[4 * h + k].as_mut_ptr() as *mut __m128i, c[k]);
            }
        }
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let mut out = [[0u8; 16]; 8];
        for l in 0..16 {
            for j in 0..8 {
                out[j][l] = d[o + l * s + j];
            }
        }
        out
    }
}

/// Inverse of [`load_t16x8`].
#[inline(always)]
pub fn store_t16x8(d: &mut [u8], o: usize, s: usize, v: &[[u8; 16]; 8]) {
    assert!(o + 15 * s + 8 <= d.len());
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is baseline.
    unsafe {
        use std::arch::x86_64::*;
        let c: [__m128i; 8] = core::array::from_fn(|j| _mm_loadu_si128(v[j].as_ptr() as *const __m128i));
        // interleave columns pairwise: (c0,c1) -> 16-bit units of (col0,col1) per row
        let t: [__m128i; 8] = core::array::from_fn(|i| {
            let (k, h) = (i / 2, i % 2);
            if h == 0 {
                _mm_unpacklo_epi8(c[2 * k], c[2 * k + 1])
            } else {
                _mm_unpackhi_epi8(c[2 * k], c[2 * k + 1])
            }
        }); // t[2k+h]: rows 8h..8h+8, cols 2k,2k+1
        let p = d.as_mut_ptr().add(o);
        for h in 0..2 {
            let a0 = _mm_unpacklo_epi16(t[h], t[2 + h]); // rows 8h..8h+4, cols 0-3
            let a1 = _mm_unpackhi_epi16(t[h], t[2 + h]); // rows 8h+4..8h+8, cols 0-3
            let b0 = _mm_unpacklo_epi16(t[4 + h], t[6 + h]); // cols 4-7
            let b1 = _mm_unpackhi_epi16(t[4 + h], t[6 + h]);
            let rows = [
                _mm_unpacklo_epi32(a0, b0),
                _mm_unpackhi_epi32(a0, b0),
                _mm_unpacklo_epi32(a1, b1),
                _mm_unpackhi_epi32(a1, b1),
            ];
            for (k, r) in rows.iter().enumerate() {
                let row = 8 * h + 2 * k;
                _mm_storel_epi64(p.add(row * s) as *mut __m128i, *r);
                _mm_storel_epi64(p.add((row + 1) * s) as *mut __m128i, _mm_unpackhi_epi64(*r, *r));
            }
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        for l in 0..16 {
            for j in 0..8 {
                d[o + l * s + j] = v[j][l];
            }
        }
    }
}

#[cfg(test)]
mod ttests {
    use super::*;
    #[test]
    fn transpose_roundtrip() {
        let mut d: Vec<u8> = (0..40 * 16).map(|i| (i * 7 % 251) as u8).collect();
        let t = load_t16x8(&d, 3, 40);
        for l in 0..16 {
            for j in 0..8 {
                assert_eq!(t[j][l], d[3 + l * 40 + j]);
            }
        }
        let orig = d.clone();
        let mut z = t;
        z[0][0] = 1;
        store_t16x8(&mut d, 3, 40, &z);
        assert_eq!(d[3], 1);
        d[3] = orig[3];
        assert_eq!(d, orig);
    }
}

/// Sum of |4×4 Hadamard| of a - b for contiguous 4×4 blocks (not halved).
#[inline(always)]
pub fn satd4x4_sum(a: &[u8; 16], b: &[u8; 16]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 {
            // SAFETY: SSE4.1 detected.
            return unsafe { satd4x4_sse41(a, b) };
        }
    }
    satd_sum_scalar(a, 4, b, 4, 4, 4)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn satd4x4_sse41(a: &[u8; 16], b: &[u8; 16]) -> u32 {
    use std::arch::x86_64::*;
    let va = _mm_loadu_si128(a.as_ptr() as *const __m128i);
    let vb = _mm_loadu_si128(b.as_ptr() as *const __m128i);
    let z = _mm_setzero_si128();
    let d0 = _mm_sub_epi16(_mm_unpacklo_epi8(va, z), _mm_unpacklo_epi8(vb, z)); // rows 0,1
    let d1 = _mm_sub_epi16(_mm_unpackhi_epi8(va, z), _mm_unpackhi_epi8(vb, z)); // rows 2,3
    let s = _mm_add_epi16(d0, d1); // [r0+r2, r1+r3]
    let t = _mm_sub_epi16(d0, d1); // [r0-r2, r1-r3]
    let x = _mm_unpacklo_epi64(s, t);
    let y = _mm_unpackhi_epi64(s, t);
    let mut acc = _mm_setzero_si128();
    for v in [_mm_add_epi16(x, y), _mm_sub_epi16(x, y)] {
        let s1 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(v, 0b10_11_00_01), 0b10_11_00_01);
        let e = _mm_add_epi16(v, s1);
        let f = _mm_sub_epi16(s1, v);
        let x1 = _mm_blend_epi16(e, f, 0b1010_1010);
        // last stage as 2·max(|p|, |q|) (see satd16_avx2)
        let ax = _mm_abs_epi16(x1);
        let s2 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(ax, 0b01_00_11_10), 0b01_00_11_10);
        acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_max_epi16(ax, s2), _mm_set1_epi16(1)));
    }
    let t = _mm_add_epi32(acc, _mm_shuffle_epi32(acc, 0b01_00_11_10));
    let t = _mm_add_epi32(t, _mm_shuffle_epi32(t, 0b10_11_00_01));
    _mm_cvtsi128_si32(t) as u32
}

#[cfg(test)]
mod t4 {
    #[test]
    fn satd4_matches() {
        for k in 0..50u32 {
            let a: [u8; 16] = core::array::from_fn(|i| ((i as u32 * 37 + k * 11) % 256) as u8);
            let b: [u8; 16] = core::array::from_fn(|i| ((i as u32 * 91 + k * 7 + 13) % 256) as u8);
            assert_eq!(super::satd4x4_sum(&a, &b), super::satd_sum_scalar(&a, 4, &b, 4, 4, 4));
        }
    }
}

/// 16×16 block (stride 16) → SoA bytes `v[y*4+x][by*4+bx]`.
#[inline(always)]
pub fn soa16_u8(a: &[u8; 256]) -> [[u8; 16]; 16] {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 {
            // SAFETY: SSE4.1 (⊃ SSSE3) detected.
            return unsafe { soa16_u8_ssse3(a) };
        }
    }
    let mut v = [[0u8; 16]; 16];
    for y in 0..4 {
        for x in 0..4 {
            for by in 0..4 {
                for bx in 0..4 {
                    v[y * 4 + x][by * 4 + bx] = a[(by * 4 + y) * 16 + bx * 4 + x];
                }
            }
        }
    }
    v
}

/// Inverse of [`soa16_u8`].
#[inline(always)]
pub fn unsoa16_u8(v: &[[u8; 16]; 16], a: &mut [u8; 256]) {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 {
            // SAFETY: SSE4.1 detected.
            return unsafe { unsoa16_u8_ssse3(v, a) };
        }
    }
    for y in 0..4 {
        for x in 0..4 {
            for by in 0..4 {
                for bx in 0..4 {
                    a[(by * 4 + y) * 16 + bx * 4 + x] = v[y * 4 + x][by * 4 + bx];
                }
            }
        }
    }
}

/// Two 8×8 blocks → SoA bytes `v[y*4+x][c*4 + by*2+bx]`.
#[inline(always)]
pub fn soa8x2_u8(a: &[[u8; 64]; 2]) -> [[u8; 8]; 16] {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 {
            // SAFETY: SSE4.1 detected.
            return unsafe { soa8x2_u8_ssse3(a) };
        }
    }
    let mut v = [[0u8; 8]; 16];
    for c in 0..2 {
        for y in 0..4 {
            for x in 0..4 {
                for by in 0..2 {
                    for bx in 0..2 {
                        v[y * 4 + x][c * 4 + by * 2 + bx] = a[c][(by * 4 + y) * 8 + bx * 4 + x];
                    }
                }
            }
        }
    }
    v
}

/// Inverse of [`soa8x2_u8`].
#[inline(always)]
pub fn unsoa8x2_u8(v: &[[u8; 8]; 16], a: &mut [[u8; 64]; 2]) {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 {
            // SAFETY: SSE4.1 detected.
            return unsafe { unsoa8x2_u8_ssse3(v, a) };
        }
    }
    for c in 0..2 {
        for y in 0..4 {
            for x in 0..4 {
                for by in 0..2 {
                    for bx in 0..2 {
                        a[c][(by * 4 + y) * 8 + bx * 4 + x] = v[y * 4 + x][c * 4 + by * 2 + bx];
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
mod tx {
    use std::arch::x86_64::*;
    #[inline(always)]
    pub unsafe fn mask() -> __m128i {
        _mm_setr_epi8(0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15)
    }
    /// 4×4 transpose of 32-bit units.
    #[inline(always)]
    pub unsafe fn t4(s: [__m128i; 4]) -> [__m128i; 4] {
        let t0 = _mm_unpacklo_epi32(s[0], s[1]);
        let t1 = _mm_unpacklo_epi32(s[2], s[3]);
        let t2 = _mm_unpackhi_epi32(s[0], s[1]);
        let t3 = _mm_unpackhi_epi32(s[2], s[3]);
        [_mm_unpacklo_epi64(t0, t1), _mm_unpackhi_epi64(t0, t1), _mm_unpacklo_epi64(t2, t3), _mm_unpackhi_epi64(t2, t3)]
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn soa16_u8_ssse3(a: &[u8; 256]) -> [[u8; 16]; 16] {
    use std::arch::x86_64::*;
    let m = tx::mask();
    let mut v = [[0u8; 16]; 16];
    for y in 0..4 {
        let s: [__m128i; 4] = core::array::from_fn(|by| {
            _mm_shuffle_epi8(_mm_loadu_si128(a.as_ptr().add((by * 4 + y) * 16) as *const __m128i), m)
        });
        let t = tx::t4(s);
        for x in 0..4 {
            _mm_storeu_si128(v[y * 4 + x].as_mut_ptr() as *mut __m128i, t[x]);
        }
    }
    v
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn unsoa16_u8_ssse3(v: &[[u8; 16]; 16], a: &mut [u8; 256]) {
    use std::arch::x86_64::*;
    let m = tx::mask();
    for y in 0..4 {
        let t: [__m128i; 4] = core::array::from_fn(|x| _mm_loadu_si128(v[y * 4 + x].as_ptr() as *const __m128i));
        let s = tx::t4(t);
        for by in 0..4 {
            _mm_storeu_si128(a.as_mut_ptr().add((by * 4 + y) * 16) as *mut __m128i, _mm_shuffle_epi8(s[by], m));
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn soa8x2_u8_ssse3(a: &[[u8; 64]; 2]) -> [[u8; 8]; 16] {
    use std::arch::x86_64::*;
    let m = tx::mask();
    let mut v = [[0u8; 8]; 16];
    for y in 0..4 {
        let s: [__m128i; 2] = core::array::from_fn(|c| {
            let r0 = _mm_loadl_epi64(a[c].as_ptr().add(y * 8) as *const __m128i);
            let r1 = _mm_loadl_epi64(a[c].as_ptr().add((4 + y) * 8) as *const __m128i);
            _mm_shuffle_epi8(_mm_unpacklo_epi64(r0, r1), m)
        });
        let lo = _mm_unpacklo_epi32(s[0], s[1]); // x=0 | x=1
        let hi = _mm_unpackhi_epi32(s[0], s[1]); // x=2 | x=3
        _mm_storel_epi64(v[y * 4].as_mut_ptr() as *mut __m128i, lo);
        _mm_storel_epi64(v[y * 4 + 1].as_mut_ptr() as *mut __m128i, _mm_unpackhi_epi64(lo, lo));
        _mm_storel_epi64(v[y * 4 + 2].as_mut_ptr() as *mut __m128i, hi);
        _mm_storel_epi64(v[y * 4 + 3].as_mut_ptr() as *mut __m128i, _mm_unpackhi_epi64(hi, hi));
    }
    v
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn unsoa8x2_u8_ssse3(v: &[[u8; 8]; 16], a: &mut [[u8; 64]; 2]) {
    use std::arch::x86_64::*;
    let m = tx::mask();
    for y in 0..4 {
        let x: [__m128i; 4] = core::array::from_fn(|k| _mm_loadl_epi64(v[y * 4 + k].as_ptr() as *const __m128i));
        let lo = _mm_unpacklo_epi64(x[0], x[1]); // [c0 x0, c1 x0, c0 x1, c1 x1] as dwords
        let hi = _mm_unpacklo_epi64(x[2], x[3]);
        // dwords lo = [c0x0, c1x0, c0x1, c1x1], hi = [c0x2, c1x2, c0x3, c1x3]
        let c0 = _mm_unpacklo_epi64(_mm_shuffle_epi32(lo, 0b11_01_10_00), _mm_shuffle_epi32(hi, 0b11_01_10_00));
        let c1 = _mm_unpackhi_epi64(_mm_shuffle_epi32(lo, 0b11_01_10_00), _mm_shuffle_epi32(hi, 0b11_01_10_00));
        for (c, s) in [c0, c1].into_iter().enumerate() {
            let r = _mm_shuffle_epi8(s, m); // [row y (8 bytes) | row 4+y]
            _mm_storel_epi64(a[c].as_mut_ptr().add(y * 8) as *mut __m128i, r);
            _mm_storel_epi64(a[c].as_mut_ptr().add((4 + y) * 8) as *mut __m128i, _mm_unpackhi_epi64(r, r));
        }
    }
}

#[cfg(test)]
mod soatests {
    use super::*;
    #[test]
    fn soa_roundtrip_matches_scalar() {
        let a: [u8; 256] = core::array::from_fn(|i| (i * 13 % 251) as u8);
        let v = soa16_u8(&a);
        for y in 0..4 {
            for x in 0..4 {
                for by in 0..4 {
                    for bx in 0..4 {
                        assert_eq!(v[y * 4 + x][by * 4 + bx], a[(by * 4 + y) * 16 + bx * 4 + x]);
                    }
                }
            }
        }
        let mut b = [0u8; 256];
        unsoa16_u8(&v, &mut b);
        assert_eq!(a, b);
        let c: [[u8; 64]; 2] = [core::array::from_fn(|i| (i * 7 % 253) as u8), core::array::from_fn(|i| (i * 3 + 100) as u8)];
        let v = soa8x2_u8(&c);
        for k in 0..2 {
            for y in 0..4 {
                for x in 0..4 {
                    for by in 0..2 {
                        for bx in 0..2 {
                            assert_eq!(v[y * 4 + x][k * 4 + by * 2 + bx], c[k][(by * 4 + y) * 8 + bx * 4 + x]);
                        }
                    }
                }
            }
        }
        let mut d = [[0u8; 64]; 2];
        unsoa8x2_u8(&v, &mut d);
        assert_eq!(c, d);
    }
}

/// Rounded average of two 16-byte rows ((a + b + 1) >> 1).
#[inline(always)]
pub fn avg16(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline on x86_64; arrays are 16 bytes.
    unsafe {
        use std::arch::x86_64::*;
        let r = _mm_avg_epu8(_mm_loadu_si128(a.as_ptr() as *const __m128i), _mm_loadu_si128(b.as_ptr() as *const __m128i));
        let mut o = [0u8; 16];
        _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, r);
        o
    }
    #[cfg(not(target_arch = "x86_64"))]
    core::array::from_fn(|i| ((a[i] as u16 + b[i] as u16 + 1) >> 1) as u8)
}

/// Load 8 lines of 4 bytes (`d[o + l*s .. o + l*s + 4]`) transposed: `out[j][l]`.
#[inline(always)]
pub fn load_t8x4(d: &[u8], o: usize, s: usize) -> [[u8; 8]; 4] {
    assert!(o + 7 * s + 4 <= d.len());
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is baseline (unaligned 4-byte reads).
    unsafe {
        use std::arch::x86_64::*;
        let p = d.as_ptr().add(o);
        let r = |l: usize| (p.add(l * s) as *const i32).read_unaligned();
        // Lines in order (0, 2, 1, 3): a byte unpack then a word unpack leave
        // column j in dword j with the lines in order 0, 1, 2, 3.
        let t = |v: __m128i| {
            let x = _mm_unpacklo_epi8(v, _mm_srli_si128::<8>(v));
            _mm_unpacklo_epi16(x, _mm_srli_si128::<8>(x))
        };
        let a = t(_mm_setr_epi32(r(0), r(2), r(1), r(3)));
        let b = t(_mm_setr_epi32(r(4), r(6), r(5), r(7)));
        let mut out = [[0u8; 8]; 4];
        _mm_storeu_si128(out.as_mut_ptr() as *mut __m128i, _mm_unpacklo_epi32(a, b));
        _mm_storeu_si128(out.as_mut_ptr().add(2) as *mut __m128i, _mm_unpackhi_epi32(a, b));
        out
    }
    #[cfg(not(target_arch = "x86_64"))]
    core::array::from_fn(|j| core::array::from_fn(|l| d[o + l * s + j]))
}

/// Store two columns (`p0`, `q0`) of 8 lines: `d[o + l*s] = p0[l]`, `d[o + l*s + 1] = q0[l]`.
#[inline(always)]
pub fn store_t8x2(d: &mut [u8], o: usize, s: usize, p0: &[u8; 8], q0: &[u8; 8]) {
    assert!(o + 7 * s + 2 <= d.len());
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is baseline (unaligned 2-byte writes).
    unsafe {
        use std::arch::x86_64::*;
        let v = _mm_unpacklo_epi8(_mm_loadl_epi64(p0.as_ptr() as *const __m128i), _mm_loadl_epi64(q0.as_ptr() as *const __m128i));
        let mut w = [0u16; 8];
        _mm_storeu_si128(w.as_mut_ptr() as *mut __m128i, v);
        let p = d.as_mut_ptr().add(o);
        for (l, &x) in w.iter().enumerate() {
            (p.add(l * s) as *mut u16).write_unaligned(x);
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    for l in 0..8 {
        d[o + l * s] = p0[l];
        d[o + l * s + 1] = q0[l];
    }
}

/// Copy a `W`×`h` block between strided buffers: one bounds check per block
/// instead of one per row.
#[inline(always)]
pub fn copy_block<const W: usize>(dst: &mut [u8], doff: usize, ds: usize, src: &[u8], soff: usize, ss: usize, h: usize) {
    assert!(h > 0 && doff + (h - 1) * ds + W <= dst.len() && soff + (h - 1) * ss + W <= src.len());
    // SAFETY: the assert bounds every row; `dst` and `src` are distinct borrows.
    unsafe {
        let (d, s) = (dst.as_mut_ptr().add(doff), src.as_ptr().add(soff));
        for y in 0..h {
            core::ptr::copy_nonoverlapping(s.add(y * ss), d.add(y * ds), W);
        }
    }
}

/// Chroma bilinear MC of an 8×8 block at 1/8-sample phase (fx, fy) (8.4.2.2.2):
/// `((8−fx)(8−fy)·A + fx(8−fy)·B + (8−fx)fy·C + fx·fy·D + 32) >> 6`, rows from
/// `d[o ..]` with stride `s`.
pub fn chroma_bilin8(d: &[u8], o: usize, s: usize, fx: i32, fy: i32, out: &mut [u8; 64]) {
    assert!(o + 8 * s + 9 <= d.len() && (0..8).contains(&fx) && (0..8).contains(&fy));
    let (w00, w01, w10, w11) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
    #[cfg(target_arch = "x86_64")]
    if level() >= 1 {
        #[target_feature(enable = "ssse3")]
        unsafe fn f(p: *const u8, s: usize, w: [i32; 4], out: &mut [u8; 64]) {
            use std::arch::x86_64::*;
            // weights as (A, B) byte pairs for maddubs; every sum fits i16 (Σw = 64)
            let wa = _mm_set1_epi16(((w[1] << 8) | w[0]) as i16);
            let wb = _mm_set1_epi16(((w[3] << 8) | w[2]) as i16);
            let r32 = _mm_set1_epi16(32);
            let pair = |q: *const u8| _mm_unpacklo_epi8(_mm_loadl_epi64(q as *const __m128i), _mm_loadl_epi64(q.add(1) as *const __m128i));
            let mut top = pair(p);
            for y in 0..8 {
                let bot = pair(p.add((y + 1) * s));
                let v = _mm_add_epi16(_mm_maddubs_epi16(top, wa), _mm_maddubs_epi16(bot, wb));
                let v = _mm_srli_epi16::<6>(_mm_add_epi16(v, r32));
                _mm_storel_epi64(out.as_mut_ptr().add(y * 8) as *mut __m128i, _mm_packus_epi16(v, v));
                top = bot;
            }
        }
        // SAFETY: the assert bounds every read (9 bytes × 9 rows); SSSE3 detected.
        unsafe { f(d.as_ptr().add(o), s, [w00, w01, w10, w11], out) };
        return;
    }
    let (w00, w01, w10, w11) = (w00 as u16, w01 as u16, w10 as u16, w11 as u16);
    for y in 0..8 {
        let r0 = &d[o + y * s..o + y * s + 9];
        let r1 = &d[o + (y + 1) * s..o + (y + 1) * s + 9];
        for x in 0..8 {
            let v = w00 * r0[x] as u16 + w01 * r0[x + 1] as u16 + w10 * r1[x] as u16 + w11 * r1[x + 1] as u16;
            out[y * 8 + x] = ((v + 32) >> 6) as u8;
        }
    }
}

/// Sum of |second differences| of a 16×16 block, horizontally (x 1..15) and
/// vertically (y 1..15).
#[inline]
pub fn second_diff16(p: &[u8; 256]) -> u32 {
    let mut s = 0u32;
    for y in 0..16 {
        let r = &p[y * 16..y * 16 + 16];
        for x in 1..15 {
            s += (r[x - 1] as i32 + r[x + 1] as i32 - 2 * r[x] as i32).unsigned_abs();
        }
    }
    for y in 1..15 {
        for x in 0..16 {
            s += (p[(y - 1) * 16 + x] as i32 + p[(y + 1) * 16 + x] as i32 - 2 * p[y * 16 + x] as i32).unsigned_abs();
        }
    }
    s
}

/// SSIM statistics of two 8×8 blocks starting at `a[0]` and `b[0]` (row
/// strides `sa`, `sb`): Σa, Σb, Σa², Σb², Σab.
#[inline]
pub fn ssim_stats8(a: &[u8], sa: usize, b: &[u8], sb: usize) -> [u32; 5] {
    assert!(a.len() >= 7 * sa + 8 && b.len() >= 7 * sb + 8);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline on x86_64; every 8-byte load is inside `a`/`b` (asserted).
    unsafe {
        use std::arch::x86_64::*;
        let z = _mm_setzero_si128();
        let (mut s, mut aa, mut bb, mut ab) = (z, z, z, z);
        for y in 0..8 {
            let va = _mm_unpacklo_epi8(_mm_loadl_epi64(a.as_ptr().add(y * sa) as *const __m128i), z);
            let vb = _mm_unpacklo_epi8(_mm_loadl_epi64(b.as_ptr().add(y * sb) as *const __m128i), z);
            // Σa in the low 4 lanes' pairs, Σb in the high: pack both as i16 sums
            s = _mm_add_epi16(s, _mm_unpacklo_epi64(_mm_hadd_epi16_sse2(va), _mm_hadd_epi16_sse2(vb)));
            aa = _mm_add_epi32(aa, _mm_madd_epi16(va, va));
            bb = _mm_add_epi32(bb, _mm_madd_epi16(vb, vb));
            ab = _mm_add_epi32(ab, _mm_madd_epi16(va, vb));
        }
        let hsum = |v: __m128i| {
            let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0b01_00_11_10>(v));
            let v = _mm_add_epi32(v, _mm_shuffle_epi32::<0b10_11_00_01>(v));
            _mm_cvtsi128_si32(v) as u32
        };
        // s: lanes 0..3 hold Σa partials, 4..7 Σb partials (i16, ≤ 8·2·255)
        let one = _mm_set1_epi16(1);
        let s32 = _mm_madd_epi16(s, one); // [a01, a23, b01, b23]
        let lo = _mm_add_epi32(s32, _mm_shuffle_epi32::<0b10_11_00_01>(s32));
        let (sum_a, sum_b) = (_mm_cvtsi128_si32(lo) as u32, _mm_cvtsi128_si32(_mm_unpackhi_epi64(lo, lo)) as u32);
        [sum_a, sum_b, hsum(aa), hsum(bb), hsum(ab)]
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let mut r = [0u32; 5];
        for y in 0..8 {
            for x in 0..8 {
                let (p, q) = (a[y * sa + x] as u32, b[y * sb + x] as u32);
                r[0] += p;
                r[1] += q;
                r[2] += p * p;
                r[3] += q * q;
                r[4] += p * q;
            }
        }
        r
    }
}

/// Pairwise sums of eight i16 lanes into four (SSE2 only; `_mm_hadd_epi16`
/// needs SSSE3).
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn _mm_hadd_epi16_sse2(v: std::arch::x86_64::__m128i) -> std::arch::x86_64::__m128i {
    use std::arch::x86_64::*;
    // madd with 1s gives four i32 pair sums; pack back to i16 (values ≤ 510)
    let p = _mm_madd_epi16(v, _mm_set1_epi16(1));
    _mm_packs_epi32(p, p)
}

/// Sum and sum of squares of a block whose length is a multiple of 16.
#[inline]
pub fn sum_sq(p: &[u8]) -> (u32, u32) {
    assert!(p.len().is_multiple_of(16));
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline on x86_64; every load is inside `p`.
    unsafe {
        use std::arch::x86_64::*;
        let z = _mm_setzero_si128();
        let (mut s, mut q) = (z, z);
        for k in (0..p.len()).step_by(16) {
            let v = _mm_loadu_si128(p.as_ptr().add(k) as *const __m128i);
            s = _mm_add_epi64(s, _mm_sad_epu8(v, z));
            let (lo, hi) = (_mm_unpacklo_epi8(v, z), _mm_unpackhi_epi8(v, z));
            q = _mm_add_epi32(q, _mm_add_epi32(_mm_madd_epi16(lo, lo), _mm_madd_epi16(hi, hi)));
        }
        let s = (_mm_cvtsi128_si64(s) + _mm_cvtsi128_si64(_mm_unpackhi_epi64(s, s))) as u32;
        let q = _mm_add_epi32(q, _mm_shuffle_epi32::<0b01_00_11_10>(q));
        let q = _mm_add_epi32(q, _mm_shuffle_epi32::<0b10_11_00_01>(q));
        (s, _mm_cvtsi128_si32(q) as u32)
    }
    #[cfg(not(target_arch = "x86_64"))]
    p.iter().fold((0, 0), |(s, q), &v| (s + v as u32, q + v as u32 * v as u32))
}

/// Saturating pack of 16 i16 lanes to u8.
#[inline(always)]
pub fn pack16(v: &[i16; 16]) -> [u8; 16] {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline; arrays are 32 / 16 bytes.
    unsafe {
        use std::arch::x86_64::*;
        let a = _mm_loadu_si128(v.as_ptr() as *const __m128i);
        let b = _mm_loadu_si128(v.as_ptr().add(8) as *const __m128i);
        let mut o = [0u8; 16];
        _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, _mm_packus_epi16(a, b));
        o
    }
    #[cfg(not(target_arch = "x86_64"))]
    core::array::from_fn(|i| v[i].clamp(0, 255) as u8)
}

/// Quantise 8 lanes: `sign(v) * min(((|v| +sat bias) * m) >> 16, 2047)`; returns
/// the levels and a per-lane non-zero mask (-1 / 0).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
#[inline]
pub unsafe fn quant8_sse41(v: &mut [i16], m: u16, bias: u16, cnt: &mut [i16]) {
    use std::arch::x86_64::*;
    let x = _mm_loadu_si128(v.as_ptr() as *const __m128i);
    let a = _mm_adds_epu16(_mm_abs_epi16(x), _mm_set1_epi16(bias as i16));
    let l = _mm_min_epu16(_mm_mulhi_epu16(a, _mm_set1_epi16(m as i16)), _mm_set1_epi16(2047));
    let r = _mm_sign_epi16(l, x);
    _mm_storeu_si128(v.as_mut_ptr() as *mut __m128i, r);
    let nz = _mm_cmpeq_epi16(l, _mm_setzero_si128()); // -1 where zero
    let c = _mm_loadu_si128(cnt.as_ptr() as *const __m128i);
    // cnt += (l != 0) == cnt - (nz + 1)... use: cnt + 1 + nz
    let c = _mm_add_epi16(_mm_add_epi16(c, _mm_set1_epi16(1)), nz);
    _mm_storeu_si128(cnt.as_mut_ptr() as *mut __m128i, c);
}

/// SAD of two contiguous 64-byte blocks (8×8).
#[inline(always)]
pub fn sad64(a: &[u8; 64], b: &[u8; 64]) -> u32 {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline; arrays are 64 bytes.
    unsafe {
        use std::arch::x86_64::*;
        let mut acc = _mm_setzero_si128();
        for i in 0..4 {
            let x = _mm_loadu_si128(a.as_ptr().add(i * 16) as *const __m128i);
            let y = _mm_loadu_si128(b.as_ptr().add(i * 16) as *const __m128i);
            acc = _mm_add_epi64(acc, _mm_sad_epu8(x, y));
        }
        (_mm_cvtsi128_si64(acc) + _mm_extract_epi16(acc, 4) as i64) as u32
    }
    #[cfg(not(target_arch = "x86_64"))]
    a.iter().zip(b).map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs()).sum()
}

/// `clip((base + b*x) >> 5)` for x in 0..16.
#[inline(always)]
pub fn plane_row16(base: i32, b: i32) -> [u8; 16] {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: SSE2 is baseline.
    unsafe {
        use std::arch::x86_64::*;
        let v: [__m128i; 4] = core::array::from_fn(|k| {
            let k = k as i32 * 4;
            _mm_srai_epi32(_mm_setr_epi32(base + b * k, base + b * (k + 1), base + b * (k + 2), base + b * (k + 3)), 5)
        });
        let lo = _mm_packs_epi32(v[0], v[1]);
        let hi = _mm_packs_epi32(v[2], v[3]);
        let mut o = [0u8; 16];
        _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, _mm_packus_epi16(lo, hi));
        o
    }
    #[cfg(not(target_arch = "x86_64"))]
    core::array::from_fn(|x| ((base + b * x as i32) >> 5).clamp(0, 255) as u8)
}

/// Maximum SAD over the 4×4 blocks of a w×h block (w = 16 or 8, rows of `w`).
#[inline(always)]
pub fn max_sad4(a: &[u8], b: &[u8], w: usize, h: usize) -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        if level() >= 1 && w == 16 && a.len() >= 16 * h && b.len() >= 16 * h {
            // SAFETY: SSSE3 detected (SSE4.1 implies it); lengths checked.
            return unsafe { max_sad4_16_ssse3(a.as_ptr(), b.as_ptr(), h) };
        }
        if level() >= 1 && w == 8 && h == 8 && a.len() >= 64 && b.len() >= 64 {
            // SAFETY: as above.
            return unsafe { max_sad4_8x8_ssse3(a.as_ptr(), b.as_ptr()) };
        }
    }
    let mut m = 0u32;
    for by in (0..h).step_by(4) {
        for bx in (0..w).step_by(4) {
            let mut s = 0u32;
            for y in 0..4 {
                let o = (by + y) * w + bx;
                for x in 0..4 {
                    s += (a[o + x] as i32 - b[o + x] as i32).unsigned_abs();
                }
            }
            m = m.max(s);
        }
    }
    m
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn max_sad4_8x8_ssse3(a: *const u8, b: *const u8) -> u32 {
    use std::arch::x86_64::*;
    let ones8 = _mm_set1_epi8(1);
    let ones16 = _mm_set1_epi16(1);
    let mut m = 0u32;
    for half in 0..2 {
        // two 16-byte loads = rows 4h..4h+4; lanes: [b0 r, b1 r, b0 r+1, b1 r+1]
        let mut acc = _mm_setzero_si128();
        for k in 0..2 {
            let off = half * 32 + k * 16;
            let x = _mm_loadu_si128(a.add(off) as *const __m128i);
            let z = _mm_loadu_si128(b.add(off) as *const __m128i);
            let d = _mm_sub_epi8(_mm_max_epu8(x, z), _mm_min_epu8(x, z));
            acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_maddubs_epi16(d, ones8), ones16));
        }
        let t = _mm_add_epi32(acc, _mm_shuffle_epi32(acc, 0b01_00_11_10)); // lanes 0,1 = blocks
        let b0 = _mm_cvtsi128_si32(t) as u32;
        let b1 = _mm_cvtsi128_si32(_mm_shuffle_epi32(t, 1)) as u32;
        m = m.max(b0).max(b1);
    }
    m
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "ssse3")]
unsafe fn max_sad4_16_ssse3(a: *const u8, b: *const u8, h: usize) -> u32 {
    use std::arch::x86_64::*;
    let ones8 = _mm_set1_epi8(1);
    let ones16 = _mm_set1_epi16(1);
    let mut mx = _mm_setzero_si128();
    let mut by = 0;
    while by < h {
        let mut acc = _mm_setzero_si128();
        for y in 0..4 {
            let x = _mm_loadu_si128(a.add((by + y) * 16) as *const __m128i);
            let z = _mm_loadu_si128(b.add((by + y) * 16) as *const __m128i);
            let d = _mm_sub_epi8(_mm_max_epu8(x, z), _mm_min_epu8(x, z));
            // bytes → pair sums (u16) → quad sums (i32): one per 4×4 block column
            acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_maddubs_epi16(d, ones8), ones16));
        }
        mx = _mm_max_epi16(mx, acc); // sums ≤ 4080 fit the low 16 bits
        by += 4;
    }
    let m = _mm_max_epi16(mx, _mm_shuffle_epi32(mx, 0b01_00_11_10));
    let m = _mm_max_epi16(m, _mm_shuffle_epi32(m, 0b10_11_00_01));
    (_mm_cvtsi128_si32(m) & 0xffff) as u32
}

#[cfg(test)]
mod maxsad_test {
    #[test]
    fn chroma_bilin8_matches() {
        let d: Vec<u8> = (0..20 * 12).map(|i| (i * 53 % 256) as u8).collect();
        for fy in 0..8 {
            for fx in 0..8 {
                let mut a = [0u8; 64];
                super::chroma_bilin8(&d, 3, 20, fx, fy, &mut a);
                let (w00, w01, w10, w11) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
                for y in 0..8 {
                    for x in 0..8 {
                        let p = |xx: usize, yy: usize| d[3 + yy * 20 + xx] as i32;
                        let v = (w00 * p(x, y) + w01 * p(x + 1, y) + w10 * p(x, y + 1) + w11 * p(x + 1, y + 1) + 32) >> 6;
                        assert_eq!(a[y * 8 + x] as i32, v, "{fx},{fy} {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn transposes_8x4() {
        let mut d: Vec<u8> = (0..20 * 10).map(|i| (i * 31 % 256) as u8).collect();
        let t = super::load_t8x4(&d, 5, 20);
        for j in 0..4 {
            for l in 0..8 {
                assert_eq!(t[j][l], d[5 + l * 20 + j]);
            }
        }
        let (p, q): ([u8; 8], [u8; 8]) = (core::array::from_fn(|i| i as u8 + 100), core::array::from_fn(|i| i as u8 + 200));
        super::store_t8x2(&mut d, 6, 20, &p, &q);
        for l in 0..8 {
            assert_eq!((d[6 + l * 20], d[7 + l * 20]), (p[l], q[l]));
        }
    }

    #[test]
    fn sum_sq_matches() {
        let p: Vec<u8> = (0..256).map(|i| (i * 97 % 256) as u8).collect();
        let r = p.iter().fold((0u32, 0u32), |(s, q), &v| (s + v as u32, q + v as u32 * v as u32));
        assert_eq!(super::sum_sq(&p), r);
        assert_eq!(super::sum_sq(&p[..64]), p[..64].iter().fold((0u32, 0u32), |(s, q), &v| (s + v as u32, q + v as u32 * v as u32)));
    }

    #[test]
    fn max_sad4_matches() {
        for k in 0..20u32 {
            let a: Vec<u8> = (0..256u32).map(|i| ((i * 37 + k * 11) % 256) as u8).collect();
            let b: Vec<u8> = (0..256u32).map(|i| ((i * 91 + k * 7) % 256) as u8).collect();
            let mut m = 0;
            for by in 0..4 {
                for bx in 0..4 {
                    let mut s = 0u32;
                    for y in 0..4 {
                        for x in 0..4 {
                            let o = (by * 4 + y) * 16 + bx * 4 + x;
                            s += (a[o] as i32 - b[o] as i32).unsigned_abs();
                        }
                    }
                    m = m.max(s);
                }
            }
            assert_eq!(super::max_sad4(&a, &b, 16, 16), m);
            let mut mc = 0;
            for by in 0..2 {
                for bx in 0..2 {
                    let mut s = 0u32;
                    for y in 0..4 {
                        for x in 0..4 {
                            let o = (by * 4 + y) * 8 + bx * 4 + x;
                            s += (a[o] as i32 - b[o] as i32).unsigned_abs();
                        }
                    }
                    mc = mc.max(s);
                }
            }
            assert_eq!(super::max_sad4(&a[..64], &b[..64], 8, 8), mc);
        }
    }
}

#[cfg(test)]
mod ssim_tests {
    #[test]
    fn ssim_stats8_matches_scalar() {
        let a: Vec<u8> = (0..24 * 8).map(|i| (i * 37 % 256) as u8).collect();
        let b: Vec<u8> = (0..20 * 8).map(|i| (255 - i * 11 % 256) as u8).collect();
        let mut r = [0u32; 5];
        for y in 0..8 {
            for x in 0..8 {
                let (p, q) = (a[y * 24 + x] as u32, b[y * 20 + x] as u32);
                r[0] += p;
                r[1] += q;
                r[2] += p * p;
                r[3] += q * q;
                r[4] += p * q;
            }
        }
        assert_eq!(super::ssim_stats8(&a, 24, &b, 20), r);
    }
}
