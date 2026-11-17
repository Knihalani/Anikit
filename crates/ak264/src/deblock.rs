//! In-loop deblocking filter (8.7), disable_deblocking_filter_idc = 0, offsets 0.

use crate::encoder::MbInfo;
use crate::frame::Pic;
use crate::transform::chroma_qp;

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
    /// tC0 by bS (0..=4): the table for bS 1..3, 0 for bS 0 and 4.
    tct: [i16; 5],
}

fn params(qp: u8) -> Params {
    let i = qp as usize;
    let t = TC0[i];
    Params {
        alpha: ALPHA[i] as i32,
        beta: BETA[i] as i32,
        tct: [0, t[0] as i16, t[1] as i16, t[2] as i16, 0],
    }
}

#[inline(always)]
fn bs(p: &MbInfo, bp: usize, q: &MbInfo, bq: usize, mb_edge: bool) -> u8 {
    if p.kind != 0 || q.kind != 0 {
        return if mb_edge { 4 } else { 3 };
    }
    if p.nz[bp] != 0 || q.nz[bq] != 0 {
        return 2;
    }
    // 4×4 block (raster) → 8×8 quadrant
    let quad = |b: usize| (b / 8) * 2 + (b % 4) / 2;
    let (mp, mq) = (p.qmv(quad(bp)), q.qmv(quad(bq)));
    ((mp.0 - mq.0).abs() >= 4 || (mp.1 - mq.1).abs() >= 4) as u8
}

/// Filter up to 16 lines across one edge, branch-free and lane-wise.
/// `o` is the offset of q0 on the first line, `xs` the step across the edge,
/// `ls` the step along it; `bs[l]` is the strength of line `l`.
#[inline(always)]
#[allow(clippy::manual_clamp)]
fn filter_edge<const N: usize, const CHROMA: bool, const STRONG: bool>(d: &mut [u8], o: usize, xs: usize, ls: usize, seg: &[u8; 4], p: &Params) {
    let mut v = [[0i16; N]; 8]; // p3 p2 p1 p0 q0 q1 q2 q3
    let vertical = xs == 1;
    // Every access is in rows/columns o − 4·xs .. o + 3·xs (+ (N − 1)·ls): one check
    // here, unchecked accesses below.
    assert!(o >= 4 * xs && o + 3 * xs + (N - 1) * ls < d.len());
    if !CHROMA && vertical {
        let t = crate::simd::load_t16x8(d, o - 4, ls);
        for j in 0..8 {
            for l in 0..N {
                v[j][l] = t[j][l] as i16;
            }
        }
    } else if !vertical {
        let lo = if CHROMA { 2 } else { 0 };
        for j in lo..8 - lo {
            let b = o + j * xs - 4 * xs;
            for l in 0..N {
                // SAFETY: within the asserted range (ls == 1 here).
                v[j][l] = unsafe { *d.get_unchecked(b + l) } as i16;
            }
        }
    } else {
        // chroma, vertical edge: 8 lines × (p1 p0 q0 q1)
        let t = crate::simd::load_t8x4(d, o - 2, ls);
        for j in 0..4 {
            for l in 0..N {
                v[2 + j][l] = t[j][l] as i16;
            }
        }
    }
    let (alpha, beta) = (p.alpha as i16, p.beta as i16);
    // per-lane constants
    let mut bsm = [0i16; N]; // -1 where bS > 0
    let mut s4 = [0i16; N]; // -1 where bS == 4
    let mut tc0 = [0i16; N];
    let shift = if N == 16 { 2 } else { 1 };
    // per-strength constants, branch-free: tC0 for bS 1..3, 0 for bS 0 and 4
    let tct = &p.tct;
    let segc: [(i16, i16, i16); 4] = core::array::from_fn(|k| {
        let b = seg[k].min(4);
        (-((b != 0) as i16), -((b == 4) as i16), tct[b as usize])
    });
    for l in 0..N {
        let c = segc[l >> shift];
        bsm[l] = c.0;
        s4[l] = c.1;
        tc0[l] = c.2;
    }
    #[inline(always)]
    fn sel(m: i16, a: i16, b: i16) -> i16 {
        (a & m) | (b & !m)
    }
    #[inline(always)]
    fn lt(a: i16, b: i16) -> i16 {
        -((a < b) as i16)
    }
    let mut out = v;
    for l in 0..N {
        let (p1, p0, q0, q1) = (v[2][l], v[3][l], v[4][l], v[5][l]);
        let filt = bsm[l] & lt((p0 - q0).abs(), alpha) & lt((p1 - p0).abs(), beta) & lt((q1 - q0).abs(), beta);
        let strong = s4[l];
        let tc0 = tc0[l];
        let base_delta = (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3;
        if CHROMA {
            let tc = tc0 + 1;
            let delta = base_delta.max(-tc).min(tc);
            let (wp0, wq0) = ((p0 + delta).max(0).min(255), (q0 - delta).max(0).min(255));
            let (np0, nq0) = if STRONG {
                (sel(strong, (2 * p1 + p0 + q1 + 2) >> 2, wp0), sel(strong, (2 * q1 + q0 + p1 + 2) >> 2, wq0))
            } else {
                (wp0, wq0)
            };
            out[3][l] = sel(filt, np0, p0);
            out[4][l] = sel(filt, nq0, q0);
        } else {
            let (p3, p2, q2, q3) = (v[0][l], v[1][l], v[6][l], v[7][l]);
            let ap = lt((p2 - p0).abs(), beta);
            let aq = lt((q2 - q0).abs(), beta);
            let tc = tc0 - ap - aq;
            let delta = base_delta.max(-tc).min(tc);
            let avg = (p0 + q0 + 1) >> 1;
            let n_p1 = sel(ap, p1 + ((p2 + avg - (p1 << 1)) >> 1).max(-tc0).min(tc0), p1);
            let n_q1 = sel(aq, q1 + ((q2 + avg - (q1 << 1)) >> 1).max(-tc0).min(tc0), q1);
            let n_p0 = (p0 + delta).max(0).min(255);
            let n_q0 = (q0 - delta).max(0).min(255);
            if !STRONG {
                // no bS 4 on this edge: normal filter only
                out[2][l] = sel(filt, n_p1, p1);
                out[3][l] = sel(filt, n_p0, p0);
                out[4][l] = sel(filt, n_q0, q0);
                out[5][l] = sel(filt, n_q1, q1);
                continue;
            }
            let small = lt((p0 - q0).abs(), (alpha >> 2) + 2);
            let sp = ap & small;
            let sq = aq & small;
            let s_p0 = sel(sp, (p2 + 2 * p1 + 2 * p0 + 2 * q0 + q1 + 4) >> 3, (2 * p1 + p0 + q1 + 2) >> 2);
            let s_p1 = sel(sp, (p2 + p1 + p0 + q0 + 2) >> 2, p1);
            let s_p2 = sel(sp, (2 * p3 + 3 * p2 + p1 + p0 + q0 + 4) >> 3, p2);
            let s_q0 = sel(sq, (p1 + 2 * p0 + 2 * q0 + 2 * q1 + q2 + 4) >> 3, (2 * q1 + q0 + p1 + 2) >> 2);
            let s_q1 = sel(sq, (p0 + q0 + q1 + q2 + 2) >> 2, q1);
            let s_q2 = sel(sq, (2 * q3 + 3 * q2 + q1 + q0 + p0 + 4) >> 3, q2);
            let fs = filt & strong;
            let fn_ = filt & !strong;
            out[1][l] = sel(fs, s_p2, p2);
            out[2][l] = sel(fs, s_p1, sel(fn_, n_p1, p1));
            out[3][l] = sel(fs, s_p0, sel(fn_, n_p0, p0));
            out[4][l] = sel(fs, s_q0, sel(fn_, n_q0, q0));
            out[5][l] = sel(fs, s_q1, sel(fn_, n_q1, q1));
            out[6][l] = sel(fs, s_q2, q2);
        }
    }
    if !CHROMA && vertical {
        let mut t = [[0u8; 16]; 8];
        for j in 0..8 {
            let row: &[i16; 16] = out[j][..16].try_into().unwrap();
            t[j] = crate::simd::pack16(row);
        }
        crate::simd::store_t16x8(d, o - 4, ls, &t);
    } else if !vertical {
        let (lo, hi) = if CHROMA { (3, 5) } else { (1, 7) };
        for j in lo..hi {
            let b = o + j * xs - 4 * xs;
            if N == 16 {
                let row: &[i16; 16] = out[j][..16].try_into().unwrap();
                let pk = crate::simd::pack16(row);
                // SAFETY: within the asserted range (ls == 1 here).
                unsafe { core::ptr::copy_nonoverlapping(pk.as_ptr(), d.as_mut_ptr().add(b), 16) };
            } else {
                for l in 0..N {
                    // SAFETY: within the asserted range.
                    unsafe { *d.get_unchecked_mut(b + l) = out[j][l] as u8 };
                }
            }
        }
    } else {
        let (p0, q0): ([u8; 8], [u8; 8]) = (core::array::from_fn(|l| out[3][l] as u8), core::array::from_fn(|l| out[4][l] as u8));
        crate::simd::store_t8x2(d, o - 1, ls, &p0, &q0);
    }
}

/// One chroma edge in both chroma planes at once: lanes 0..8 are the 8 lines of
/// Cb, lanes 8..16 the same lines of Cr (they share bS and QP), so the filter runs
/// as one 16-lane pass instead of two 8-lane ones.
#[inline(always)]
#[allow(clippy::manual_clamp)]
fn filter_chroma_uv<const STRONG: bool>(u: &mut [u8], v: &mut [u8], o: usize, xs: usize, ls: usize, seg: &[u8; 4], p: &Params) {
    let vertical = xs == 1;
    assert!(o >= 2 * xs && o + xs + 7 * ls < u.len() && o + xs + 7 * ls < v.len());
    let mut w = [[0i16; 16]; 4]; // p1 p0 q0 q1
    if vertical {
        let (tu, tv) = (crate::simd::load_t8x4(u, o - 2, ls), crate::simd::load_t8x4(v, o - 2, ls));
        for j in 0..4 {
            for l in 0..8 {
                w[j][l] = tu[j][l] as i16;
                w[j][l + 8] = tv[j][l] as i16;
            }
        }
    } else {
        for j in 0..4 {
            let b = o + j * xs - 2 * xs;
            let (mut ru, mut rv) = ([0u8; 8], [0u8; 8]);
            ru.copy_from_slice(&u[b..b + 8]);
            rv.copy_from_slice(&v[b..b + 8]);
            for l in 0..8 {
                w[j][l] = ru[l] as i16;
                w[j][l + 8] = rv[l] as i16;
            }
        }
    }
    let (alpha, beta) = (p.alpha as i16, p.beta as i16);
    let segc: [(i16, i16, i16); 4] = core::array::from_fn(|k| {
        let b = seg[k].min(4);
        (-((b != 0) as i16), -((b == 4) as i16), p.tct[b as usize])
    });
    let (mut bsm, mut s4, mut tc0) = ([0i16; 16], [0i16; 16], [0i16; 16]);
    for l in 0..16 {
        // chroma line l (of 8) uses luma segment l / 2
        let c = segc[(l & 7) >> 1];
        bsm[l] = c.0;
        s4[l] = c.1;
        tc0[l] = c.2;
    }
    let sel = |m: i16, a: i16, b: i16| (a & m) | (b & !m);
    let lt = |a: i16, b: i16| -((a < b) as i16);
    let (mut np0, mut nq0) = ([0i16; 16], [0i16; 16]);
    for l in 0..16 {
        let (p1, p0, q0, q1) = (w[0][l], w[1][l], w[2][l], w[3][l]);
        let filt = bsm[l] & lt((p0 - q0).abs(), alpha) & lt((p1 - p0).abs(), beta) & lt((q1 - q0).abs(), beta);
        let base_delta = (((q0 - p0) << 2) + (p1 - q1) + 4) >> 3;
        let tc = tc0[l] + 1;
        let delta = base_delta.max(-tc).min(tc);
        let (wp0, wq0) = ((p0 + delta).max(0).min(255), (q0 - delta).max(0).min(255));
        let (a0, b0) = if STRONG {
            let strong = s4[l];
            (sel(strong, (2 * p1 + p0 + q1 + 2) >> 2, wp0), sel(strong, (2 * q1 + q0 + p1 + 2) >> 2, wq0))
        } else {
            (wp0, wq0)
        };
        np0[l] = sel(filt, a0, p0);
        nq0[l] = sel(filt, b0, q0);
    }
    let pu: [u8; 8] = core::array::from_fn(|l| np0[l] as u8);
    let qu: [u8; 8] = core::array::from_fn(|l| nq0[l] as u8);
    let pv: [u8; 8] = core::array::from_fn(|l| np0[l + 8] as u8);
    let qv: [u8; 8] = core::array::from_fn(|l| nq0[l + 8] as u8);
    if vertical {
        crate::simd::store_t8x2(u, o - 1, ls, &pu, &qu);
        crate::simd::store_t8x2(v, o - 1, ls, &pv, &qv);
    } else {
        u[o - xs..o - xs + 8].copy_from_slice(&pu);
        u[o..o + 8].copy_from_slice(&qu);
        v[o - xs..o - xs + 8].copy_from_slice(&pv);
        v[o..o + 8].copy_from_slice(&qv);
    }
}

/// Mutable planes of a band of macroblock rows, starting at the band's first
/// sample row (`yp`/`cp` are the horizontal pads).
pub(crate) struct View<'a> {
    pub y: &'a mut [u8],
    pub u: &'a mut [u8],
    pub v: &'a mut [u8],
    pub ys: usize,
    pub cs: usize,
    pub yp: usize,
    pub cp: usize,
}

/// Deblock the whole picture (one slice). Marks every macroblock whose samples
/// may change in `touched`.
pub(crate) fn deblock(p: &mut Pic, info: &[MbInfo], mbw: usize, mbh: usize, touched: &mut [bool]) {
    let (ys, yp, cs, cp) = (p.y.stride, p.y.pad, p.u.stride, p.u.pad);
    let mut view = View {
        y: &mut p.y.data[yp * ys..],
        u: &mut p.u.data[cp * cs..],
        v: &mut p.v.data[cp * cs..],
        ys,
        cs,
        yp,
        cp,
    };
    deblock_rows(&mut view, info, mbw, mbh, touched);
}

/// Deblock the macroblock rows of `view` (`rows` of them); `info`/`touched` are
/// band-local. Edges at the band's top are not filtered (a slice boundary with
/// disable_deblocking_filter_idc 2, or the picture edge).
pub(crate) fn deblock_rows(view: &mut View, info: &[MbInfo], mbw: usize, rows: usize, touched: &mut [bool]) {
    #[cfg(target_arch = "x86_64")]
    {
        if crate::simd::level() >= 2 {
            #[target_feature(enable = "avx2")]
            unsafe fn f(view: &mut View, info: &[MbInfo], mbw: usize, rows: usize, touched: &mut [bool]) {
                deblock_impl(view, info, mbw, rows, touched)
            }
            // SAFETY: AVX2 detected at run time.
            return unsafe { f(view, info, mbw, rows, touched) };
        }
    }
    deblock_impl(view, info, mbw, rows, touched)
}

#[inline(always)]
fn deblock_impl(view: &mut View, info: &[MbInfo], mbw: usize, rows: usize, touched: &mut [bool]) {
    for mby in 0..rows {
        for mbx in 0..mbw {
            let i = mby * mbw + mbx;
            let cur = &info[i];
            // Fast path: copies next to copies with the same motion have bS 0 everywhere.
            if !cur.coded
                && (mbx == 0 || (!info[i - 1].coded && info[i - 1].mv == cur.mv))
                && (mby == 0 || (!info[i - mbw].coded && info[i - mbw].mv == cur.mv))
            {
                continue;
            }
            let (any, l, t) = deblock_mb(view, info, mbw, mbx, mby);
            touched[i] |= any;
            if l {
                touched[i - 1] = true;
            }
            if t {
                touched[i - mbw] = true;
            }
        }
    }
}

/// Returns whether the left / top macroblock edges were filtered.
#[inline(always)]
fn deblock_mb(p: &mut View, info: &[MbInfo], mbw: usize, mbx: usize, mby: usize) -> (bool, bool, bool) {
    let cur = &info[mby * mbw + mbx];
    // bS[dir][edge][segment]
    let mut b = [[[0u8; 4]; 4]; 2];
    let mut any = false;
    // Internal edges: one 16×16 partition, so the MVs agree and bS is 3 (intra)
    // or 2/0 by the coefficient flags of the two blocks.
    let nzm = cur.nz.iter().enumerate().fold(0u32, |m, (i, &n)| m | (((n != 0) as u32) << i));
    let intra = cur.kind != 0;
    for e in 1..4 {
        for k in 0..4 {
            let mut v = if intra { 3 } else { 2 * ((nzm >> (k * 4 + e - 1) | nzm >> (k * 4 + e)) & 1) as u8 };
            let mut h = if intra { 3 } else { 2 * ((nzm >> ((e - 1) * 4 + k) | nzm >> (e * 4 + k)) & 1) as u8 };
            if e == 2 && cur.part != 0 && !intra {
                // edges between 8×8 partitions: motion vector difference
                let far = |a: (i16, i16), b: (i16, i16)| ((a.0 - b.0).abs() >= 4 || (a.1 - b.1).abs() >= 4) as u8;
                let r = k / 2;
                if v == 0 {
                    v = far(cur.qmv(r * 2), cur.qmv(r * 2 + 1));
                }
                if h == 0 {
                    h = far(cur.qmv(r), cur.qmv(r + 2));
                }
            }
            b[0][e][k] = v;
            b[1][e][k] = h;
            any |= v != 0 || h != 0;
        }
    }
    for k in 0..4 {
        // macroblock edges: vertical (x = 0), segment k (rows 4k..), and horizontal (y = 0)
        let v = if mbx == 0 { 0 } else { bs(&info[mby * mbw + mbx - 1], k * 4 + 3, cur, k * 4, true) };
        let h = if mby == 0 { 0 } else { bs(&info[(mby - 1) * mbw + mbx], 12 + k, cur, k, true) };
        b[0][0][k] = v;
        b[1][0][k] = h;
        any |= v != 0 || h != 0;
    }
    if !any {
        return (false, false, false);
    }
    // Internal edges use the macroblock's QP; edges to a neighbour use the
    // average of both QPs (luma) and of both chroma QPs (chroma).
    let avg = |a: u8, b: u8| ((a as u32 + b as u32 + 1) >> 1) as u8;
    let (lpi, cpi) = (params(cur.qp), params(chroma_qp(cur.qp)));
    let nb = |n: Option<&MbInfo>| match n {
        Some(n) if n.qp != cur.qp => (params(avg(n.qp, cur.qp)), params(avg(chroma_qp(n.qp), chroma_qp(cur.qp)))),
        _ => (lpi, cpi),
    };
    let (lpl, cpl) = nb(if mbx > 0 { Some(&info[mby * mbw + mbx - 1]) } else { None });
    let (lpt, cpt) = nb(if mby > 0 { Some(&info[(mby - 1) * mbw + mbx]) } else { None });
    let (s, base) = (p.ys, mby * 16 * p.ys + p.yp + mbx * 16);
    for e in 0..4 {
        if b[0][e] == [0; 4] {
            continue;
        }
        // only macroblock edges can carry bS 4 (the strong filter)
        if e == 0 && b[0][0].contains(&4) {
            filter_edge::<16, false, true>(p.y, base, 1, s, &b[0][0], &lpl);
        } else {
            filter_edge::<16, false, false>(p.y, base + e * 4, 1, s, &b[0][e], if e == 0 { &lpl } else { &lpi });
        }
    }
    for e in 0..4 {
        if b[1][e] == [0; 4] {
            continue;
        }
        if e == 0 && b[1][0].contains(&4) {
            filter_edge::<16, false, true>(p.y, base, s, 1, &b[1][0], &lpt);
        } else {
            filter_edge::<16, false, false>(p.y, base + e * 4 * s, s, 1, &b[1][e], if e == 0 { &lpt } else { &lpi });
        }
    }
    let (s, base) = (p.cs, mby * 8 * p.cs + p.cp + mbx * 8);
    // chroma edges 0 and 4 use luma edges 0 and 2; chroma line l uses luma segment l/2
    let (u, v) = (&mut *p.u, &mut *p.v);
    for (ce, le) in [(0usize, 0usize), (1, 2)] {
        let bs = &b[0][le];
        if *bs != [0; 4] {
            let par = if ce == 0 { &cpl } else { &cpi };
            if ce == 0 && bs.contains(&4) {
                filter_chroma_uv::<true>(u, v, base, 1, s, bs, par);
            } else {
                filter_chroma_uv::<false>(u, v, base + ce * 4, 1, s, bs, par);
            }
        }
    }
    for (ce, le) in [(0usize, 0usize), (1, 2)] {
        let bs = &b[1][le];
        if *bs != [0; 4] {
            let par = if ce == 0 { &cpt } else { &cpi };
            if ce == 0 && bs.contains(&4) {
                filter_chroma_uv::<true>(u, v, base, s, 1, bs, par);
            } else {
                filter_chroma_uv::<false>(u, v, base + ce * 4 * s, s, 1, bs, par);
            }
        }
    }
    (true, b[0][0].iter().any(|&v| v != 0), b[1][0].iter().any(|&v| v != 0))
}

