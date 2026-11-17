//! The encoder: frame/slice/macroblock coding.

use crate::bits::{se_len, write_nal, BitWriter};
use crate::cavlc::write_block;
use crate::dsp;
use crate::frame::{Pic, YuvFrame};
use crate::headers::*;
use crate::intra::{self, Edges, Edges4};
use crate::soa::{self, QuantS};
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
    /// Quarter-pel motion refinement (smaller files on sub-pixel motion, slower).
    pub subpel: bool,
    /// CABAC entropy coding (Main profile; smaller files) instead of CAVLC
    /// (Constrained Baseline; faster).
    pub cabac: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        EncoderConfig { width: 0, height: 0, fps: 30.0, qp: 22, gop: 250, deblock: false, threads: 1, full_range: false, subpel: false, cabac: true }
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
    /// Per-macroblock "may have changed since the previous frame" flags,
    /// row-major, `ceil(w/16) * ceil(h/16)` entries (e.g. display-list bounding
    /// boxes). Flagged macroblocks are compared exactly; unflagged ones are
    /// trusted to be unchanged and coded as copies without being read.
    /// `None`: every macroblock is compared.
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
    /// Bits spent on intra / inter macroblocks (diagnostics).
    pub bits_intra: u64,
    pub bits_inter: u64,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct MbInfo {
    /// 0 = inter (incl. skip), 1 = I16, 2 = I4x4
    pub kind: u8,
    pub skip: bool,
    /// Went through mode decision (not a copy of the reference).
    pub coded: bool,
    pub mv: (i16, i16),
    /// Total coefficients per 4×4 luma block (raster), for nC and deblocking.
    pub nz: [u8; 16],
    pub nzc: [[u8; 4]; 2],
    /// I4x4 modes (raster), 2 when not I4x4.
    pub m4: [u8; 16],
    pub cbp: u8,
    /// intra_chroma_pred_mode (0 for inter).
    pub cmode: u8,
    /// Motion vector difference (quarter pels; 0 for skip / intra).
    pub mvd: (i16, i16),
    /// coded_block_flag of luma DC (bit 0), Cb DC (bit 1), Cr DC (bit 2).
    pub dcf: u8,
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
    luma: [[i16; 16]; 16],
    dc: [i32; 16],
    cdc: [[i32; 4]; 2],
    cac: [[[i16; 16]; 4]; 2],
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
    pub qs: QuantS,
    pub qcs: QuantS,
    /// SAD bounds (luma 16×16, chroma 8×8) under which inter residuals are all zero.
    /// (luma, chroma, chroma DC limit on the 8x8 SAD), inter.
    pub zero_sad: (u32, u32, u32),
    pub zero_sad_intra: (u32, u32),
    /// Intra per-4×4 AC bounds (luma, chroma) and DC limits (luma I16 Hadamard on
    /// SAD/2, chroma 8×8 SAD).
    pub zero_sad4_intra: (u32, u32),
    pub zero_dc_intra: (u32, u32),
    /// Per-4×4-block SAD bounds (luma, chroma AC), inter.
    pub zero_sad4: (u32, u32),
    pub lambda: i32,
    pub src: &'a Pic,
    pub refp: &'a Pic,
    pub changed: &'a [bool],
    pub stale: &'a [bool],
    pub hint: &'a [(i16, i16)],
    pub idr: bool,
    pub max_mvy: i32,
    pub subpel: bool,
    /// Frame-level half-sample planes of the reference (when many macroblocks change).
    pub hpel: Option<&'a crate::mc::HpelPlanes>,
    pub cabac: bool,
    pub hdr_frame_num: u32,
    pub idr_id: u32,
    pub deblock: bool,
    /// Deblock inside each slice (multi-slice frames, idc 2).
    pub slice_deblock: bool,
}

pub(crate) const NO_HINT: (i16, i16) = (i16::MIN, i16::MIN);

const SKIP_INFO: MbInfo = MbInfo {
    kind: 0,
    skip: true,
    coded: false,
    mv: (0, 0),
    nz: [0; 16],
    nzc: [[0; 4]; 2],
    m4: [2; 16],
    cbp: 0,
    cmode: 0,
    mvd: (0, 0),
    dcf: 0,
};

/// Mutable band of the reconstructed picture owned by one slice.
pub(crate) struct Band<'a> {
    pub y: &'a mut [u8],
    pub u: &'a mut [u8],
    pub v: &'a mut [u8],
    pub info: &'a mut [MbInfo],
    /// Macroblocks changed by in-slice deblocking.
    pub touched: &'a mut [bool],
    pub mby0: usize,
    pub mby1: usize,
}

/// Result of motion search (quarter-pel units).
struct Inter {
    mv: (i32, i32),
    /// SATD + lambda * MV bits.
    cost: u32,
    mvp: (i32, i32),
    skip_mv: (i32, i32),
    pred: [u8; 256],
    /// Luma SAD at the integer-pel result.
    sad: u32,
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
    cabac: Option<crate::cabac::Cabac>,
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

    /// (mvp for P16x16, P_Skip mv) in quarter pels.
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

    /// Reference block at an integer-pel displacement.
    fn luma_ref_int<'r>(&self, mbx: usize, mby: usize, mv: (i32, i32)) -> (&'r [u8], usize)
    where
        'a: 'r,
    {
        let p = &self.c.refp.y;
        let o = p.off(mbx as isize * 16 + mv.0 as isize, mby as isize * 16 + mv.1 as isize);
        (&p.data[o..], p.stride)
    }

    fn mc_chroma(&self, comp: usize, mbx: usize, mby: usize, mv: (i32, i32), out: &mut [u8; 64]) {
        let p = if comp == 0 { &self.c.refp.u } else { &self.c.refp.v };
        let (mx, my) = mv; // quarter luma = 1/8 chroma units
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
        let (w00, w01, w10, w11) = (w00 as u16, w01 as u16, w10 as u16, w11 as u16);
        for y in 0..8 {
            let r0: &[u8; 9] = d[o + y * s..o + y * s + 9].try_into().unwrap();
            let r1: &[u8; 9] = d[o + (y + 1) * s..o + (y + 1) * s + 9].try_into().unwrap();
            let row = &mut out[y * 8..y * 8 + 8];
            for x in 0..8 {
                let v = w00 * r0[x] as u16 + w01 * r0[x + 1] as u16 + w10 * r1[x] as u16 + w11 * r1[x + 1] as u16;
                row[x] = ((v + 32) >> 6) as u8;
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

        let mut inter: Option<Inter> = None;
        let (mut mvp, mut skip_mv) = ((0, 0), (0, 0));
        if !c.idr {
            (mvp, skip_mv) = self.mv_pred(mbx, mby);
            if !c.changed[mbi] {
                // Unchanged: copy of the reference.
                self.code_copy(mbx, mby, mvp, skip_mv);
                return;
            }
        }
        self.stats.changed += 1;

        // Early inter: the predicted candidates already match (typical for layers
        // moving coherently) → no intra analysis, no search.
        if !c.idr {
            let q = self.motion_search(mbx, mby, src_y, mvp, skip_mv, mbi, true, None);
            if q.sad <= lambda as u32 * 24 {
                self.code_inter(mbx, mby, src_y, src_c, &q);
                return;
            }
            inter = Some(q);
        }

        // Intra 16x16 decision.
        let e16 = self.edges16(mbx, mby);
        let mut pred = [0u8; 256];
        let mut pred16 = [0u8; 256];
        let mut best16 = (u32::MAX, 2u8);
        // SAD pre-selection, SATD for the winner (P frames); full SATD for IDRs.
        for m in 0..4u8 {
            if !Self::intra16_modes_ok(&e16, m) {
                continue;
            }
            intra::pred16(&e16, m, &mut pred);
            let s = if c.idr { dsp::satd(src_y, &pred, 16, 16) } else { crate::simd::sad16(src_y, 16, &pred, 16) };
            if s < best16.0 {
                best16 = (s, m);
                pred16 = pred;
            }
        }
        if !c.idr {
            best16.0 = dsp::satd(src_y, &pred16, 16, 16);
        }
        let cost16 = best16.0 + lambda as u32 * if c.idr { 4 } else { 8 };
        // Full motion search unless intra prediction is already near-perfect.
        // (Skipped too when the predicted candidates are far worse than intra.)
        let hopeless = inter.as_ref().is_some_and(|q| q.sad > 4 * best16.0);
        if !c.idr && best16.0 > lambda as u32 * 4 && !hopeless {
            let full = self.motion_search(mbx, mby, src_y, mvp, skip_mv, mbi, false, inter.as_ref());
            if full.cost < inter.as_ref().map_or(u32::MAX, |q| q.cost) {
                inter = Some(full);
            }
        }

        let mut kind = Kind::I16(best16.1);
        let mut best_cost = cost16;
        // Sub-pel refinement only when inter is competitive and imperfect.
        if let Some(it) = inter.as_mut() {
            if c.subpel && it.cost < cost16 + cost16 / 2 && it.cost > lambda as u32 * 8 {
                self.refine_subpel(mbx, mby, src_y, it);
            }
        }
        if let Some(it) = &inter {
            if it.cost <= best_cost {
                best_cost = it.cost;
                kind = Kind::P16;
            }
        }

        let try_i4 = best_cost > lambda as u32 * 24 + 256 && (c.idr || kind != Kind::P16 || best_cost > lambda as u32 * 64 + 1024);
        if try_i4 {
            // Encode I4x4 tentatively (writes recon).
            let mut cd = Coded::new();
            let (cost4, modes) = self.code_i4(mbx, mby, src_y, &mut cd, true);
            if cost4 < best_cost {
                self.finish_intra(mbx, mby, src_c, &mut cd, Kind::I4, Some(modes), None);
                return;
            }
        }
        match kind {
            Kind::P16 => {
                let it = inter.as_ref().unwrap();
                self.code_inter(mbx, mby, src_y, src_c, it);
            }
            Kind::I16(_) | Kind::I4 => {
                let mut cd = Coded::new();
                self.finish_intra(mbx, mby, src_c, &mut cd, kind, None, Some((src_y, &pred16)));
            }
        }
    }

    /// Motion search: integer candidates (zero, skip MV, predictor, hint,
    /// neighbours), a small integer diamond, then half- and quarter-pel refinement.
    fn motion_search(
        &self,
        mbx: usize,
        mby: usize,
        src_y: &[u8; 256],
        mvp: (i32, i32),
        skip_mv: (i32, i32),
        mbi: usize,
        quick: bool,
        seed: Option<&Inter>,
    ) -> Inter {
        let c = self.c;
        let lambda = c.lambda as u32;
        let w16 = (c.mbw * 16) as i32;
        let h16 = (c.refp.y.h) as i32;
        let (bx, by) = (mbx as i32 * 16, mby as i32 * 16);
        // integer-pel search window
        let xr = (-16 - bx, w16 - bx);
        let yr = ((-16 - by).max(-c.max_mvy), (h16 - by).min(c.max_mvy - 1));
        let clamp = |mv: (i32, i32)| (mv.0.clamp(xr.0, xr.1), mv.1.clamp(yr.0, yr.1));
        let mvcost = |q: (i32, i32)| -> u32 { lambda * (se_len(q.0 - mvp.0) + se_len(q.1 - mvp.1)) };
        let eval = |mv: (i32, i32)| -> u32 {
            let (r, rs) = self.luma_ref_int(mbx, mby, mv);
            dsp::sad16(src_y, 16, r, rs) + mvcost((mv.0 * 4, mv.1 * 4))
        };
        let round = |q: (i32, i32)| ((q.0 + 2) >> 2, (q.1 + 2) >> 2);
        let mut best = (u32::MAX, (0, 0));
        let mut cands = [(0i32, 0i32); 7];
        let mut n = 0;
        fn push_c(cands: &mut [(i32, i32); 7], n: &mut usize, mv: (i32, i32)) {
            if !cands[..*n].contains(&mv) {
                cands[*n] = mv;
                *n += 1;
            }
        }
        macro_rules! push {
            ($mv:expr) => {
                push_c(&mut cands, &mut n, clamp($mv))
            };
        }
        push!((0, 0));
        push!(round(skip_mv));
        push!(round(mvp));
        let h = c.hint[mbi];
        if h != NO_HINT {
            push!(round((h.0 as i32, h.1 as i32)));
        }
        let (x, y) = (mbx as isize, mby as isize);
        // A quick search already evaluated the candidates above.
        let known = if let Some(q) = seed {
            let mv = (q.mv.0 >> 2, q.mv.1 >> 2);
            best = (q.sad + mvcost((mv.0 * 4, mv.1 * 4)), mv);
            n
        } else {
            0
        };
        if !quick {
            for nb in [self.nb(x - 1, y), self.nb(x, y - 1), self.nb(x + 1, y - 1)] {
                if nb.inter {
                    push!(round(nb.mv));
                }
            }
        }
        for &mv in &cands[known..n] {
            let s = eval(mv);
            if s < best.0 {
                best = (s, mv);
            }
        }
        // small diamond refinement
        if !quick && best.0 > mvcost((best.1 .0 * 4, best.1 .1 * 4)) {
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
        let base = best.1;
        let base_q = (base.0 * 4, base.1 * 4);
        let mut pred = [0u8; 256];
        {
            let (r, rs) = self.luma_ref_int(mbx, mby, base);
            for y in 0..16 {
                pred[y * 16..y * 16 + 16].copy_from_slice(&r[y * rs..y * rs + 16]);
            }
        }
        let mv = base_q;
        let sad = best.0 - mvcost(base_q);
        // SATD only matters when the result competes with intra.
        let cost = if quick && sad <= lambda * 24 { best.0 } else { dsp::satd(src_y, &pred, 16, 16) + mvcost(base_q) };
        Inter { mv, cost, mvp, skip_mv, pred, sad }
    }

    /// Half- then quarter-pel refinement around the integer result.
    fn refine_subpel(&self, mbx: usize, mby: usize, src_y: &[u8; 256], it: &mut Inter) {
        if it.sad == 0 || it.mv.0 & 3 != 0 || it.mv.1 & 3 != 0 {
            return;
        }
        let lambda = self.c.lambda as u32;
        let mvp = it.mvp;
        let mvcost = |q: (i32, i32)| -> u32 { lambda * (se_len(q.0 - mvp.0) + se_len(q.1 - mvp.1)) };
        let base_q = it.mv;
        let (bx, by) = ((mbx as i32 * 16 + (base_q.0 >> 2)) as isize, (mby as i32 * 16 + (base_q.1 >> 2)) as isize);
        let win = if self.c.hpel.is_none() { Some(crate::mc::Hpel::new(&self.c.refp.y, bx, by)) } else { None };
        let mut p = [0u8; 256];
        let mut d = (0, 0);
        // SAD for the candidates (cheap), SATD for the final cost.
        let mut best = it.sad + mvcost(base_q);
        let start = it.mv;
        for step in [2, 1] {
            let centre = d;
            for (sx, sy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let cand = (centre.0 + sx * step, centre.1 + sy * step);
                match (&win, self.c.hpel) {
                    (Some(w), _) => w.pred(cand.0, cand.1, &mut p),
                    (None, Some(hp)) => hp.pred(
                        &self.c.refp.y,
                        bx + (cand.0 >> 2) as isize,
                        by + (cand.1 >> 2) as isize,
                        cand.0 & 3,
                        cand.1 & 3,
                        &mut p,
                    ),
                    _ => unreachable!(),
                }
                let q = (base_q.0 + cand.0, base_q.1 + cand.1);
                let cst = crate::simd::sad16(src_y, 16, &p, 16) + mvcost(q);
                if cst < best {
                    best = cst;
                    it.mv = q;
                    d = cand;
                    it.pred = p;
                }
            }
        }
        if it.mv != start {
            let c = dsp::satd(src_y, &it.pred, 16, 16) + mvcost(it.mv);
            it.cost = c;
            it.sad = best - mvcost(it.mv);
        }
    }

    /// Unchanged macroblock: copy the co-located reference (MV 0, no residual).
    fn code_copy(&mut self, mbx: usize, mby: usize, mvp: (i32, i32), skip_mv: (i32, i32)) {
        // recon = ref at mv 0 (already equal unless stale)
        if self.c.stale[mby * self.c.mbw + mbx] {
            self.copy_pred(mbx, mby, (0, 0));
        }
        let mut skip = skip_mv == (0, 0);
        if !skip && self.pred_equals_zero(mbx, mby, skip_mv) {
            skip = true;
        }
        let mv = if skip { skip_mv } else { (0, 0) };
        let mvd = if skip { (0, 0) } else { (-mvp.0, -mvp.1) };
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo {
            kind: 0,
            skip,
            coded: false,
            mv: (mv.0 as i16, mv.1 as i16),
            mvd: (mvd.0 as i16, mvd.1 as i16),
            m4: [2; 16],
            ..Default::default()
        };
        if skip {
            self.emit_skip(mbx, mby);
            self.stats.skip += 1;
        } else {
            self.emit_p16(mbx, mby, mvd, &Coded::new());
            self.stats.inter += 1;
        }
        self.emit_end(mbx, mby);
    }

    /// Does motion compensation at `mv` reproduce the co-located reference exactly?
    fn pred_equals_zero(&self, mbx: usize, mby: usize, mv: (i32, i32)) -> bool {
        let c = self.c;
        let w16 = (c.mbw * 16) as i32 * 4;
        let h16 = c.refp.y.h as i32 * 4;
        let (bx, by) = (mbx as i32 * 64 + mv.0, mby as i32 * 64 + mv.1);
        if bx < -64 || bx > w16 || by < -64 || by > h16 || mv.1.abs() >= c.max_mvy * 4 {
            return false;
        }
        let mut a = [0u8; 256];
        crate::mc::luma_pred(&c.refp.y, mbx as isize * 16, mby as isize * 16, mv, &mut a);
        let (z, sz) = self.luma_ref_int(mbx, mby, (0, 0));
        if !dsp::block_eq(&a, 16, z, sz, 16, 16) {
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
        let (r, rs) = self.luma_ref_int(mbx, mby, (mv.0 >> 2, mv.1 >> 2));
        debug_assert!(mv.0 & 3 == 0 && mv.1 & 3 == 0);
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

    fn code_inter(&mut self, mbx: usize, mby: usize, src_y: &[u8; 256], src_c: &[[u8; 64]; 2], it: &Inter) {
        let (mv, mvp, skip_mv) = (it.mv, it.mvp, it.skip_mv);
        let pred = &it.pred;
        let mut predc = [[0u8; 64]; 2];
        self.mc_chroma(0, mbx, mby, mv, &mut predc[0]);
        self.mc_chroma(1, mbx, mby, mv, &mut predc[1]);
        let mut cd = Coded::new();

        // Residuals that provably quantise to zero are not transformed.
        let mut recon = [0u8; 256];
        let sad = crate::simd::sad16(src_y, 16, pred, 16);
        let ry = if sad > self.c.zero_sad.0 && crate::simd::max_sad4(src_y, pred, 16, 16) > self.c.zero_sad4.0 {
            luma_inter(src_y, pred, &self.c.qs, &mut cd, &mut recon);
            &recon
        } else {
            pred
        };
        let mut reconc = [[0u8; 64]; 2];
        let zc = self.c.zero_sad.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &predc[0]), crate::simd::sad64(&src_c[1], &predc[1]));
        // chroma: AC by block, DC (2x2 Hadamard of block sums) by the 8x8 SAD
        let dc_ok = s0 <= self.c.zero_sad.2 && s1 <= self.c.zero_sad.2;
        let rc = if (s0 > zc || s1 > zc)
            && !(dc_ok
                && crate::simd::max_sad4(&src_c[0], &predc[0], 8, 8) <= self.c.zero_sad4.1
                && crate::simd::max_sad4(&src_c[1], &predc[1], 8, 8) <= self.c.zero_sad4.1)
        {
            chroma_resid(src_c, &predc, &self.c.qc, &self.c.qcs, false, &mut cd, &mut reconc);
            &reconc
        } else {
            &predc
        };
        self.store_recon(mbx, mby, ry, rc);
        let skip = cd.cbp == 0 && mv == skip_mv;
        let mvd = if skip { (0, 0) } else { (mv.0 - mvp.0, mv.1 - mvp.1) };
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo {
            kind: 0,
            skip,
            coded: true,
            mv: (mv.0 as i16, mv.1 as i16),
            nz: cd.nz,
            nzc: cd.nzc,
            m4: [2; 16],
            cbp: cd.cbp,
            cmode: 0,
            mvd: (mvd.0 as i16, mvd.1 as i16),
            dcf: dc_flags(&cd, false),
        };
        if skip {
            self.emit_skip(mbx, mby);
            self.stats.skip += 1;
        } else {
            self.stats.inter += 1;
            self.emit_p16(mbx, mby, mvd, &cd);
        }
        self.emit_end(mbx, mby);
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
                let s = (crate::simd::satd4x4_sum(&s4, &pred) >> 1) + lambda * if m == pm { 1 } else { 4 };
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
            cd.luma[bi] = co.map(|v| v as i16);
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
        i16: Option<(&[u8; 256], &[u8; 256])>,
    ) {
        let q = self.c.q;
        if let (Kind::I16(_), Some((src, pred))) = (kind, i16) {
            let mut recon = *pred;
            let sad = crate::simd::sad16(src, 16, pred, 16);
            let (b4, dcl) = (self.c.zero_sad4_intra.0, self.c.zero_dc_intra.0);
            if sad > self.c.zero_sad_intra.0 && !(sad < 2 * dcl && crate::simd::max_sad4(src, pred, 16, 16) <= b4) {
                luma_i16(src, pred, &q, &self.c.qs, cd, &mut recon);
            }
            let base = self.yi(mbx * 16, mby * 16);
            for y in 0..16 {
                self.b.y[base + y * self.ys..base + y * self.ys + 16].copy_from_slice(&recon[y * 16..y * 16 + 16]);
            }
        }
        // chroma mode decision
        let ec = [self.edges8(0, mbx, mby), self.edges8(1, mbx, mby)];
        let mut best = (u32::MAX, 0u8);
        let mut predc = [[0u8; 64]; 2];
        // Chroma modes tried: DC plus the one matching the luma I16 direction (all for I4x4).
        let cands: &[u8] = match kind {
            Kind::I16(0) => &[0, 2],
            Kind::I16(1) => &[0, 1],
            Kind::I16(3) => &[0, 3],
            Kind::I16(_) => &[0],
            _ => &[0, 1, 2, 3],
        };
        for &m in cands {
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
        let zc = self.c.zero_sad_intra.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &predc[0]), crate::simd::sad64(&src_c[1], &predc[1]));
        let (b4, dcl) = (self.c.zero_sad4_intra.1, self.c.zero_dc_intra.1);
        let zero_c = s0 <= dcl
            && s1 <= dcl
            && crate::simd::max_sad4(&src_c[0], &predc[0], 8, 8) <= b4
            && crate::simd::max_sad4(&src_c[1], &predc[1], 8, 8) <= b4;
        if (s0 > zc || s1 > zc) && !zero_c {
            chroma_resid(src_c, &predc, &self.c.qc, &self.c.qcs, true, cd, &mut reconc);
        } else {
            reconc = predc;
        }
        let base = self.ci(mbx * 8, mby * 8);
        let cs = self.cs;
        for y in 0..8 {
            self.b.u[base + y * cs..base + y * cs + 8].copy_from_slice(&reconc[0][y * 8..y * 8 + 8]);
            self.b.v[base + y * cs..base + y * cs + 8].copy_from_slice(&reconc[1][y * 8..y * 8 + 8]);
        }

        // ---- syntax ----
        let modes4 = modes.unwrap_or([2; 16]);
        let is16 = matches!(kind, Kind::I16(_));
        // I4x4 mode signalling (prev flag, rem) in block order
        let mut m4sig = [(true, 0u8); 16];
        if !is16 {
            let mut cur = [2u8; 16];
            for k in 0..16 {
                let (bx, by) = (BLK_X[k], BLK_Y[k]);
                let pm = self.pred_mode4(mbx, mby, bx, by, &cur);
                let m = modes4[by * 4 + bx];
                cur[by * 4 + bx] = m;
                m4sig[k] = if m == pm { (true, 0) } else { (false, if m < pm { m } else { m - 1 }) };
            }
        }
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo {
            kind: if is16 { 1 } else { 2 },
            skip: false,
            coded: true,
            mv: (0, 0),
            nz: cd.nz,
            nzc: cd.nzc,
            m4: modes4,
            cbp: cd.cbp,
            cmode,
            mvd: (0, 0),
            dcf: dc_flags(cd, is16),
        };
        if is16 {
            self.stats.intra16 += 1;
        } else {
            self.stats.intra4 += 1;
        }
        self.emit_intra(mbx, mby, kind, &m4sig, cmode, cd);
        self.emit_end(mbx, mby);
    }

    #[inline(always)]
    fn skip_ctx_inc(&self, mbx: usize, mby: usize) -> usize {
        let a = mbx > 0 && !self.info(mbx - 1, mby).skip;
        let b = mby > self.b.mby0 && !self.info(mbx, mby - 1).skip;
        a as usize + b as usize
    }

    fn nbrs(&self, mbx: usize, mby: usize) -> (Option<MbInfo>, Option<MbInfo>) {
        let a = if mbx > 0 { Some(*self.info(mbx - 1, mby)) } else { None };
        let b = if mby > self.b.mby0 { Some(*self.info(mbx, mby - 1)) } else { None };
        (a, b)
    }

    fn emit_skip(&mut self, mbx: usize, mby: usize) {
        if self.cabac.is_some() {
            let inc = self.skip_ctx_inc(mbx, mby);
            if let Some(cb) = self.cabac.as_mut() {
                cb.decision(self.w, 11 + inc, 1);
            }
        } else {
            self.skip_run += 1;
        }
    }

    /// end_of_slice_flag (CABAC).
    fn emit_end(&mut self, mbx: usize, mby: usize) {
        if let Some(cb) = self.cabac.as_mut() {
            let last = mbx + 1 == self.c.mbw && mby + 1 == self.b.mby1;
            cb.terminate(self.w, last as u32);
        }
    }

    fn emit_p16(&mut self, mbx: usize, mby: usize, mvd: (i32, i32), cd: &Coded) {
        if self.cabac.is_none() {
            self.w.ue(self.skip_run);
            self.skip_run = 0;
            self.w.ue(0); // P_L0_16x16
            self.w.se(mvd.0);
            self.w.se(mvd.1);
            self.w.ue(CBP_INTER[cd.cbp as usize] as u32);
            if cd.cbp != 0 {
                self.w.se(0);
                self.write_residual(mbx, mby, cd, false);
            }
            return;
        }
        let (a, b) = self.nbrs(mbx, mby);
        let cb = self.cabac.as_mut().unwrap();
        let w = &mut *self.w;
        let inc = a.map_or(0, |m| !m.skip as usize) + b.map_or(0, |m| !m.skip as usize);
        cb.decision(w, 11 + inc, 0);
        cb.decision(w, 14, 0);
        cb.decision(w, 15, 0);
        cb.decision(w, 16, 0);
        let sx = a.map_or(0, |m| m.mvd.0.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd.0.unsigned_abs() as u32);
        let sy = a.map_or(0, |m| m.mvd.1.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd.1.unsigned_abs() as u32);
        cb.mvd(w, 40, mvd.0, sx);
        cb.mvd(w, 47, mvd.1, sy);
        cabac_cbp(cb, w, &a, &b, cd.cbp);
        if cd.cbp != 0 {
            cb.decision(w, 60, 0); // mb_qp_delta = 0
            cabac_residual(cb, w, &a, &b, cd, false, false);
        }
    }

    fn emit_intra(&mut self, mbx: usize, mby: usize, kind: Kind, m4sig: &[(bool, u8); 16], cmode: u8, cd: &Coded) {
        let p_slice = !self.c.idr;
        if self.cabac.is_none() {
            if p_slice {
                self.w.ue(self.skip_run);
                self.skip_run = 0;
            }
            let off = if p_slice { 5 } else { 0 };
            match kind {
                Kind::I16(m) => {
                    let cbpl = if cd.cbp & 15 != 0 { 1 } else { 0 };
                    self.w.ue(off + 1 + m as u32 + 4 * (cd.cbp >> 4) as u32 + 12 * cbpl);
                    self.w.ue(cmode as u32);
                    self.w.se(0);
                    self.write_residual(mbx, mby, cd, true);
                }
                _ => {
                    self.w.ue(off);
                    for &(f, r) in m4sig {
                        self.w.bit(f);
                        if !f {
                            self.w.put(3, r as u32);
                        }
                    }
                    self.w.ue(cmode as u32);
                    self.w.ue(CBP_INTRA[cd.cbp as usize] as u32);
                    if cd.cbp != 0 {
                        self.w.se(0);
                        self.write_residual(mbx, mby, cd, false);
                    }
                }
            }
            return;
        }
        let (a, b) = self.nbrs(mbx, mby);
        let cb = self.cabac.as_mut().unwrap();
        let w = &mut *self.w;
        // mb_type
        let bin0_ctx = if p_slice {
            let inc = a.map_or(0, |m| !m.skip as usize) + b.map_or(0, |m| !m.skip as usize);
            cb.decision(w, 11 + inc, 0);
            cb.decision(w, 14, 1); // intra prefix
            17
        } else {
            3 + a.map_or(0, |m| (m.kind != 2) as usize) + b.map_or(0, |m| (m.kind != 2) as usize)
        };
        match kind {
            Kind::I16(m) => {
                cb.decision(w, bin0_ctx, 1);
                cb.terminate(w, 0);
                let cbpc = (cd.cbp >> 4) as u32;
                // suffix contexts: I slice 6,7,8|9,9|10 ; P slice 18,19,19|20,20|20
                let (c_l, c_c, c_c2, c_p1, c_p2, n_p1, n_p2) =
                    if p_slice { (18, 19, 19, 20, 20, 20, 20) } else { (6, 7, 8, 9, 10, 9, 10) };
                cb.decision(w, c_l, (cd.cbp & 15 != 0) as u32);
                cb.decision(w, c_c, (cbpc != 0) as u32);
                if cbpc != 0 {
                    cb.decision(w, c_c2, (cbpc == 2) as u32);
                    cb.decision(w, c_p1, (m >> 1) as u32);
                    cb.decision(w, c_p2, (m & 1) as u32);
                } else {
                    cb.decision(w, n_p1, (m >> 1) as u32);
                    cb.decision(w, n_p2, (m & 1) as u32);
                }
            }
            _ => {
                cb.decision(w, bin0_ctx, 0);
                for &(f, r) in m4sig {
                    cb.decision(w, 68, f as u32);
                    if !f {
                        cb.decision(w, 69, (r & 1) as u32);
                        cb.decision(w, 69, ((r >> 1) & 1) as u32);
                        cb.decision(w, 69, ((r >> 2) & 1) as u32);
                    }
                }
            }
        }
        // intra_chroma_pred_mode
        let inc = a.map_or(0, |m| (m.kind != 0 && m.cmode != 0) as usize)
            + b.map_or(0, |m| (m.kind != 0 && m.cmode != 0) as usize);
        cb.decision(w, 64 + inc, (cmode > 0) as u32);
        if cmode > 0 {
            cb.decision(w, 67, (cmode > 1) as u32);
            if cmode > 1 {
                cb.decision(w, 67, (cmode > 2) as u32);
            }
        }
        let is16 = matches!(kind, Kind::I16(_));
        if !is16 {
            cabac_cbp(cb, w, &a, &b, cd.cbp);
        }
        if is16 || cd.cbp != 0 {
            cb.decision(w, 60, 0);
            cabac_residual(cb, w, &a, &b, cd, is16, true);
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
                        scan[j] = blk[ZIGZAG[j + 1]] as i32;
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
                    scan[j] = blk[ZIGZAG[j]] as i32;
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
                        scan[j] = blk[ZIGZAG[j + 1]] as i32;
                    }
                    let nc = self.nc_chroma(mbx, mby, comp, b & 1, b >> 1, &cd.nzc[comp]);
                    write_block(self.w, &scan[..15], nc);
                }
            }
        }
    }
}

fn dc_flags(cd: &Coded, i16: bool) -> u8 {
    let mut f = 0;
    if i16 && cd.dc.iter().any(|&v| v != 0) {
        f |= 1;
    }
    if cd.cbp >> 4 != 0 {
        if cd.cdc[0].iter().any(|&v| v != 0) {
            f |= 2;
        }
        if cd.cdc[1].iter().any(|&v| v != 0) {
            f |= 4;
        }
    }
    f
}

fn cabac_cbp(cb: &mut crate::cabac::Cabac, w: &mut BitWriter, a: &Option<MbInfo>, b: &Option<MbInfo>, cbp: u8) {
    let bit = |v: u8, k: usize| (v >> k) & 1 != 0;
    // condTerm = 0 if unavailable or the neighbouring 8x8 has coefficients; else 1
    let ext = |m: &Option<MbInfo>, k: usize| m.map_or(0, |m| (!bit(m.cbp, k)) as usize);
    for b8 in 0..4 {
        let ca = if b8 & 1 == 1 { (!bit(cbp, b8 - 1)) as usize } else { ext(a, b8 + 1) };
        let cbb = if b8 >= 2 { (!bit(cbp, b8 - 2)) as usize } else { ext(b, b8 + 2) };
        cb.decision(w, 73 + ca + 2 * cbb, bit(cbp, b8) as u32);
    }
    let c = cbp >> 4;
    let ca = a.map_or(0, |m| (m.cbp >> 4 != 0) as usize);
    let cbb = b.map_or(0, |m| (m.cbp >> 4 != 0) as usize);
    cb.decision(w, 77 + ca + 2 * cbb, (c != 0) as u32);
    if c != 0 {
        let ca = a.map_or(0, |m| (m.cbp >> 4 == 2) as usize);
        let cbb = b.map_or(0, |m| (m.cbp >> 4 == 2) as usize);
        cb.decision(w, 81 + ca + 2 * cbb, (c == 2) as u32);
    }
}

fn cabac_residual(
    cb: &mut crate::cabac::Cabac,
    w: &mut BitWriter,
    a: &Option<MbInfo>,
    b: &Option<MbInfo>,
    cd: &Coded,
    i16: bool,
    intra: bool,
) {
    use crate::cabac::Cat;
    // condTermFlag of a neighbouring macroblock's block: unavailable → intra
    #[inline(always)]
    fn ext(m: &Option<MbInfo>, intra: bool, f: impl Fn(&MbInfo) -> bool) -> usize {
        match m {
            None => intra as usize,
            Some(m) => f(m) as usize,
        }
    }
    let mut scan = [0i32; 16];
    if i16 {
        for k in 0..16 {
            scan[k] = cd.dc[ZIGZAG[k]];
        }
        let inc = ext(a, intra, |m| m.dcf & 1 != 0) + 2 * ext(b, intra, |m| m.dcf & 1 != 0);
        cb.residual(w, &scan, Cat::LumaDc, inc);
    }
    let luma_inc = |bx: usize, by: usize| -> usize {
        let ia = if bx > 0 { (cd.nz[by * 4 + bx - 1] > 0) as usize } else { ext(a, intra, |m| m.nz[by * 4 + 3] > 0) };
        let ib = if by > 0 { (cd.nz[(by - 1) * 4 + bx] > 0) as usize } else { ext(b, intra, |m| m.nz[12 + bx] > 0) };
        ia + 2 * ib
    };
    for k in 0..16 {
        let (bx, by) = (BLK_X[k], BLK_Y[k]);
        let blk = &cd.luma[by * 4 + bx];
        if i16 {
            if cd.cbp & 15 == 0 {
                break;
            }
            for j in 0..15 {
                scan[j] = blk[ZIGZAG[j + 1]] as i32;
            }
            cb.residual(w, &scan[..15], Cat::LumaAc, luma_inc(bx, by));
        } else {
            if cd.cbp & (1 << (k / 4)) == 0 {
                continue;
            }
            for j in 0..16 {
                scan[j] = blk[ZIGZAG[j]] as i32;
            }
            cb.residual(w, &scan, Cat::Luma4x4, luma_inc(bx, by));
        }
    }
    let cbpc = cd.cbp >> 4;
    if cbpc != 0 {
        for comp in 0..2 {
            let bitn = 1 << (1 + comp);
            let inc = ext(a, intra, |m| m.dcf & bitn != 0) + 2 * ext(b, intra, |m| m.dcf & bitn != 0);
            cb.residual(w, &cd.cdc[comp], Cat::ChromaDc, inc);
        }
    }
    if cbpc == 2 {
        for comp in 0..2 {
            for blk in 0..4 {
                let (bx, by) = (blk & 1, blk >> 1);
                let ia =
                    if bx > 0 { (cd.nzc[comp][by * 2] > 0) as usize } else { ext(a, intra, |m| m.nzc[comp][by * 2 + 1] > 0) };
                let ib = if by > 0 { (cd.nzc[comp][bx] > 0) as usize } else { ext(b, intra, |m| m.nzc[comp][2 + bx] > 0) };
                let src = &cd.cac[comp][blk];
                for j in 0..15 {
                    scan[j] = src[ZIGZAG[j + 1]] as i32;
                }
                cb.residual(w, &scan[..15], Cat::ChromaAc, ia + 2 * ib);
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
fn decimate_score(c: &[i16; 16]) -> u32 {
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

/// Compile `$impl` twice (AVX2 and baseline) and dispatch at run time.
macro_rules! multiversion {
    ($name:ident => $imp:ident ( $($a:ident : $t:ty),* )) => {
        fn $name($($a: $t),*) {
            #[cfg(target_arch = "x86_64")]
            {
                if crate::simd::level() >= 2 {
                    #[target_feature(enable = "avx2")]
                    unsafe fn f($($a: $t),*) { $imp($($a),*) }
                    // SAFETY: AVX2 was detected at run time.
                    return unsafe { f($($a),*) };
                }
            }
            $imp($($a),*)
        }
    };
}

multiversion!(luma_inter => luma_inter_impl(src: &[u8; 256], pred: &[u8; 256], qs: &QuantS, cd: &mut Coded, recon: &mut [u8; 256]));
multiversion!(luma_i16 => luma_i16_impl(src: &[u8; 256], pred: &[u8; 256], q: &Quant, qs: &QuantS, cd: &mut Coded, recon: &mut [u8; 256]));
multiversion!(chroma_resid => chroma_resid_impl(src: &[[u8; 64]; 2], pred: &[[u8; 64]; 2], qc: &Quant, qcs: &QuantS, intra: bool, cd: &mut Coded, recon: &mut [[u8; 64]; 2]));

#[inline(always)]
fn luma_inter_impl(src: &[u8; 256], pred: &[u8; 256], qs: &QuantS, cd: &mut Coded, recon: &mut [u8; 256]) {
    let (mut c, ps) = soa::gather16(src, pred);
    soa::fdct(&mut c);
    let nz = soa::quant(&mut c, qs, false, 0);
    let mut score8 = [0u32; 4];
    for b in 0..16 {
        cd.nz[b] = nz[b];
        if nz[b] > 0 {
            for k in 0..16 {
                cd.luma[b][k] = c[k][b];
            }
            score8[(b >> 3) * 2 + ((b & 3) >> 1)] += decimate_score(&cd.luma[b]);
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
                for row in c.iter_mut() {
                    row[b] = 0;
                }
            }
        } else if any {
            cd.cbp |= 1 << b8;
        }
    }
    if cd.cbp & 15 == 0 {
        *recon = *pred;
        return;
    }
    let res = soa::dequant_idct(&c, qs, 0);
    soa::scatter16(&res, &ps, recon);
}

#[inline(always)]
fn luma_i16_impl(src: &[u8; 256], pred: &[u8; 256], q: &Quant, qs: &QuantS, cd: &mut Coded, recon: &mut [u8; 256]) {
    let (mut c, ps) = soa::gather16(src, pred);
    soa::fdct(&mut c);
    let mut dc = [0i32; 16];
    for b in 0..16 {
        dc[b] = c[0][b] as i32;
    }
    let nz = soa::quant(&mut c, qs, true, 1);
    c[0] = [0; 16];
    let mut any_ac = false;
    for b in 0..16 {
        cd.nz[b] = nz[b];
        if nz[b] > 0 {
            any_ac = true;
            for k in 0..16 {
                cd.luma[b][k] = c[k][b];
            }
        }
    }
    let mut dcl = luma_dc_forward(&dc, q);
    clamp_levels(&mut dcl);
    cd.dc = dcl;
    if any_ac {
        cd.cbp |= 15;
    }
    let dcy = luma_dc_inverse(&dcl, q.qp, q.v0);
    for b in 0..16 {
        c[0][b] = dcy[b] as i16;
    }
    let res = soa::dequant_idct(&c, qs, 1);
    soa::scatter16(&res, &ps, recon);
}

#[inline(always)]
fn chroma_resid_impl(
    src: &[[u8; 64]; 2],
    pred: &[[u8; 64]; 2],
    qc: &Quant,
    qcs: &QuantS,
    intra: bool,
    cd: &mut Coded,
    recon: &mut [[u8; 64]; 2],
) {
    let (mut c, ps) = soa::gather8x2(src, pred);
    soa::fdct(&mut c);
    let mut dc = [[0i32; 4]; 2];
    for l in 0..8 {
        dc[l >> 2][l & 3] = c[0][l] as i32;
    }
    let nz = soa::quant(&mut c, qcs, intra, 1);
    c[0] = [0; 8];
    let mut score = 0;
    for l in 0..8 {
        let (comp, b) = (l >> 2, l & 3);
        cd.nzc[comp][b] = nz[l];
        if nz[l] > 0 {
            for k in 0..16 {
                cd.cac[comp][b][k] = c[k][l];
            }
            if !intra {
                score += decimate_score(&cd.cac[comp][b]);
            }
        }
    }
    let mut any_dc = false;
    for comp in 0..2 {
        let l = chroma_dc_forward(&dc[comp], qc, intra).map(|v| v.clamp(-2047, 2047));
        cd.cdc[comp] = l;
        any_dc |= l.iter().any(|&v| v != 0);
    }
    let mut any_ac = nz.iter().any(|&n| n != 0);
    // Inter: drop sparse chroma AC (x264-style decimation).
    if any_ac && !intra && score < 7 {
        cd.cac = [[[0; 16]; 4]; 2];
        cd.nzc = [[0; 4]; 2];
        c = [[0; 8]; 16];
        any_ac = false;
    }
    let cbpc = if any_ac {
        2
    } else if any_dc {
        1
    } else {
        0
    };
    cd.cbp |= cbpc << 4;
    if cbpc == 0 {
        *recon = *pred;
        return;
    }
    for comp in 0..2 {
        let dcs = chroma_dc_inverse(&cd.cdc[comp], qc.qp, qc.v0);
        for b in 0..4 {
            c[0][comp * 4 + b] = dcs[b] as i16;
        }
    }
    let res = soa::dequant_idct(&c, qcs, 1);
    soa::scatter8x2(&res, &ps, recon);
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
            slice_edges: c.slice_deblock,
            cabac: c.cabac,
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
        cabac: None,
    };
    if c.cabac {
        while !s.w.byte_aligned() {
            s.w.put(1, 1); // cabac_alignment_one_bit
        }
        s.cabac = Some(crate::cabac::Cabac::new(c.q.qp, !c.idr));
    }
    let sy = &c.src.y;
    let su = &c.src.u;
    let sv = &c.src.v;
    let mut src_y = [0u8; 256];
    let mut src_c = [[0u8; 64]; 2];
    for mby in mby0..mby1 {
        for mbx in 0..c.mbw {
            if !c.idr && !c.changed[mby * c.mbw + mbx] {
                // Cheap P_Skip test: the skip MV is zero when A or B is missing or
                // an inter neighbour with MV 0.
                let zero = |i: &MbInfo| i.kind == 0 && i.mv == (0, 0);
                if mbx == 0 || mby == mby0 || zero(s.info(mbx - 1, mby)) || zero(s.info(mbx, mby - 1)) {
                    let i = (mby - mby0) * c.mbw + mbx;
                    if !c.stale[mby * c.mbw + mbx] {
                        // Fast path: P_Skip, recon already equal to the reference.
                        s.b.info[i] = SKIP_INFO;
                        s.stats.skip += 1;
                        if let Some(cb) = s.cabac.as_mut() {
                            let a = mbx > 0 && !s.b.info[i - 1].skip;
                            let b = mby > mby0 && !s.b.info[i - c.mbw].skip;
                            cb.decision(s.w, 11 + a as usize + b as usize, 1);
                            let last = mbx + 1 == c.mbw && mby + 1 == mby1;
                            cb.terminate(s.w, last as u32);
                        } else {
                            s.skip_run += 1;
                        }
                    } else {
                        s.code_copy(mbx, mby, (0, 0), (0, 0));
                    }
                } else {
                    let (mvp, skip_mv) = s.mv_pred(mbx, mby);
                    s.code_copy(mbx, mby, mvp, skip_mv);
                }
                continue;
            }
            let o = sy.off(mbx as isize * 16, mby as isize * 16);
            for y in 0..16 {
                src_y[y * 16..y * 16 + 16].copy_from_slice(&sy.data[o + y * sy.stride..o + y * sy.stride + 16]);
            }
            let o = su.off(mbx as isize * 8, mby as isize * 8);
            for y in 0..8 {
                src_c[0][y * 8..y * 8 + 8].copy_from_slice(&su.data[o + y * su.stride..o + y * su.stride + 8]);
                src_c[1][y * 8..y * 8 + 8].copy_from_slice(&sv.data[o + y * sv.stride..o + y * sv.stride + 8]);
            }
            let b0 = s.w.bits_written();
            let (i0, n0) = (s.stats.intra16 + s.stats.intra4, s.stats.inter);
            s.encode_mb(mbx, mby, &src_y, &src_c);
            let db = (s.w.bits_written() - b0) as u64;
            if s.stats.intra16 + s.stats.intra4 > i0 {
                s.stats.bits_intra += db;
            } else if s.stats.inter > n0 {
                s.stats.bits_inter += db;
            }
        }
    }
    if c.slice_deblock {
        let b = &mut s.b;
        let mut view = crate::deblock::View { y: &mut *b.y, u: &mut *b.u, v: &mut *b.v, ys: s.ys, cs: s.cs, yp: s.yp, cp: s.cp };
        crate::deblock::deblock_rows(&mut view, b.info, c.mbw, mby1 - mby0, &c.q, b.touched);
    }
    if let Some(cb) = s.cabac.take() {
        s.w.append_bytes(&cb.out);
        return s.stats;
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
    /// Macroblocks where `recon` (the picture before the reference) differs from `refp`.
    stale: Vec<bool>,
    touched: Vec<bool>,
    hpel: Option<crate::mc::HpelPlanes>,
    /// `hpel` holds the planes of the current reference.
    hpel_valid: bool,
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
    /// CABAC payload of an all-skip P slice (it never changes).
    skip_cache: Vec<u8>,
}

impl Encoder {
    pub fn new(cfg: EncoderConfig) -> Self {
        Self::with_idr_base(cfg, 0)
    }

    /// An encoder whose IDR pictures use ids starting at `idr_base` (so that
    /// independently encoded chunks can be concatenated).
    pub fn with_idr_base(cfg: EncoderConfig, idr_base: u32) -> Self {
        assert!(cfg.width >= 2 && cfg.height >= 2, "ak264: frame size too small");
        assert!(cfg.width.is_multiple_of(2) && cfg.height.is_multiple_of(2), "ak264: width and height must be even (4:2:0)");
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
            stale: vec![true; mbw * mbh],
            touched: vec![false; mbw * mbh],
            hpel: None,
            hpel_valid: false,
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
            skip_cache: Vec::new(),
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
        let idr = self.frame_in_gop == 0 || self.cfg.gop <= 1 || self.frame_in_gop >= self.cfg.gop as u64;
        if idr {
            self.frame_in_gop = 0;
        }
        self.out.clear();
        let mut stats = FrameStats { idr, ..Default::default() };
        let nmb = self.mbw * self.mbh;
        let mut any_changed = true;
        if idr {
            self.cur.y.load(f.y, f.y_stride, w, h);
            self.cur.u.load(f.u, f.c_stride, cw, ch);
            self.cur.v.load(f.v, f.c_stride, cw, ch);
        } else {
            let mbw = self.mbw;
            let Pic { y, u, v } = &mut self.cur;
            match &hints.changed {
                Some(c) => {
                    assert_eq!(c.len(), nmb, "ak264: changed map must have one entry per macroblock");
                    // Only macroblocks that may have changed are compared (exactly).
                    for (i, (dst, &maybe)) in self.changed.iter_mut().zip(c.iter()).enumerate() {
                        *dst = maybe && crate::diff::load_mb(y, u, v, f, i % mbw, i / mbw, true);
                    }
                }
                None => {
                    // Read-only flags (in parallel across rows when slices are used),
                    // then an exact compare + copy of the flagged macroblocks.
                    {
                        let (yr, ur, vr) = (&*y, &*u, &*v);
                        if self.writers.len() > 1 {
                            use rayon::prelude::*;
                            self.changed
                                .par_chunks_mut(mbw)
                                .enumerate()
                                .for_each(|(mby, row)| crate::diff::flag_row(yr, ur, vr, f, mby, row));
                        } else {
                            for (mby, row) in self.changed.chunks_mut(mbw).enumerate() {
                                crate::diff::flag_row(yr, ur, vr, f, mby, row);
                            }
                        }
                    }
                    // Inner macroblocks were compared exactly; edge ones were only flagged.
                    let full_cols = (w / 16).min(cw / 8);
                    let full_rows = (h / 16).min(ch / 8);
                    let exact_flags = crate::simd::level() >= 2;
                    for my in 0..self.mbh {
                        let row = &mut self.changed[my * mbw..(my + 1) * mbw];
                        let mut mx = 0;
                        while mx < mbw {
                            if !row[mx] {
                                mx += 1;
                                continue;
                            }
                            if exact_flags && my < full_rows && mx < full_cols {
                                // a run of changed inner macroblocks: copy row spans
                                let mut end = mx;
                                while end < full_cols && row[end] {
                                    end += 1;
                                }
                                crate::diff::copy_run(y, u, v, f, my, mx, end);
                                mx = end;
                            } else {
                                row[mx] = crate::diff::load_mb(y, u, v, f, mx, my, true);
                                mx += 1;
                            }
                        }
                    }
                }
            }
            any_changed = self.changed.iter().any(|&c| c);
            self.hint.fill(NO_HINT);
            for m in &hints.motions {
                let (x0, y0) = (m.x as usize / 16, m.y as usize / 16);
                let x1 = ((m.x + m.w) as usize).div_ceil(16).min(self.mbw);
                let y1 = ((m.y + m.h) as usize).div_ceil(16).min(self.mbh);
                for my in y0..y1 {
                    for mx in x0..x1 {
                        self.hint[my * self.mbw + mx] = ((-m.dx * 4) as i16, (-m.dy * 4) as i16);
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
                    full_range: self.cfg.full_range,
                    cabac: self.cfg.cabac,
                },
                self.level,
            );
            write_nal(&mut self.out, 3, 7, &self.hdr.buf);
            self.hdr.clear();
            write_pps(&mut self.hdr, self.cfg.qp, self.cfg.cabac);
            write_nal(&mut self.out, 3, 8, &self.hdr.buf);
        }
        let frame_num = (self.frame_in_gop % (1 << LOG2_MAX_FRAME_NUM)) as u32;
        if !any_changed {
            // All-skip P slice; the reference is unchanged.
            let cabac = self.cfg.cabac;
            let w = &mut self.writers[0];
            w.clear();
            write_slice_header(
                w,
                &SliceHdr { first_mb: 0, idr: false, frame_num, idr_id: 0, deblock: self.cfg.deblock, slice_edges: false, cabac },
            );
            if cabac {
                while !w.byte_aligned() {
                    w.put(1, 1);
                }
                if self.skip_cache.is_empty() {
                    let mut cw = BitWriter::new();
                    let mut cb = crate::cabac::Cabac::new(self.cfg.qp, true);
                    for i in 0..nmb {
                        cb.decision(&mut cw, 11, 1);
                        cb.terminate(&mut cw, (i + 1 == nmb) as u32);
                    }
                    self.skip_cache = cb.out;
                }
                w.append_bytes(&self.skip_cache);
            } else {
                w.ue(nmb as u32);
                w.trailing();
            }
            write_nal(&mut self.out, 2, 1, &w.buf);
            stats.skip = nmb as u32;
        } else {
            self.encode_slices(idr, frame_num, &mut stats);
            for ((st, inf), &t) in self.stale.iter_mut().zip(&self.info).zip(&self.touched) {
                *st = inf.coded || t;
            }
            if self.cfg.deblock && self.writers.len() == 1 {
                crate::deblock::deblock(&mut self.recon, &self.info, self.mbw, self.mbh, &self.q, &mut self.stale);
            }
            self.recon.extend();
            std::mem::swap(&mut self.recon, &mut self.refp);
            self.hpel_valid = false;
        }
        if idr {
            self.idr_count = self.idr_count.wrapping_add(1);
        }
        self.frame_in_gop += 1;
        stats.bytes = self.out.len();
        self.last = stats;
        &self.out
    }

    /// Slice row ranges. P frames balance the expected work: a changed macroblock
    /// costs about 40 unchanged ones.
    fn slice_rows(&self, idr: bool) -> Vec<(usize, usize)> {
        let n = self.writers.len();
        if n == 1 || idr {
            return (0..n).map(|i| (i * self.mbh / n, (i + 1) * self.mbh / n)).filter(|r| r.1 > r.0).collect();
        }
        let w: Vec<u64> = self.changed.chunks(self.mbw).map(|r| self.mbw as u64 + 40 * r.iter().filter(|&&c| c).count() as u64).collect();
        let total: u64 = w.iter().sum();
        let mut rows = Vec::with_capacity(n);
        let (mut start, mut acc) = (0usize, 0u64);
        for (y, &wy) in w.iter().enumerate() {
            acc += wy;
            let k = rows.len() as u64 + 1;
            // close the band once its share is reached, leaving rows for the rest
            if acc * n as u64 >= total * k && rows.len() + 1 < n && self.mbh - (y + 1) >= n - rows.len() - 1 {
                rows.push((start, y + 1));
                start = y + 1;
            }
        }
        rows.push((start, self.mbh));
        rows
    }

    fn encode_slices(&mut self, idr: bool, frame_num: u32, stats: &mut FrameStats) {
        // Sub-pel refinement on a busy frame: build the half-sample planes once.
        if self.cfg.subpel && !idr && !self.hpel_valid {
            let busy = self.changed.iter().filter(|&&c| c).count() * 4 > self.changed.len();
            if busy {
                let r = &self.refp.y;
                self.hpel.get_or_insert_with(|| crate::mc::HpelPlanes::new(r)).compute(r);
                self.hpel_valid = true;
            }
        }
        let ctx = FrameCtx {
            mbw: self.mbw,
            q: self.q,
            qc: self.qc,
            qs: QuantS::new(&self.q),
            qcs: QuantS::new(&self.qc),
            zero_sad: {
                let (l, c) = crate::soa::zero_sad_bounds(&self.q, &QuantS::new(&self.q), &self.qc, &QuantS::new(&self.qc), false);
                (l, c, crate::soa::chroma_dc_limit(&self.qc, false))
            },
            zero_sad4: crate::soa::zero_sad4_bounds(&QuantS::new(&self.q), &QuantS::new(&self.qc), false),
            zero_sad4_intra: crate::soa::zero_sad4_bounds(&QuantS::new(&self.q), &QuantS::new(&self.qc), true),
            zero_dc_intra: (crate::soa::chroma_dc_limit(&self.q, true), crate::soa::chroma_dc_limit(&self.qc, true)),
            zero_sad_intra: crate::soa::zero_sad_bounds(&self.q, &QuantS::new(&self.q), &self.qc, &QuantS::new(&self.qc), true),
            lambda: self.lambda,
            src: &self.cur,
            refp: &self.refp,
            changed: &self.changed,
            stale: &self.stale,
            hint: &self.hint,
            idr,
            max_mvy: max_mv_y(self.level),
            subpel: self.cfg.subpel,
            hpel: if self.hpel_valid { self.hpel.as_ref() } else { None },
            cabac: self.cfg.cabac,
            hdr_frame_num: frame_num,
            idr_id: self.idr_count & 0xffff,
            deblock: self.cfg.deblock,
            slice_deblock: self.cfg.deblock && self.writers.len() > 1,
        };
        let rows = self.slice_rows(idr);
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
            self.touched.fill(false);
            let mut trest: &mut [bool] = &mut self.touched;
            for &(a, b) in &rows {
                let n = b - a;
                let (y, yr) = std::mem::take(&mut yrest).split_at_mut(n * 16 * ys);
                let (u, ur) = std::mem::take(&mut urest).split_at_mut(n * 8 * cs);
                let (v, vr) = std::mem::take(&mut vrest).split_at_mut(n * 8 * cs);
                let (inf, ir) = std::mem::take(&mut irest).split_at_mut(n * self.mbw);
                let (tch, tr) = std::mem::take(&mut trest).split_at_mut(n * self.mbw);
                trest = tr;
                yrest = yr;
                urest = ur;
                vrest = vr;
                irest = ir;
                bands.push(Band { y, u, v, info: inf, touched: tch, mby0: a, mby1: b });
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
            stats.bits_intra += r.bits_intra;
            stats.bits_inter += r.bits_inter;
        }
    }
}
