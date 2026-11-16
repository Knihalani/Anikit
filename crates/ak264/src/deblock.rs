//! In-loop deblocking filter (8.7), disable_deblocking_filter_idc = 0, offsets 0.

use crate::encoder::MbInfo;
use crate::frame::{Pic, Plane};
use crate::transform::{chroma_qp, Quant};

const ALPHA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 5, 6, 7, 8, 9, 10, 12, 13, 15, 17, 20, 22, 25, 28, 32, 36, 40,
    45, 50, 56, 63, 71, 80, 90, 101, 113, 127, 144, 162, 182, 203, 226, 255, 255,
];
const BETA: [u8; 52] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11,
    12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18,
];
const TC0: [[u8; 3]; 52] = [
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0],
    [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 1], [0, 0, 1], [0, 0, 1],
    [0, 0, 1], [0, 1, 1], [0, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 1], [1, 1, 2], [1, 1, 2], [1, 1, 2],
    [1, 1, 2], [1, 2, 3], [1, 2, 3], [2, 2, 3], [2, 2, 4], [2, 3, 4], [2, 3, 4], [3, 3, 5], [3, 4, 6], [3, 4, 6],
    [4, 5, 7], [4, 5, 8], [4, 6, 9], [5, 7, 10], [6, 8, 11], [6, 8, 13], [7, 10, 14], [8, 11, 16], [9, 12, 18],
    [10, 13, 20], [11, 15, 23], [13, 17, 25],
];

#[derive(Clone, Copy)]
struct Params {
    alpha: i32,
    beta: i32,
    tc0: [i32; 3],
}

fn params(qp: u8) -> Params {
    let i = qp as usize;
    Params { alpha: ALPHA[i] as i32, beta: BETA[i] as i32, tc0: TC0[i].map(|v| v as i32) }
}

#[inline(always)]
fn bs(p: &MbInfo, bp: usize, q: &MbInfo, bq: usize, mb_edge: bool) -> u8 {
    if p.kind != 0 || q.kind != 0 {
        return if mb_edge { 4 } else { 3 };
    }
    if p.nz[bp] != 0 || q.nz[bq] != 0 {
        return 2;
    }
    (p.mv != q.mv) as u8
}

/// Filter `n` lines across one edge. `o` is the offset of q0 on the first line,
/// `xs` the step across the edge, `ls` the step along it.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn filter_line(d: &mut [u8], o: usize, xs: usize, ls: usize, n: usize, bs: u8, p: &Params, chroma: bool) {
    if bs == 0 {
        return;
    }
    let (alpha, beta) = (p.alpha, p.beta);
    for k in 0..n {
        let q0i = o + k * ls;
        let at = |i: isize| -> usize { (q0i as isize + i * xs as isize) as usize };
        let p0 = d[at(-1)] as i32;
        let q0 = d[at(0)] as i32;
        let p1 = d[at(-2)] as i32;
        let q1 = d[at(1)] as i32;
        if (p0 - q0).abs() >= alpha || (p1 - p0).abs() >= beta || (q1 - q0).abs() >= beta {
            continue;
        }
        if chroma {
            if bs < 4 {
                let tc = p.tc0[bs as usize - 1] + 1;
                let delta = ((((q0 - p0) << 2) + (p1 - q1) + 4) >> 3).clamp(-tc, tc);
                d[at(-1)] = (p0 + delta).clamp(0, 255) as u8;
                d[at(0)] = (q0 - delta).clamp(0, 255) as u8;
            } else {
                d[at(-1)] = ((2 * p1 + p0 + q1 + 2) >> 2) as u8;
                d[at(0)] = ((2 * q1 + q0 + p1 + 2) >> 2) as u8;
            }
            continue;
        }
        let p2 = d[at(-3)] as i32;
        let q2 = d[at(2)] as i32;
        let ap = (p2 - p0).abs() < beta;
        let aq = (q2 - q0).abs() < beta;
        if bs < 4 {
            let tc0 = p.tc0[bs as usize - 1];
            let tc = tc0 + ap as i32 + aq as i32;
            let delta = ((((q0 - p0) << 2) + (p1 - q1) + 4) >> 3).clamp(-tc, tc);
            d[at(-1)] = (p0 + delta).clamp(0, 255) as u8;
            d[at(0)] = (q0 - delta).clamp(0, 255) as u8;
            if ap {
                d[at(-2)] = (p1 + ((p2 + ((p0 + q0 + 1) >> 1) - (p1 << 1)) >> 1).clamp(-tc0, tc0)) as u8;
            }
            if aq {
                d[at(1)] = (q1 + ((q2 + ((p0 + q0 + 1) >> 1) - (q1 << 1)) >> 1).clamp(-tc0, tc0)) as u8;
            }
        } else {
            let strong = (p0 - q0).abs() < ((alpha >> 2) + 2);
            if ap && strong {
                let p3 = d[at(-4)] as i32;
                d[at(-1)] = ((p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3) as u8;
                d[at(-2)] = ((p2 + p1 + p0 + q0 + 2) >> 2) as u8;
                d[at(-3)] = ((2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3) as u8;
            } else {
                d[at(-1)] = ((2 * p1 + p0 + q1 + 2) >> 2) as u8;
            }
            if aq && strong {
                let q3 = d[at(3)] as i32;
                d[at(0)] = ((p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3) as u8;
                d[at(1)] = ((p0 + q0 + q1 + q2 + 2) >> 2) as u8;
                d[at(2)] = ((2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3) as u8;
            } else {
                d[at(0)] = ((2 * q1 + q0 + p1 + 2) >> 2) as u8;
            }
        }
    }
}

/// Deblock a whole reconstructed picture in decoding order.
pub(crate) fn deblock(p: &mut Pic, info: &[MbInfo], mbw: usize, mbh: usize, q: &Quant, _slices: Vec<(usize, usize)>) {
    let lp = params(q.qp);
    let cpar = params(chroma_qp(q.qp));
    for mby in 0..mbh {
        for mbx in 0..mbw {
            deblock_mb(p, info, mbw, mbx, mby, &lp, &cpar);
        }
    }
}

fn deblock_mb(p: &mut Pic, info: &[MbInfo], mbw: usize, mbx: usize, mby: usize, lp: &Params, cp: &Params) {
    let cur = &info[mby * mbw + mbx];
    // bS[dir][edge][segment]
    let mut b = [[[0u8; 4]; 4]; 2];
    let mut any = false;
    for e in 0..4 {
        for k in 0..4 {
            // vertical edge e (x = 4e), segment k (rows 4k..)
            let v = if e == 0 {
                if mbx == 0 {
                    0
                } else {
                    bs(&info[mby * mbw + mbx - 1], k * 4 + 3, cur, k * 4, true)
                }
            } else {
                bs(cur, k * 4 + e - 1, cur, k * 4 + e, false)
            };
            let h = if e == 0 {
                if mby == 0 {
                    0
                } else {
                    bs(&info[(mby - 1) * mbw + mbx], 12 + k, cur, k, true)
                }
            } else {
                bs(cur, (e - 1) * 4 + k, cur, e * 4 + k, false)
            };
            b[0][e][k] = v;
            b[1][e][k] = h;
            any |= v != 0 || h != 0;
        }
    }
    if !any {
        return;
    }
    let lum = &mut p.y;
    let (s, base) = (lum.stride, lum.off(mbx as isize * 16, mby as isize * 16));
    for e in 0..4 {
        for k in 0..4 {
            filter_line(&mut lum.data, base + e * 4 + k * 4 * s, 1, s, 4, b[0][e][k], lp, false);
        }
    }
    for e in 0..4 {
        for k in 0..4 {
            filter_line(&mut lum.data, base + e * 4 * s + k * 4, s, 1, 4, b[1][e][k], lp, false);
        }
    }
    for pl in [&mut p.u, &mut p.v] {
        chroma_mb(pl, mbx, mby, &b, cp);
    }
}

fn chroma_mb(pl: &mut Plane, mbx: usize, mby: usize, b: &[[[u8; 4]; 4]; 2], cp: &Params) {
    let (s, base) = (pl.stride, pl.off(mbx as isize * 8, mby as isize * 8));
    // chroma edges 0 and 4 map to luma edges 0 and 2; chroma sample pairs map to luma segments.
    for (ce, le) in [(0usize, 0usize), (1, 2)] {
        for k in 0..4 {
            filter_line(&mut pl.data, base + ce * 4 + k * 2 * s, 1, s, 2, b[0][le][k], cp, true);
        }
    }
    for (ce, le) in [(0usize, 0usize), (1, 2)] {
        for k in 0..4 {
            filter_line(&mut pl.data, base + ce * 4 * s + k * 2, s, 1, 2, b[1][le][k], cp, true);
        }
    }
}
