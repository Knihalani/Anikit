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
        let a = &c[y * 4..y * 4 + 4];
        let s01 = a[0] + a[1];
        let d01 = a[0] - a[1];
        let s23 = a[2] + a[3];
        let d23 = a[2] - a[3];
        t[y * 4] = s01 + s23;
        t[y * 4 + 1] = s01 - s23;
        t[y * 4 + 2] = d01 - d23;
        t[y * 4 + 3] = d01 + d23;
    }
    let mut o = [0i32; 16];
    for x in 0..4 {
        let s01 = t[x] + t[4 + x];
        let d01 = t[x] - t[4 + x];
        let s23 = t[8 + x] + t[12 + x];
        let d23 = t[8 + x] - t[12 + x];
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
pub fn luma_dc_forward(dc: &[i32; 16], q: &Quant) -> [i32; 16] {
    let mut h = hadamard4(dc);
    let f = q.f_intra * 2;
    for v in h.iter_mut() {
        *v = q1(*v >> 1, q.mf[0], f, q.qbits + 1);
    }
    h
}

/// Luma DC inverse: levels → dcY (8.5.10).
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
