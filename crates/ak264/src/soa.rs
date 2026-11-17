//! Macroblock residual coding in SoA layout: `x[pos][lane]` holds coefficient `pos`
//! (raster within the 4×4 block) of block `lane`. Every step is lane-wise integer
//! arithmetic that LLVM vectorises; callers dispatch AVX2/baseline builds.

use crate::transform::Quant;

/// Quantiser constants for the 16-bit `((|c| + bias) * m) >> 16` quantiser.
#[derive(Clone, Copy)]
pub struct QuantS {
    pub m: [u16; 16],
    pub bias: [[u16; 16]; 2], // [intra, inter]
    pub dq: [i16; 16],
}

impl QuantS {
    pub fn new(q: &Quant) -> Self {
        let mut m = [0u16; 16];
        let mut bias = [[0u16; 16]; 2];
        let mut dq = [0i16; 16];
        for i in 0..16 {
            let mf = q.mf[i] as i64;
            let qb = q.qbits as i64;
            // M = mf * 2^(16 - qbits), rounded
            let mm = if qb <= 16 { mf << (16 - qb) } else { (mf + (1 << (qb - 17))) >> (qb - 16) };
            m[i] = mm.clamp(1, 65535) as u16;
            let scale = (1i64 << qb) as f64 / mf as f64;
            bias[0][i] = (scale / 3.0).round() as u16;
            bias[1][i] = (scale / 6.0).round() as u16;
            dq[i] = q.dq[i] as i16;
        }
        QuantS { m, bias, dq }
    }
}

pub type Soa<const N: usize> = [[i16; N]; 16];

impl QuantS {
    /// Largest |coefficient| that quantises to 0 at every position `from..16` (inter).
    pub fn zero_coef(&self, from: usize, intra: bool) -> u32 {
        let b = &self.bias[!intra as usize];
        (from..16).map(|k| (65535 / self.m[k] as u32).saturating_sub(b[k] as u32)).min().unwrap_or(0)
    }
}

/// Bounds on the SAD of a 16×16 luma block / one 8×8 chroma block at or below
/// which a residual provably quantises to all zeros (|coef| ≤ 4·SAD; DC sums ≤ SAD).
/// Inter: luma 4×4 blocks. Intra: I16 (AC + Hadamard DC).
pub fn zero_sad_bounds(q: &Quant, qs: &QuantS, qc: &Quant, qcs: &QuantS, intra: bool) -> (u32, u32) {
    // largest |h| with (|h| * mf0 + 2f) >> (qbits + 1) == 0
    let dc_lim = |q: &Quant| {
        let f = if intra { q.f_intra } else { q.f_inter } as i64;
        (((1i64 << (q.qbits + 1)) - 2 * f - 1) / q.mf[0] as i64).max(0) as u32
    };
    let luma = if intra {
        // AC positions 1..16; DC: |y| = |h >> 1| <= ceil(SAD / 2) <= dc_lim
        (qs.zero_coef(1, true) / 4).min((2 * dc_lim(q)).saturating_sub(1))
    } else {
        qs.zero_coef(0, false) / 4
    };
    let chroma = (qcs.zero_coef(1, intra) / 4).min(dc_lim(qc));
    (luma, chroma)
}

/// Forward 4×4 core transform, lane-wise.
#[inline(always)]
pub fn fdct<const N: usize>(r: &mut Soa<N>) {
    for y in 0..4 {
        for l in 0..N {
            let (a0, a1, a2, a3) = (r[y * 4][l], r[y * 4 + 1][l], r[y * 4 + 2][l], r[y * 4 + 3][l]);
            let s03 = a0 + a3;
            let d03 = a0 - a3;
            let s12 = a1 + a2;
            let d12 = a1 - a2;
            r[y * 4][l] = s03 + s12;
            r[y * 4 + 1][l] = 2 * d03 + d12;
            r[y * 4 + 2][l] = s03 - s12;
            r[y * 4 + 3][l] = d03 - 2 * d12;
        }
    }
    for x in 0..4 {
        for l in 0..N {
            let (a0, a1, a2, a3) = (r[x][l], r[4 + x][l], r[8 + x][l], r[12 + x][l]);
            let s03 = a0 + a3;
            let d03 = a0 - a3;
            let s12 = a1 + a2;
            let d12 = a1 - a2;
            r[x][l] = s03 + s12;
            r[4 + x][l] = 2 * d03 + d12;
            r[8 + x][l] = s03 - s12;
            r[12 + x][l] = d03 - 2 * d12;
        }
    }
}

/// Quantise in place (positions `from..16`); returns per-lane non-zero counts.
#[inline(always)]
pub fn quant<const N: usize>(c: &mut Soa<N>, q: &QuantS, intra: bool, from: usize) -> [u8; N] {
    let b = &q.bias[!intra as usize];
    let mut cnt = [0i16; N];
    #[cfg(target_arch = "x86_64")]
    {
        if N.is_multiple_of(8) && crate::simd::level() >= 1 {
            for k in from..16 {
                for h in (0..N).step_by(8) {
                    // SAFETY: SSE4.1 detected; slices are 8 lanes.
                    unsafe { crate::simd::quant8_sse41(&mut c[k][h..h + 8], q.m[k], b[k], &mut cnt[h..h + 8]) };
                }
            }
            let mut nz = [0u8; N];
            for l in 0..N {
                nz[l] = cnt[l] as u8;
            }
            return nz;
        }
    }
    for k in from..16 {
        let (m, bk) = (q.m[k], b[k]);
        let row = &mut c[k];
        for l in 0..N {
            let v = row[l];
            let sgn = v >> 15; // 0 or -1
            let a = v.unsigned_abs().saturating_add(bk);
            let lv = (((a as u32) * (m as u32)) >> 16) as u16;
            let lv = lv.min(2047) as i16;
            row[l] = (lv ^ sgn) - sgn;
            cnt[l] -= -((lv != 0) as i16);
        }
    }
    let mut nz = [0u8; N];
    for l in 0..N {
        nz[l] = cnt[l] as u8;
    }
    nz
}

/// Dequantise positions `from..16` and inverse transform; returns the residual.
/// Position 0 is taken as already scaled when `from == 1`.
#[inline(always)]
pub fn dequant_idct<const N: usize>(c: &Soa<N>, q: &QuantS, from: usize) -> Soa<N> {
    let mut d = *c;
    for k in from..16 {
        for l in 0..N {
            d[k][l] = d[k][l].wrapping_mul(q.dq[k]);
        }
    }
    for y in 0..4 {
        for l in 0..N {
            let (a0, a1, a2, a3) = (d[y * 4][l], d[y * 4 + 1][l], d[y * 4 + 2][l], d[y * 4 + 3][l]);
            let e0 = a0.wrapping_add(a2);
            let e1 = a0.wrapping_sub(a2);
            let e2 = (a1 >> 1).wrapping_sub(a3);
            let e3 = a1.wrapping_add(a3 >> 1);
            d[y * 4][l] = e0.wrapping_add(e3);
            d[y * 4 + 1][l] = e1.wrapping_add(e2);
            d[y * 4 + 2][l] = e1.wrapping_sub(e2);
            d[y * 4 + 3][l] = e0.wrapping_sub(e3);
        }
    }
    for x in 0..4 {
        for l in 0..N {
            let (f0, f1, f2, f3) = (d[x][l], d[4 + x][l], d[8 + x][l], d[12 + x][l]);
            let g0 = f0.wrapping_add(f2);
            let g1 = f0.wrapping_sub(f2);
            let g2 = (f1 >> 1).wrapping_sub(f3);
            let g3 = f1.wrapping_add(f3 >> 1);
            d[x][l] = g0.wrapping_add(g3).wrapping_add(32) >> 6;
            d[4 + x][l] = g1.wrapping_add(g2).wrapping_add(32) >> 6;
            d[8 + x][l] = g1.wrapping_sub(g2).wrapping_add(32) >> 6;
            d[12 + x][l] = g0.wrapping_sub(g3).wrapping_add(32) >> 6;
        }
    }
    d
}

/// Residual of a 16×16 block as SoA over its 16 4×4 blocks (lane = by*4+bx);
/// also returns the prediction in SoA form for [`scatter16`].
#[inline(always)]
pub fn gather16(src: &[u8; 256], pred: &[u8; 256]) -> (Soa<16>, [[u8; 16]; 16]) {
    let s = crate::simd::soa16_u8(src);
    let p = crate::simd::soa16_u8(pred);
    let mut r = [[0i16; 16]; 16];
    for k in 0..16 {
        for l in 0..16 {
            r[k][l] = s[k][l] as i16 - p[k][l] as i16;
        }
    }
    (r, p)
}

/// Residual of two 8×8 chroma blocks as SoA over 8 4×4 blocks (lane = comp*4 + by*2+bx).
#[inline(always)]
pub fn gather8x2(src: &[[u8; 64]; 2], pred: &[[u8; 64]; 2]) -> (Soa<8>, [[u8; 8]; 16]) {
    let s = crate::simd::soa8x2_u8(src);
    let p = crate::simd::soa8x2_u8(pred);
    let mut r = [[0i16; 8]; 16];
    for k in 0..16 {
        for l in 0..8 {
            r[k][l] = s[k][l] as i16 - p[k][l] as i16;
        }
    }
    (r, p)
}

#[inline(always)]
pub fn scatter16(res: &Soa<16>, pred: &[[u8; 16]; 16], out: &mut [u8; 256]) {
    let mut o = [[0u8; 16]; 16];
    for k in 0..16 {
        for l in 0..16 {
            o[k][l] = (pred[k][l] as i16 + res[k][l]).clamp(0, 255) as u8;
        }
    }
    crate::simd::unsoa16_u8(&o, out);
}

#[inline(always)]
pub fn scatter8x2(res: &Soa<8>, pred: &[[u8; 8]; 16], out: &mut [[u8; 64]; 2]) {
    let mut o = [[0u8; 8]; 16];
    for k in 0..16 {
        for l in 0..8 {
            o[k][l] = (pred[k][l] as i16 + res[k][l]).clamp(0, 255) as u8;
        }
    }
    crate::simd::unsoa8x2_u8(&o, out);
}
