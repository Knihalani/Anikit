//! The encoder: frame/slice/macroblock coding.

use crate::bits::{se_len, write_nal, BitWriter};
use crate::cavlc::write_block;
use crate::dsp;
use crate::frame::{Pic, YuvFrame};
use crate::headers::*;
use crate::intra::{self, Edges, Edges4};
use crate::transform::*;

/// Border (in luma samples) kept around reference pictures for motion compensation.
pub(crate) const PAD: usize = 32;

/// Encoder settings.
#[derive(Clone, Debug)]
pub struct EncoderConfig {
    /// Picture size in luma samples (even numbers; any size, padded internally to 16).
    pub width: u32,
    pub height: u32,
    /// Frame rate (signalled in the VUI).
    pub fps: f64,
    /// Constant quantiser 0..=51 (lower is better). 20–26 is typical; default 22.
    pub qp: u8,
    /// IDR interval in frames (also the chunk length of [`crate::encode_chunks`]).
    pub gop: u32,
    /// In-loop deblocking filter.
    pub deblock: bool,
    /// Slices per frame encoded in parallel by [`Encoder::encode`] (1 = single slice).
    pub threads: usize,
    /// Full-range (JPEG) levels instead of limited (16–235).
    pub full_range: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        EncoderConfig { width: 0, height: 0, fps: 30.0, qp: 22, gop: 250, deblock: false, threads: 1, full_range: false }
    }
}

/// A known integer translation of a region since the previous frame: the content
/// now inside the rectangle (luma pixels, current frame) moved by `(dx, dy)`, i.e.
/// `cur(x, y) == prev(x - dx, y - dy)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionHint {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub dx: i32,
    pub dy: i32,
}

/// What the renderer knows about a frame.
#[derive(Clone, Debug, Default)]
pub struct FrameHints {
    /// Per-macroblock "changed since the previous frame" flags, row-major,
    /// `ceil(w/16) * ceil(h/16)` entries. `None`: computed by exact comparison.
    /// A macroblock marked unchanged is coded as a copy of the previous frame.
    pub changed: Option<Vec<bool>>,
    /// Known translations (seeds for motion search).
    pub motions: Vec<MotionHint>,
}

/// Per-frame statistics.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    pub idr: bool,
    pub bytes: usize,
    pub skip: u32,
    pub inter: u32,
    pub intra16: u32,
    pub intra4: u32,
    /// Macroblocks that went through mode decision.
    pub changed: u32,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct MbInfo {
    /// 0 = inter (incl. skip), 1 = I16, 2 = I4x4
    pub kind: u8,
    pub skip: bool,
    pub mv: (i16, i16),
    /// Total coefficients per 4×4 luma block (raster), for nC and deblocking.
    pub nz: [u8; 16],
    pub nzc: [[u8; 4]; 2],
    /// I4x4 modes (raster), 2 when not I4x4.
    pub m4: [u8; 16],
}

const BLK_X: [usize; 16] = [0, 1, 0, 1, 2, 3, 2, 3, 0, 1, 0, 1, 2, 3, 2, 3];
const BLK_Y: [usize; 16] = [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 3, 3, 2, 2, 3, 3];
const fn blk_idx(bx: usize, by: usize) -> usize {
    (by / 2) * 8 + (bx / 2) * 4 + (by % 2) * 2 + (bx % 2)
}

/// coded_block_pattern → codeNum (Table 9-4, ChromaArrayType 1).
const CBP_INTRA: [u8; 48] = [
    3, 29, 30, 17, 31, 18, 37, 8, 32, 38, 19, 9, 20, 10, 11, 2, 16, 33, 34, 21, 35, 22, 39, 4, 36, 40, 23, 5, 24, 6, 7, 1,
    41, 42, 43, 25, 44, 26, 46, 12, 45, 47, 27, 13, 28, 14, 15, 0,
];
const CBP_INTER: [u8; 48] = [
    0, 2, 3, 7, 4, 8, 17, 13, 5, 18, 9, 14, 10, 15, 16, 11, 1, 32, 33, 36, 34, 37, 44, 40, 35, 45, 38, 41, 39, 42, 43, 19,
    6, 24, 25, 20, 26, 21, 46, 28, 27, 47, 22, 29, 23, 30, 31, 12,
];

/// Residual data of one macroblock (levels in raster order per 4×4 block).
#[derive(Clone)]
struct Coded {
    luma: [[i32; 16]; 16],
    dc: [i32; 16],
    cdc: [[i32; 4]; 2],
    cac: [[[i32; 16]; 4]; 2],
    nz: [u8; 16],
    nzc: [[u8; 4]; 2],
    cbp: u8,
}

impl Coded {
    fn new() -> Self {
        Coded {
            luma: [[0; 16]; 16],
            dc: [0; 16],
            cdc: [[0; 4]; 2],
            cac: [[[0; 16]; 4]; 2],
            nz: [0; 16],
            nzc: [[0; 4]; 2],
            cbp: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    P16,
    I16(u8),
    I4,
}

/// Shared, read-only state for coding one frame.
pub(crate) struct FrameCtx<'a> {
    pub mbw: usize,
    pub q: Quant,
    pub qc: Quant,
    pub lambda: i32,
    pub src: &'a Pic,
    pub refp: &'a Pic,
    pub changed: &'a [bool],
    pub hint: &'a [(i16, i16)],
    pub idr: bool,
    pub max_mvy: i32,
    pub hdr_frame_num: u32,
    pub idr_id: u32,
    pub deblock: bool,
}

pub(crate) const NO_HINT: (i16, i16) = (i16::MIN, i16::MIN);

/// Mutable band of the reconstructed picture owned by one slice.
pub(crate) struct Band<'a> {
    pub y: &'a mut [u8],
    pub u: &'a mut [u8],
    pub v: &'a mut [u8],
    pub info: &'a mut [MbInfo],
    pub mby0: usize,
    pub mby1: usize,
}

#[derive(Clone, Copy)]
struct Nb {
    avail: bool,
    inter: bool,
    mv: (i32, i32),
}

struct SliceEnc<'a, 'b> {
    c: &'a FrameCtx<'a>,
    b: Band<'b>,
    w: &'b mut BitWriter,
    ys: usize,
    yp: usize,
    cs: usize,
    cp: usize,
    skip_run: u32,
    stats: FrameStats,
}

#[inline(always)]
fn median(a: i32, b: i32, c: i32) -> i32 {
    a.max(b).min(a.min(b).max(c))
}

impl<'a, 'b> SliceEnc<'a, 'b> {
    #[inline(always)]
    fn yi(&self, x: usize, y: usize) -> usize {
        (y - self.b.mby0 * 16) * self.ys + self.yp + x
    }
    #[inline(always)]
    fn ci(&self, x: usize, y: usize) -> usize {
        (y - self.b.mby0 * 8) * self.cs + self.cp + x
    }
    #[inline(always)]
    fn info(&self, mbx: usize, mby: usize) -> &MbInfo {
        &self.b.info[(mby - self.b.mby0) * self.c.mbw + mbx]
    }

    fn nb(&self, mbx: isize, mby: isize) -> Nb {
        if mbx < 0 || mby < self.b.mby0 as isize || mbx >= self.c.mbw as isize {
            return Nb { avail: false, inter: false, mv: (0, 0) };
        }
        let i = self.info(mbx as usize, mby as usize);
        if i.kind == 0 {
            Nb { avail: true, inter: true, mv: (i.mv.0 as i32, i.mv.1 as i32) }
        } else {
            Nb { avail: true, inter: false, mv: (0, 0) }
        }
    }

    /// (mvp for P16x16, P_Skip mv) in full pels.
    fn mv_pred(&self, mbx: usize, mby: usize) -> ((i32, i32), (i32, i32)) {
        let (x, y) = (mbx as isize, mby as isize);
        let a = self.nb(x - 1, y);
        let mut b = self.nb(x, y - 1);
        let mut c = self.nb(x + 1, y - 1);
        if !c.avail {
            c = self.nb(x - 1, y - 1);
        }
        let skip_zero = !a.avail || !b.avail || (a.inter && a.mv == (0, 0)) || (b.inter && b.mv == (0, 0));
        if !b.avail && !c.avail && a.avail {
            b = a;
            c = a;
        }
        let n = a.inter as u32 + b.inter as u32 + c.inter as u32;
        let mvp = if n == 1 {
            if a.inter {
                a.mv
            } else if b.inter {
                b.mv
            } else {
                c.mv
            }
        } else {
            (median(a.mv.0, b.mv.0, c.mv.0), median(a.mv.1, b.mv.1, c.mv.1))
        };
        (mvp, if skip_zero { (0, 0) } else { mvp })
    }

    // ---------- prediction helpers ----------

    fn luma_ref<'r>(&self, mbx: usize, mby: usize, mv: (i32, i32)) -> (&'r [u8], usize)
    where
        'a: 'r,
    {
        let p = &self.c.refp.y;
        let o = p.off(mbx as isize * 16 + mv.0 as isize, mby as isize * 16 + mv.1 as isize);
        (&p.data[o..], p.stride)
    }

    fn mc_chroma(&self, comp: usize, mbx: usize, mby: usize, mv: (i32, i32), out: &mut [u8; 64]) {
        let p = if comp == 0 { &self.c.refp.u } else { &self.c.refp.v };
        let (mx, my) = (mv.0 * 4, mv.1 * 4); // 1/8 chroma units
        let (ix, fx) = (mx >> 3, mx & 7);
        let (iy, fy) = (my >> 3, my & 7);
        let o = p.off(mbx as isize * 8 + ix as isize, mby as isize * 8 + iy as isize);
        let s = p.stride;
        let d = &p.data;
        if fx == 0 && fy == 0 {
            for y in 0..8 {
                out[y * 8..y * 8 + 8].copy_from_slice(&d[o + y * s..o + y * s + 8]);
            }
            return;
        }
        let (w00, w01, w10, w11) = ((8 - fx) * (8 - fy), fx * (8 - fy), (8 - fx) * fy, fx * fy);
        for y in 0..8 {
            for x in 0..8 {
                let i = o + y * s + x;
                let v = w00 * d[i] as i32 + w01 * d[i + 1] as i32 + w10 * d[i + s] as i32 + w11 * d[i + s + 1] as i32;
                out[y * 8 + x] = ((v + 32) >> 6) as u8;
            }
        }
    }

    fn edges16(&self, mbx: usize, mby: usize) -> Edges<16> {
        let (x0, y0) = (mbx * 16, mby * 16);
        let has_top = mby > self.b.mby0;
        let has_left = mbx > 0;
        let mut e = Edges { top: [0; 16], left: [0; 16], tl: 0, has_top, has_left, has_tl: has_top && has_left };
        if has_top {
            let i = self.yi(x0, y0 - 1);
            e.top.copy_from_slice(&self.b.y[i..i + 16]);
        }
        if has_left {
            for k in 0..16 {
                e.left[k] = self.b.y[self.yi(x0 - 1, y0 + k)];
            }
        }
        if e.has_tl {
            e.tl = self.b.y[self.yi(x0 - 1, y0 - 1)];
        }
        e
    }

    fn edges8(&self, comp: usize, mbx: usize, mby: usize) -> Edges<8> {
        let (x0, y0) = (mbx * 8, mby * 8);
        let has_top = mby > self.b.mby0;
        let has_left = mbx > 0;
        let pl: &[u8] = if comp == 0 { self.b.u } else { self.b.v };
        let mut e = Edges { top: [0; 8], left: [0; 8], tl: 0, has_top, has_left, has_tl: has_top && has_left };
        if has_top {
            let i = self.ci(x0, y0 - 1);
            e.top.copy_from_slice(&pl[i..i + 8]);
        }
        if has_left {
            for k in 0..8 {
                e.left[k] = pl[self.ci(x0 - 1, y0 + k)];
            }
        }
        if e.has_tl {
            e.tl = pl[self.ci(x0 - 1, y0 - 1)];
        }
        e
    }

    fn intra16_modes_ok(e: &Edges<16>, m: u8) -> bool {
        match m {
            0 => e.has_top,
            1 => e.has_left,
            2 => true,
            _ => e.has_top && e.has_left && e.has_tl,
        }
    }

    // ---------- the macroblock ----------

    fn encode_mb(&mut self, mbx: usize, mby: usize, src_y: &[u8; 256], src_c: &[[u8; 64]; 2]) {
        let c = self.c;
        let mbi = mby * c.mbw + mbx;
        let lambda = c.lambda;

        let mut best_inter: Option<((i32, i32), u32, (i32, i32), (i32, i32))> = None; // (mv, cost, mvp, skipmv)
        if !c.idr {
            let (mvp, skip_mv) = self.mv_pred(mbx, mby);
            if !c.changed[mbi] {
                // Unchanged: copy of the reference.
                self.code_copy(mbx, mby, mvp, skip_mv);
                return;
            }
            best_inter = Some(self.motion_search(mbx, mby, src_y, mvp, skip_mv, mbi));
        }
        self.stats.changed += 1;

        // Intra 16x16 decision.
        let e16 = self.edges16(mbx, mby);
        let mut pred = [0u8; 256];
        let mut best16 = (u32::MAX, 2u8);
        for m in 0..4u8 {
            if !Self::intra16_modes_ok(&e16, m) {
                continue;
            }
            intra::pred16(&e16, m, &mut pred);
            let s = dsp::satd(src_y, &pred, 16, 16);
            if s < best16.0 {
                best16 = (s, m);
            }
        }
        let cost16 = best16.0 + lambda as u32 * if c.idr { 4 } else { 8 };

        let mut kind = Kind::I16(best16.1);
        let mut best_cost = cost16;
        let mut inter_mv = (0, 0);
        if let Some((mv, sad_cost, _, _)) = best_inter {
            // re-measure inter with SATD for a fair comparison
            let (r, rs) = self.luma_ref(mbx, mby, mv);
            let mut p = [0u8; 256];
            for y in 0..16 {
                p[y * 16..y * 16 + 16].copy_from_slice(&r[y * rs..y * rs + 16]);
            }
            let s = dsp::satd(src_y, &p, 16, 16);
            let cost = s + sad_cost; // sad_cost carries the mv bit cost only
            if cost <= best_cost {
                best_cost = cost;
                kind = Kind::P16;
                inter_mv = mv;
            }
        }

        // Chroma intra mode (needed for both intra kinds).
        let mut cmode = 0u8;
        let mut cd = Coded::new();
        let try_i4 = best_cost > lambda as u32 * 24 + 256;
        if kind != Kind::P16 || try_i4 {
            if try_i4 {
                // Encode I4x4 tentatively (writes recon).
                let (cost4, modes) = self.code_i4(mbx, mby, src_y, &mut cd, true);
                if cost4 < best_cost {
                    kind = Kind::I4;
                    best_cost = cost4;
                    let _ = best_cost;
                    self.finish_intra(mbx, mby, src_c, &mut cd, kind, Some(modes));
                    return;
                }
                cd = Coded::new();
            }
        }
        match kind {
            Kind::P16 => {
                let (mvp, skip_mv) = (best_inter.unwrap().2, best_inter.unwrap().3);
                self.code_inter(mbx, mby, src_y, src_c, inter_mv, mvp, skip_mv);
            }
            Kind::I16(_) | Kind::I4 => {
                cmode = cmode.max(0);
                let _ = cmode;
                self.finish_intra(mbx, mby, src_c, &mut cd, kind, None);
            }
        }
    }

    /// Integer-pel motion search seeded by zero, hints and neighbours.
    /// Returns (mv, lambda * mv bits, mvp, skip mv).
    fn motion_search(
        &self,
        mbx: usize,
        mby: usize,
        src_y: &[u8; 256],
        mvp: (i32, i32),
        skip_mv: (i32, i32),
        mbi: usize,
    ) -> ((i32, i32), u32, (i32, i32), (i32, i32)) {
        let c = self.c;
        let lambda = c.lambda;
        let w16 = (c.mbw * 16) as i32;
        let h16 = (c.refp.y.h) as i32;
        let (bx, by) = (mbx as i32 * 16, mby as i32 * 16);
        let xr = (-16 - bx, w16 - bx);
        let yr = ((-16 - by).max(-c.max_mvy), (h16 - by).min(c.max_mvy));
        let clamp = |mv: (i32, i32)| (mv.0.clamp(xr.0, xr.1), mv.1.clamp(yr.0, yr.1));
        let mvcost = |mv: (i32, i32)| -> u32 {
            lambda as u32 * (se_len(4 * (mv.0 - mvp.0)) + se_len(4 * (mv.1 - mvp.1)))
        };
        let eval = |mv: (i32, i32)| -> u32 {
            let (r, rs) = self.luma_ref(mbx, mby, mv);
            dsp::sad16(src_y, 16, r, rs) + mvcost(mv)
        };
        let mut best = (u32::MAX, (0, 0));
        let mut cands = [(0i32, 0i32); 7];
        let mut n = 0;
        let mut push = |mv: (i32, i32)| {
            let mv = clamp(mv);
            if !cands[..n].contains(&mv) {
                cands[n] = mv;
                n += 1;
            }
        };
        push((0, 0));
        push(skip_mv);
        push(mvp);
        let h = c.hint[mbi];
        if h != NO_HINT {
            push((h.0 as i32, h.1 as i32));
        }
        let (x, y) = (mbx as isize, mby as isize);
        for nb in [self.nb(x - 1, y), self.nb(x, y - 1), self.nb(x + 1, y - 1)] {
            if nb.inter {
                push(nb.mv);
            }
        }
        for &mv in &cands[..n] {
            let s = eval(mv);
            if s < best.0 {
                best = (s, mv);
            }
        }
        // small diamond refinement
        if best.0 > mvcost(best.1) {
            let mut steps = 0;
            loop {
                let cur = best;
                for d in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                    let mv = clamp((cur.1 .0 + d.0, cur.1 .1 + d.1));
                    if mv == cur.1 {
                        continue;
                    }
                    let s = eval(mv);
                    if s < best.0 {
                        best = (s, mv);
                    }
                }
                steps += 1;
                if best.1 == cur.1 || steps >= 8 {
                    break;
                }
            }
        }
        (best.1, mvcost(best.1), mvp, skip_mv)
    }

    /// Unchanged macroblock: copy the co-located reference (MV 0, no residual).
    fn code_copy(&mut self, mbx: usize, mby: usize, mvp: (i32, i32), skip_mv: (i32, i32)) {
        // recon = ref at mv 0
        self.copy_pred(mbx, mby, (0, 0));
        let mut skip = skip_mv == (0, 0);
        if !skip && self.pred_equals_zero(mbx, mby, skip_mv) {
            skip = true;
        }
        let mv = if skip { skip_mv } else { (0, 0) };
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo { kind: 0, skip, mv: (mv.0 as i16, mv.1 as i16), nz: [0; 16], nzc: [[0; 4]; 2], m4: [2; 16] };
        if skip {
            self.skip_run += 1;
            self.stats.skip += 1;
        } else {
            self.w.ue(self.skip_run);
            self.skip_run = 0;
            self.w.ue(0); // P_L0_16x16
            self.w.se(-4 * mvp.0);
            self.w.se(-4 * mvp.1);
            self.w.ue(CBP_INTER[0] as u32);
            self.stats.inter += 1;
        }
    }

    /// Does motion compensation at `mv` reproduce the co-located reference exactly?
    fn pred_equals_zero(&self, mbx: usize, mby: usize, mv: (i32, i32)) -> bool {
        let c = self.c;
        let w16 = (c.mbw * 16) as i32;
        let h16 = c.refp.y.h as i32;
        let (bx, by) = (mbx as i32 * 16 + mv.0, mby as i32 * 16 + mv.1);
        if bx < -16 || bx > w16 || by < -16 || by > h16 || mv.1.abs() > c.max_mvy {
            return false;
        }
        let (a, sa) = self.luma_ref(mbx, mby, mv);
        let (z, sz) = self.luma_ref(mbx, mby, (0, 0));
        if !dsp::block_eq(a, sa, z, sz, 16, 16) {
            return false;
        }
        let mut p0 = [0u8; 64];
        let mut p1 = [0u8; 64];
        for comp in 0..2 {
            self.mc_chroma(comp, mbx, mby, mv, &mut p0);
            self.mc_chroma(comp, mbx, mby, (0, 0), &mut p1);
            if p0 != p1 {
                return false;
            }
        }
        true
    }

    fn copy_pred(&mut self, mbx: usize, mby: usize, mv: (i32, i32)) {
        let (r, rs) = self.luma_ref(mbx, mby, mv);
        let base = self.yi(mbx * 16, mby * 16);
        for y in 0..16 {
            self.b.y[base + y * self.ys..base + y * self.ys + 16].copy_from_slice(&r[y * rs..y * rs + 16]);
        }
        let mut p = [0u8; 64];
        for comp in 0..2 {
            self.mc_chroma(comp, mbx, mby, mv, &mut p);
            let base = self.ci(mbx * 8, mby * 8);
            let cs = self.cs;
            let pl: &mut [u8] = if comp == 0 { self.b.u } else { self.b.v };
            for y in 0..8 {
                pl[base + y * cs..base + y * cs + 8].copy_from_slice(&p[y * 8..y * 8 + 8]);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn code_inter(
        &mut self,
        mbx: usize,
        mby: usize,
        src_y: &[u8; 256],
        src_c: &[[u8; 64]; 2],
        mv: (i32, i32),
        mvp: (i32, i32),
        skip_mv: (i32, i32),
    ) {
        let mut pred = [0u8; 256];
        {
            let (r, rs) = self.luma_ref(mbx, mby, mv);
            for y in 0..16 {
                pred[y * 16..y * 16 + 16].copy_from_slice(&r[y * rs..y * rs + 16]);
            }
        }
        let mut predc = [[0u8; 64]; 2];
        self.mc_chroma(0, mbx, mby, mv, &mut predc[0]);
        self.mc_chroma(1, mbx, mby, mv, &mut predc[1]);
        let mut cd = Coded::new();
        let q = self.c.q;
        let mut recon = [0u8; 256];
        luma_inter(src_y, &pred, &q, &mut cd, &mut recon);
        let mut reconc = [[0u8; 64]; 2];
        chroma_resid(src_c, &predc, &self.c.qc, false, &mut cd, &mut reconc);
        self.store_recon(mbx, mby, &recon, &reconc);
        let skip = cd.cbp == 0 && mv == skip_mv;
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] =
            MbInfo { kind: 0, skip, mv: (mv.0 as i16, mv.1 as i16), nz: cd.nz, nzc: cd.nzc, m4: [2; 16] };
        if skip {
            self.skip_run += 1;
            self.stats.skip += 1;
            return;
        }
        self.stats.inter += 1;
        self.w.ue(self.skip_run);
        self.skip_run = 0;
        self.w.ue(0);
        self.w.se(4 * (mv.0 - mvp.0));
        self.w.se(4 * (mv.1 - mvp.1));
        self.w.ue(CBP_INTER[cd.cbp as usize] as u32);
        if cd.cbp != 0 {
            self.w.se(0);
            self.write_residual(mbx, mby, &cd, false);
        }
    }

    fn store_recon(&mut self, mbx: usize, mby: usize, y16: &[u8; 256], c8: &[[u8; 64]; 2]) {
        let base = self.yi(mbx * 16, mby * 16);
        for y in 0..16 {
            self.b.y[base + y * self.ys..base + y * self.ys + 16].copy_from_slice(&y16[y * 16..y * 16 + 16]);
        }
        let base = self.ci(mbx * 8, mby * 8);
        let cs = self.cs;
        for y in 0..8 {
            self.b.u[base + y * cs..base + y * cs + 8].copy_from_slice(&c8[0][y * 8..y * 8 + 8]);
            self.b.v[base + y * cs..base + y * cs + 8].copy_from_slice(&c8[1][y * 8..y * 8 + 8]);
        }
    }

    fn edges4(&self, mbx: usize, mby: usize, bx: usize, by: usize) -> Edges4 {
        let top_mb = mby > self.b.mby0;
        let left_mb = mbx > 0;
        let has_top = by > 0 || top_mb;
        let has_left = bx > 0 || left_mb;
        let has_tl = has_top && has_left;
        let has_tr = if by == 0 {
            if bx < 3 {
                top_mb
            } else {
                top_mb && mbx + 1 < self.c.mbw
            }
        } else {
            bx < 3 && blk_idx(bx + 1, by - 1) < blk_idx(bx, by)
        };
        let (x0, y0) = (mbx * 16 + bx * 4, mby * 16 + by * 4);
        let mut e = Edges4 { top: [0; 8], left: [0; 4], tl: 0, has_top, has_left, has_tl };
        if has_top {
            let i = self.yi(x0, y0 - 1);
            e.top[..4].copy_from_slice(&self.b.y[i..i + 4]);
            if has_tr {
                e.top[4..].copy_from_slice(&self.b.y[i + 4..i + 8]);
            } else {
                let v = e.top[3];
                e.top[4..].fill(v);
            }
        }
        if has_left {
            for k in 0..4 {
                e.left[k] = self.b.y[self.yi(x0 - 1, y0 + k)];
            }
        }
        if has_tl {
            e.tl = self.b.y[self.yi(x0 - 1, y0 - 1)];
        }
        e
    }

    fn pred_mode4(&self, mbx: usize, mby: usize, bx: usize, by: usize, modes: &[u8; 16]) -> u8 {
        let a = if bx > 0 {
            Some(modes[by * 4 + bx - 1])
        } else if mbx > 0 {
            let i = self.info(mbx - 1, mby);
            Some(if i.kind == 2 { i.m4[by * 4 + 3] } else { 2 })
        } else {
            None
        };
        let b = if by > 0 {
            Some(modes[(by - 1) * 4 + bx])
        } else if mby > self.b.mby0 {
            let i = self.info(mbx, mby - 1);
            Some(if i.kind == 2 { i.m4[12 + bx] } else { 2 })
        } else {
            None
        };
        match (a, b) {
            (Some(a), Some(b)) => a.min(b),
            _ => 2,
        }
    }

    /// Code luma as I4x4 into recon and `cd`. Returns (cost, modes raster).
    fn code_i4(&mut self, mbx: usize, mby: usize, src: &[u8; 256], cd: &mut Coded, _decide: bool) -> (u32, [u8; 16]) {
        let lambda = self.c.lambda as u32;
        let q = self.c.q;
        let mut modes = [2u8; 16];
        let mut cost = lambda * if self.c.idr { 12 } else { 20 };
        let mut pred = [0u8; 16];
        let mut best_pred = [0u8; 16];
        for (&bx, &by) in BLK_X.iter().zip(BLK_Y.iter()) {
            let e = self.edges4(mbx, mby, bx, by);
            let pm = self.pred_mode4(mbx, mby, bx, by, &modes);
            let mut s4 = [0u8; 16];
            for y in 0..4 {
                s4[y * 4..y * 4 + 4].copy_from_slice(&src[(by * 4 + y) * 16 + bx * 4..(by * 4 + y) * 16 + bx * 4 + 4]);
            }
            let mut best = (u32::MAX, 2u8);
            for m in 0..9u8 {
                if !intra::mode4_ok(&e, m) {
                    continue;
                }
                intra::pred4(&e, m, &mut pred);
                let s = dsp::satd4(&s4, &pred, 4, 0, 0) + lambda * if m == pm { 1 } else { 4 };
                if s < best.0 {
                    best = (s, m);
                    best_pred = pred;
                }
            }
            cost += best.0;
            modes[by * 4 + bx] = best.1;
            // code the block
            let mut r = [0i32; 16];
            for i in 0..16 {
                r[i] = s4[i] as i32 - best_pred[i] as i32;
            }
            let mut co = fdct4(&r);
            let n = quant4(&mut co, &q, true, false);
            clamp_levels(&mut co);
            let bi = by * 4 + bx;
            cd.luma[bi] = co;
            cd.nz[bi] = n as u8;
            let mut out = best_pred;
            if n > 0 {
                dequant4(&mut co, &q, false);
                let res = idct4(&co);
                for i in 0..16 {
                    out[i] = (best_pred[i] as i32 + res[i]).clamp(0, 255) as u8;
                }
            }
            let base = self.yi(mbx * 16 + bx * 4, mby * 16 + by * 4);
            for y in 0..4 {
                self.b.y[base + y * self.ys..base + y * self.ys + 4].copy_from_slice(&out[y * 4..y * 4 + 4]);
            }
        }
        for b8 in 0..4 {
            let (x8, y8) = ((b8 & 1) * 2, (b8 >> 1) * 2);
            if cd.nz[y8 * 4 + x8] | cd.nz[y8 * 4 + x8 + 1] | cd.nz[y8 * 4 + x8 + 4] | cd.nz[y8 * 4 + x8 + 5] != 0 {
                cd.cbp |= 1 << b8;
            }
        }
        (cost, modes)
    }

    /// Finish an intra macroblock (I16 luma if needed, chroma) and write it.
    fn finish_intra(
        &mut self,
        mbx: usize,
        mby: usize,
        src_c: &[[u8; 64]; 2],
        cd: &mut Coded,
        kind: Kind,
        modes: Option<[u8; 16]>,
    ) {
        let q = self.c.q;
        if let Kind::I16(m) = kind {
            let e16 = self.edges16(mbx, mby);
            let mut pred = [0u8; 256];
            intra::pred16(&e16, m, &mut pred);
            let mut src = [0u8; 256];
            let base = self.c.src.y.off(mbx as isize * 16, mby as isize * 16);
            let ss = self.c.src.y.stride;
            for y in 0..16 {
                src[y * 16..y * 16 + 16].copy_from_slice(&self.c.src.y.data[base + y * ss..base + y * ss + 16]);
            }
            let mut recon = [0u8; 256];
            luma_i16(&src, &pred, &q, cd, &mut recon);
            let base = self.yi(mbx * 16, mby * 16);
            for y in 0..16 {
                self.b.y[base + y * self.ys..base + y * self.ys + 16].copy_from_slice(&recon[y * 16..y * 16 + 16]);
            }
        }
        // chroma mode decision
        let ec = [self.edges8(0, mbx, mby), self.edges8(1, mbx, mby)];
        let mut best = (u32::MAX, 0u8);
        let mut predc = [[0u8; 64]; 2];
        for m in 0..4u8 {
            let ok = match m {
                0 => true,
                1 => ec[0].has_left,
                2 => ec[0].has_top,
                _ => ec[0].has_top && ec[0].has_left,
            };
            if !ok {
                continue;
            }
            let mut s = 0;
            let mut p = [[0u8; 64]; 2];
            for comp in 0..2 {
                intra::pred_chroma(&ec[comp], m, &mut p[comp]);
                s += dsp::satd(&src_c[comp], &p[comp], 8, 8);
            }
            if s < best.0 {
                best = (s, m);
                predc = p;
            }
        }
        let cmode = best.1;
        let mut reconc = [[0u8; 64]; 2];
        chroma_resid(src_c, &predc, &self.c.qc, true, cd, &mut reconc);
        let base = self.ci(mbx * 8, mby * 8);
        let cs = self.cs;
        for y in 0..8 {
            self.b.u[base + y * cs..base + y * cs + 8].copy_from_slice(&reconc[0][y * 8..y * 8 + 8]);
            self.b.v[base + y * cs..base + y * cs + 8].copy_from_slice(&reconc[1][y * 8..y * 8 + 8]);
        }

        // ---- syntax ----
        if !self.c.idr {
            self.w.ue(self.skip_run);
            self.skip_run = 0;
        }
        let off = if self.c.idr { 0 } else { 5 };
        let modes4 = modes.unwrap_or([2; 16]);
        match kind {
            Kind::I16(m) => {
                let cbpl = if cd.cbp & 15 != 0 { 1 } else { 0 };
                self.w.ue(off + 1 + m as u32 + 4 * (cd.cbp >> 4) as u32 + 12 * cbpl);
                self.w.ue(cmode as u32);
                self.w.se(0);
                self.stats.intra16 += 1;
            }
            _ => {
                self.w.ue(off);
                let mut cur = [2u8; 16];
                for k in 0..16 {
                    let (bx, by) = (BLK_X[k], BLK_Y[k]);
                    let pm = self.pred_mode4(mbx, mby, bx, by, &cur);
                    let m = modes4[by * 4 + bx];
                    cur[by * 4 + bx] = m;
                    if m == pm {
                        self.w.bit(true);
                    } else {
                        self.w.bit(false);
                        self.w.put(3, if m < pm { m } else { m - 1 } as u32);
                    }
                }
                self.w.ue(cmode as u32);
                self.w.ue(CBP_INTRA[cd.cbp as usize] as u32);
                if cd.cbp != 0 {
                    self.w.se(0);
                }
                self.stats.intra4 += 1;
            }
        }
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        let is16 = matches!(kind, Kind::I16(_));
        self.b.info[i] = MbInfo {
            kind: if is16 { 1 } else { 2 },
            skip: false,
            mv: (0, 0),
            nz: cd.nz,
            nzc: cd.nzc,
            m4: modes4,
        };
        if is16 {
            self.write_residual(mbx, mby, cd, true);
        } else if cd.cbp != 0 {
            self.write_residual(mbx, mby, cd, false);
        }
    }

    fn nc_luma(&self, mbx: usize, mby: usize, bx: usize, by: usize, nz: &[u8; 16]) -> i32 {
        let a = if bx > 0 {
            Some(nz[by * 4 + bx - 1])
        } else if mbx > 0 {
            Some(self.info(mbx - 1, mby).nz[by * 4 + 3])
        } else {
            None
        };
        let b = if by > 0 {
            Some(nz[(by - 1) * 4 + bx])
        } else if mby > self.b.mby0 {
            Some(self.info(mbx, mby - 1).nz[12 + bx])
        } else {
            None
        };
        match (a, b) {
            (Some(a), Some(b)) => (a as i32 + b as i32 + 1) >> 1,
            (Some(a), None) => a as i32,
            (None, Some(b)) => b as i32,
            _ => 0,
        }
    }

    fn nc_chroma(&self, mbx: usize, mby: usize, comp: usize, bx: usize, by: usize, nz: &[u8; 4]) -> i32 {
        let a = if bx > 0 {
            Some(nz[by * 2])
        } else if mbx > 0 {
            Some(self.info(mbx - 1, mby).nzc[comp][by * 2 + 1])
        } else {
            None
        };
        let b = if by > 0 {
            Some(nz[bx])
        } else if mby > self.b.mby0 {
            Some(self.info(mbx, mby - 1).nzc[comp][2 + bx])
        } else {
            None
        };
        match (a, b) {
            (Some(a), Some(b)) => (a as i32 + b as i32 + 1) >> 1,
            (Some(a), None) => a as i32,
            (None, Some(b)) => b as i32,
            _ => 0,
        }
    }

    fn write_residual(&mut self, mbx: usize, mby: usize, cd: &Coded, i16: bool) {
        let mut scan = [0i32; 16];
        if i16 {
            for k in 0..16 {
                scan[k] = cd.dc[ZIGZAG[k]];
            }
            let nc = self.nc_luma(mbx, mby, 0, 0, &cd.nz);
            write_block(self.w, &scan, nc);
        }
        if i16 {
            if cd.cbp & 15 != 0 {
                for k in 0..16 {
                    let (bx, by) = (BLK_X[k], BLK_Y[k]);
                    let blk = &cd.luma[by * 4 + bx];
                    for j in 0..15 {
                        scan[j] = blk[ZIGZAG[j + 1]];
                    }
                    let nc = self.nc_luma(mbx, mby, bx, by, &cd.nz);
                    write_block(self.w, &scan[..15], nc);
                }
            }
        } else {
            for k in 0..16 {
                if cd.cbp & (1 << (k / 4)) == 0 {
                    continue;
                }
                let (bx, by) = (BLK_X[k], BLK_Y[k]);
                let blk = &cd.luma[by * 4 + bx];
                for j in 0..16 {
                    scan[j] = blk[ZIGZAG[j]];
                }
                let nc = self.nc_luma(mbx, mby, bx, by, &cd.nz);
                write_block(self.w, &scan, nc);
            }
        }
        let cbpc = cd.cbp >> 4;
        if cbpc != 0 {
            for comp in 0..2 {
                write_block(self.w, &cd.cdc[comp], -1);
            }
        }
        if cbpc == 2 {
            for comp in 0..2 {
                for b in 0..4 {
                    let blk = &cd.cac[comp][b];
                    for j in 0..15 {
                        scan[j] = blk[ZIGZAG[j + 1]];
                    }
                    let nc = self.nc_chroma(mbx, mby, comp, b & 1, b >> 1, &cd.nzc[comp]);
                    write_block(self.w, &scan[..15], nc);
                }
            }
        }
    }
}

#[inline(always)]
fn clamp_levels(c: &mut [i32; 16]) {
    for v in c.iter_mut() {
        *v = (*v).clamp(-2047, 2047);
    }
}

/// x264-style decimation score of a 4×4 block (raster levels, scan order walk).
fn decimate_score(c: &[i32; 16]) -> u32 {
    const DS: [u32; 16] = [3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut i = 15isize;
    while i >= 0 && c[ZIGZAG[i as usize]] == 0 {
        i -= 1;
    }
    let mut score = 0;
    while i >= 0 {
        let v = c[ZIGZAG[i as usize]];
        if v.abs() > 1 {
            return 9;
        }
        i -= 1;
        let mut run = 0;
        while i >= 0 && c[ZIGZAG[i as usize]] == 0 {
            run += 1;
            i -= 1;
        }
        score += DS[run];
    }
    score
}

fn luma_inter(src: &[u8; 256], pred: &[u8; 256], q: &Quant, cd: &mut Coded, recon: &mut [u8; 256]) {
    let mut score8 = [0u32; 4];
    for by in 0..4 {
        for bx in 0..4 {
            let mut r = [0i32; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let i = (by * 4 + y) * 16 + bx * 4 + x;
                    r[y * 4 + x] = src[i] as i32 - pred[i] as i32;
                }
            }
            let mut c = fdct4(&r);
            let n = quant4(&mut c, q, false, false);
            clamp_levels(&mut c);
            let bi = by * 4 + bx;
            cd.luma[bi] = c;
            cd.nz[bi] = n as u8;
            if n > 0 {
                score8[(by / 2) * 2 + bx / 2] += decimate_score(&c);
            }
        }
    }
    let total: u32 = score8.iter().sum();
    for b8 in 0..4 {
        let (x8, y8) = ((b8 & 1) * 2, (b8 >> 1) * 2);
        let blks = [y8 * 4 + x8, y8 * 4 + x8 + 1, y8 * 4 + x8 + 4, y8 * 4 + x8 + 5];
        let any = blks.iter().any(|&b| cd.nz[b] != 0);
        if any && (score8[b8] < 4 || total < 6) {
            for &b in &blks {
                cd.luma[b] = [0; 16];
                cd.nz[b] = 0;
            }
        } else if any {
            cd.cbp |= 1 << b8;
        }
    }
    *recon = *pred;
    for bi in 0..16 {
        if cd.nz[bi] == 0 {
            continue;
        }
        let mut c = cd.luma[bi];
        dequant4(&mut c, q, false);
        let res = idct4(&c);
        let (bx, by) = (bi & 3, bi >> 2);
        for y in 0..4 {
            for x in 0..4 {
                let i = (by * 4 + y) * 16 + bx * 4 + x;
                recon[i] = (pred[i] as i32 + res[y * 4 + x]).clamp(0, 255) as u8;
            }
        }
    }
}

fn luma_i16(src: &[u8; 256], pred: &[u8; 256], q: &Quant, cd: &mut Coded, recon: &mut [u8; 256]) {
    let mut dc = [0i32; 16];
    let mut any_ac = false;
    for by in 0..4 {
        for bx in 0..4 {
            let mut r = [0i32; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let i = (by * 4 + y) * 16 + bx * 4 + x;
                    r[y * 4 + x] = src[i] as i32 - pred[i] as i32;
                }
            }
            let mut c = fdct4(&r);
            let bi = by * 4 + bx;
            dc[bi] = c[0];
            let n = quant4(&mut c, q, true, true);
            c[0] = 0;
            clamp_levels(&mut c);
            cd.luma[bi] = c;
            cd.nz[bi] = n as u8;
            any_ac |= n > 0;
        }
    }
    let mut dcl = luma_dc_forward(&dc, q);
    clamp_levels(&mut dcl);
    cd.dc = dcl;
    if any_ac {
        cd.cbp |= 15;
    } else {
        cd.nz = [0; 16];
    }
    let dcy = luma_dc_inverse(&dcl, q.qp, q.v0);
    for bi in 0..16 {
        let mut c = cd.luma[bi];
        dequant4(&mut c, q, true);
        c[0] = dcy[bi];
        let res = idct4(&c);
        let (bx, by) = (bi & 3, bi >> 2);
        for y in 0..4 {
            for x in 0..4 {
                let i = (by * 4 + y) * 16 + bx * 4 + x;
                recon[i] = (pred[i] as i32 + res[y * 4 + x]).clamp(0, 255) as u8;
            }
        }
    }
}

fn chroma_resid(
    src: &[[u8; 64]; 2],
    pred: &[[u8; 64]; 2],
    qc: &Quant,
    intra: bool,
    cd: &mut Coded,
    recon: &mut [[u8; 64]; 2],
) {
    let mut any_dc = false;
    let mut any_ac = false;
    for comp in 0..2 {
        let mut dc = [0i32; 4];
        for b in 0..4 {
            let (bx, by) = (b & 1, b >> 1);
            let mut r = [0i32; 16];
            for y in 0..4 {
                for x in 0..4 {
                    let i = (by * 4 + y) * 8 + bx * 4 + x;
                    r[y * 4 + x] = src[comp][i] as i32 - pred[comp][i] as i32;
                }
            }
            let mut c = fdct4(&r);
            dc[b] = c[0];
            let n = quant4(&mut c, qc, intra, true);
            c[0] = 0;
            clamp_levels(&mut c);
            cd.cac[comp][b] = c;
            cd.nzc[comp][b] = n as u8;
        }
        let l = chroma_dc_forward(&dc, qc, intra);
        let l = l.map(|v| v.clamp(-2047, 2047));
        cd.cdc[comp] = l;
        any_dc |= l.iter().any(|&v| v != 0);
    }
    // Inter: drop sparse chroma AC (x264-style decimation).
    if !intra {
        let mut score = 0;
        for comp in 0..2 {
            for b in 0..4 {
                if cd.nzc[comp][b] != 0 {
                    score += decimate_score(&cd.cac[comp][b]);
                }
            }
        }
        if score < 7 {
            cd.cac = [[[0; 16]; 4]; 2];
            cd.nzc = [[0; 4]; 2];
        }
    }
    for comp in 0..2 {
        any_ac |= cd.nzc[comp].iter().any(|&n| n != 0);
    }
    let cbpc = if any_ac {
        2
    } else if any_dc {
        1
    } else {
        0
    };
    cd.cbp |= cbpc << 4;
    if cbpc < 2 {
        cd.nzc = [[0; 4]; 2];
    }
    for comp in 0..2 {
        let dcs = chroma_dc_inverse(&cd.cdc[comp], qc.qp, qc.v0);
        for b in 0..4 {
            let (bx, by) = (b & 1, b >> 1);
            let mut c = cd.cac[comp][b];
            dequant4(&mut c, qc, true);
            c[0] = dcs[b];
            let res = if c.iter().all(|&v| v == 0) { [0; 16] } else { idct4(&c) };
            for y in 0..4 {
                for x in 0..4 {
                    let i = (by * 4 + y) * 8 + bx * 4 + x;
                    recon[comp][i] = (pred[comp][i] as i32 + res[y * 4 + x]).clamp(0, 255) as u8;
                }
            }
        }
    }
}

/// Encode one slice (rows `band.mby0..band.mby1`) into `w` as a complete RBSP.
pub(crate) fn encode_slice(c: &FrameCtx, band: Band, w: &mut BitWriter) -> FrameStats {
    w.clear();
    let (mby0, mby1) = (band.mby0, band.mby1);
    write_slice_header(
        w,
        &SliceHdr {
            first_mb: (mby0 * c.mbw) as u32,
            idr: c.idr,
            frame_num: c.hdr_frame_num,
            idr_id: c.idr_id,
            deblock: c.deblock,
        },
    );
    let mut s = SliceEnc {
        c,
        b: band,
        w,
        ys: c.refp.y.stride,
        yp: c.refp.y.pad,
        cs: c.refp.u.stride,
        cp: c.refp.u.pad,
        skip_run: 0,
        stats: FrameStats::default(),
    };
    let sy = &c.src.y;
    let su = &c.src.u;
    let sv = &c.src.v;
    let mut src_y = [0u8; 256];
    let mut src_c = [[0u8; 64]; 2];
    for mby in mby0..mby1 {
        for mbx in 0..c.mbw {
            let o = sy.off(mbx as isize * 16, mby as isize * 16);
            for y in 0..16 {
                src_y[y * 16..y * 16 + 16].copy_from_slice(&sy.data[o + y * sy.stride..o + y * sy.stride + 16]);
            }
            let o = su.off(mbx as isize * 8, mby as isize * 8);
            for y in 0..8 {
                src_c[0][y * 8..y * 8 + 8].copy_from_slice(&su.data[o + y * su.stride..o + y * su.stride + 8]);
                src_c[1][y * 8..y * 8 + 8].copy_from_slice(&sv.data[o + y * sv.stride..o + y * sv.stride + 8]);
            }
            s.encode_mb(mbx, mby, &src_y, &src_c);
        }
    }
    if s.skip_run > 0 {
        let r = s.skip_run;
        s.w.ue(r);
    }
    s.w.trailing();
    s.stats
}

/// Encoder state between frames.
pub struct Encoder {
    cfg: EncoderConfig,
    mbw: usize,
    mbh: usize,
    q: Quant,
    qc: Quant,
    lambda: i32,
    level: u8,
    cur: Pic,
    prev: Pic,
    recon: Pic,
    refp: Pic,
    info: Vec<MbInfo>,
    changed: Vec<bool>,
    hint: Vec<(i16, i16)>,
    frame_in_gop: u64,
    idr_count: u32,
    out: Vec<u8>,
    writers: Vec<BitWriter>,
    hdr: BitWriter,
    last: FrameStats,
}

impl Encoder {
    pub fn new(cfg: EncoderConfig) -> Self {
        Self::with_idr_base(cfg, 0)
    }

    /// An encoder whose IDR pictures use ids starting at `idr_base` (so that
    /// independently encoded chunks can be concatenated).
    pub fn with_idr_base(cfg: EncoderConfig, idr_base: u32) -> Self {
        assert!(cfg.width >= 2 && cfg.height >= 2, "ak264: frame size too small");
        assert!(cfg.width % 2 == 0 && cfg.height % 2 == 0, "ak264: width and height must be even (4:2:0)");
        assert!(cfg.qp <= 51, "ak264: qp must be 0..=51");
        let mbw = (cfg.width as usize).div_ceil(16);
        let mbh = (cfg.height as usize).div_ceil(16);
        let q = Quant::new(cfg.qp);
        let qc = Quant::new(chroma_qp(cfg.qp));
        let lambda = (0.85f64 * 2f64.powf((cfg.qp as f64 - 12.0) / 6.0)).round().max(1.0) as i32;
        let level = level_idc(mbw, mbh, cfg.fps);
        let threads = cfg.threads.clamp(1, mbh);
        Encoder {
            mbw,
            mbh,
            q,
            qc,
            lambda,
            level,
            cur: Pic::new(mbw, mbh, 0),
            prev: Pic::new(mbw, mbh, 0),
            recon: Pic::new(mbw, mbh, PAD),
            refp: Pic::new(mbw, mbh, PAD),
            info: vec![MbInfo::default(); mbw * mbh],
            changed: vec![true; mbw * mbh],
            hint: vec![NO_HINT; mbw * mbh],
            frame_in_gop: 0,
            idr_count: idr_base,
            out: Vec::new(),
            writers: (0..threads).map(|_| BitWriter::new()).collect(),
            hdr: BitWriter::new(),
            last: FrameStats::default(),
            cfg,
        }
    }

    pub fn config(&self) -> &EncoderConfig {
        &self.cfg
    }

    /// Statistics of the last encoded frame.
    pub fn last_stats(&self) -> FrameStats {
        self.last
    }

    /// Macroblock grid size (columns, rows).
    pub fn mb_size(&self) -> (usize, usize) {
        (self.mbw, self.mbh)
    }

    /// Force the next frame to be an IDR.
    pub fn force_idr(&mut self) {
        self.frame_in_gop = 0;
    }

    /// The decoder's view of the last frame (cropped), for verification.
    pub fn reconstruction(&self) -> crate::frame::YuvBuffer {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let mut b = crate::frame::YuvBuffer::new(w as u32, h as u32);
        let p = &self.refp;
        for y in 0..h {
            let o = p.y.off(0, y as isize);
            b.y[y * w..y * w + w].copy_from_slice(&p.y.data[o..o + w]);
        }
        let (cw, ch) = (w / 2, h / 2);
        for y in 0..ch {
            let o = p.u.off(0, y as isize);
            b.u[y * cw..y * cw + cw].copy_from_slice(&p.u.data[o..o + cw]);
            b.v[y * cw..y * cw + cw].copy_from_slice(&p.v.data[o..o + cw]);
        }
        b
    }

    /// Encode one frame; returns its Annex-B bytes (SPS/PPS precede every IDR).
    pub fn encode(&mut self, f: &YuvFrame, hints: &FrameHints) -> &[u8] {
        assert_eq!((f.width, f.height), (self.cfg.width, self.cfg.height), "ak264: frame size mismatch");
        let (w, h) = (f.width as usize, f.height as usize);
        let (cw, ch) = (w / 2, h / 2);
        self.cur.y.load(f.y, f.y_stride, w, h);
        self.cur.u.load(f.u, f.c_stride, cw, ch);
        self.cur.v.load(f.v, f.c_stride, cw, ch);
        let idr = self.frame_in_gop == 0 || self.cfg.gop <= 1 || self.frame_in_gop >= self.cfg.gop as u64;
        if idr {
            self.frame_in_gop = 0;
        }
        self.out.clear();
        let mut stats = FrameStats { idr, ..Default::default() };
        let nmb = self.mbw * self.mbh;
        let mut any_changed = true;
        if !idr {
            match &hints.changed {
                Some(c) => {
                    assert_eq!(c.len(), nmb, "ak264: changed map must have one entry per macroblock");
                    self.changed.copy_from_slice(c);
                }
                None => crate::diff::changed_map(&self.cur, &self.prev, self.mbw, self.mbh, &mut self.changed),
            }
            any_changed = self.changed.iter().any(|&c| c);
            self.hint.fill(NO_HINT);
            for m in &hints.motions {
                let (x0, y0) = (m.x as usize / 16, m.y as usize / 16);
                let x1 = ((m.x + m.w) as usize).div_ceil(16).min(self.mbw);
                let y1 = ((m.y + m.h) as usize).div_ceil(16).min(self.mbh);
                for my in y0..y1 {
                    for mx in x0..x1 {
                        self.hint[my * self.mbw + mx] = (-m.dx as i16, -m.dy as i16);
                    }
                }
            }
        }
        if idr {
            self.hdr.clear();
            write_sps(
                &mut self.hdr,
                &SeqInfo {
                    mbw: self.mbw,
                    mbh: self.mbh,
                    width: self.cfg.width,
                    height: self.cfg.height,
                    fps: self.cfg.fps,
                    qp: self.cfg.qp,
                    full_range: self.cfg.full_range,
                },
                self.level,
            );
            write_nal(&mut self.out, 3, 7, &self.hdr.buf);
            self.hdr.clear();
            write_pps(&mut self.hdr, self.cfg.qp);
            write_nal(&mut self.out, 3, 8, &self.hdr.buf);
        }
        let frame_num = (self.frame_in_gop % (1 << LOG2_MAX_FRAME_NUM)) as u32;
        if !any_changed {
            // All-skip P slice; the reference is unchanged.
            let w = &mut self.writers[0];
            w.clear();
            write_slice_header(
                w,
                &SliceHdr { first_mb: 0, idr: false, frame_num, idr_id: 0, deblock: self.cfg.deblock },
            );
            w.ue(nmb as u32);
            w.trailing();
            write_nal(&mut self.out, 2, 1, &w.buf);
            stats.skip = nmb as u32;
        } else {
            self.encode_slices(idr, frame_num, &mut stats);
            if self.cfg.deblock {
                let rows = self.slice_rows();
                crate::deblock::deblock(&mut self.recon, &self.info, self.mbw, self.mbh, &self.q, rows);
            }
            self.recon.extend();
            std::mem::swap(&mut self.recon, &mut self.refp);
        }
        if idr {
            self.idr_count = self.idr_count.wrapping_add(1);
        }
        std::mem::swap(&mut self.cur, &mut self.prev);
        self.frame_in_gop += 1;
        stats.bytes = self.out.len();
        self.last = stats;
        &self.out
    }

    fn slice_rows(&self) -> Vec<(usize, usize)> {
        let n = self.writers.len();
        (0..n).map(|i| (i * self.mbh / n, (i + 1) * self.mbh / n)).filter(|r| r.1 > r.0).collect()
    }

    fn encode_slices(&mut self, idr: bool, frame_num: u32, stats: &mut FrameStats) {
        let ctx = FrameCtx {
            mbw: self.mbw,
            q: self.q,
            qc: self.qc,
            lambda: self.lambda,
            src: &self.cur,
            refp: &self.refp,
            changed: &self.changed,
            hint: &self.hint,
            idr,
            max_mvy: max_mv_y(self.level),
            hdr_frame_num: frame_num,
            idr_id: self.idr_count & 0xffff,
            deblock: self.cfg.deblock,
        };
        let rows = self.slice_rows();
        // Split recon planes and MB info into bands.
        let mut bands = Vec::with_capacity(rows.len());
        {
            let r = &mut self.recon;
            let (ys, yp) = (r.y.stride, r.y.pad);
            let (cs, cp) = (r.u.stride, r.u.pad);
            let mut yrest: &mut [u8] = &mut r.y.data[yp * ys..];
            let mut urest: &mut [u8] = &mut r.u.data[cp * cs..];
            let mut vrest: &mut [u8] = &mut r.v.data[cp * cs..];
            let mut irest: &mut [MbInfo] = &mut self.info;
            for &(a, b) in &rows {
                let n = b - a;
                let (y, yr) = std::mem::take(&mut yrest).split_at_mut(n * 16 * ys);
                let (u, ur) = std::mem::take(&mut urest).split_at_mut(n * 8 * cs);
                let (v, vr) = std::mem::take(&mut vrest).split_at_mut(n * 8 * cs);
                let (inf, ir) = std::mem::take(&mut irest).split_at_mut(n * self.mbw);
                yrest = yr;
                urest = ur;
                vrest = vr;
                irest = ir;
                bands.push(Band { y, u, v, info: inf, mby0: a, mby1: b });
            }
        }
        let writers = &mut self.writers[..rows.len()];
        let results: Vec<FrameStats> = if bands.len() == 1 {
            let b = bands.pop().unwrap();
            vec![encode_slice(&ctx, b, &mut writers[0])]
        } else {
            use rayon::prelude::*;
            bands.into_par_iter().zip(writers.par_iter_mut()).map(|(b, w)| encode_slice(&ctx, b, w)).collect()
        };
        let (ref_idc, nal_type) = if idr { (3, 5) } else { (2, 1) };
        for (i, r) in results.iter().enumerate() {
            write_nal(&mut self.out, ref_idc, nal_type, &self.writers[i].buf);
            stats.skip += r.skip;
            stats.inter += r.inter;
            stats.intra16 += r.intra16;
            stats.intra4 += r.intra4;
            stats.changed += r.changed;
        }
    }
}
