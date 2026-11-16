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
        let s2 = _mm_shufflelo_epi16(_mm_shufflehi_epi16(x1, 0b01_00_11_10), 0b01_00_11_10);
        let e = _mm_add_epi16(x1, s2);
        let f = _mm_sub_epi16(s2, x1);
        let x2 = _mm_blend_epi16(e, f, 0b1100_1100);
        acc = _mm_add_epi32(acc, _mm_madd_epi16(_mm_abs_epi16(x2), _mm_set1_epi16(1)));
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
