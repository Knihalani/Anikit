//! 8×8 transform for inter macroblocks (High profile, CABAC).

use super::*;
use crate::transform::{dequant8, fdct8, idct8, quant8, Quant8, ZIGZAG8};

/// x264's decimation weights for 8×8 blocks (by zero run before a ±1).
const DS8: [u32; 64] = {
    let mut t = [0u32; 64];
    let mut i = 0;
    while i < 24 {
        t[i] = if i < 4 {
            3
        } else if i < 12 {
            2
        } else {
            1
        };
        i += 1;
    }
    t
};

fn decimate_score64(c: &[i32; 64]) -> u32 {
    let mut i = 63isize;
    while i >= 0 && c[ZIGZAG8[i as usize]] == 0 {
        i -= 1;
    }
    let mut score = 0;
    while i >= 0 {
        if c[ZIGZAG8[i as usize]].abs() > 1 {
            return 9;
        }
        i -= 1;
        let mut run = 0;
        while i >= 0 && c[ZIGZAG8[i as usize]] == 0 {
            run += 1;
            i -= 1;
        }
        score += DS8[run];
    }
    score
}

#[inline(always)]
fn resid8(src: &[u8; 256], pred: &[u8; 256], b8: usize) -> [i32; 64] {
    let (x0, y0) = ((b8 & 1) * 8, (b8 >> 1) * 8);
    let mut r = [0i32; 64];
    for y in 0..8 {
        let o = (y0 + y) * 16 + x0;
        for x in 0..8 {
            r[y * 8 + x] = src[o + x] as i32 - pred[o + x] as i32;
        }
    }
    r
}

/// Would the 8×8 transform suit this residual better (SA8D < SATD)?
pub(super) fn prefer_t8(src: &[u8; 256], pred: &[u8; 256]) -> bool {
    // x264's sa8d scale: (sum + 2) >> 2 per 8×8 block
    let s8: u32 = (0..4).map(|b8| (crate::simd::had8_sum(src, pred, (b8 >> 1) * 128 + (b8 & 1) * 8) + 2) >> 2).sum();
    s8 < dsp::satd(src, pred, 16, 16)
}

/// Inter luma residual with the 8×8 transform: levels into `cd.l8`, per-4×4
/// nonzero counts (each 4×4 of a coded 8×8 block gets the block's count, for
/// contexts and deblocking), cbp, and the reconstruction.
pub(super) fn luma_inter8(src: &[u8; 256], pred: &[u8; 256], q8: &Quant8, cd: &mut Coded, recon: &mut [u8; 256]) {
    cd.t8 = true;
    let mut lv = [[0i32; 64]; 4];
    let mut score = [0u32; 4];
    for b8 in 0..4 {
        let mut c = fdct8(&resid8(src, pred, b8));
        if quant8(&mut c, q8) > 0 {
            score[b8] = decimate_score64(&c);
        }
        lv[b8] = c;
    }
    let total: u32 = score.iter().sum();
    *recon = *pred;
    for b8 in 0..4 {
        let n = lv[b8].iter().filter(|&&v| v != 0).count();
        if n == 0 || score[b8] < 4 || total < 6 {
            continue;
        }
        cd.cbp |= 1 << b8;
        let (bx, by) = ((b8 & 1) * 2, (b8 >> 1) * 2);
        for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            cd.nz[(by + dy) * 4 + bx + dx] = n.min(16) as u8;
        }
        for i in 0..64 {
            cd.l8[b8][i] = lv[b8][i] as i16;
        }
        let mut d = lv[b8];
        dequant8(&mut d, q8);
        let r = idct8(&d);
        let (x0, y0) = ((b8 & 1) * 8, (b8 >> 1) * 8);
        for i in 0..64 {
            let o = (y0 + i / 8) * 16 + x0 + i % 8;
            recon[o] = (pred[o] as i32 + r[i]).clamp(0, 255) as u8;
        }
    }
}

/// transform_size_8x8_flag context increment (neighbours using the 8×8 transform).
pub(super) fn t8_ctx(a: &Option<MbInfo>, b: &Option<MbInfo>) -> usize {
    a.is_some_and(|m| m.t8) as usize + b.is_some_and(|m| m.t8) as usize
}

/// An 8×8 residual block (CABAC, cat 5: no coded_block_flag).
pub(super) fn cabac_block8(cb: &mut crate::cabac::Cabac, w: &mut BitWriter, l: &[i16; 64]) {
    let mut scan = [0i32; 64];
    for k in 0..64 {
        scan[k] = l[ZIGZAG8[k]] as i32;
    }
    cb.residual8(w, &scan);
}
