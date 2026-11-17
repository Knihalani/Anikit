//! Sample interpolation kernels.
//!
//! Positions are 32.32 fixed point in frames of the padded sample buffer, so
//! phase never drifts. The SIMD paths (SSE2 baseline, AVX when present) are
//! bit-identical to the portable scalar code: every lane performs the same
//! IEEE operations in the same order, and no fused multiply-adds are used.
//!
//! - [`Interp::Cubic`]: 4-point, 3rd-order Hermite (Catmull-Rom).
//! - [`Interp::Sinc`]: 16-tap Kaiser-windowed sinc, 2048 phases. When a
//!   sample is played faster than its rate (pitched up), the cut-off follows
//!   the output Nyquist in quarter-octave steps up to 4× so transposition
//!   does not alias.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// Interpolation quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interp {
    /// 4-point cubic Hermite.
    Cubic = 0,
    /// 16-tap windowed-sinc polyphase with anti-aliasing for transposition.
    Sinc = 1,
}

static INTERP: AtomicU8 = AtomicU8::new(Interp::Sinc as u8);

/// Set the default interpolation for SoundFonts loaded from now on
/// (default [`Interp::Sinc`]); see also `SoundFont::set_interp`.
pub fn set_interp(i: Interp) {
    INTERP.store(i as u8, Ordering::Relaxed);
}

/// The default interpolation.
pub fn interp() -> Interp {
    from_u8(INTERP.load(Ordering::Relaxed))
}

/// Decode an [`Interp`] stored as `u8`.
pub fn from_u8(v: u8) -> Interp {
    if v == Interp::Cubic as u8 {
        Interp::Cubic
    } else {
        Interp::Sinc
    }
}

/// Fixed-point one.
pub const ONE: u64 = 1 << 32;

/// Sinc taps.
pub const TAPS: usize = 16;
const PBITS: u32 = 11;
const PHASES: usize = 1 << PBITS;
/// Number of anti-alias cut-off steps (quarter octaves, 1× … 4×).
pub const BUCKETS: usize = 9;
/// Base cut-off relative to the input Nyquist frequency.
const CUTOFF: f64 = 0.92;
const BETA: f64 = 8.0;
const FRAC: f32 = 1.0 / 16_777_216.0;

/// Frames left of the read index a kernel touches.
pub const fn left(i: Interp) -> usize {
    match i {
        Interp::Cubic => 1,
        Interp::Sinc => TAPS / 2 - 1,
    }
}
/// Frames right of the read index a kernel touches.
pub const fn right(i: Interp) -> usize {
    match i {
        Interp::Cubic => 2,
        Interp::Sinc => TAPS / 2,
    }
}

/// One phase of the polyphase filter (one cache line).
#[repr(C, align(64))]
#[derive(Clone, Copy)]
pub struct Row(pub [f32; TAPS]);

/// One cut-off's polyphase table (2048 phases, nearest phase: the timing
/// error is at most 1/4096 frame).
pub struct SincTable {
    rows: Vec<Row>,
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..64 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

fn build(scale: f64) -> SincTable {
    let half = (TAPS / 2) as f64;
    let i0b = bessel_i0(BETA);
    let mut rows = Vec::with_capacity(PHASES);
    for p in 0..PHASES {
        // centre of the phase interval this row serves
        let x = (p as f64 + 0.5) / PHASES as f64;
        let mut row = [0f64; TAPS];
        for (t, r) in row.iter_mut().enumerate() {
            // tap t sits at offset j = t − (TAPS/2 − 1) from the read index
            let u = (t as f64 - (half - 1.0)) - x;
            let w = if u.abs() >= half { 0.0 } else { bessel_i0(BETA * (1.0 - (u / half).powi(2)).sqrt()) / i0b };
            let a = std::f64::consts::PI * u * scale;
            let s = if a.abs() < 1e-12 { 1.0 } else { a.sin() / a };
            *r = s * w;
        }
        let sum: f64 = row.iter().sum();
        rows.push(Row(row.map(|v| (v / sum) as f32)));
    }
    SincTable { rows }
}

/// The polyphase table for an anti-alias bucket (built on first use).
pub fn table(bucket: usize) -> &'static SincTable {
    static T: [OnceLock<SincTable>; BUCKETS] = [const { OnceLock::new() }; BUCKETS];
    let b = bucket.min(BUCKETS - 1);
    T[b].get_or_init(|| build(CUTOFF / (b as f64 / 4.0).exp2()))
}

/// Anti-alias bucket for a phase increment (frames of input per output frame).
#[inline]
pub fn bucket(inc: f64) -> usize {
    if inc <= 1.0 {
        0
    } else {
        ((inc.log2() * 4.0).ceil() as usize).min(BUCKETS - 1)
    }
}

#[inline(always)]
fn frac24(pos: u64) -> f32 {
    ((pos as u32) >> 8) as f32 * FRAC
}

#[inline(always)]
fn cubic_one(s: &[f32], x: f32) -> f32 {
    let (ym1, y0, y1, y2) = (s[0], s[1], s[2], s[3]);
    let c1 = 0.5 * (y1 - ym1);
    let c2 = ((ym1 - 2.5 * y0) + 2.0 * y1) - 0.5 * y2;
    let c3 = 0.5 * (y2 - ym1) + 1.5 * (y0 - y1);
    ((c3 * x + c2) * x + c1) * x + y0
}

#[inline(always)]
fn sinc_one(s: &[f32], c: &[f32; TAPS]) -> f32 {
    let mut b = [0f32; 4];
    for (m, bm) in b.iter_mut().enumerate() {
        let lo = s[m] * c[m] + s[m + 8] * c[m + 8];
        let hi = s[m + 4] * c[m + 4] + s[m + 12] * c[m + 12];
        *bm = lo + hi;
    }
    (b[0] + b[1]) + (b[2] + b[3])
}

fn check(d: &[f32], pos: u64, inc: u64, n: usize, l: usize, r: usize) {
    let first = (pos >> 32) as usize;
    let last = (pos.wrapping_add(inc.wrapping_mul(n.saturating_sub(1) as u64)) >> 32) as usize;
    assert!(first >= l && last + r < d.len(), "interpolation window out of bounds");
}

/// Portable cubic Hermite (bit-identical to the SIMD path).
pub fn cubic_scalar(d: &[f32], mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    for o in out.iter_mut() {
        let i = (pos >> 32) as usize;
        *o = cubic_one(&d[i - 1..i + 3], frac24(pos));
        pos = pos.wrapping_add(inc);
    }
    pos
}

/// Portable windowed sinc (bit-identical to the SIMD path).
pub fn sinc_scalar(d: &[f32], tab: &SincTable, mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    for o in out.iter_mut() {
        let i = (pos >> 32) as usize;
        let c = &tab.rows[((pos as u32) >> (32 - PBITS)) as usize].0;
        *o = sinc_one(&d[i + 1 - TAPS / 2..i + 1 + TAPS / 2], c);
        pos = pos.wrapping_add(inc);
    }
    pos
}

/// Render `out.len()` frames by cubic Hermite interpolation; returns the new position.
#[inline]
pub fn cubic(d: &[f32], pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    check(d, pos, inc, out.len(), 1, 2);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is part of the x86_64 baseline.
    unsafe {
        x86::cubic(d, pos, inc, out)
    }
    #[cfg(not(target_arch = "x86_64"))]
    cubic_scalar(d, pos, inc, out)
}

/// Render `out.len()` frames with the windowed-sinc table; returns the new position.
#[inline]
pub fn sinc(d: &[f32], tab: &SincTable, pos: u64, inc: u64, out: &mut [f32]) -> u64 {
    check(d, pos, inc, out.len(), TAPS / 2 - 1, TAPS / 2);
    #[cfg(target_arch = "x86_64")]
    // SAFETY: bounds checked above; SSE2 is part of the x86_64 baseline and
    // the AVX path only runs when the CPU reports AVX.
    unsafe {
        if x86::has_avx() {
            x86::sinc_avx(d, tab, pos, inc, out)
        } else {
            x86::sinc_sse2(d, tab, pos, inc, out)
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    sinc_scalar(d, tab, pos, inc, out)
}

/// Whether the CPU supports AVX2 (cached).
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    static F: AtomicU8 = AtomicU8::new(0);
    match F.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let a = std::is_x86_feature_detected!("avx2");
            F.store(if a { 2 } else { 1 }, Ordering::Relaxed);
            a
        }
    }
}

#[cfg(target_arch = "x86_64")]
mod x86 {
    use super::*;
    use core::arch::x86_64::*;

    pub fn has_avx() -> bool {
        static F: AtomicU8 = AtomicU8::new(0);
        match F.load(Ordering::Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let a = std::is_x86_feature_detected!("avx");
                F.store(if a { 2 } else { 1 }, Ordering::Relaxed);
                a
            }
        }
    }

    #[inline(always)]
    unsafe fn transpose(r0: __m128, r1: __m128, r2: __m128, r3: __m128) -> (__m128, __m128, __m128, __m128) {
        let t0 = _mm_unpacklo_ps(r0, r1);
        let t1 = _mm_unpacklo_ps(r2, r3);
        let t2 = _mm_unpackhi_ps(r0, r1);
        let t3 = _mm_unpackhi_ps(r2, r3);
        (_mm_movelh_ps(t0, t1), _mm_movehl_ps(t1, t0), _mm_movelh_ps(t2, t3), _mm_movehl_ps(t3, t2))
    }

    #[inline(always)]
    unsafe fn fracs(q0: u64, q1: u64, q2: u64, q3: u64) -> __m128 {
        let f = _mm_set_epi32(
            ((q3 as u32) >> 8) as i32,
            ((q2 as u32) >> 8) as i32,
            ((q1 as u32) >> 8) as i32,
            ((q0 as u32) >> 8) as i32,
        );
        _mm_mul_ps(_mm_cvtepi32_ps(f), _mm_set1_ps(FRAC))
    }

    pub unsafe fn cubic(d: &[f32], mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
        let n = out.len();
        let p = d.as_ptr();
        let o = out.as_mut_ptr();
        let (half, k25, two, k15) = (_mm_set1_ps(0.5), _mm_set1_ps(2.5), _mm_set1_ps(2.0), _mm_set1_ps(1.5));
        let mut i = 0;
        while i + 4 <= n {
            let q0 = pos;
            let q1 = q0.wrapping_add(inc);
            let q2 = q1.wrapping_add(inc);
            let q3 = q2.wrapping_add(inc);
            let r0 = _mm_loadu_ps(p.add((q0 >> 32) as usize - 1));
            let r1 = _mm_loadu_ps(p.add((q1 >> 32) as usize - 1));
            let r2 = _mm_loadu_ps(p.add((q2 >> 32) as usize - 1));
            let r3 = _mm_loadu_ps(p.add((q3 >> 32) as usize - 1));
            let (ym1, y0, y1, y2) = transpose(r0, r1, r2, r3);
            let x = fracs(q0, q1, q2, q3);
            let c1 = _mm_mul_ps(half, _mm_sub_ps(y1, ym1));
            let c2 = _mm_sub_ps(_mm_add_ps(_mm_sub_ps(ym1, _mm_mul_ps(k25, y0)), _mm_mul_ps(two, y1)), _mm_mul_ps(half, y2));
            let c3 = _mm_add_ps(_mm_mul_ps(half, _mm_sub_ps(y2, ym1)), _mm_mul_ps(k15, _mm_sub_ps(y0, y1)));
            let r = _mm_add_ps(_mm_mul_ps(_mm_add_ps(_mm_mul_ps(_mm_add_ps(_mm_mul_ps(c3, x), c2), x), c1), x), y0);
            _mm_storeu_ps(o.add(i), r);
            pos = q3.wrapping_add(inc);
            i += 4;
        }
        cubic_scalar(d, pos, inc, &mut out[i..])
    }

    #[inline(always)]
    unsafe fn row(tab: &SincTable, q: u64) -> *const f32 {
        tab.rows.get_unchecked(((q as u32) >> (32 - PBITS)) as usize).0.as_ptr()
    }

    /// The four partial sums b[m] = (s[m]c[m] + s[m+8]c[m+8]) + (s[m+4]c[m+4] + s[m+12]c[m+12]).
    #[inline(always)]
    unsafe fn b_sse(p: *const f32, q: u64, tab: &SincTable) -> __m128 {
        let s = p.add((q >> 32) as usize + 1 - TAPS / 2);
        let c = row(tab, q);
        let lo = _mm_add_ps(_mm_mul_ps(_mm_loadu_ps(s), _mm_load_ps(c)), _mm_mul_ps(_mm_loadu_ps(s.add(8)), _mm_load_ps(c.add(8))));
        let hi = _mm_add_ps(
            _mm_mul_ps(_mm_loadu_ps(s.add(4)), _mm_load_ps(c.add(4))),
            _mm_mul_ps(_mm_loadu_ps(s.add(12)), _mm_load_ps(c.add(12))),
        );
        _mm_add_ps(lo, hi)
    }

    #[inline(always)]
    unsafe fn finish(b0: __m128, b1: __m128, b2: __m128, b3: __m128) -> __m128 {
        let (c0, c1, c2, c3) = transpose(b0, b1, b2, b3);
        _mm_add_ps(_mm_add_ps(c0, c1), _mm_add_ps(c2, c3))
    }

    pub unsafe fn sinc_sse2(d: &[f32], tab: &SincTable, mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
        let n = out.len();
        let p = d.as_ptr();
        let o = out.as_mut_ptr();
        let mut i = 0;
        while i + 4 <= n {
            let q0 = pos;
            let q1 = q0.wrapping_add(inc);
            let q2 = q1.wrapping_add(inc);
            let q3 = q2.wrapping_add(inc);
            let r = finish(b_sse(p, q0, tab), b_sse(p, q1, tab), b_sse(p, q2, tab), b_sse(p, q3, tab));
            _mm_storeu_ps(o.add(i), r);
            pos = q3.wrapping_add(inc);
            i += 4;
        }
        sinc_scalar(d, tab, pos, inc, &mut out[i..])
    }

    #[inline(always)]
    unsafe fn b_avx(p: *const f32, q: u64, tab: &SincTable) -> __m128 {
        let s = p.add((q >> 32) as usize + 1 - TAPS / 2);
        let c = row(tab, q);
        // lanes l: s[l]c[l] + s[l+8]c[l+8]; then lane m + lane m+4
        let a = _mm256_add_ps(
            _mm256_mul_ps(_mm256_loadu_ps(s), _mm256_load_ps(c)),
            _mm256_mul_ps(_mm256_loadu_ps(s.add(8)), _mm256_load_ps(c.add(8))),
        );
        _mm_add_ps(_mm256_castps256_ps128(a), _mm256_extractf128_ps(a, 1))
    }

    #[target_feature(enable = "avx")]
    pub unsafe fn sinc_avx(d: &[f32], tab: &SincTable, mut pos: u64, inc: u64, out: &mut [f32]) -> u64 {
        let n = out.len();
        let p = d.as_ptr();
        let o = out.as_mut_ptr();
        let mut i = 0;
        while i + 4 <= n {
            let q0 = pos;
            let q1 = q0.wrapping_add(inc);
            let q2 = q1.wrapping_add(inc);
            let q3 = q2.wrapping_add(inc);
            let r = finish(b_avx(p, q0, tab), b_avx(p, q1, tab), b_avx(p, q2, tab), b_avx(p, q3, tab));
            _mm_storeu_ps(o.add(i), r);
            pos = q3.wrapping_add(inc);
            i += 4;
        }
        sinc_scalar(d, tab, pos, inc, &mut out[i..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sf2_interp_tables_unity_dc() {
        for b in 0..BUCKETS {
            for r in table(b).rows.iter() {
                let s: f32 = r.0.iter().sum();
                assert!((s - 1.0).abs() < 1e-5);
            }
        }
        assert_eq!(bucket(0.5), 0);
        assert_eq!(bucket(1.0), 0);
        assert_eq!(bucket(1.1), 1);
        assert_eq!(bucket(2.0), 4);
        assert_eq!(bucket(100.0), BUCKETS - 1);
    }

    #[test]
    fn sf2_interp_simd_matches_scalar() {
        let d: Vec<f32> = (0..4096).map(|i| ((i as f32) * 0.37).sin() * ((i * 31 % 17) as f32 / 17.0)).collect();
        for inc in [ONE / 3 + 12345, ONE - 7, ONE + ONE / 5 + 99, 3 * ONE + 17] {
            let pos = 40 * ONE + 0x1234_5678;
            let (mut a, mut b) = ([0f32; 203], [0f32; 203]);
            let pa = cubic(&d, pos, inc, &mut a);
            let pb = cubic_scalar(&d, pos, inc, &mut b);
            assert_eq!((pa, a), (pb, b));
            let t = table(bucket(inc as f64 / ONE as f64));
            let pa = sinc(&d, t, pos, inc, &mut a);
            let pb = sinc_scalar(&d, t, pos, inc, &mut b);
            assert_eq!((pa, a), (pb, b));
        }
    }

    #[test]
    fn sf2_interp_cubic_exact_at_integer_positions() {
        let d: Vec<f32> = (0..64).map(|i| ((i * 7919) % 101) as f32 / 101.0).collect();
        let mut o = [0f32; 16];
        cubic(&d, 20 * ONE, ONE, &mut o);
        for (k, v) in o.iter().enumerate() {
            assert!((v - d[20 + k]).abs() < 1e-6);
        }
    }
}
