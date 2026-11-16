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
    #[inline(always)]
    pub fn unpack(p: [u32; 4]) -> [Raw; 3] {
        let v = w::u32x4(p[0], p[1], p[2], p[3]);
        let m = w::u32x4_splat(255);
        let c = |s: u32| w::f32x4_convert_u32x4(w::v128_and(w::u32x4_shr(v, s), m));
        [c(0), c(8), c(16)]
    }
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
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
    pub fn select(m: MaskRaw, a: Raw, b: Raw) -> Raw {
        [if m[0] { a[0] } else { b[0] }, if m[1] { a[1] } else { b[1] }, if m[2] { a[2] } else { b[2] }, if m[3] { a[3] } else { b[3] }]
    }
    #[inline(always)]
    pub fn pack(r: Raw, g: Raw, b: Raw) -> [u32; 4] {
        let q = |v: f32| ((v * 255.0 + 0.5) as u32).min(255);
        core::array::from_fn(|i| q(r[i]) | (q(g[i]) << 8) | (q(b[i]) << 16) | 0xff00_0000)
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
