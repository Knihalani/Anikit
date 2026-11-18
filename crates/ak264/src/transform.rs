//! 4×4 integer transforms, Hadamard DC transforms, quantisation and scaling
//! (ITU-T H.264 8.5.10–8.5.12, flat scaling matrices).

/// Zig-zag (frame) scan: scan index → raster index (y*4+x).
pub const ZIGZAG: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];

const MF: [[i32; 3]; 6] = [
    [13107, 5243, 8066],
    [11916, 4660, 7490],
    [10082, 4194, 6554],
    [9362, 3647, 5825],
    [8192, 3355, 5243],
    [7282, 2893, 4559],
];
const V: [[i32; 3]; 6] = [[10, 16, 13], [11, 18, 14], [13, 20, 16], [14, 23, 18], [16, 25, 20], [18, 29, 23]];

#[inline(always)]
const fn class(i: usize) -> usize {
    let (x, y) = (i & 3, i >> 2);
    if x % 2 == 0 && y % 2 == 0 {
        0
    } else if x % 2 == 1 && y % 2 == 1 {
        1
    } else {
        2
    }
}

/// Chroma QP from luma QP (offset 0), Table 8-15.
pub fn chroma_qp(qp: u8) -> u8 {
    const T: [u8; 22] = [29, 30, 31, 32, 32, 33, 34, 34, 35, 35, 36, 36, 37, 37, 37, 38, 38, 38, 39, 39, 39, 39];
    if qp < 30 {
        qp
    } else {
        T[(qp - 30) as usize]
    }
}

/// Quantiser state for one QP.
#[derive(Clone, Copy)]
pub struct Quant {
    pub qp: u8,
    /// Multiplication factors, raster order.
    pub mf: [i32; 16],
    /// Dequant scale (v << qp/6), raster order.
    pub dq: [i32; 16],
    pub qbits: u32,
    /// Rounding offsets for intra and inter.
    pub f_intra: i32,
    pub f_inter: i32,
    /// v(0,0) for DC dequant.
    pub v0: i32,
}

impl Quant {
    pub fn new(qp: u8) -> Self {
        let q6 = (qp / 6) as u32;
        let r = (qp % 6) as usize;
        let mut mf = [0; 16];
        let mut dq = [0; 16];
        for i in 0..16 {
            mf[i] = MF[r][class(i)];
            dq[i] = V[r][class(i)] << q6;
        }
        let qbits = 15 + q6;
        Quant { qp, mf, dq, qbits, f_intra: (1 << qbits) / 3, f_inter: (1 << qbits) / 6, v0: V[r][0] }
    }
}

/// Forward 4×4 core transform of a residual block (raster).
#[inline(always)]
pub fn fdct4(r: &[i32; 16]) -> [i32; 16] {
    let mut t = [0i32; 16];
    for y in 0..4 {
        let a = &r[y * 4..y * 4 + 4];
        let s03 = a[0] + a[3];
        let d03 = a[0] - a[3];
        let s12 = a[1] + a[2];
        let d12 = a[1] - a[2];
        t[y * 4] = s03 + s12;
        t[y * 4 + 1] = 2 * d03 + d12;
        t[y * 4 + 2] = s03 - s12;
        t[y * 4 + 3] = d03 - 2 * d12;
    }
    let mut o = [0i32; 16];
    for x in 0..4 {
        let s03 = t[x] + t[12 + x];
        let d03 = t[x] - t[12 + x];
        let s12 = t[4 + x] + t[8 + x];
        let d12 = t[4 + x] - t[8 + x];
        o[x] = s03 + s12;
        o[4 + x] = 2 * d03 + d12;
        o[8 + x] = s03 - s12;
        o[12 + x] = d03 - 2 * d12;
    }
    o
}

/// Inverse 4×4 transform (8.5.12.2) of scaled coefficients; returns residual after (x+32)>>6.
#[inline(always)]
pub fn idct4(d: &[i32; 16]) -> [i32; 16] {
    let mut f = [0i32; 16];
    for y in 0..4 {
        let a = &d[y * 4..y * 4 + 4];
        let e0 = a[0] + a[2];
        let e1 = a[0] - a[2];
        let e2 = (a[1] >> 1) - a[3];
        let e3 = a[1] + (a[3] >> 1);
        f[y * 4] = e0 + e3;
        f[y * 4 + 1] = e1 + e2;
        f[y * 4 + 2] = e1 - e2;
        f[y * 4 + 3] = e0 - e3;
    }
    let mut r = [0i32; 16];
    for x in 0..4 {
        let g0 = f[x] + f[8 + x];
        let g1 = f[x] - f[8 + x];
        let g2 = (f[4 + x] >> 1) - f[12 + x];
        let g3 = f[4 + x] + (f[12 + x] >> 1);
        r[x] = (g0 + g3 + 32) >> 6;
        r[4 + x] = (g1 + g2 + 32) >> 6;
        r[8 + x] = (g1 - g2 + 32) >> 6;
        r[12 + x] = (g0 - g3 + 32) >> 6;
    }
    r
}

/// 4×4 Hadamard (unnormalised), raster in/out.
#[inline(always)]
pub fn hadamard4(c: &[i32; 16]) -> [i32; 16] {
    let mut t = [0i32; 16];
    for y in 0..4 {
        let (a0, a1, a2, a3) = (c[y * 4], c[y * 4 + 1], c[y * 4 + 2], c[y * 4 + 3]);
        let s01 = a0 + a1;
        let d01 = a0 - a1;
        let s23 = a2 + a3;
        let d23 = a2 - a3;
        t[y * 4] = s01 + s23;
        t[y * 4 + 1] = s01 - s23;
        t[y * 4 + 2] = d01 - d23;
        t[y * 4 + 3] = d01 + d23;
    }
    let mut o = [0i32; 16];
    for x in 0..4 {
        let (a0, a1, a2, a3) = (t[x], t[4 + x], t[8 + x], t[12 + x]);
        let s01 = a0 + a1;
        let d01 = a0 - a1;
        let s23 = a2 + a3;
        let d23 = a2 - a3;
        o[x] = s01 + s23;
        o[4 + x] = s01 - s23;
        o[8 + x] = d01 - d23;
        o[12 + x] = d01 + d23;
    }
    o
}

#[inline(always)]
fn q1(c: i32, mf: i32, f: i32, qbits: u32) -> i32 {
    let l = (c.abs() * mf + f) >> qbits;
    if c < 0 {
        -l
    } else {
        l
    }
}

/// Quantise a transformed 4×4 block in place (raster). Skips index 0 if `ac_only`.
/// Returns the number of non-zero levels.
#[inline(always)]
pub fn quant4(c: &mut [i32; 16], q: &Quant, intra: bool, ac_only: bool) -> u32 {
    let f = if intra { q.f_intra } else { q.f_inter };
    let mut nz = 0;
    for i in (ac_only as usize)..16 {
        c[i] = q1(c[i], q.mf[i], f, q.qbits);
        nz += (c[i] != 0) as u32;
    }
    nz
}

/// Scale (dequantise) a 4×4 block of levels in place; index 0 kept if `has_dc`.
#[inline(always)]
pub fn dequant4(c: &mut [i32; 16], q: &Quant, keep_dc: bool) {
    for i in (keep_dc as usize)..16 {
        c[i] *= q.dq[i];
    }
}

/// Luma DC (Intra16x16): forward Hadamard of 16 DCs (raster by block position) and quantise.
#[inline]
pub fn luma_dc_forward(dc: &[i32; 16], q: &Quant) -> [i32; 16] {
    let mut h = hadamard4(dc);
    let f = q.f_intra * 2;
    for v in h.iter_mut() {
        *v = q1(*v >> 1, q.mf[0], f, q.qbits + 1);
    }
    h
}

/// Luma DC inverse: levels → dcY (8.5.10).
#[inline]
pub fn luma_dc_inverse(levels: &[i32; 16], qp: u8, v0: i32) -> [i32; 16] {
    let mut f = hadamard4(levels);
    let q6 = (qp / 6) as i32;
    let ls = 16 * v0;
    for v in f.iter_mut() {
        *v = if qp >= 36 {
            (*v * ls) << (q6 - 6)
        } else {
            (*v * ls + (1 << (5 - q6))) >> (6 - q6)
        };
    }
    f
}

/// Chroma DC forward (2×2 Hadamard + quant). Input raster [dc0, dc1, dc2, dc3].
pub fn chroma_dc_forward(dc: &[i32; 4], q: &Quant, intra: bool) -> [i32; 4] {
    let h = hadamard2(dc);
    let f = 2 * if intra { q.f_intra } else { q.f_inter };
    [
        q1(h[0], q.mf[0], f, q.qbits + 1),
        q1(h[1], q.mf[0], f, q.qbits + 1),
        q1(h[2], q.mf[0], f, q.qbits + 1),
        q1(h[3], q.mf[0], f, q.qbits + 1),
    ]
}

#[inline(always)]
pub fn hadamard2(c: &[i32; 4]) -> [i32; 4] {
    [c[0] + c[1] + c[2] + c[3], c[0] - c[1] + c[2] - c[3], c[0] + c[1] - c[2] - c[3], c[0] - c[1] - c[2] + c[3]]
}

/// Chroma DC inverse (8.5.11.2), 4:2:0.
pub fn chroma_dc_inverse(levels: &[i32; 4], qpc: u8, v0: i32) -> [i32; 4] {
    let f = hadamard2(levels);
    let q6 = (qpc / 6) as i32;
    let ls = 16 * v0;
    [((f[0] * ls) << q6) >> 5, ((f[1] * ls) << q6) >> 5, ((f[2] * ls) << q6) >> 5, ((f[3] * ls) << q6) >> 5]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_low_qp() {
        let q = Quant::new(4);
        let r: [i32; 16] = core::array::from_fn(|i| (i as i32 * 7 % 23) - 11);
        let mut c = fdct4(&r);
        quant4(&mut c, &q, true, false);
        dequant4(&mut c, &q, false);
        let o = idct4(&c);
        for i in 0..16 {
            assert!((o[i] - r[i]).abs() <= 1, "{:?} vs {:?}", o, r);
        }
    }
}

// ---------------------------------------------------------------- 8×8 (High profile)

/// 8×8 zig-zag (frame) scan: scan index → raster index (y*8+x).
pub const ZIGZAG8: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57,
    50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// normAdjust8x8 (Table 8-16 / equation 8-319), by QP%6 and position class.
const V8: [[i32; 6]; 6] = [
    [20, 18, 32, 19, 25, 24],
    [22, 19, 35, 21, 28, 26],
    [26, 23, 42, 24, 33, 31],
    [28, 25, 45, 26, 35, 33],
    [32, 28, 51, 30, 40, 38],
    [36, 32, 58, 34, 46, 43],
];
/// Forward multipliers matching V8 (2^22 / (V8·norm), as x264).
const MF8: [[i32; 6]; 6] = [
    [13107, 11428, 20972, 12222, 16777, 15481],
    [11916, 10826, 19174, 11058, 14980, 14290],
    [10082, 8943, 15978, 9675, 12710, 11985],
    [9362, 8228, 14913, 8931, 11984, 11259],
    [8192, 7346, 13159, 7740, 10486, 9777],
    [7282, 6428, 11570, 6830, 9118, 8640],
];

#[inline(always)]
const fn class8(i: usize) -> usize {
    let (x, y) = (i & 7, i >> 3);
    if x % 4 == 0 && y % 4 == 0 {
        0
    } else if x % 2 == 1 && y % 2 == 1 {
        1
    } else if x % 4 == 2 && y % 4 == 2 {
        2
    } else if (x % 4 == 0 && y % 2 == 1) || (x % 2 == 1 && y % 4 == 0) {
        3
    } else if (x % 4 == 0 && y % 4 == 2) || (x % 4 == 2 && y % 4 == 0) {
        4
    } else {
        5
    }
}

/// 8×8 quantiser state for one QP (flat scaling lists).
#[derive(Clone, Copy)]
pub struct Quant8 {
    pub qp: u8,
    pub mf: [i32; 64],
    /// LevelScale8x8 = 16·normAdjust8x8, raster.
    pub ls: [i32; 64],
    pub qbits: u32,
    pub f_inter: i32,
}

impl Quant8 {
    pub fn new(qp: u8) -> Self {
        let r = (qp % 6) as usize;
        let mut mf = [0; 64];
        let mut ls = [0; 64];
        for i in 0..64 {
            mf[i] = MF8[r][class8(i)];
            ls[i] = 16 * V8[r][class8(i)];
        }
        let qbits = 16 + (qp / 6) as u32;
        Quant8 { qp, mf, ls, qbits, f_inter: (1 << qbits) / 6 }
    }
}

/// Forward 8×8 core transform (rows then columns), raster in and out.
pub fn fdct8(r: &[i32; 64]) -> [i32; 64] {
    #[inline(always)]
    fn d1(s: [i32; 8]) -> [i32; 8] {
        let (s07, s16, s25, s34) = (s[0] + s[7], s[1] + s[6], s[2] + s[5], s[3] + s[4]);
        let (a0, a1, a2, a3) = (s07 + s34, s16 + s25, s07 - s34, s16 - s25);
        let (d07, d16, d25, d34) = (s[0] - s[7], s[1] - s[6], s[2] - s[5], s[3] - s[4]);
        let a4 = d16 + d25 + (d07 + (d07 >> 1));
        let a5 = d07 - d34 - (d25 + (d25 >> 1));
        let a6 = d07 + d34 - (d16 + (d16 >> 1));
        let a7 = d16 - d25 + (d34 + (d34 >> 1));
        [a0 + a1, a4 + (a7 >> 2), a2 + (a3 >> 1), a5 + (a6 >> 2), a0 - a1, a6 - (a5 >> 2), (a2 >> 1) - a3, (a4 >> 2) - a7]
    }
    let mut t = [0i32; 64];
    for y in 0..8 {
        let row: [i32; 8] = r[y * 8..y * 8 + 8].try_into().unwrap();
        t[y * 8..y * 8 + 8].copy_from_slice(&d1(row));
    }
    let mut o = [0i32; 64];
    for x in 0..8 {
        let col = d1(std::array::from_fn(|y| t[y * 8 + x]));
        for y in 0..8 {
            o[y * 8 + x] = col[y];
        }
    }
    o
}

/// Inverse 8×8 transform of scaled coefficients (8.5.13), including the final
/// (x + 32) >> 6: the residual, raster.
pub fn idct8(d: &[i32; 64]) -> [i32; 64] {
    #[inline(always)]
    fn i1(s: [i32; 8]) -> [i32; 8] {
        let a0 = s[0] + s[4];
        let a2 = s[0] - s[4];
        let a4 = (s[2] >> 1) - s[6];
        let a6 = (s[6] >> 1) + s[2];
        let (b0, b2, b4, b6) = (a0 + a6, a2 + a4, a2 - a4, a0 - a6);
        let a1 = -s[3] + s[5] - s[7] - (s[7] >> 1);
        let a3 = s[1] + s[7] - s[3] - (s[3] >> 1);
        let a5 = -s[1] + s[7] + s[5] + (s[5] >> 1);
        let a7 = s[3] + s[5] + s[1] + (s[1] >> 1);
        let b1 = (a7 >> 2) + a1;
        let b3 = a3 + (a5 >> 2);
        let b5 = (a3 >> 2) - a5;
        let b7 = a7 - (a1 >> 2);
        [b0 + b7, b2 + b5, b4 + b3, b6 + b1, b6 - b1, b4 - b3, b2 - b5, b0 - b7]
    }
    let mut t = [0i32; 64];
    for y in 0..8 {
        let row: [i32; 8] = d[y * 8..y * 8 + 8].try_into().unwrap();
        t[y * 8..y * 8 + 8].copy_from_slice(&i1(row));
    }
    let mut o = [0i32; 64];
    for x in 0..8 {
        let col = i1(std::array::from_fn(|y| t[y * 8 + x]));
        for y in 0..8 {
            o[y * 8 + x] = (col[y] + 32) >> 6;
        }
    }
    o
}

/// Quantise in place (inter rounding); returns the number of nonzero levels.
pub fn quant8(c: &mut [i32; 64], q: &Quant8) -> u32 {
    let mut n = 0;
    for i in 0..64 {
        let a = ((c[i].unsigned_abs() as i64 * q.mf[i] as i64 + q.f_inter as i64) >> q.qbits) as i32;
        let v = if c[i] < 0 { -a } else { a };
        c[i] = v.clamp(-32768, 32767);
        n += (v != 0) as u32;
    }
    n
}

/// Scale levels for the inverse transform (8.5.12.1, flat lists).
pub fn dequant8(c: &mut [i32; 64], q: &Quant8) {
    let q6 = (q.qp / 6) as u32;
    for i in 0..64 {
        c[i] = if q6 >= 6 { (c[i] * q.ls[i]) << (q6 - 6) } else { (c[i] * q.ls[i] + (1 << (5 - q6))) >> (6 - q6) };
    }
}

#[cfg(test)]
mod t8 {
    use super::*;
    #[test]
    fn roundtrip8_low_qp() {
        let r: [i32; 64] = std::array::from_fn(|i| ((i * 37 + (i / 8) * 11) % 41) as i32 - 20);
        let q = Quant8::new(4);
        let mut c = fdct8(&r);
        quant8(&mut c, &q);
        dequant8(&mut c, &q);
        let o = idct8(&c);
        for i in 0..64 {
            assert!((o[i] - r[i]).abs() <= 1, "{i}: {} vs {}", o[i], r[i]);
        }
    }
}
