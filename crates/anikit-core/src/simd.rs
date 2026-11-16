//! A tiny portable 4-lane f32 vector for pixel shaders.
//!
//! On wasm32 with `simd128` every operation is one `f32x4` instruction (the
//! auto-vectoriser cannot be relied on for shader-shaped code); elsewhere it
//! is four scalar lanes, which LLVM vectorises well enough natively.

use core::ops::{Add, Div, Mul, Neg, Sub};

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod imp {
    use core::arch::wasm32 as w;
    pub type Raw = w::v128;
    pub type MaskRaw = w::v128;
    #[inline(always)]
    pub fn splat(x: f32) -> Raw {
        w::f32x4_splat(x)
    }
    #[inline(always)]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Raw {
        w::f32x4(a, b, c, d)
    }
    #[inline(always)]
    pub fn arr(v: Raw) -> [f32; 4] {
        [w::f32x4_extract_lane::<0>(v), w::f32x4_extract_lane::<1>(v), w::f32x4_extract_lane::<2>(v), w::f32x4_extract_lane::<3>(v)]
    }
    #[inline(always)]
    pub fn add(a: Raw, b: Raw) -> Raw {
        w::f32x4_add(a, b)
    }
    #[inline(always)]
    pub fn sub(a: Raw, b: Raw) -> Raw {
        w::f32x4_sub(a, b)
    }
    #[inline(always)]
    pub fn mul(a: Raw, b: Raw) -> Raw {
        w::f32x4_mul(a, b)
    }
    #[inline(always)]
    pub fn div(a: Raw, b: Raw) -> Raw {
        w::f32x4_div(a, b)
    }
    #[inline(always)]
    pub fn neg(a: Raw) -> Raw {
        w::f32x4_neg(a)
    }
    #[inline(always)]
    pub fn sqrt(a: Raw) -> Raw {
        w::f32x4_sqrt(a)
    }
    #[inline(always)]
    pub fn floor(a: Raw) -> Raw {
        w::f32x4_floor(a)
    }
    #[inline(always)]
    pub fn abs(a: Raw) -> Raw {
        w::f32x4_abs(a)
    }
    // pmin/pmax: NaN-propagation differs from f32::min/max, but shader inputs are finite.
    #[inline(always)]
    pub fn min(a: Raw, b: Raw) -> Raw {
        w::f32x4_pmin(a, b)
    }
    #[inline(always)]
    pub fn max(a: Raw, b: Raw) -> Raw {
        w::f32x4_pmax(a, b)
    }
    #[inline(always)]
    pub fn lt(a: Raw, b: Raw) -> MaskRaw {
        w::f32x4_lt(a, b)
    }
    #[inline(always)]
    pub fn gt(a: Raw, b: Raw) -> MaskRaw {
        w::f32x4_gt(a, b)
    }
    #[inline(always)]
    pub fn le(a: Raw, b: Raw) -> MaskRaw {
        w::f32x4_le(a, b)
    }
    #[inline(always)]
    pub fn bits(m: MaskRaw) -> u32 {
        w::i32x4_bitmask(m) as u32
    }
    #[inline(always)]
    pub fn select(m: MaskRaw, a: Raw, b: Raw) -> Raw {
        w::v128_bitselect(a, b, m)
    }
    /// Quantise lanes in 0…1 to bytes and pack r|g<<8|b<<16|a<<24.
    #[inline(always)]
    pub fn pack(r: Raw, g: Raw, b: Raw) -> [u32; 4] {
        let q = |v: Raw| {
            let s = w::f32x4_add(w::f32x4_mul(v, w::f32x4_splat(255.0)), w::f32x4_splat(0.5));
            w::i32x4_min(w::i32x4_trunc_sat_f32x4(s), w::i32x4_splat(255))
        };
        let v = w::v128_or(w::v128_or(q(r), w::i32x4_shl(q(g), 8)), w::v128_or(w::i32x4_shl(q(b), 16), w::i32x4_splat(0xff00_0000u32 as i32)));
        [w::u32x4_extract_lane::<0>(v), w::u32x4_extract_lane::<1>(v), w::u32x4_extract_lane::<2>(v), w::u32x4_extract_lane::<3>(v)]
    }
    /// The four bytes of `p` (lowest first) as lanes.
    #[inline(always)]
    pub fn bytes(p: u32) -> Raw {
        w::f32x4_convert_u32x4(w::u32x4_extend_low_u16x8(w::u16x8_extend_low_u8x16(w::u32x4_splat(p))))
    }
    /// Transpose four vectors (rows ↔ lanes).
    #[inline(always)]
    pub fn transpose(r: [Raw; 4]) -> [Raw; 4] {
        let t0 = w::i32x4_shuffle::<0, 4, 1, 5>(r[0], r[1]);
        let t1 = w::i32x4_shuffle::<2, 6, 3, 7>(r[0], r[1]);
        let t2 = w::i32x4_shuffle::<0, 4, 1, 5>(r[2], r[3]);
        let t3 = w::i32x4_shuffle::<2, 6, 3, 7>(r[2], r[3]);
        [w::i32x4_shuffle::<0, 1, 4, 5>(t0, t2), w::i32x4_shuffle::<2, 3, 6, 7>(t0, t2), w::i32x4_shuffle::<0, 1, 4, 5>(t1, t3), w::i32x4_shuffle::<2, 3, 6, 7>(t1, t3)]
    }
    #[inline(always)]
    pub fn unpack(p: [u32; 4]) -> [Raw; 3] {
        let v = w::u32x4(p[0], p[1], p[2], p[3]);
        let m = w::u32x4_splat(255);
        let c = |s: u32| w::f32x4_convert_u32x4(w::v128_and(w::u32x4_shr(v, s), m));
        [c(0), c(8), c(16)]
    }
}

// x86-64 with SSE4.1 (native builds: the Node addon, Python, benches): one SSE
// instruction per operation, with the WebAssembly semantics bit for bit
// (pmin/pmax operand order, saturating float → int), so native renders match
// the wasm ones exactly.
#[cfg(all(target_arch = "x86_64", target_feature = "sse4.1", not(all(target_arch = "wasm32", target_feature = "simd128"))))]
#[allow(unsafe_code, unused_unsafe)] // the only unsafe code in the core: bit casts between SSE registers and arrays
mod imp {
    use core::arch::x86_64 as x;
    pub type Raw = x::__m128;
    pub type MaskRaw = x::__m128;
    #[inline(always)]
    pub fn splat(v: f32) -> Raw {
        unsafe { x::_mm_set1_ps(v) }
    }
    #[inline(always)]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Raw {
        unsafe { x::_mm_setr_ps(a, b, c, d) }
    }
    #[inline(always)]
    pub fn arr(v: Raw) -> [f32; 4] {
        unsafe { core::mem::transmute::<Raw, [f32; 4]>(v) }
    }
    #[inline(always)]
    pub fn add(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_add_ps(a, b) }
    }
    #[inline(always)]
    pub fn sub(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_sub_ps(a, b) }
    }
    #[inline(always)]
    pub fn mul(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_mul_ps(a, b) }
    }
    #[inline(always)]
    pub fn div(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_div_ps(a, b) }
    }
    #[inline(always)]
    pub fn neg(a: Raw) -> Raw {
        unsafe { x::_mm_xor_ps(a, x::_mm_set1_ps(-0.0)) }
    }
    #[inline(always)]
    pub fn sqrt(a: Raw) -> Raw {
        unsafe { x::_mm_sqrt_ps(a) }
    }
    #[inline(always)]
    pub fn floor(a: Raw) -> Raw {
        unsafe { x::_mm_floor_ps(a) }
    }
    #[inline(always)]
    pub fn abs(a: Raw) -> Raw {
        unsafe { x::_mm_andnot_ps(x::_mm_set1_ps(-0.0), a) }
    }
    // wasm pmin(a, b) = b < a ? b : a, which is minps(b, a); pmax likewise.
    #[inline(always)]
    pub fn min(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_min_ps(b, a) }
    }
    #[inline(always)]
    pub fn max(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_max_ps(b, a) }
    }
    #[inline(always)]
    pub fn lt(a: Raw, b: Raw) -> MaskRaw {
        unsafe { x::_mm_cmplt_ps(a, b) }
    }
    #[inline(always)]
    pub fn gt(a: Raw, b: Raw) -> MaskRaw {
        unsafe { x::_mm_cmpgt_ps(a, b) }
    }
    #[inline(always)]
    pub fn le(a: Raw, b: Raw) -> MaskRaw {
        unsafe { x::_mm_cmple_ps(a, b) }
    }
    #[inline(always)]
    pub fn bits(m: MaskRaw) -> u32 {
        unsafe { x::_mm_movemask_ps(m) as u32 }
    }
    #[inline(always)]
    pub fn select(m: MaskRaw, a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm_or_ps(x::_mm_and_ps(m, a), x::_mm_andnot_ps(m, b)) }
    }
    /// wasm `i32x4.trunc_sat_f32x4`: NaN → 0, saturating at the i32 range.
    #[inline(always)]
    unsafe fn trunc_sat(v: Raw) -> x::__m128i {
        let t = x::_mm_cvttps_epi32(v); // NaN and out of range → i32::MIN
        let big = x::_mm_castps_si128(x::_mm_cmpge_ps(v, x::_mm_set1_ps(2147483648.0)));
        let nan = x::_mm_castps_si128(x::_mm_cmpunord_ps(v, v));
        let t = x::_mm_blendv_epi8(t, x::_mm_set1_epi32(i32::MAX), big);
        x::_mm_andnot_si128(nan, t)
    }
    #[inline(always)]
    pub fn pack(r: Raw, g: Raw, b: Raw) -> [u32; 4] {
        unsafe {
            let q = |v: Raw| x::_mm_min_epi32(trunc_sat(x::_mm_add_ps(x::_mm_mul_ps(v, x::_mm_set1_ps(255.0)), x::_mm_set1_ps(0.5))), x::_mm_set1_epi32(255));
            let v = x::_mm_or_si128(x::_mm_or_si128(q(r), x::_mm_slli_epi32(q(g), 8)), x::_mm_or_si128(x::_mm_slli_epi32(q(b), 16), x::_mm_set1_epi32(0xff00_0000u32 as i32)));
            core::mem::transmute::<x::__m128i, [u32; 4]>(v)
        }
    }
    #[inline(always)]
    pub fn bytes(p: u32) -> Raw {
        unsafe { x::_mm_cvtepi32_ps(x::_mm_cvtepu8_epi32(x::_mm_cvtsi32_si128(p as i32))) }
    }
    #[inline(always)]
    pub fn transpose(r: [Raw; 4]) -> [Raw; 4] {
        unsafe {
            let t0 = x::_mm_unpacklo_ps(r[0], r[1]);
            let t1 = x::_mm_unpackhi_ps(r[0], r[1]);
            let t2 = x::_mm_unpacklo_ps(r[2], r[3]);
            let t3 = x::_mm_unpackhi_ps(r[2], r[3]);
            [x::_mm_movelh_ps(t0, t2), x::_mm_movehl_ps(t2, t0), x::_mm_movelh_ps(t1, t3), x::_mm_movehl_ps(t3, t1)]
        }
    }
    #[inline(always)]
    pub fn unpack(p: [u32; 4]) -> [Raw; 3] {
        unsafe {
            let v = core::mem::transmute::<[u32; 4], x::__m128i>(p);
            let m = x::_mm_set1_epi32(255);
            [x::_mm_cvtepi32_ps(x::_mm_and_si128(v, m)), x::_mm_cvtepi32_ps(x::_mm_and_si128(x::_mm_srli_epi32(v, 8), m)), x::_mm_cvtepi32_ps(x::_mm_and_si128(x::_mm_srli_epi32(v, 16), m))]
        }
    }
}

#[cfg(not(any(all(target_arch = "wasm32", target_feature = "simd128"), all(target_arch = "x86_64", target_feature = "sse4.1"))))]
mod imp {
    pub type Raw = [f32; 4];
    pub type MaskRaw = [bool; 4];
    #[inline(always)]
    fn map(a: Raw, f: impl Fn(f32) -> f32) -> Raw {
        [f(a[0]), f(a[1]), f(a[2]), f(a[3])]
    }
    #[inline(always)]
    fn zip(a: Raw, b: Raw, f: impl Fn(f32, f32) -> f32) -> Raw {
        [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
    }
    #[inline(always)]
    pub fn splat(x: f32) -> Raw {
        [x; 4]
    }
    #[inline(always)]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Raw {
        [a, b, c, d]
    }
    #[inline(always)]
    pub fn arr(v: Raw) -> [f32; 4] {
        v
    }
    #[inline(always)]
    pub fn add(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| x + y)
    }
    #[inline(always)]
    pub fn sub(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| x - y)
    }
    #[inline(always)]
    pub fn mul(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| x * y)
    }
    #[inline(always)]
    pub fn div(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| x / y)
    }
    #[inline(always)]
    pub fn neg(a: Raw) -> Raw {
        map(a, |x| -x)
    }
    #[inline(always)]
    pub fn sqrt(a: Raw) -> Raw {
        crate::math::sqrt4(a)
    }
    #[inline(always)]
    pub fn floor(a: Raw) -> Raw {
        crate::math::floor4(a)
    }
    #[inline(always)]
    pub fn abs(a: Raw) -> Raw {
        map(a, f32::abs)
    }
    #[inline(always)]
    pub fn min(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| if y < x { y } else { x })
    }
    #[inline(always)]
    pub fn max(a: Raw, b: Raw) -> Raw {
        zip(a, b, |x, y| if x < y { y } else { x })
    }
    #[inline(always)]
    pub fn lt(a: Raw, b: Raw) -> MaskRaw {
        [a[0] < b[0], a[1] < b[1], a[2] < b[2], a[3] < b[3]]
    }
    #[inline(always)]
    pub fn gt(a: Raw, b: Raw) -> MaskRaw {
        [a[0] > b[0], a[1] > b[1], a[2] > b[2], a[3] > b[3]]
    }
    #[inline(always)]
    pub fn le(a: Raw, b: Raw) -> MaskRaw {
        [a[0] <= b[0], a[1] <= b[1], a[2] <= b[2], a[3] <= b[3]]
    }
    #[inline(always)]
    pub fn bits(m: MaskRaw) -> u32 {
        m[0] as u32 | (m[1] as u32) << 1 | (m[2] as u32) << 2 | (m[3] as u32) << 3
    }
    #[inline(always)]
    pub fn select(m: MaskRaw, a: Raw, b: Raw) -> Raw {
        [if m[0] { a[0] } else { b[0] }, if m[1] { a[1] } else { b[1] }, if m[2] { a[2] } else { b[2] }, if m[3] { a[3] } else { b[3] }]
    }
    #[inline(always)]
    pub fn pack(r: Raw, g: Raw, b: Raw) -> [u32; 4] {
        let q = |v: f32| ((v * 255.0 + 0.5) as u32).min(255);
        core::array::from_fn(|i| q(r[i]) | (q(g[i]) << 8) | (q(b[i]) << 16) | 0xff00_0000)
    }
    #[inline(always)]
    pub fn bytes(p: u32) -> Raw {
        core::array::from_fn(|i| ((p >> (8 * i)) & 255) as f32)
    }
    #[inline(always)]
    pub fn transpose(r: [Raw; 4]) -> [Raw; 4] {
        core::array::from_fn(|k| [r[0][k], r[1][k], r[2][k], r[3][k]])
    }
    #[inline(always)]
    pub fn unpack(p: [u32; 4]) -> [Raw; 3] {
        let c = |s: u32| core::array::from_fn(|i| ((p[i] >> s) & 255) as f32);
        [c(0), c(8), c(16)]
    }
}

/// Four f32 lanes.
#[derive(Clone, Copy)]
pub struct V4(imp::Raw);
/// Lane mask from a comparison.
#[derive(Clone, Copy)]
pub struct M4(imp::MaskRaw);

impl M4 {
    /// One bit per lane (lane 0 in bit 0).
    #[inline(always)]
    pub fn bits(self) -> u32 {
        imp::bits(self.0)
    }
}

impl V4 {
    #[inline(always)]
    pub fn splat(x: f32) -> Self {
        Self(imp::splat(x))
    }
    #[inline(always)]
    pub fn new(a: f32, b: f32, c: f32, d: f32) -> Self {
        Self(imp::new(a, b, c, d))
    }
    #[inline(always)]
    pub fn from_array(a: [f32; 4]) -> Self {
        Self::new(a[0], a[1], a[2], a[3])
    }
    #[inline(always)]
    pub fn to_array(self) -> [f32; 4] {
        imp::arr(self.0)
    }
    /// The four bytes of an RGBA word as lanes (r, g, b, a), exactly.
    #[inline(always)]
    pub fn bytes(p: u32) -> Self {
        Self(imp::bytes(p))
    }
    /// Transpose a 4×4 block: lane `k` of row `i` becomes lane `i` of row `k`.
    #[inline(always)]
    pub fn transpose(r: [Self; 4]) -> [Self; 4] {
        imp::transpose([r[0].0, r[1].0, r[2].0, r[3].0]).map(Self)
    }
    #[inline(always)]
    pub fn sqrt(self) -> Self {
        Self(imp::sqrt(self.0))
    }
    #[inline(always)]
    pub fn floor(self) -> Self {
        Self(imp::floor(self.0))
    }
    #[inline(always)]
    pub fn fract(self) -> Self {
        self - self.floor()
    }
    #[inline(always)]
    pub fn abs(self) -> Self {
        Self(imp::abs(self.0))
    }
    #[inline(always)]
    pub fn min(self, o: Self) -> Self {
        Self(imp::min(self.0, o.0))
    }
    #[inline(always)]
    pub fn max(self, o: Self) -> Self {
        Self(imp::max(self.0, o.0))
    }
    #[inline(always)]
    pub fn minf(self, o: f32) -> Self {
        self.min(Self::splat(o))
    }
    #[inline(always)]
    pub fn maxf(self, o: f32) -> Self {
        self.max(Self::splat(o))
    }
    #[inline(always)]
    pub fn clamp(self, lo: f32, hi: f32) -> Self {
        self.maxf(lo).minf(hi)
    }
    #[inline(always)]
    pub fn lt(self, o: Self) -> M4 {
        M4(imp::lt(self.0, o.0))
    }
    #[inline(always)]
    pub fn gt(self, o: Self) -> M4 {
        M4(imp::gt(self.0, o.0))
    }
    #[inline(always)]
    pub fn le(self, o: Self) -> M4 {
        M4(imp::le(self.0, o.0))
    }
    #[inline(always)]
    pub fn ltf(self, o: f32) -> M4 {
        self.lt(Self::splat(o))
    }
    #[inline(always)]
    pub fn gtf(self, o: f32) -> M4 {
        self.gt(Self::splat(o))
    }
    /// `m ? a : b` per lane.
    #[inline(always)]
    pub fn select(m: M4, a: Self, b: Self) -> Self {
        Self(imp::select(m.0, a.0, b.0))
    }
    /// Quantise r, g, b lanes (0…1) to packed RGBA words with opaque alpha.
    #[inline(always)]
    pub fn pack_rgb(r: Self, g: Self, b: Self) -> [u32; 4] {
        imp::pack(r.0, g.0, b.0)
    }
    /// Red, green and blue (0…255) of four packed little-endian RGBA pixels.
    #[inline(always)]
    pub fn unpack_rgb(p: [u32; 4]) -> [Self; 3] {
        let [r, g, b] = imp::unpack(p);
        [Self(r), Self(g), Self(b)]
    }
}

impl Add for V4 {
    type Output = Self;
    #[inline(always)]
    fn add(self, o: Self) -> Self {
        Self(imp::add(self.0, o.0))
    }
}
impl Sub for V4 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, o: Self) -> Self {
        Self(imp::sub(self.0, o.0))
    }
}
impl Mul for V4 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, o: Self) -> Self {
        Self(imp::mul(self.0, o.0))
    }
}
impl Div for V4 {
    type Output = Self;
    #[inline(always)]
    fn div(self, o: Self) -> Self {
        Self(imp::div(self.0, o.0))
    }
}
impl Neg for V4 {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self(imp::neg(self.0))
    }
}
impl Add<f32> for V4 {
    type Output = Self;
    #[inline(always)]
    fn add(self, o: f32) -> Self {
        self + V4::splat(o)
    }
}
impl Sub<f32> for V4 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, o: f32) -> Self {
        self - V4::splat(o)
    }
}
impl Mul<f32> for V4 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, o: f32) -> Self {
        self * V4::splat(o)
    }
}
impl Div<f32> for V4 {
    type Output = Self;
    #[inline(always)]
    fn div(self, o: f32) -> Self {
        self * (1.0 / o)
    }
}
impl Add<V4> for f32 {
    type Output = V4;
    #[inline(always)]
    fn add(self, o: V4) -> V4 {
        V4::splat(self) + o
    }
}
impl Sub<V4> for f32 {
    type Output = V4;
    #[inline(always)]
    fn sub(self, o: V4) -> V4 {
        V4::splat(self) - o
    }
}
impl Mul<V4> for f32 {
    type Output = V4;
    #[inline(always)]
    fn mul(self, o: V4) -> V4 {
        V4::splat(self) * o
    }
}
impl Div<V4> for f32 {
    type Output = V4;
    #[inline(always)]
    fn div(self, o: V4) -> V4 {
        V4::splat(self) / o
    }
}

/// Dot product of two 3-vectors of lanes.
#[inline(always)]
pub fn dot3(a: &[V4; 3], b: &[V4; 3]) -> V4 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// ---------------------------------------------------------------------------
// Eight lanes: one AVX register on x86-64 with AVX2, two `V4`s elsewhere. Every
// operation is the `V4` one lane by lane (same operand order for min/max, same
// reciprocal multiply for `/ f32`), so shading eight pixels gives the bits of
// shading them four at a time.
// ---------------------------------------------------------------------------

#[cfg(all(target_arch = "x86_64", target_feature = "avx2", not(all(target_arch = "wasm32", target_feature = "simd128"))))]
#[allow(unsafe_code, unused_unsafe)] // bit casts between AVX registers and arrays
mod imp8 {
    use super::V4;
    use core::arch::x86_64 as x;
    pub type Raw = x::__m256;
    #[inline(always)]
    pub fn splat(v: f32) -> Raw {
        unsafe { x::_mm256_set1_ps(v) }
    }
    #[inline(always)]
    pub fn from_array(a: [f32; 8]) -> Raw {
        unsafe { core::mem::transmute::<[f32; 8], Raw>(a) }
    }
    #[inline(always)]
    pub fn to_array(v: Raw) -> [f32; 8] {
        unsafe { core::mem::transmute::<Raw, [f32; 8]>(v) }
    }
    #[inline(always)]
    pub fn halves(v: Raw) -> (V4, V4) {
        let a = to_array(v);
        (V4::new(a[0], a[1], a[2], a[3]), V4::new(a[4], a[5], a[6], a[7]))
    }
    #[inline(always)]
    pub fn from_halves(lo: V4, hi: V4) -> Raw {
        let (l, h) = (lo.to_array(), hi.to_array());
        from_array([l[0], l[1], l[2], l[3], h[0], h[1], h[2], h[3]])
    }
    #[inline(always)]
    pub fn add(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_add_ps(a, b) }
    }
    #[inline(always)]
    pub fn sub(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_sub_ps(a, b) }
    }
    #[inline(always)]
    pub fn mul(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_mul_ps(a, b) }
    }
    #[inline(always)]
    pub fn div(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_div_ps(a, b) }
    }
    #[inline(always)]
    pub fn neg(a: Raw) -> Raw {
        unsafe { x::_mm256_xor_ps(a, x::_mm256_set1_ps(-0.0)) }
    }
    #[inline(always)]
    pub fn sqrt(a: Raw) -> Raw {
        unsafe { x::_mm256_sqrt_ps(a) }
    }
    #[inline(always)]
    pub fn floor(a: Raw) -> Raw {
        unsafe { x::_mm256_floor_ps(a) }
    }
    #[inline(always)]
    pub fn abs(a: Raw) -> Raw {
        unsafe { x::_mm256_andnot_ps(x::_mm256_set1_ps(-0.0), a) }
    }
    #[inline(always)]
    pub fn min(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_min_ps(b, a) }
    }
    #[inline(always)]
    pub fn max(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_max_ps(b, a) }
    }
    #[inline(always)]
    pub fn lt(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_cmp_ps::<{ x::_CMP_LT_OS }>(a, b) }
    }
    #[inline(always)]
    pub fn gt(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_cmp_ps::<{ x::_CMP_GT_OS }>(a, b) }
    }
    #[inline(always)]
    pub fn le(a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_cmp_ps::<{ x::_CMP_LE_OS }>(a, b) }
    }
    #[inline(always)]
    pub fn bits(m: Raw) -> u32 {
        unsafe { x::_mm256_movemask_ps(m) as u32 }
    }
    #[inline(always)]
    pub fn select(m: Raw, a: Raw, b: Raw) -> Raw {
        unsafe { x::_mm256_or_ps(x::_mm256_and_ps(m, a), x::_mm256_andnot_ps(m, b)) }
    }
    /// A mask with lanes `bits & (1 << l)` set.
    #[inline(always)]
    pub fn mask_from_bits(b: u32) -> Raw {
        let a: [u32; 8] = core::array::from_fn(|l| if b & (1 << l) != 0 { u32::MAX } else { 0 });
        unsafe { core::mem::transmute::<[u32; 8], Raw>(a) }
    }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "avx2", not(all(target_arch = "wasm32", target_feature = "simd128")))))]
mod imp8 {
    use super::{M4, V4};
    pub type Raw = (V4, V4);
    #[inline(always)]
    pub fn splat(v: f32) -> Raw {
        (V4::splat(v), V4::splat(v))
    }
    #[inline(always)]
    pub fn from_array(a: [f32; 8]) -> Raw {
        (V4::new(a[0], a[1], a[2], a[3]), V4::new(a[4], a[5], a[6], a[7]))
    }
    #[inline(always)]
    pub fn to_array(v: Raw) -> [f32; 8] {
        let (l, h) = (v.0.to_array(), v.1.to_array());
        [l[0], l[1], l[2], l[3], h[0], h[1], h[2], h[3]]
    }
    #[inline(always)]
    pub fn halves(v: Raw) -> (V4, V4) {
        v
    }
    #[inline(always)]
    pub fn from_halves(lo: V4, hi: V4) -> Raw {
        (lo, hi)
    }
    #[inline(always)]
    pub fn add(a: Raw, b: Raw) -> Raw {
        (a.0 + b.0, a.1 + b.1)
    }
    #[inline(always)]
    pub fn sub(a: Raw, b: Raw) -> Raw {
        (a.0 - b.0, a.1 - b.1)
    }
    #[inline(always)]
    pub fn mul(a: Raw, b: Raw) -> Raw {
        (a.0 * b.0, a.1 * b.1)
    }
    #[inline(always)]
    pub fn div(a: Raw, b: Raw) -> Raw {
        (a.0 / b.0, a.1 / b.1)
    }
    #[inline(always)]
    pub fn neg(a: Raw) -> Raw {
        (-a.0, -a.1)
    }
    #[inline(always)]
    pub fn sqrt(a: Raw) -> Raw {
        (a.0.sqrt(), a.1.sqrt())
    }
    #[inline(always)]
    pub fn floor(a: Raw) -> Raw {
        (a.0.floor(), a.1.floor())
    }
    #[inline(always)]
    pub fn abs(a: Raw) -> Raw {
        (a.0.abs(), a.1.abs())
    }
    #[inline(always)]
    pub fn min(a: Raw, b: Raw) -> Raw {
        (a.0.min(b.0), a.1.min(b.1))
    }
    #[inline(always)]
    pub fn max(a: Raw, b: Raw) -> Raw {
        (a.0.max(b.0), a.1.max(b.1))
    }
    // Masks as lanes of 1.0 / 0.0 (selected through V4's own masks).
    #[inline(always)]
    fn m(a: M4, b: M4) -> Raw {
        let one = |k: M4| V4::select(k, V4::splat(1.0), V4::splat(0.0));
        (one(a), one(b))
    }
    #[inline(always)]
    pub fn lt(a: Raw, b: Raw) -> Raw {
        m(a.0.lt(b.0), a.1.lt(b.1))
    }
    #[inline(always)]
    pub fn gt(a: Raw, b: Raw) -> Raw {
        m(a.0.gt(b.0), a.1.gt(b.1))
    }
    #[inline(always)]
    pub fn le(a: Raw, b: Raw) -> Raw {
        m(a.0.le(b.0), a.1.le(b.1))
    }
    #[inline(always)]
    pub fn bits(k: Raw) -> u32 {
        k.0.gtf(0.5).bits() | k.1.gtf(0.5).bits() << 4
    }
    #[inline(always)]
    pub fn select(k: Raw, a: Raw, b: Raw) -> Raw {
        (V4::select(k.0.gtf(0.5), a.0, b.0), V4::select(k.1.gtf(0.5), a.1, b.1))
    }
    #[inline(always)]
    pub fn mask_from_bits(b: u32) -> Raw {
        let a: [f32; 8] = core::array::from_fn(|l| if b & (1 << l) != 0 { 1.0 } else { 0.0 });
        from_array(a)
    }
}

/// Eight f32 lanes (see `imp8`).
#[derive(Clone, Copy)]
pub struct V8(imp8::Raw);
/// Lane mask from a comparison of [`V8`]s.
#[derive(Clone, Copy)]
pub struct M8(imp8::Raw);

impl M8 {
    /// One bit per lane (lane 0 in bit 0).
    #[inline(always)]
    pub fn bits(self) -> u32 {
        imp8::bits(self.0)
    }
    #[inline(always)]
    pub fn from_bits(b: u32) -> Self {
        Self(imp8::mask_from_bits(b))
    }
}

impl M4 {
    /// A mask with lane l set iff bit l of `b` is.
    #[inline(always)]
    pub fn from_bits(b: u32) -> Self {
        V4::new((b & 1) as f32, ((b >> 1) & 1) as f32, ((b >> 2) & 1) as f32, ((b >> 3) & 1) as f32).gtf(0.5)
    }
}

impl V8 {
    pub const LANES: usize = 8;
    #[inline(always)]
    pub fn splat(x: f32) -> Self {
        Self(imp8::splat(x))
    }
    #[inline(always)]
    pub fn from_array(a: [f32; 8]) -> Self {
        Self(imp8::from_array(a))
    }
    #[inline(always)]
    pub fn to_array(self) -> [f32; 8] {
        imp8::to_array(self.0)
    }
    /// Lanes 0–3 and 4–7.
    #[inline(always)]
    pub fn halves(self) -> (V4, V4) {
        imp8::halves(self.0)
    }
    #[inline(always)]
    pub fn from_halves(lo: V4, hi: V4) -> Self {
        Self(imp8::from_halves(lo, hi))
    }
    #[inline(always)]
    pub fn sqrt(self) -> Self {
        Self(imp8::sqrt(self.0))
    }
    #[inline(always)]
    pub fn floor(self) -> Self {
        Self(imp8::floor(self.0))
    }
    #[inline(always)]
    pub fn fract(self) -> Self {
        self - self.floor()
    }
    #[inline(always)]
    pub fn abs(self) -> Self {
        Self(imp8::abs(self.0))
    }
    #[inline(always)]
    pub fn min(self, o: Self) -> Self {
        Self(imp8::min(self.0, o.0))
    }
    #[inline(always)]
    pub fn max(self, o: Self) -> Self {
        Self(imp8::max(self.0, o.0))
    }
    #[inline(always)]
    pub fn minf(self, o: f32) -> Self {
        self.min(Self::splat(o))
    }
    #[inline(always)]
    pub fn maxf(self, o: f32) -> Self {
        self.max(Self::splat(o))
    }
    #[inline(always)]
    pub fn clamp(self, lo: f32, hi: f32) -> Self {
        self.maxf(lo).minf(hi)
    }
    #[inline(always)]
    pub fn lt(self, o: Self) -> M8 {
        M8(imp8::lt(self.0, o.0))
    }
    #[inline(always)]
    pub fn gt(self, o: Self) -> M8 {
        M8(imp8::gt(self.0, o.0))
    }
    #[inline(always)]
    pub fn le(self, o: Self) -> M8 {
        M8(imp8::le(self.0, o.0))
    }
    #[inline(always)]
    pub fn ltf(self, o: f32) -> M8 {
        self.lt(Self::splat(o))
    }
    #[inline(always)]
    pub fn gtf(self, o: f32) -> M8 {
        self.gt(Self::splat(o))
    }
    /// `m ? a : b` per lane.
    #[inline(always)]
    pub fn select(m: M8, a: Self, b: Self) -> Self {
        Self(imp8::select(m.0, a.0, b.0))
    }
    /// [`V4::pack_rgb`] for both halves.
    #[inline(always)]
    pub fn pack_rgb(r: Self, g: Self, b: Self) -> [u32; 8] {
        let ((r0, r1), (g0, g1), (b0, b1)) = (r.halves(), g.halves(), b.halves());
        let (lo, hi) = (V4::pack_rgb(r0, g0, b0), V4::pack_rgb(r1, g1, b1));
        [lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3]]
    }
}

macro_rules! v8_ops {
    ($($tr:ident $f:ident),*) => {$(
        impl $tr for V8 {
            type Output = Self;
            #[inline(always)]
            fn $f(self, o: Self) -> Self {
                Self(imp8::$f(self.0, o.0))
            }
        }
        impl $tr<V8> for f32 {
            type Output = V8;
            #[inline(always)]
            fn $f(self, o: V8) -> V8 {
                V8::splat(self).$f(o)
            }
        }
    )*};
}
v8_ops!(Add add, Sub sub, Mul mul, Div div);
impl Add<f32> for V8 {
    type Output = Self;
    #[inline(always)]
    fn add(self, o: f32) -> Self {
        self + V8::splat(o)
    }
}
impl Sub<f32> for V8 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, o: f32) -> Self {
        self - V8::splat(o)
    }
}
impl Mul<f32> for V8 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, o: f32) -> Self {
        self * V8::splat(o)
    }
}
impl Div<f32> for V8 {
    type Output = Self;
    /// Like `V4`: a multiply by the reciprocal.
    #[inline(always)]
    fn div(self, o: f32) -> Self {
        self * (1.0 / o)
    }
}
impl Neg for V8 {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self(imp8::neg(self.0))
    }
}

/// [`dot3`] for eight lanes.
#[inline(always)]
pub fn dot3_8(a: &[V8; 3], b: &[V8; 3]) -> V8 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}


/// Lanes from packed integer data (gathered words decoded four at a time).
impl V4 {
    /// The bit patterns `p & mask`, reinterpreted as f32.
    #[inline(always)]
    pub fn from_bits_masked(p: [u32; 4], mask: u32) -> Self {
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            use core::arch::wasm32 as w;
            Self(w::v128_and(w::u32x4(p[0], p[1], p[2], p[3]), w::u32x4_splat(mask)))
        }
        #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
        {
            let a: [f32; 4] = core::array::from_fn(|i| f32::from_bits(p[i] & mask));
            Self(imp::new(a[0], a[1], a[2], a[3]))
        }
    }
    /// The integers `p & mask`, converted to f32.
    #[inline(always)]
    pub fn from_u32_masked(p: [u32; 4], mask: u32) -> Self {
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            use core::arch::wasm32 as w;
            Self(w::f32x4_convert_u32x4(w::v128_and(w::u32x4(p[0], p[1], p[2], p[3]), w::u32x4_splat(mask))))
        }
        #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
        {
            let a: [f32; 4] = core::array::from_fn(|i| (p[i] & mask) as f32);
            Self(imp::new(a[0], a[1], a[2], a[3]))
        }
    }
    /// Sum of the four lanes, as (a + b) + (c + d).
    #[inline(always)]
    pub fn sum(self) -> f32 {
        let a = self.to_array();
        (a[0] + a[1]) + (a[2] + a[3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lanes() {
        let a = V4::new(1.0, -2.0, 4.0, 9.0);
        assert_eq!((a * 2.0 + 1.0).to_array(), [3.0, -3.0, 9.0, 19.0]);
        assert_eq!(a.abs().sqrt().to_array(), [1.0, 1.4142135, 2.0, 3.0]);
        assert_eq!(V4::select(a.gtf(0.0), a, V4::splat(0.0)).to_array(), [1.0, 0.0, 4.0, 9.0]);
        assert_eq!(V4::new(1.25, -0.5, 0.0, 3.0).fract().to_array(), [0.25, 0.5, 0.0, 0.0]);
        assert_eq!(V4::pack_rgb(V4::splat(1.0), V4::splat(0.0), V4::splat(2.0))[0], 0xffff_00ff);
        let [r, g, b] = V4::unpack_rgb([0x0001_0203, 0xff40_8020, 0, 0x00ff_ffff]);
        assert_eq!((r.to_array()[0], g.to_array()[0], b.to_array()[0]), (3.0, 2.0, 1.0));
        assert_eq!((r.to_array()[1], g.to_array()[1], b.to_array()[1]), (32.0, 128.0, 64.0));
        assert_eq!(b.to_array()[3], 255.0);
    }
}
