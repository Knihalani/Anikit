//! Vector math emitted as Cranelift IR on `f32x4` / `i32x4`.
//!
//! Every function here is accurate to a few ulp over the ranges shaders use
//! (Cephes-style range reduction plus minimax polynomials), branch-free, and
//! deterministic: no fused multiply-adds, so x86-64 and aarch64 agree.

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types::{F32X4, I32, I32X4, I8X16};
use cranelift_codegen::ir::{Endianness, InstBuilder, MemFlagsData, Value};
use cranelift_frontend::FunctionBuilder;

pub type B<'a, 'b> = &'a mut FunctionBuilder<'b>;

fn lane_flags() -> MemFlagsData {
    MemFlagsData::new().with_endianness(Endianness::Little)
}

pub fn fsplat(b: B, v: f32) -> Value {
    let s = b.ins().f32const(v);
    b.ins().splat(F32X4, s)
}

pub fn isplat(b: B, v: i32) -> Value {
    let s = b.ins().iconst(I32, v as i64);
    b.ins().splat(I32X4, s)
}

/// Reinterpret `f32x4` ↔ `i32x4` (free).
pub fn as_i(b: B, v: Value) -> Value {
    if b.func.dfg.value_type(v) == I32X4 {
        return v;
    }
    b.ins().bitcast(I32X4, lane_flags(), v)
}

pub fn as_f(b: B, v: Value) -> Value {
    if b.func.dfg.value_type(v) == F32X4 {
        return v;
    }
    b.ins().bitcast(F32X4, lane_flags(), v)
}

pub fn as_bytes(b: B, v: Value) -> Value {
    b.ins().bitcast(I8X16, lane_flags(), v)
}

/// `m ? x : y` per lane; `m` is an `i32x4` lane mask (0 or -1).
pub fn sel(b: B, m: Value, x: Value, y: Value) -> Value {
    let ty = b.func.dfg.value_type(x);
    let m = if ty == F32X4 { as_f(b, m) } else { as_i(b, m) };
    b.ins().bitselect(m, x, y)
}

pub fn flt(b: B, x: Value, y: Value) -> Value {
    b.ins().fcmp(FloatCC::LessThan, x, y)
}

pub fn fgt(b: B, x: Value, y: Value) -> Value {
    b.ins().fcmp(FloatCC::GreaterThan, x, y)
}

/// GLSL `min`: `y < x ? y : x` (returns `x` when `y` is NaN).
/// |x| with the sign of `s` (vector `fcopysign` isn't available on every backend).
pub fn copysign(b: B, x: Value, s: Value) -> Value {
    let xi = as_i(b, x);
    let si = as_i(b, s);
    let m = isplat(b, i32::MAX);
    let mag = b.ins().band(xi, m);
    let sg = b.ins().band_not(si, m);
    let r = b.ins().bor(mag, sg);
    as_f(b, r)
}

pub fn fmin(b: B, x: Value, y: Value) -> Value {
    let m = flt(b, y, x);
    sel(b, m, y, x)
}

pub fn fmax(b: B, x: Value, y: Value) -> Value {
    let m = fgt(b, y, x);
    sel(b, m, y, x)
}

pub fn clamp(b: B, x: Value, lo: Value, hi: Value) -> Value {
    let t = fmax(b, x, lo);
    fmin(b, t, hi)
}

/// Horner: `c[0]·xⁿ + … + c[n]`.
pub fn poly(b: B, x: Value, c: &[f32]) -> Value {
    let mut r = fsplat(b, c[0]);
    for &k in &c[1..] {
        let k = fsplat(b, k);
        r = b.ins().fmul(r, x);
        r = b.ins().fadd(r, k);
    }
    r
}

/// Split a constant into parts with `bits` significant bits each, so `q·part`
/// is exact for small integers `q` (Cody–Waite range reduction).
fn split(v: f64, bits: u32) -> [f32; 3] {
    let trunc = |x: f64| -> f64 {
        if x == 0.0 {
            return 0.0;
        }
        let e = x.abs().log2().floor() as i32;
        let scale = 2f64.powi(bits as i32 - 1 - e);
        (x * scale).trunc() / scale
    };
    let a = trunc(v);
    let bb = trunc(v - a);
    let c = (v - a - bb) as f32;
    [a as f32, bb as f32, c]
}

/// `x - q·(c₀ + c₁ + c₂)` without losing bits.
fn reduce(b: B, x: Value, q: Value, c: [f32; 3]) -> Value {
    let mut r = x;
    for k in c {
        let k = fsplat(b, k);
        let t = b.ins().fmul(q, k);
        r = b.ins().fsub(r, t);
    }
    r
}

fn sign_bit_from(b: B, qi: Value, bit: i32) -> Value {
    // (qi & bit) << (31 - log2(bit)) puts that bit into the sign position.
    let m = isplat(b, bit);
    let t = b.ins().band(qi, m);
    let sh = 31 - bit.trailing_zeros() as i64;
    b.ins().ishl_imm_s(t, sh)
}

fn xor_sign(b: B, v: Value, sign: Value) -> Value {
    let vi = as_i(b, v);
    let r = b.ins().bxor(vi, sign);
    as_f(b, r)
}

/// Integer value of a float that is already integral and below 2²² in
/// magnitude (rounding results): add 1.5·2²³ and read the low mantissa bits.
/// Two instructions instead of the NaN-safe saturating conversion.
fn int_of_integral(b: B, q: Value) -> Value {
    let magic = fsplat(b, 12_582_912.0);
    let t = b.ins().fadd(q, magic);
    let ti = as_i(b, t);
    let mi = isplat(b, 0x4B40_0000);
    b.ins().isub(ti, mi)
}

/// Shared quadrant reduction: returns (sin(r), cos(r), quadrant) with x = r + q·π/2.
fn sincos_core(b: B, x: Value) -> (Value, Value, Value) {
    let two_over_pi = fsplat(b, std::f32::consts::FRAC_2_PI);
    let t = b.ins().fmul(x, two_over_pi);
    let q = b.ins().nearest(t);
    let r = reduce(b, x, q, split(std::f64::consts::FRAC_PI_2, 12));
    // Only the low two bits are used, so huge arguments are harmless.
    let qi = int_of_integral(b, q);
    let r2 = b.ins().fmul(r, r);
    // sin(r) = r + r³·P(r²)
    let ps = poly(b, r2, &[-1.951_529_6e-4, 8.332_160_9e-3, -1.666_665_5e-1]);
    let r3 = b.ins().fmul(r2, r);
    let s = b.ins().fmul(r3, ps);
    let s = b.ins().fadd(r, s);
    // cos(r) = 1 - r²/2 + r⁴·Q(r²)
    let pc = poly(b, r2, &[2.443_315_7e-5, -1.388_731_6e-3, 4.166_664_6e-2]);
    let r4 = b.ins().fmul(r2, r2);
    let c = b.ins().fmul(r4, pc);
    let half = fsplat(b, 0.5);
    let h = b.ins().fmul(r2, half);
    let one = fsplat(b, 1.0);
    let c0 = b.ins().fsub(one, h);
    let c = b.ins().fadd(c0, c);
    (s, c, qi)
}

fn quadrant(b: B, s: Value, c: Value, qi: Value) -> Value {
    let one = isplat(b, 1);
    let odd = b.ins().band(qi, one);
    let z = isplat(b, 0);
    let odd = b.ins().icmp(IntCC::NotEqual, odd, z);
    let v = sel(b, odd, c, s);
    let sg = sign_bit_from(b, qi, 2);
    xor_sign(b, v, sg)
}

pub fn sin(b: B, x: Value) -> Value {
    let (s, c, q) = sincos_core(b, x);
    quadrant(b, s, c, q)
}

pub fn cos(b: B, x: Value) -> Value {
    let (s, c, q) = sincos_core(b, x);
    let one = isplat(b, 1);
    let q1 = b.ins().iadd(q, one);
    quadrant(b, s, c, q1)
}

pub fn tan(b: B, x: Value) -> Value {
    let (s, c, q) = sincos_core(b, x);
    let one = isplat(b, 1);
    let q1 = b.ins().iadd(q, one);
    let sn = quadrant(b, s, c, q);
    let cs = quadrant(b, s, c, q1);
    b.ins().fdiv(sn, cs)
}

/// 2^n for integer lanes n in [-126, 127].
fn pow2i(b: B, n: Value) -> Value {
    let bias = isplat(b, 127);
    let e = b.ins().iadd(n, bias);
    let e = b.ins().ishl_imm_s(e, 23);
    as_f(b, e)
}

/// p · 2^n for n in about [-252, 254], with gradual overflow/underflow.
fn ldexp_wide(b: B, p: Value, n: Value) -> Value {
    let n1 = b.ins().sshr_imm_s(n, 1);
    let n2 = b.ins().isub(n, n1);
    let s1 = pow2i(b, n1);
    let s2 = pow2i(b, n2);
    let t = b.ins().fmul(p, s1);
    b.ins().fmul(t, s2)
}

/// e^r for |r| ≤ ln2/2.
fn exp_poly(b: B, r: Value) -> Value {
    let p = poly(b, r, &[1.987_569_1e-4, 1.398_199_9e-3, 8.333_452e-3, 4.166_579_6e-2, 1.666_666_5e-1, 5.000_000_1e-1]);
    let r2 = b.ins().fmul(r, r);
    let t = b.ins().fmul(p, r2);
    let t = b.ins().fadd(t, r);
    let one = fsplat(b, 1.0);
    b.ins().fadd(t, one)
}

pub fn exp(b: B, x: Value) -> Value {
    let lo = fsplat(b, -104.0);
    let hi = fsplat(b, 89.0);
    let xc = clamp(b, x, lo, hi);
    let l2e = fsplat(b, std::f32::consts::LOG2_E);
    let t = b.ins().fmul(xc, l2e);
    let q = b.ins().nearest(t);
    let r = reduce(b, xc, q, split(std::f64::consts::LN_2, 12));
    let p = exp_poly(b, r);
    let n = int_of_integral(b, q); // |q| ≤ 150 after clamping
    let v = ldexp_wide(b, p, n);
    // NaN in → NaN out.
    let nan = b.ins().fcmp(FloatCC::Unordered, x, x);
    sel(b, nan, x, v)
}

pub fn exp2(b: B, x: Value) -> Value {
    let lo = fsplat(b, -150.0);
    let hi = fsplat(b, 129.0);
    let xc = clamp(b, x, lo, hi);
    let q = b.ins().nearest(xc);
    let f = b.ins().fsub(xc, q);
    let ln2 = fsplat(b, std::f32::consts::LN_2);
    let r = b.ins().fmul(f, ln2);
    let p = exp_poly(b, r);
    let n = int_of_integral(b, q);
    let v = ldexp_wide(b, p, n);
    let nan = b.ins().fcmp(FloatCC::Unordered, x, x);
    sel(b, nan, x, v)
}

/// Returns (exponent as f32, f) with x = 2^e · (1 + f), 1+f in [√½, √2),
/// and the polynomial tail y such that ln(1+f) = f + y.
fn log_core(b: B, x: Value) -> (Value, Value, Value) {
    // Scale denormals up first.
    let tiny = fsplat(b, f32::MIN_POSITIVE);
    let is_tiny = flt(b, x, tiny);
    let big = fsplat(b, 33_554_432.0); // 2^25
    let xs = b.ins().fmul(x, big);
    let xn = sel(b, is_tiny, xs, x);
    let k25 = isplat(b, 25);
    let k0 = isplat(b, 0);
    let adj = sel(b, is_tiny, k25, k0);
    let bits = as_i(b, xn);
    let e = b.ins().ushr_imm_s(bits, 23);
    let k127 = isplat(b, 127);
    let e = b.ins().isub(e, k127);
    let e = b.ins().isub(e, adj);
    let km = isplat(b, 0x007f_ffff);
    let mant = b.ins().band(bits, km);
    let ko = isplat(b, 0x3f80_0000);
    let mant = b.ins().bor(mant, ko);
    let m = as_f(b, mant);
    let sqrt2 = fsplat(b, std::f32::consts::SQRT_2);
    let hi = fgt(b, m, sqrt2);
    let kh = fsplat(b, 0.5);
    let halfm = b.ins().fmul(m, kh);
    let m = sel(b, hi, halfm, m);
    let e1 = b.ins().isub(e, hi); // hi is -1 where true
    let ef = b.ins().fcvt_from_sint(F32X4, e1);
    let one = fsplat(b, 1.0);
    let f = b.ins().fsub(m, one);
    let z = b.ins().fmul(f, f);
    let p = poly(
        b,
        f,
        &[7.037_683_6e-2, -1.151_461e-1, 1.167_699_9e-1, -1.242_014_1e-1, 1.424_932_3e-1, -1.666_805_8e-1, 2.000_071_5e-1, -2.499_999_4e-1, 3.333_333e-1],
    );
    let y = b.ins().fmul(p, f);
    let y = b.ins().fmul(y, z);
    let kh = fsplat(b, 0.5);
    let hz = b.ins().fmul(z, kh);
    let y = b.ins().fsub(y, hz);
    (ef, f, y)
}

fn log_special(b: B, x: Value, v: Value) -> Value {
    let zero = fsplat(b, 0.0);
    let is_zero = b.ins().fcmp(FloatCC::Equal, x, zero);
    let ninf = fsplat(b, f32::NEG_INFINITY);
    let v = sel(b, is_zero, ninf, v);
    let neg = flt(b, x, zero);
    let nan = b.ins().fcmp(FloatCC::Unordered, x, x);
    let bad = b.ins().bor(neg, nan);
    let qnan = fsplat(b, f32::NAN);
    let v = sel(b, bad, qnan, v);
    let inf = fsplat(b, f32::INFINITY);
    let is_inf = b.ins().fcmp(FloatCC::Equal, x, inf);
    sel(b, is_inf, inf, v)
}

pub fn log(b: B, x: Value) -> Value {
    let (e, f, y) = log_core(b, x);
    let lo = fsplat(b, -2.121_944_4e-4);
    let hi = fsplat(b, 0.693_359_4);
    let t = b.ins().fmul(e, lo);
    let y = b.ins().fadd(y, t);
    let r = b.ins().fadd(f, y);
    let t = b.ins().fmul(e, hi);
    let r = b.ins().fadd(r, t);
    log_special(b, x, r)
}

pub fn log2(b: B, x: Value) -> Value {
    let (e, f, y) = log_core(b, x);
    let l = b.ins().fadd(f, y);
    let l2e = fsplat(b, std::f32::consts::LOG2_E);
    let t = b.ins().fmul(l, l2e);
    let r = b.ins().fadd(t, e);
    log_special(b, x, r)
}

/// `pow`: exp2(y·log2|x|) like a GPU, but exact in sign for negative bases
/// with integral exponents (GPU compilers turn `pow(x, 2.0)` into `x * x`,
/// and shader code relies on it); NaN for negative bases otherwise.
pub fn pow(b: B, x: Value, y: Value) -> Value {
    let ax = b.ins().fabs(x);
    let l = log2(b, ax);
    let t = b.ins().fmul(l, y);
    let v = exp2(b, t);
    let zero = fsplat(b, 0.0);
    let one = fsplat(b, 1.0);
    let neg = flt(b, x, zero);
    let ty = b.ins().trunc(y);
    let integral = b.ins().fcmp(FloatCC::Equal, ty, y);
    // Odd exponent: y/2 is not integral.
    let half = fsplat(b, 0.5);
    let hy = b.ins().fmul(y, half);
    let thy = b.ins().trunc(hy);
    let odd = b.ins().fcmp(FloatCC::NotEqual, thy, hy);
    let flip = b.ins().band(neg, odd);
    let nv = b.ins().fneg(v);
    let v = sel(b, flip, nv, v);
    let bad = b.ins().band_not(neg, integral);
    let nan = fsplat(b, f32::NAN);
    let v = sel(b, bad, nan, v);
    let y0 = b.ins().fcmp(FloatCC::Equal, y, zero);
    let x1 = b.ins().fcmp(FloatCC::Equal, x, one);
    let unit = b.ins().bor(y0, x1);
    sel(b, unit, one, v)
}

pub fn atan(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let t3p8 = fsplat(b, 2.414_213_6);
    let tp8 = fsplat(b, 0.414_213_57);
    let big = fgt(b, ax, t3p8);
    let mid = fgt(b, ax, tp8);
    let one = fsplat(b, 1.0);
    let neg1 = fsplat(b, -1.0);
    let inv = b.ins().fdiv(neg1, ax);
    let num = b.ins().fsub(ax, one);
    let den = b.ins().fadd(ax, one);
    let mr = b.ins().fdiv(num, den);
    let xr = sel(b, mid, mr, ax);
    let xr = sel(b, big, inv, xr);
    let q4 = fsplat(b, std::f32::consts::FRAC_PI_4);
    let q2 = fsplat(b, std::f32::consts::FRAC_PI_2);
    let zero = fsplat(b, 0.0);
    let y0 = sel(b, mid, q4, zero);
    let y0 = sel(b, big, q2, y0);
    let z = b.ins().fmul(xr, xr);
    let p = poly(b, z, &[8.053_744_5e-2, -1.387_768_5e-1, 1.997_771_1e-1, -3.333_295e-1]);
    let t = b.ins().fmul(p, z);
    let t = b.ins().fmul(t, xr);
    let y = b.ins().fadd(t, xr);
    let y = b.ins().fadd(y, y0);
    copysign(b, y, x)
}

pub fn atan2(b: B, y: Value, x: Value) -> Value {
    let q = b.ins().fdiv(y, x);
    let a = atan(b, q);
    let zero = fsplat(b, 0.0);
    let pi = fsplat(b, std::f32::consts::PI);
    let spi = copysign(b, pi, y);
    let xneg = flt(b, x, zero);
    let corr = b.ins().fadd(a, spi);
    let r = sel(b, xneg, corr, a);
    // 0/0 → 0 (and the sign-correct π for x < 0).
    let nan = b.ins().fcmp(FloatCC::Unordered, r, r);
    let yz = b.ins().fcmp(FloatCC::Equal, y, zero);
    let both = b.ins().band(nan, yz);
    let fallback = sel(b, xneg, spi, zero);
    sel(b, both, fallback, r)
}

/// asin for |x| ≤ 1 (NaN outside).
pub fn asin(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let half = fsplat(b, 0.5);
    let one = fsplat(b, 1.0);
    let big = fgt(b, ax, half);
    let om = b.ins().fsub(one, ax);
    let zb = b.ins().fmul(om, half);
    let sb = b.ins().sqrt(zb);
    let zs = b.ins().fmul(ax, ax);
    let z = sel(b, big, zb, zs);
    let xx = sel(b, big, sb, ax);
    let p = poly(b, z, &[4.216_319_9e-2, 2.418_131_1e-2, 4.547_002_6e-2, 7.495_300_3e-2, 1.666_675_2e-1]);
    let t = b.ins().fmul(p, z);
    let t = b.ins().fmul(t, xx);
    let r = b.ins().fadd(t, xx);
    let two = fsplat(b, 2.0);
    let r2 = b.ins().fmul(r, two);
    let hp = fsplat(b, std::f32::consts::FRAC_PI_2);
    let rb = b.ins().fsub(hp, r2);
    let r = sel(b, big, rb, r);
    let gt1 = fgt(b, ax, one);
    let nan = fsplat(b, f32::NAN);
    let r = sel(b, gt1, nan, r);
    copysign(b, r, x)
}

pub fn acos(b: B, x: Value) -> Value {
    // Cephes: |x| ≤ ½ → π/2 − asin(x); x > ½ → 2·asin(√((1−x)/2)); x < −½ → π − 2·asin(√((1+x)/2)).
    let one = fsplat(b, 1.0);
    let half = fsplat(b, 0.5);
    let ax = b.ins().fabs(x);
    let t = b.ins().fsub(one, ax);
    let t = b.ins().fmul(t, half);
    let zero = fsplat(b, 0.0);
    let t = fmax(b, t, zero);
    let s = b.ins().sqrt(t);
    let a = asin(b, s);
    let two = fsplat(b, 2.0);
    let r2 = b.ins().fmul(a, two);
    let pi = fsplat(b, std::f32::consts::PI);
    let rneg = b.ins().fsub(pi, r2);
    let hp = fsplat(b, std::f32::consts::FRAC_PI_2);
    let ai = asin(b, x);
    let mid = b.ins().fsub(hp, ai);
    let neg = flt(b, x, zero);
    let big = sel(b, neg, rneg, r2);
    let is_big = fgt(b, ax, half);
    let r = sel(b, is_big, big, mid);
    let gt1 = fgt(b, ax, one);
    let nan = fsplat(b, f32::NAN);
    sel(b, gt1, nan, r)
}

pub fn sinh(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let e = exp(b, ax);
    let one = fsplat(b, 1.0);
    let ie = b.ins().fdiv(one, e);
    let d = b.ins().fsub(e, ie);
    let half = fsplat(b, 0.5);
    let big = b.ins().fmul(d, half);
    let z = b.ins().fmul(x, x);
    let p = poly(b, z, &[2.037_219_1e-4, 8.330_283_7e-3, 1.666_671_6e-1]);
    let t = b.ins().fmul(p, z);
    let t = b.ins().fmul(t, ax);
    let small = b.ins().fadd(t, ax);
    let is_big = fgt(b, ax, one);
    let r = sel(b, is_big, big, small);
    copysign(b, r, x)
}

pub fn cosh(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let e = exp(b, ax);
    let one = fsplat(b, 1.0);
    let ie = b.ins().fdiv(one, e);
    let d = b.ins().fadd(e, ie);
    let half = fsplat(b, 0.5);
    b.ins().fmul(d, half)
}

pub fn tanh(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let two = fsplat(b, 2.0);
    let one = fsplat(b, 1.0);
    let t2 = b.ins().fmul(ax, two);
    let e = exp(b, t2);
    let ep = b.ins().fadd(e, one);
    let q = b.ins().fdiv(two, ep);
    let big = b.ins().fsub(one, q);
    let z = b.ins().fmul(x, x);
    let p = poly(b, z, &[-5.704_988_7e-3, 2.063_908_9e-2, -5.373_971_6e-2, 1.333_144_2e-1, -3.333_328_2e-1]);
    let t = b.ins().fmul(p, z);
    let t = b.ins().fmul(t, ax);
    let small = b.ins().fadd(t, ax);
    let lim = fsplat(b, 0.625);
    let is_big = fgt(b, ax, lim);
    let r = sel(b, is_big, big, small);
    copysign(b, r, x)
}

pub fn asinh(b: B, x: Value) -> Value {
    let ax = b.ins().fabs(x);
    let one = fsplat(b, 1.0);
    let z = b.ins().fmul(ax, ax);
    let z1 = b.ins().fadd(z, one);
    let s = b.ins().sqrt(z1);
    let t = b.ins().fadd(ax, s);
    let r = log(b, t);
    copysign(b, r, x)
}

pub fn acosh(b: B, x: Value) -> Value {
    let one = fsplat(b, 1.0);
    let z = b.ins().fmul(x, x);
    let z1 = b.ins().fsub(z, one);
    let s = b.ins().sqrt(z1);
    let t = b.ins().fadd(x, s);
    log(b, t)
}

pub fn atanh(b: B, x: Value) -> Value {
    let one = fsplat(b, 1.0);
    let n = b.ins().fadd(one, x);
    let d = b.ins().fsub(one, x);
    let q = b.ins().fdiv(n, d);
    let l = log(b, q);
    let half = fsplat(b, 0.5);
    b.ins().fmul(l, half)
}

/// GLSL `fract`: x − floor(x), kept below 1.
pub fn fract(b: B, x: Value) -> Value {
    let f = b.ins().floor(x);
    let r = b.ins().fsub(x, f);
    let top = fsplat(b, 0.999_999_94);
    fmin(b, r, top)
}

pub fn sign(b: B, x: Value) -> Value {
    let zero = fsplat(b, 0.0);
    let one = fsplat(b, 1.0);
    let pos = fgt(b, x, zero);
    let neg = flt(b, x, zero);
    let m1 = fsplat(b, -1.0);
    let r = sel(b, pos, one, zero);
    sel(b, neg, m1, r)
}

pub fn smoothstep(b: B, e0: Value, e1: Value, x: Value) -> Value {
    let n = b.ins().fsub(x, e0);
    let d = b.ins().fsub(e1, e0);
    let t = b.ins().fdiv(n, d);
    let zero = fsplat(b, 0.0);
    let one = fsplat(b, 1.0);
    let t = clamp(b, t, zero, one);
    let two = fsplat(b, 2.0);
    let three = fsplat(b, 3.0);
    let k = b.ins().fmul(t, two);
    let k = b.ins().fsub(three, k);
    let tt = b.ins().fmul(t, t);
    b.ins().fmul(tt, k)
}

pub fn mix(b: B, x: Value, y: Value, a: Value) -> Value {
    let one = fsplat(b, 1.0);
    let ia = b.ins().fsub(one, a);
    let l = b.ins().fmul(x, ia);
    let r = b.ins().fmul(y, a);
    b.ins().fadd(l, r)
}

pub fn step(b: B, edge: Value, x: Value) -> Value {
    let m = flt(b, x, edge);
    let zero = fsplat(b, 0.0);
    let one = fsplat(b, 1.0);
    sel(b, m, zero, one)
}

/// Lane shuffle of a 4-lane vector by lane indices.
pub fn shuffle4(b: B, v: Value, idx: [u8; 4]) -> Value {
    let ty = b.func.dfg.value_type(v);
    let bytes = as_bytes(b, v);
    let mut mask = [0u8; 16];
    for (l, &i) in idx.iter().enumerate() {
        for k in 0..4 {
            mask[l * 4 + k] = i * 4 + k as u8;
        }
    }
    let imm = b.func.dfg.immediates.push(cranelift_codegen::ir::ConstantData::from(&mask[..]));
    let s = b.ins().shuffle(bytes, bytes, imm);
    b.ins().bitcast(ty, lane_flags(), s)
}

/// True when any lane of an `i32x4` mask is set.
pub fn any(b: B, m: Value) -> Value {
    b.ins().vany_true(m)
}


