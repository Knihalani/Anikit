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
/// Sum of |second differences| of a 16×16 block below which it is a smooth ramp
/// (about 2 per sample; a ±1 dither scores about 1).
pub(crate) const SMOOTH_HF: u32 = 2 * 448;

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
    /// Perceptual skip: in P frames, a changed macroblock whose samples (Y, Cb, Cr)
    /// all lie within this distance of the co-located *reconstructed* reference is
    /// coded as a copy. Comparing against the reconstruction keeps the error
    /// bounded by the threshold (it cannot accumulate). 0 = off; 1–2 hides
    /// renderer dither and ±1 drift.
    pub skip_threshold: u8,
    /// Adaptive quantisation strength: smooth macroblocks (fades, lit floors,
    /// soft gradients; small second differences) get a lower QP, −(4 + up to 4
    /// for smoothness + up to 3 for darkness) × strength, with a wide AC dead
    /// zone so their noise is not coded. Busy macroblocks keep `qp`.
    /// 0 = constant QP. It stops gradients turning into a patchwork of blocks, but
    /// costs size on smooth-heavy UI content (see docs/video/encoder.md).
    /// Negative (the default, −0.5): automatic per frame — |value| on frames
    /// dominated by smooth textured content (3D renders, video), 0 elsewhere.
    pub aq_strength: f32,
    /// Per-frame quality floor: after each frame the luma SSIM of the
    /// reconstruction is measured, and while it is below this floor the
    /// following frames use a lower QP (back up towards `qp` when there is
    /// margin). 0 = constant QP. Deterministic (it depends only on the frames
    /// already coded in this stream or chunk).
    pub ssim_floor: f32,
    /// How far below `qp` the SSIM floor may take the QP (bounds the size on
    /// content such as film grain, where SSIM rises slowly with bits).
    pub ssim_max_drop: u8,
    /// Consecutive B pictures between reference pictures (0 = none). Only with
    /// CABAC and `threads == 1` (chunk-parallel encoding); otherwise ignored.
    /// Output is delayed: call [`Encoder::flush`] at the end of the stream.
    pub bframes: u8,
    /// Try 16×8, 8×16 and 8×8 partitions for P macroblocks (CABAC only). About
    /// 1.5% smaller at equal SSIM on motion graphics, at ~1.5× the encode time.
    pub partitions: bool,
    /// 8×8 transform for inter macroblocks (High profile; CABAC only).
    pub transform8: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        EncoderConfig { width: 0, height: 0, fps: 30.0, qp: 22, gop: 250, deblock: false, threads: 1, full_range: false, subpel: true, cabac: true, skip_threshold: 0, aq_strength: -0.5, ssim_floor: 0.985, ssim_max_drop: 8, bframes: 0, partitions: false, transform8: true }
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
    /// Film grain the renderer added: standard deviation of the luma grain at
    /// mid-grey, in 8-bit code values (0 = none). The renderer's grain weight
    /// `0.35 + 2.6·l·(1−l)` (l = luma 0..1) is applied per block. The SSIM floor
    /// then treats that much missing source variance as free, so re-drawn
    /// random grain doesn't drive the QP down while lost detail still does.
    pub grain: f32,
}

impl FrameHints {
    /// Per-macroblock motion hints from a global camera transform (zoom, pan,
    /// rotation), for frames where every macroblock changes.
    ///
    /// `prev_of` maps a current-frame luma position to where that content was in
    /// the previous frame: `(px, py) = (a·x + b·y + c, d·x + e·y + f)` with
    /// `[a, b, c, d, e, f]`. Each macroblock gets the integer motion of its
    /// centre; quarter-pel refinement (on by default) finishes the job.
    /// Macroblocks that don't move get no hint.
    pub fn from_affine(width: u32, height: u32, prev_of: [f64; 6]) -> FrameHints {
        let [a, b, c, d, e, f] = prev_of;
        let (mbw, mbh) = (width.div_ceil(16), height.div_ceil(16));
        let mut motions = Vec::new();
        for my in 0..mbh {
            for mx in 0..mbw {
                let (x, y) = (mx as f64 * 16.0 + 8.0, my as f64 * 16.0 + 8.0);
                let (px, py) = (a * x + b * y + c, d * x + e * y + f);
                let (dx, dy) = ((x - px).round() as i32, (y - py).round() as i32);
                if (dx, dy) != (0, 0) {
                    motions.push(MotionHint { x: mx * 16, y: my * 16, w: 16, h: 16, dx, dy });
                }
            }
        }
        FrameHints { changed: None, motions, grain: 0.0 }
    }
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
    /// QP_Y of the macroblock (for deblocking).
    pub qp: u8,
    pub cbp: u8,
    /// intra_chroma_pred_mode (0 for inter).
    pub cmode: u8,
    /// Motion vector difference (quarter pels; 0 for skip / intra).
    pub mvd: (i16, i16),
    /// coded_block_flag of luma DC (bit 0), Cb DC (bit 1), Cr DC (bit 2).
    pub dcf: u8,
    /// B slices: prediction lists used (bit 0 L0, bit 1 L1; 0 for intra), the
    /// L1 motion vector and its difference, and B_Skip / B_Direct_16x16.
    pub refs: u8,
    pub mv1: (i16, i16),
    pub mvd1: (i16, i16),
    pub direct: bool,
    /// P partitioning (0 16×16, 1 16×8, 2 8×16, 3 8×8) and, when partitioned,
    /// the L0 vector and vector difference of each 8×8 quadrant (raster).
    pub part: u8,
    pub mv8: [(i16, i16); 4],
    pub mvd8: [(i16, i16); 4],
    /// transform_size_8x8_flag.
    pub t8: bool,
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
    /// 8×8 transform: luma levels per 8×8 block (raster) instead of `luma`.
    t8: bool,
    l8: [[i16; 64]; 4],
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
            t8: false,
            l8: [[0; 64]; 4],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    P16,
    I16(u8),
    I4,
}

/// Everything that depends on the quantiser parameter, precomputed per QP.
pub(crate) struct QpSet {
    pub q: Quant,
    pub qc: Quant,
    pub qs: QuantS,
    pub qcs: QuantS,
    pub q8: crate::transform::Quant8,
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
}

impl QpSet {
    fn new(qp: u8) -> Self {
        Self::build(qp, false)
    }

    /// `smooth`: AC coefficients (every position but the block DC) get a wide
    /// dead zone (rounding 1/8 instead of 1/3 intra, 1/6 inter). Used with a
    /// lower QP on smooth macroblocks, it buys DC precision (what a gentle
    /// gradient needs) without coding their low-amplitude noise.
    fn build(qp: u8, smooth: bool) -> Self {
        let q = Quant::new(qp);
        let qc = Quant::new(chroma_qp(qp));
        let (mut qs, mut qcs) = (QuantS::new(&q), QuantS::new(&qc));
        if smooth {
            for t in [&mut qs, &mut qcs] {
                for i in 1..16 {
                    let b = ((65536.0 / t.m[i] as f64) / 8.0).round() as u16;
                    t.bias[0][i] = b;
                    t.bias[1][i] = b;
                }
            }
        }
        let (l, c) = crate::soa::zero_sad_bounds(&q, &qs, &qc, &qcs, false);
        QpSet {
            zero_sad: (l, c, crate::soa::chroma_dc_limit(&qc, false)),
            zero_sad4: crate::soa::zero_sad4_bounds(&qs, &qcs, false),
            zero_sad4_intra: crate::soa::zero_sad4_bounds(&qs, &qcs, true),
            zero_dc_intra: (crate::soa::chroma_dc_limit(&q, true), crate::soa::chroma_dc_limit(&qc, true)),
            zero_sad_intra: crate::soa::zero_sad_bounds(&q, &qs, &qc, &qcs, true),
            lambda: (0.85f64 * 2f64.powf((qp as f64 - 12.0) / 6.0)).round().max(1.0) as i32,
            q8: crate::transform::Quant8::new(qp),
            q,
            qc,
            qs,
            qcs,
        }
    }
}

/// Shared, read-only state for coding one frame.
pub(crate) struct FrameCtx<'a> {
    pub mbw: usize,
    /// Slice QP (the configured QP, lowered by the SSIM floor) and the parameter
    /// sets of every QP.
    pub qp: u8,
    pub base_qp: u8,
    pub qsets: &'a [QpSet],
    /// The same with wide AC dead zones, for smooth macroblocks under AQ.
    pub qsets_smooth: &'a [QpSet],
    /// Adaptive quantisation strength (0 = off).
    pub aq: f32,
    pub src: &'a Pic,
    /// Reference (L0) and, in B slices, the L1 reference (else the same as L0).
    pub refp: &'a Pic,
    pub refp1: &'a Pic,
    /// B slice: co-located (L1 reference) macroblock info and the macroblocks
    /// identical in the B frame and both references' sources.
    pub bslice: bool,
    pub col: &'a [MbInfo],
    /// B slice: (distance to the L0 reference, distance between the references).
    pub bdist: (i32, i32),
    pub changed: &'a [bool],
    pub stale: &'a [bool],
    pub hint: &'a [(i16, i16)],
    pub idr: bool,
    pub max_mvy: i32,
    pub subpel: bool,
    pub skip_t: u8,
    /// Frame-level half-sample planes of the reference (when many macroblocks change).
    pub hpel: Option<&'a crate::mc::HpelPlanes>,
    pub hpel1: Option<&'a crate::mc::HpelPlanes>,
    pub cabac: bool,
    /// P partitions (see [`EncoderConfig::partitions`]).
    pub partitions: bool,
    /// 8×8 transform allowed (transform_8x8_mode_flag).
    pub t8: bool,
    pub hdr_frame_num: u32,
    /// pic_order_cnt_lsb (POC type 0 streams).
    pub poc: Option<u32>,
    pub idr_id: u32,
    pub deblock: bool,
    /// Deblock inside each slice (multi-slice frames, idc 2).
    pub slice_deblock: bool,
}

pub(crate) const NO_HINT: (i16, i16) = (i16::MIN, i16::MIN);

/// A frame held for a B picture: pixels, display index in the GOP, input
/// index, change map since the previous input frame, grain hint.
struct Held {
    buf: crate::frame::YuvBuffer,
    disp: u64,
    gidx: u64,
    changed: Option<Vec<bool>>,
    grain: f32,
}

/// A B picture whose SSIM estimate is this far below the following P
/// picture's is coded again at the P QP.
const B_SSIM_SLACK: f64 = 0.001;

/// QP of B pictures above the P pictures'.
const B_QP_OFFSET: i32 = 2;

/// Is macroblock (mx, my) identical in `a` and `b` (luma and chroma)?
fn mb_equal(a: &Pic, b: &Pic, mx: usize, my: usize) -> bool {
    for (pa, pb, n) in [(&a.y, &b.y, 16usize), (&a.u, &b.u, 8), (&a.v, &b.v, 8)] {
        let (oa, ob) = (pa.off((mx * n) as isize, (my * n) as isize), pb.off((mx * n) as isize, (my * n) as isize));
        for r in 0..n {
            if pa.data[oa + r * pa.stride..oa + r * pa.stride + n] != pb.data[ob + r * pb.stride..ob + r * pb.stride + n] {
                return false;
            }
        }
    }
    true
}

mod bframe;
mod part;
mod t8;

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
    qp: 0,
    dcf: 0,
    refs: 1,
    mv1: (0, 0),
    mvd1: (0, 0),
    direct: false,
    part: 0,
    mv8: [(0, 0); 4],
    mvd8: [(0, 0); 4],
    t8: false,
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
#[derive(Clone)]
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
    /// Quantiser of the macroblock being coded (adaptive quantisation).
    p: &'a QpSet,
    mb_qp: u8,
    /// QP of the previous macroblock in decoding order (QP_Y,PRED).
    qp_pred: u8,
    /// This / the previous macroblock coded a nonzero mb_qp_delta (CABAC context).
    dqp_nz: bool,
    dqp_prev_nz: bool,
    /// Reference list that prediction helpers and motion search use (B slices).
    rsel: usize,
    /// Motion search: integer positions already evaluated in this search
    /// (direct-mapped; entries hold the search's generation and the vector).
    seen: Vec<std::cell::Cell<(u32, (i32, i32))>>,
    seen_gen: std::cell::Cell<u32>,
    /// B slices: an extra search candidate (the co-located vector, scaled) and
    /// no long-range rings (the P picture already found the motion).
    extra_cand: Option<(i32, i32)>,
    no_rings: bool,
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

    #[inline(always)]
    fn rp(&self) -> &'a Pic {
        if self.rsel == 0 { self.c.refp } else { self.c.refp1 }
    }
    #[inline(always)]
    fn hp(&self) -> Option<&'a crate::mc::HpelPlanes> {
        if self.rsel == 0 { self.c.hpel } else { self.c.hpel1 }
    }

    fn nb(&self, mbx: isize, mby: isize, q: usize) -> Nb {
        self.nbl(mbx, mby, self.rsel, q)
    }

    /// Neighbour for motion vector prediction in list `l` (B slices: only
    /// macroblocks predicting from that list count as inter), at its 8×8
    /// quadrant `q` (the one touching this macroblock).
    fn nbl(&self, mbx: isize, mby: isize, l: usize, q: usize) -> Nb {
        if mbx < 0 || mby < self.b.mby0 as isize || mbx >= self.c.mbw as isize {
            return Nb { avail: false, inter: false, mv: (0, 0) };
        }
        let i = self.info(mbx as usize, mby as usize);
        if i.kind == 0 && (!self.c.bslice || (i.refs >> l) & 1 != 0) {
            let mv = if l == 0 { i.qmv(q) } else { i.mv1 };
            Nb { avail: true, inter: true, mv: (mv.0 as i32, mv.1 as i32) }
        } else {
            Nb { avail: true, inter: false, mv: (0, 0) }
        }
    }

    /// (mvp for P16x16, P_Skip mv) in quarter pels.
    fn mv_pred(&self, mbx: usize, mby: usize) -> ((i32, i32), (i32, i32)) {
        let (x, y) = (mbx as isize, mby as isize);
        let a = self.nb(x - 1, y, 1);
        let mut b = self.nb(x, y - 1, 2);
        let mut c = self.nb(x + 1, y - 1, 2);
        if !c.avail {
            c = self.nb(x - 1, y - 1, 3);
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
        let p = &self.rp().y;
        let o = p.off(mbx as isize * 16 + mv.0 as isize, mby as isize * 16 + mv.1 as isize);
        (&p.data[o..], p.stride)
    }

    /// Every sample within `t` of the co-located reference (luma and chroma)?
    fn near_ref(&self, mbx: usize, mby: usize, src_y: &[u8; 256], src_c: &[[u8; 64]; 2], t: u8) -> bool {
        let (r, rs) = self.luma_ref_int(mbx, mby, (0, 0));
        if !dsp::max_abs_le(src_y, 16, r, rs, 16, 16, t) {
            return false;
        }
        for (comp, sc) in src_c.iter().enumerate() {
            let p = if comp == 0 { &self.rp().u } else { &self.rp().v };
            let o = p.off(mbx as isize * 8, mby as isize * 8);
            if !dsp::max_abs_le(sc, 8, &p.data[o..], p.stride, 8, 8, t) {
                return false;
            }
        }
        true
    }

    fn mc_chroma(&self, comp: usize, mbx: usize, mby: usize, mv: (i32, i32), out: &mut [u8; 64]) {
        let p = if comp == 0 { &self.rp().u } else { &self.rp().v };
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
        crate::simd::chroma_bilin8(d, o, s, fx, fy, out);
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
            let (i, ys) = (self.yi(x0 - 1, y0), self.ys);
            let col = &self.b.y[i..i + 15 * ys + 1];
            for k in 0..16 {
                e.left[k] = col[k * ys];
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
            let (i, cs) = (self.ci(x0 - 1, y0), self.cs);
            let col = &pl[i..i + 7 * cs + 1];
            for k in 0..8 {
                e.left[k] = col[k * cs];
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
        let (off, smooth) = if c.aq > 0.0 { aq_offset(src_y, src_c, c.aq) } else { (0, false) };
        self.mb_qp = (c.qp as i32 + off).clamp(0, 51) as u8;
        self.p = if smooth { &c.qsets_smooth[self.mb_qp as usize] } else { &c.qsets[self.mb_qp as usize] };
        let mbi = mby * c.mbw + mbx;
        let lambda = self.p.lambda;

        let mut inter: Option<Inter> = None;
        let (mut mvp, mut skip_mv) = ((0, 0), (0, 0));
        if !c.idr {
            (mvp, skip_mv) = self.mv_pred(mbx, mby);
            if !c.changed[mbi] {
                // Unchanged: copy of the reference.
                self.code_copy(mbx, mby, mvp, skip_mv);
                return;
            }
            if c.skip_t > 0 && self.near_ref(mbx, mby, src_y, src_c, c.skip_t) {
                self.stats.changed += 1;
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
            if c.idr {
                intra::pred16(&e16, m, &mut pred);
                let s = dsp::satd(src_y, &pred, 16, 16);
                if s < best16.0 {
                    best16 = (s, m);
                    pred16 = pred;
                }
            } else {
                // P frames: SAD straight from the edges, build only the winner
                let s = intra::sad16_mode(&e16, m, src_y);
                if s < best16.0 {
                    best16 = (s, m);
                }
            }
        }
        if !c.idr {
            intra::pred16(&e16, best16.1, &mut pred16);
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

        // In P frames I4x4 rarely beats the best of P16/I16 unless both are poor.
        let try_i4 = best_cost > lambda as u32 * 24 + 256 && (c.idr || best_cost > lambda as u32 * 64 + 1024)
            // Smooth ramps (fades, soft gradients): 4×4 predictions turn them into a
            // staircase of 4×4 squares; I16 (plane) and inter keep them smooth.
            // (CAVLC at QP < 2: I16 DC levels would pass CAVLC's ±2047 escape.)
            && ((!c.cabac && self.mb_qp < 2) || crate::simd::second_diff16(src_y) >= SMOOTH_HF);
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
                if c.partitions && it.cost > lambda as u32 * 8 {
                    if let Some(pc) = self.try_partitions(mbx, mby, src_y, it) {
                        self.code_part(mbx, mby, src_y, src_c, &pc);
                        return;
                    }
                }
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
        let lambda = self.p.lambda as u32;
        let w16 = (c.mbw * 16) as i32;
        let h16 = (self.rp().y.h) as i32;
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
        let mut cands = [(0i32, 0i32); 9];
        let mut n = 0;
        fn push_c(cands: &mut [(i32, i32); 9], n: &mut usize, mv: (i32, i32)) {
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
        if let Some(e) = self.extra_cand {
            push!(round(e));
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
            for nb in [self.nb(x - 1, y, 1), self.nb(x, y - 1, 2), self.nb(x + 1, y - 1, 2)] {
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
        if !quick && best.0 > mvcost((best.1 .0 * 4, best.1 .1 * 4)) {
            // Rings at radius 4..16 around the best candidate (large motion that
            // no predictor points to), hexagon steps (x264's "hex"), then the
            // 8 neighbours.
            const RING: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];
            let c0 = best.1;
            let rings: &[i32] = if self.no_rings { &[] } else { &[4, 8, 12, 16] };
            for &r in rings {
                for d in RING {
                    let mv = clamp((c0.0 + d.0 * r, c0.1 + d.1 * r));
                    let s = eval(mv);
                    if s < best.0 {
                        best = (s, mv);
                    }
                }
            }
            // Positions already evaluated in this search can't improve on `best`.
            let g = self.seen_gen.get().wrapping_add(1);
            self.seen_gen.set(g);
            let fresh = |mv: (i32, i32)| {
                let e = &self.seen[((mv.0 & 31) | ((mv.1 & 31) << 5)) as usize];
                if e.get() == (g, mv) {
                    return false;
                }
                e.set((g, mv));
                true
            };
            fresh(best.1);
            const HEX: [(i32, i32); 6] = [(-2, 0), (-1, -2), (1, -2), (2, 0), (1, 2), (-1, 2)];
            for _ in 0..16 {
                let cur = best;
                for d in HEX {
                    let mv = clamp((cur.1 .0 + d.0, cur.1 .1 + d.1));
                    if !fresh(mv) {
                        continue;
                    }
                    let s = eval(mv);
                    if s < best.0 {
                        best = (s, mv);
                    }
                }
                if best.1 == cur.1 {
                    break;
                }
            }
            for _ in 0..8 {
                let cur = best;
                for d in RING {
                    let mv = clamp((cur.1 .0 + d.0, cur.1 .1 + d.1));
                    if !fresh(mv) {
                        continue;
                    }
                    let s = eval(mv);
                    if s < best.0 {
                        best = (s, mv);
                    }
                }
                if best.1 == cur.1 {
                    break;
                }
            }
        }
        let base = best.1;
        let base_q = (base.0 * 4, base.1 * 4);
        // The full search often ends where its seed (a quick search) did; the seed
        // already holds that prediction and its SATD cost (seeds reaching here have
        // SAD above the quick-exit bound, so their cost is SATD-based).
        if let Some(q) = seed {
            if !quick && q.mv == base_q && q.sad > lambda * 24 {
                return q.clone();
            }
        }
        let mut pred = [0u8; 256];
        {
            let (r, rs) = self.luma_ref_int(mbx, mby, base);
            crate::simd::copy_block::<16>(&mut pred, 0, 16, r, 0, rs, 16);
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
        let lambda = self.p.lambda as u32;
        let mvp = it.mvp;
        let mvcost = |q: (i32, i32)| -> u32 { lambda * (se_len(q.0 - mvp.0) + se_len(q.1 - mvp.1)) };
        let base_q = it.mv;
        let (bx, by) = ((mbx as i32 * 16 + (base_q.0 >> 2)) as isize, (mby as i32 * 16 + (base_q.1 >> 2)) as isize);
        let win = if self.hp().is_none() { Some(crate::mc::Hpel::new(&self.rp().y, bx, by)) } else { None };
        let mut p = [0u8; 256];
        let mut d = (0, 0);
        // SAD for the candidates (cheap), SATD for the final cost.
        let mut best = it.sad + mvcost(base_q);
        let start = it.mv;
        const SQ: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
        const CROSS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
        for (step, pat) in [(2, &SQ[..]), (1, &CROSS[..])] {
            let centre = d;
            let mut cands = [(0i32, 0i32); 8];
            for (c, &(sx, sy)) in cands.iter_mut().zip(pat) {
                *c = (centre.0 + sx * step, centre.1 + sy * step);
            }
            let cands = &cands[..pat.len()];
            let mut sads = [0u32; 8];
            match (&win, self.hp()) {
                (Some(w), _) => {
                    for (c, r) in cands.iter().zip(sads.iter_mut()) {
                        w.pred(c.0, c.1, &mut p);
                        *r = crate::simd::sad16(src_y, 16, &p, 16);
                    }
                }
                (None, Some(hp)) => hp.sad_cands(&self.rp().y, bx, by, cands, src_y, &mut sads),
                _ => unreachable!(),
            }
            for (&cand, &sad) in cands.iter().zip(&sads) {
                let q = (base_q.0 + cand.0, base_q.1 + cand.1);
                let cst = sad + mvcost(q);
                if cst < best {
                    best = cst;
                    it.mv = q;
                    d = cand;
                }
            }
        }
        if it.mv != start {
            let (cx, cy) = (d.0, d.1);
            match (&win, self.hp()) {
                (Some(w), _) => w.pred(cx, cy, &mut p),
                (None, Some(hp)) => {
                    hp.pred(&self.rp().y, bx + (cx >> 2) as isize, by + (cy >> 2) as isize, cx & 3, cy & 3, &mut p)
                }
                _ => unreachable!(),
            }
            it.pred = p;
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
            qp: 0,
            kind: 0,
            skip,
            coded: false,
            mv: (mv.0 as i16, mv.1 as i16),
            mvd: (mvd.0 as i16, mvd.1 as i16),
            m4: [2; 16],
            refs: 1,
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
        let h16 = self.rp().y.h as i32 * 4;
        let (bx, by) = (mbx as i32 * 64 + mv.0, mby as i32 * 64 + mv.1);
        if bx < -64 || bx > w16 || by < -64 || by > h16 || mv.1.abs() >= c.max_mvy * 4 {
            return false;
        }
        let mut a = [0u8; 256];
        match self.hp() {
            Some(hp) => hp.pred(
                &self.rp().y,
                mbx as isize * 16 + (mv.0 >> 2) as isize,
                mby as isize * 16 + (mv.1 >> 2) as isize,
                mv.0 & 3,
                mv.1 & 3,
                &mut a,
            ),
            None => crate::mc::luma_pred(&self.rp().y, mbx as isize * 16, mby as isize * 16, mv, &mut a),
        }
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
        crate::simd::copy_block::<16>(self.b.y, base, self.ys, r, 0, rs, 16);
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
        let ry = if sad > self.p.zero_sad.0 && crate::simd::max_sad4(src_y, pred, 16, 16) > self.p.zero_sad4.0 {
            self.luma_resid(src_y, pred, &mut cd, &mut recon);
            &recon
        } else {
            pred
        };
        let mut reconc = [[0u8; 64]; 2];
        let zc = self.p.zero_sad.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &predc[0]), crate::simd::sad64(&src_c[1], &predc[1]));
        // chroma: AC by block, DC (2x2 Hadamard of block sums) by the 8x8 SAD
        let dc_ok = s0 <= self.p.zero_sad.2 && s1 <= self.p.zero_sad.2;
        let rc = if (s0 > zc || s1 > zc)
            && !(dc_ok
                && crate::simd::max_sad4(&src_c[0], &predc[0], 8, 8) <= self.p.zero_sad4.1
                && crate::simd::max_sad4(&src_c[1], &predc[1], 8, 8) <= self.p.zero_sad4.1)
        {
            chroma_resid(src_c, &predc, &self.p.qc, &self.p.qcs, false, self.dc_max(), &mut cd, &mut reconc);
            &reconc
        } else {
            &predc
        };
        self.store_recon(mbx, mby, ry, rc);
        let skip = cd.cbp == 0 && mv == skip_mv;
        let mvd = if skip { (0, 0) } else { (mv.0 - mvp.0, mv.1 - mvp.1) };
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo {
            qp: 0,
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
            refs: 1,
            mv1: (0, 0),
            mvd1: (0, 0),
            direct: false,
            part: 0,
            mv8: [(0, 0); 4],
            mvd8: [(0, 0); 4],
            t8: cd.t8 && cd.cbp & 15 != 0,
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

    /// Inter luma residual with the 4×4 or (when it suits the residual) the
    /// 8×8 transform.
    fn luma_resid(&self, src: &[u8; 256], pred: &[u8; 256], cd: &mut Coded, recon: &mut [u8; 256]) {
        if self.c.t8 && t8::prefer_t8(src, pred) {
            t8::luma_inter8(src, pred, &self.p.q8, cd, recon);
        } else {
            luma_inter(src, pred, &self.p.qs, cd, recon);
        }
    }

    fn store_recon(&mut self, mbx: usize, mby: usize, y16: &[u8; 256], c8: &[[u8; 64]; 2]) {
        let base = self.yi(mbx * 16, mby * 16);
        crate::simd::copy_block::<16>(self.b.y, base, self.ys, y16, 0, 16, 16);
        let base = self.ci(mbx * 8, mby * 8);
        let cs = self.cs;
        crate::simd::copy_block::<8>(self.b.u, base, cs, &c8[0], 0, 8, 8);
        crate::simd::copy_block::<8>(self.b.v, base, cs, &c8[1], 0, 8, 8);
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
        let lambda = self.p.lambda as u32;
        let q = self.p.q;
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
        let q = self.p.q;
        if let (Kind::I16(_), Some((src, pred))) = (kind, i16) {
            let mut recon = *pred;
            let sad = crate::simd::sad16(src, 16, pred, 16);
            let (b4, dcl) = (self.p.zero_sad4_intra.0, self.p.zero_dc_intra.0);
            if sad > self.p.zero_sad_intra.0 && !(sad < 2 * dcl && crate::simd::max_sad4(src, pred, 16, 16) <= b4) {
                luma_i16(src, pred, &q, &self.p.qs, self.dc_max(), cd, &mut recon);
            }
            let base = self.yi(mbx * 16, mby * 16);
            crate::simd::copy_block::<16>(self.b.y, base, self.ys, &recon, 0, 16, 16);
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
        let zc = self.p.zero_sad_intra.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &predc[0]), crate::simd::sad64(&src_c[1], &predc[1]));
        let (b4, dcl) = (self.p.zero_sad4_intra.1, self.p.zero_dc_intra.1);
        let zero_c = s0 <= dcl
            && s1 <= dcl
            && crate::simd::max_sad4(&src_c[0], &predc[0], 8, 8) <= b4
            && crate::simd::max_sad4(&src_c[1], &predc[1], 8, 8) <= b4;
        if (s0 > zc || s1 > zc) && !zero_c {
            chroma_resid(src_c, &predc, &self.p.qc, &self.p.qcs, true, self.dc_max(), cd, &mut reconc);
        } else {
            reconc = predc;
        }
        let base = self.ci(mbx * 8, mby * 8);
        let cs = self.cs;
        crate::simd::copy_block::<8>(self.b.u, base, cs, &reconc[0], 0, 8, 8);
        crate::simd::copy_block::<8>(self.b.v, base, cs, &reconc[1], 0, 8, 8);

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
            qp: 0,
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
            refs: 0,
            mv1: (0, 0),
            mvd1: (0, 0),
            direct: false,
            part: 0,
            mv8: [(0, 0); 4],
            mvd8: [(0, 0); 4],
            t8: false,
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

    /// Largest DC level: CAVLC escapes reach ±2047 in Main/Baseline; CABAC codes
    /// the full ±2^15 range (which QP 0–3 needs for flat-area DCs).
    #[inline]
    fn dc_max(&self) -> i32 {
        if self.c.cabac { 32767 } else { 2047 }
    }

    /// mb_qp_delta (CAVLC); the macroblock now has QP `mb_qp`.
    fn cavlc_dqp(&mut self) {
        let d = self.mb_qp as i32 - self.qp_pred as i32;
        self.w.se(d);
        self.qp_pred = self.mb_qp;
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
                self.cavlc_dqp();
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
        let sx = a.map_or(0, |m| m.qmvd(1).0.unsigned_abs() as u32) + b.map_or(0, |m| m.qmvd(2).0.unsigned_abs() as u32);
        let sy = a.map_or(0, |m| m.qmvd(1).1.unsigned_abs() as u32) + b.map_or(0, |m| m.qmvd(2).1.unsigned_abs() as u32);
        cb.mvd(w, 40, mvd.0, sx);
        cb.mvd(w, 47, mvd.1, sy);
        cabac_cbp(cb, w, &a, &b, cd.cbp);
        if self.c.t8 && cd.cbp & 15 != 0 {
            cb.decision(w, 399 + t8::t8_ctx(&a, &b), cd.t8 as u32);
        }
        if cd.cbp != 0 {
            let d = self.mb_qp as i32 - self.qp_pred as i32;
            cabac_dqp(cb, w, d, self.dqp_prev_nz);
            self.qp_pred = self.mb_qp;
            self.dqp_nz = d != 0;
            let cb = self.cabac.as_mut().unwrap();
            let w = &mut *self.w;
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
                    self.cavlc_dqp();
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
                        self.cavlc_dqp();
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
        let bin0_ctx = if self.c.bslice {
            // B slice: mb_skip_flag 0, then the intra prefix 1 1 1 1 0 1 of mb_type
            let inc = a.map_or(0, |m| !m.skip as usize) + b.map_or(0, |m| !m.skip as usize);
            cb.decision(w, 24 + inc, 0);
            let tinc = a.map_or(0, |m| !m.direct as usize) + b.map_or(0, |m| !m.direct as usize);
            cb.decision(w, 27 + tinc, 1);
            cb.decision(w, 27 + 3, 1);
            cb.decision(w, 27 + 4, 1);
            cb.decision(w, 27 + 5, 1);
            cb.decision(w, 27 + 5, 0);
            cb.decision(w, 27 + 5, 1);
            32
        } else if p_slice {
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
                let (c_l, c_c, c_c2, c_p1, c_p2, n_p1, n_p2) = if self.c.bslice {
                    (33, 34, 34, 35, 35, 35, 35)
                } else if p_slice {
                    (18, 19, 19, 20, 20, 20, 20)
                } else {
                    (6, 7, 8, 9, 10, 9, 10)
                };
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
                if self.c.t8 {
                    cb.decision(w, 399 + t8::t8_ctx(&a, &b), 0); // transform_size_8x8_flag: I4x4
                }
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
            let d = self.mb_qp as i32 - self.qp_pred as i32;
            cabac_dqp(cb, w, d, self.dqp_prev_nz);
            self.qp_pred = self.mb_qp;
            self.dqp_nz = d != 0;
            let cb = self.cabac.as_mut().unwrap();
            let w = &mut *self.w;
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

/// Luma PSNR (dB) above which the SSIM floor no longer lowers the QP.
const FLOOR_PSNR_CAP: f64 = 45.0;

/// Bring the half-sample planes `hp` of reference luma `r` up to date in the
/// macroblocks flagged in `dirty` (and the filter support around them).
fn update_hpel(hp: &mut crate::mc::HpelPlanes, r: &crate::frame::Plane, dirty: &[bool], mbw: usize, mbh: usize) {
    let (p, s, rows) = (r.pad, r.stride, r.data.len() / r.stride);
    // Runs of set MBs in `mask` (one MB row) → plane columns.
    let runs = |mask: &dyn Fn(usize) -> bool, f: &mut dyn FnMut(usize, usize)| {
        let mut x = 0;
        while x < mbw {
            if !mask(x) {
                x += 1;
                continue;
            }
            let x0 = x;
            // a one-macroblock gap costs less to filter through than a
            // second run (rounding and dilation at both ends)
            while x < mbw && (mask(x) || (x + 1 < mbw && mask(x + 1))) {
                x += 1;
            }
            f(if x0 == 0 { 0 } else { p + x0 * 16 }, if x == mbw { s } else { p + x * 16 });
        }
    };
    // Horizontal filter over the changed MBs, then the vertical/centre
    // filters over rows whose 6-tap support reaches a changed MB row:
    // rows 0-1 of an MB row also see the row above, rows 13-15 the row below.
    for my in 0..mbh {
        let lo = if my == 0 { 0 } else { p + my * 16 };
        let hi = if my == mbh - 1 { rows } else { p + my * 16 + 16 };
        let d = |x: usize| dirty[my * mbw + x];
        runs(&d, &mut |xa, xb| hp.filter(r, xa, xb, lo, hi, 0, 0));
    }
    for my in 0..mbh {
        let lo = if my == 0 { 0 } else { p + my * 16 };
        let hi = if my == mbh - 1 { rows } else { p + my * 16 + 16 };
        let (m2, m13) = (p + my * 16 + 2, p + my * 16 + 13);
        let d = |x: usize| dirty[my * mbw + x];
        let up = |x: usize| d(x) || (my > 0 && dirty[(my - 1) * mbw + x]);
        let dn = |x: usize| d(x) || (my + 1 < mbh && dirty[(my + 1) * mbw + x]);
        runs(&up, &mut |xa, xb| hp.filter(r, xa, xb, 0, 0, lo, m2));
        runs(&d, &mut |xa, xb| hp.filter(r, xa, xb, 0, 0, m2, m13));
        runs(&dn, &mut |xa, xb| hp.filter(r, xa, xb, 0, 0, m13, hi));
    }
}

/// Mean SSIM (and PSNR) of the luma planes over 8×8 blocks on a 2×2-subsampled block grid
/// (enough for rate decisions; not the reporting metric). `a` is the source;
/// with a grain hint, the expected grain variance is taken off its variance
/// (the clean signal's), and each block's value is capped at 1.
fn ssim_luma(a: &crate::frame::Plane, b: &crate::frame::Plane, w: usize, h: usize, grain: f32) -> (f64, f64) {
    let g = grain as f64;
    let mut sse = 0u64;
    const C1: f64 = 6.5025; // (0.01·255)²
    const C2: f64 = 58.5225; // (0.03·255)²
    let (mut sum, mut n) = (0f64, 0u32);
    for by in (0..h / 8).step_by(2) {
        for bx in (0..w / 8).step_by(2) {
            let (oa, ob) = (a.off(bx as isize * 8, by as isize * 8), b.off(bx as isize * 8, by as isize * 8));
            let [sa, sb, saa, sbb, sab] = crate::simd::ssim_stats8(&a.data[oa..], a.stride, &b.data[ob..], b.stride);
            sse += saa as u64 + sbb as u64 - 2 * sab as u64;
            let (ma, mb) = (sa as f64 / 64.0, sb as f64 / 64.0);
            let mut va = saa as f64 / 64.0 - ma * ma;
            if g > 0.0 {
                let l = ((ma - 16.0) / 219.0).clamp(0.0, 1.0);
                let s = g * (0.35 + 2.6 * l * (1.0 - l));
                va = (va - s * s).max(0.0);
            }
            let vb = sbb as f64 / 64.0 - mb * mb;
            let cab = sab as f64 / 64.0 - ma * mb;
            sum += (((2.0 * ma * mb + C1) * (2.0 * cab + C2)) / ((ma * ma + mb * mb + C1) * (va + vb + C2))).min(1.0);
            n += 1;
        }
    }
    if n == 0 {
        (1.0, 99.0)
    } else {
        let mse = sse as f64 / (64 * n) as f64;
        (sum / n as f64, 10.0 * (65025.0 / mse.max(1e-3)).log10())
    }
}

/// Adaptive-quantisation QP offset of a macroblock. One-sided: only flat, dark
/// macroblocks (where blocking and banding show once shadows are lifted) get a
/// lower QP; busy or bright ones keep the frame QP, so text and detail lose nothing.
fn aq_offset(y: &[u8; 256], _c: &[[u8; 64]; 2], strength: f32) -> (i32, bool) {
    // Smooth: small second differences. A ramp of any slope (a fade, a lit
    // floor) scores ~0, a ±1 dither or denoiser noise ~1 per sample, edges and
    // texture far more. Their block means quantise in visible steps at QP 23.
    let hf = crate::simd::second_diff16(y);
    if hf >= SMOOTH_HF {
        return (0, false);
    }
    // dark: mean luma under 80 (limited range; black is 16), a little more
    const DARK: u32 = 80;
    let (sy, _) = crate::simd::sum_sq(y);
    let smooth = ((SMOOTH_HF as f32).log2() - (hf.max(1) as f32).log2()).min(4.0);
    let mean = (sy >> 8) as f32;
    let dark = ((DARK as f32 - mean).max(0.0) / 16.0).min(3.0);
    // a smooth macroblock starts at −4: half the quantiser step for its DC
    (-(strength * (4.0 + smooth + dark)).round() as i32, true)
}

/// mb_qp_delta: unary of the mapped value, contexts 60 + (previous MB coded a
/// nonzero delta), 62, then 63.
fn cabac_dqp(cb: &mut crate::cabac::Cabac, w: &mut BitWriter, d: i32, prev_nz: bool) {
    let k = if d > 0 { 2 * d - 1 } else { -2 * d } as u32;
    cb.decision(w, 60 + prev_nz as usize, (k > 0) as u32);
    for i in 1..=k {
        cb.decision(w, if i == 1 { 62 } else { 63 }, (i < k) as u32);
    }
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
            if cd.nz[by * 4 + bx] == 0 {
                cb.residual_empty(w, Cat::LumaAc, luma_inc(bx, by));
                continue;
            }
            for j in 0..15 {
                scan[j] = blk[ZIGZAG[j + 1]] as i32;
            }
            cb.residual(w, &scan[..15], Cat::LumaAc, luma_inc(bx, by));
        } else if cd.t8 {
            if k % 4 == 0 && cd.cbp & (1 << (k / 4)) != 0 {
                t8::cabac_block8(cb, w, &cd.l8[k / 4]);
            }
        } else {
            if cd.cbp & (1 << (k / 4)) == 0 {
                continue;
            }
            if cd.nz[by * 4 + bx] == 0 {
                cb.residual_empty(w, Cat::Luma4x4, luma_inc(bx, by));
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
                if cd.nzc[comp][blk] == 0 {
                    cb.residual_empty(w, Cat::ChromaAc, ia + 2 * ib);
                    continue;
                }
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
multiversion!(luma_i16 => luma_i16_impl(src: &[u8; 256], pred: &[u8; 256], q: &Quant, qs: &QuantS, dc_max: i32, cd: &mut Coded, recon: &mut [u8; 256]));
multiversion!(chroma_resid => chroma_resid_impl(src: &[[u8; 64]; 2], pred: &[[u8; 64]; 2], qc: &Quant, qcs: &QuantS, intra: bool, dc_max: i32, cd: &mut Coded, recon: &mut [[u8; 64]; 2]));

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
fn luma_i16_impl(src: &[u8; 256], pred: &[u8; 256], q: &Quant, qs: &QuantS, dc_max: i32, cd: &mut Coded, recon: &mut [u8; 256]) {
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
    let dcl = luma_dc_forward(&dc, q).map(|v| v.clamp(-dc_max, dc_max));
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
    dc_max: i32,
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
        let l = chroma_dc_forward(&dc[comp], qc, intra).map(|v| v.clamp(-dc_max, dc_max));
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
            qp_delta: c.qp as i32 - c.base_qp as i32,
            b: c.bslice,
            poc: c.poc,
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
        p: &c.qsets[c.qp as usize],
        mb_qp: c.qp,
        qp_pred: c.qp,
        dqp_nz: false,
        dqp_prev_nz: false,
        rsel: 0,
        seen: vec![std::cell::Cell::new((0, (0, 0))); 1024],
        seen_gen: std::cell::Cell::new(0),
        extra_cand: None,
        no_rings: false,
    };
    if c.cabac {
        while !s.w.byte_aligned() {
            s.w.put(1, 1); // cabac_alignment_one_bit
        }
        s.cabac = Some(crate::cabac::Cabac::new(c.qp, !c.idr));
    }
    let sy = &c.src.y;
    let su = &c.src.u;
    let sv = &c.src.v;
    let mut src_y = [0u8; 256];
    let mut src_c = [[0u8; 64]; 2];
    for mby in mby0..mby1 {
        for mbx in 0..c.mbw {
            s.dqp_prev_nz = s.dqp_nz;
            s.dqp_nz = false;
            if c.bslice && !c.changed[mby * c.mbw + mbx] {
                s.code_b_static(mbx, mby);
                s.b.info[(mby - mby0) * c.mbw + mbx].qp = s.qp_pred;
                continue;
            }
            if !c.idr && !c.changed[mby * c.mbw + mbx] {
                // Cheap P_Skip test: the skip MV is zero when A or B is missing or
                // an inter neighbour with MV 0.
                let zero = |i: &MbInfo, q: usize| i.kind == 0 && i.qmv(q) == (0, 0);
                if mbx == 0 || mby == mby0 || zero(s.info(mbx - 1, mby), 1) || zero(s.info(mbx, mby - 1), 2) {
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
                s.b.info[(mby - mby0) * c.mbw + mbx].qp = s.qp_pred;
                continue;
            }
            let o = sy.off(mbx as isize * 16, mby as isize * 16);
            crate::simd::copy_block::<16>(&mut src_y, 0, 16, &sy.data, o, sy.stride, 16);
            let o = su.off(mbx as isize * 8, mby as isize * 8);
            crate::simd::copy_block::<8>(&mut src_c[0], 0, 8, &su.data, o, su.stride, 8);
            crate::simd::copy_block::<8>(&mut src_c[1], 0, 8, &sv.data, o, sv.stride, 8);
            let b0 = s.w.bits_written();
            let (i0, n0) = (s.stats.intra16 + s.stats.intra4, s.stats.inter);
            if c.bslice {
                s.encode_mb_b(mbx, mby, &src_y, &src_c);
            } else {
                s.encode_mb(mbx, mby, &src_y, &src_c);
            }
            s.b.info[(mby - mby0) * c.mbw + mbx].qp = s.qp_pred;
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
        crate::deblock::deblock_rows(&mut view, b.info, c.mbw, mby1 - mby0, b.touched);
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
    qsets: Vec<QpSet>,
    qsets_smooth: Vec<QpSet>,
    /// Frame QP offset from the SSIM floor (≤ 0).
    qp_adj: i32,
    /// Grain hint of the frame being coded.
    grain: f32,
    level: u8,
    cur: Pic,
    /// Macroblocks where `recon` (the picture before the reference) differs from `refp`.
    stale: Vec<bool>,
    touched: Vec<bool>,
    hpel: Option<crate::mc::HpelPlanes>,
    /// Macroblocks of the reference changed since `hpel` was last updated.
    hpel_dirty: Vec<bool>,
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
    /// B-frames: consecutive B pictures (0 = off), frames held for them
    /// (buffer, display index in the GOP, input index), spare buffers, the
    /// change map since the last reference, and the B picture's state.
    bf: usize,
    /// 8×8 transform on (High profile).
    t8: bool,
    pend: Vec<Held>,

    pend_pool: Vec<crate::frame::YuvBuffer>,
    acc: Option<Vec<bool>>,
    src_prev: Pic,
    bsrc: Pic,
    brecon: Pic,
    binfo: Vec<MbInfo>,
    btouched: Vec<bool>,
    bchanged: Vec<bool>,
    bhint: Vec<(i16, i16)>,
    hpel1: Option<crate::mc::HpelPlanes>,
    hpel1_dirty: Vec<bool>,
    /// The last P picture was coded (false: all-skip, the reference is unchanged).
    last_p_swapped: bool,
    /// frame_num of the next reference picture; POC of the picture being coded.
    ref_num: u32,
    /// Display indices of the previous and the latest reference picture.
    ref_disp: u64,
    /// SSIM estimate of the latest P picture (B pictures' quality guard), and
    /// the QP offset the next B picture starts at.
    p_ssim: f64,
    b_off: i32,
    p_disp: u64,
    cur_poc: Option<u32>,
    frames_in: u64,
    keep_recons: bool,
    recons: Vec<(u64, bool, crate::frame::YuvBuffer)>,
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
        let qsets = (0..52).map(QpSet::new).collect();
        let qsets_smooth = (0..52).map(|q| QpSet::build(q, true)).collect();
        let level = level_idc(mbw, mbh, cfg.fps);
        let threads = cfg.threads.clamp(1, mbh);
        Encoder {
            mbw,
            mbh,
            qsets,
            qsets_smooth,
            qp_adj: 0,
            grain: 0.0,
            level,
            cur: Pic::new(mbw, mbh, 0),
            stale: vec![true; mbw * mbh],
            touched: vec![false; mbw * mbh],
            hpel: None,
            hpel_dirty: vec![true; mbw * mbh],
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
            bf: if cfg.cabac && threads == 1 { cfg.bframes as usize } else { 0 },
            t8: cfg.cabac && cfg.transform8,
            pend: Vec::new(),

            pend_pool: Vec::new(),
            acc: None,
            src_prev: Pic::new(mbw, mbh, 0),
            bsrc: Pic::new(mbw, mbh, 0),
            brecon: Pic::new(mbw, mbh, PAD),
            binfo: vec![MbInfo::default(); mbw * mbh],
            btouched: vec![false; mbw * mbh],
            bchanged: vec![true; mbw * mbh],
            bhint: vec![NO_HINT; mbw * mbh],
            hpel1: None,
            hpel1_dirty: vec![true; mbw * mbh],
            last_p_swapped: true,
            ref_num: 0,
            ref_disp: 0,
            p_disp: 0,
            p_ssim: 0.0,
            b_off: B_QP_OFFSET,
            cur_poc: None,
            frames_in: 0,
            keep_recons: false,
            recons: Vec::new(),
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
        pic_to_yuv(&self.refp, self.cfg.width as usize, self.cfg.height as usize)
    }

    /// Keep a copy of every coded picture's reconstruction (B pictures without
    /// deblocking), for [`Encoder::take_recons`]. Testing aid.
    #[doc(hidden)]
    pub fn keep_recons(&mut self, on: bool) {
        self.keep_recons = on;
    }

    /// Reconstructions kept since the last call, as (input frame index,
    /// reference picture, picture), in decoding order.
    #[doc(hidden)]
    pub fn take_recons(&mut self) -> Vec<(u64, bool, crate::frame::YuvBuffer)> {
        std::mem::take(&mut self.recons)
    }
}

fn pic_to_yuv(p: &Pic, w: usize, h: usize) -> crate::frame::YuvBuffer {
    {
        let mut b = crate::frame::YuvBuffer::new(w as u32, h as u32);
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
}

impl Encoder {
    /// Encode one frame; returns the Annex-B bytes that are ready (SPS/PPS
    /// precede every IDR). Without B-frames that is this frame. With B-frames,
    /// frames that will be B pictures are held until the next P picture is
    /// coded, so a call may return nothing or several frames (in decoding
    /// order); [`Encoder::flush`] codes what is held at the end of the stream.
    pub fn encode(&mut self, f: &YuvFrame, hints: &FrameHints) -> &[u8] {
        assert_eq!((f.width, f.height), (self.cfg.width, self.cfg.height), "ak264: frame size mismatch");
        self.out.clear();
        let gidx = self.frames_in;
        self.frames_in += 1;
        let idr = self.frame_in_gop == 0 || self.cfg.gop <= 1 || self.frame_in_gop >= self.cfg.gop as u64;
        if self.bf == 0 {
            if idr {
                self.frame_in_gop = 0;
            }
            let disp = self.frame_in_gop;
            self.code_frame(f, hints, idr, disp, gidx);
            self.frame_in_gop += 1;
            return &self.out;
        }
        if idr {
            self.flush_pending();
            self.frame_in_gop = 0;
            self.code_frame(f, hints, true, 0, gidx);
            self.acc_reset();
            self.frame_in_gop += 1;
            return &self.out;
        }
        let disp = self.frame_in_gop;
        self.frame_in_gop += 1;
        if self.pend.len() < self.bf {
            self.acc_merge(hints);
            let mut b = self.pend_pool.pop().unwrap_or_else(|| crate::frame::YuvBuffer::new(self.cfg.width, self.cfg.height));
            b.copy_from(f);
            let n = self.mbw * self.mbh;
            let changed = hints.changed.clone().filter(|c| c.len() == n);
            self.pend.push(Held { buf: b, disp, gidx, changed, grain: hints.grain });
            return &self.out;
        }
        if self.b_pays(Some(hints.grain)) {
            self.acc_merge(hints);
            let h = FrameHints { changed: self.acc.clone(), motions: Vec::new(), grain: hints.grain };
            self.src_prev.clone_from(&self.cur);
            self.code_frame(f, &h, false, disp, gidx);
            self.acc_reset();
            self.code_pending_b(hints.grain);
        } else {
            self.code_pending_p();
            self.code_frame(f, hints, false, disp, gidx);
            self.acc_reset();
        }
        &self.out
    }

    /// Code the held frames (and the next frame, with grain hint `next`) as B
    /// pictures before a P picture, or as a chain of P pictures? B pictures
    /// unless the frames carry film grain: re-drawn grain is unpredictable, and
    /// a B picture's higher QP smooths it away.
    fn b_pays(&self, next: Option<f32>) -> bool {
        self.pend.len() + next.is_some() as usize >= 2 && self.pend.iter().all(|h| h.grain <= 0.0) && next.is_none_or(|g| g <= 0.0)
    }

    /// Code the held frames as a chain of P pictures.
    fn code_pending_p(&mut self) {
        let pend = std::mem::take(&mut self.pend);
        for h in pend {
            let hints = FrameHints { changed: h.changed, grain: h.grain, ..Default::default() };
            self.code_frame(&h.buf.frame(), &hints, false, h.disp, h.gidx);
            self.pend_pool.push(h.buf);
        }
        self.acc_reset();
    }

    /// Code the frames held for B pictures (the last one as a P picture) and
    /// return their bytes. Call at the end of a stream when B-frames are on.
    pub fn flush(&mut self) -> &[u8] {
        self.out.clear();
        self.flush_pending();
        &self.out
    }

    fn flush_pending(&mut self) {
        if !self.b_pays(None) {
            self.code_pending_p();
            return;
        }
        let last = self.pend.pop().unwrap();
        let h = FrameHints { changed: self.acc.clone(), ..Default::default() };
        self.src_prev.clone_from(&self.cur);
        self.code_frame(&last.buf.frame(), &h, false, last.disp, last.gidx);
        self.pend_pool.push(last.buf);
        self.acc_reset();
        self.code_pending_b(0.0);
    }

    /// Change map since the last reference picture: nothing changed yet.
    fn acc_reset(&mut self) {
        let n = self.mbw * self.mbh;
        match &mut self.acc {
            Some(a) if a.len() == n => a.fill(false),
            _ => self.acc = Some(vec![false; n]),
        }
    }

    /// OR a frame's change map into the map since the last reference (no map:
    /// everything may have changed).
    fn acc_merge(&mut self, hints: &FrameHints) {
        let n = self.mbw * self.mbh;
        match (&mut self.acc, hints.changed.as_ref().filter(|c| c.len() == n)) {
            (Some(a), Some(c)) => {
                for (x, &y) in a.iter_mut().zip(c) {
                    *x |= y;
                }
            }
            _ => self.acc = None,
        }
    }

    /// Code the held frames as B pictures between the previous reference
    /// (`recon` after the swap, or `refp` if the P picture was all-skip) and
    /// the P picture just coded (`refp`).
    fn code_pending_b(&mut self, grain: f32) {
        let swapped = self.last_p_swapped;
        if self.cfg.subpel {
            let r = &self.refp.y;
            if swapped {
                // L0 planes (`hpel`) are the previous reference's; `hpel1` (an
                // older picture's, or new) is brought up to the P picture with
                // its own change map.
                if self.hpel1.is_none() {
                    self.hpel1 = Some(crate::mc::HpelPlanes::new(r));
                    self.hpel1_dirty.fill(true);
                }
                update_hpel(self.hpel1.as_mut().unwrap(), r, &self.hpel1_dirty, self.mbw, self.mbh);
                self.hpel1_dirty.fill(false);
            } else {
                let hp = self.hpel.get_or_insert_with(|| crate::mc::HpelPlanes::new(r));
                update_hpel(hp, r, &self.hpel_dirty, self.mbw, self.mbh);
                self.hpel_dirty.fill(false);
            }
        }
        let pend = std::mem::take(&mut self.pend);
        for h in pend {
            self.code_b(&h.buf, h.disp, h.gidx, swapped, grain);
            self.pend_pool.push(h.buf);
        }
        if self.cfg.subpel && swapped {
            std::mem::swap(&mut self.hpel, &mut self.hpel1);
            std::mem::swap(&mut self.hpel_dirty, &mut self.hpel1_dirty);
        }
    }

    /// Code one B picture (not a reference; its reconstruction is not deblocked).
    fn code_b(&mut self, src: &crate::frame::YuvBuffer, disp: u64, gidx: u64, swapped: bool, grain: f32) {
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let (cw, ch) = (w / 2, h / 2);
        self.bsrc.y.load(&src.y, w, w, h);
        self.bsrc.u.load(&src.u, cw, cw, ch);
        self.bsrc.v.load(&src.v, cw, cw, ch);
        self.grain = grain;
        // Macroblocks identical in this picture and both references' sources.
        let (mbw, mbh) = (self.mbw, self.mbh);
        for my in 0..mbh {
            for mx in 0..mbw {
                let i = my * mbw + mx;
                self.bchanged[i] = self.changed[i] || !mb_equal(&self.bsrc, &self.src_prev, mx, my);
            }
        }
        let mut stats = FrameStats::default();
        // B pictures use a higher QP, unless that leaves this one visibly worse
        // than the P picture after it: then it is coded again at the P QP.
        let mut off = self.b_off;
        let res = loop {
            let ctx = FrameCtx {
                mbw: self.mbw,
                qp: (self.cfg.qp as i32 + self.qp_adj + off).clamp(0, 51) as u8,
                base_qp: self.cfg.qp,
                qsets: &self.qsets,
                qsets_smooth: &self.qsets_smooth,
                aq: self.aq_for_frame(),
                src: &self.bsrc,
                refp: if swapped { &self.recon } else { &self.refp },
                refp1: &self.refp,
                bslice: true,
                col: &self.info,
                bdist: ((disp - self.ref_disp) as i32, (self.p_disp - self.ref_disp) as i32),
                changed: &self.bchanged,
                stale: &self.stale,
                hint: &self.bhint,
                idr: false,
                max_mvy: max_mv_y(self.level),
                subpel: self.cfg.subpel,
                skip_t: 0,
                hpel: if self.cfg.subpel { self.hpel.as_ref() } else { None },
                hpel1: if self.cfg.subpel { if swapped { self.hpel1.as_ref() } else { self.hpel.as_ref() } } else { None },
                cabac: true,
                partitions: false,
                t8: self.t8,
                hdr_frame_num: self.ref_num,
                poc: Some((2 * disp) as u32),
                idr_id: 0,
                deblock: self.cfg.deblock,
                slice_deblock: false,
            };
            let r = &mut self.brecon;
            let (ys, yp) = (r.y.stride, r.y.pad);
            let (cs, cp) = (r.u.stride, r.u.pad);
            let band = Band {
                y: &mut r.y.data[yp * ys..],
                u: &mut r.u.data[cp * cs..],
                v: &mut r.v.data[cp * cs..],
                info: &mut self.binfo,
                touched: &mut self.btouched,
                mby0: 0,
                mby1: self.mbh,
            };
            let res = encode_slice(&ctx, band, &mut self.writers[0]);
            if off > 0 && self.p_ssim > 0.0 {
                let (sb, _) = ssim_luma(&self.bsrc.y, &self.brecon.y, w, h, grain);
                if sb < self.p_ssim - B_SSIM_SLACK {
                    // next B pictures start lower; this one goes to the P QP
                    self.b_off = (off - 1).max(0);
                    off = 0;
                    continue;
                }
                if sb >= self.p_ssim {
                    self.b_off = (self.b_off + 1).min(B_QP_OFFSET);
                }
            }
            break res;
        };
        let out0 = self.out.len();
        write_nal(&mut self.out, 0, 1, &self.writers[0].buf);
        stats.skip = res.skip;
        stats.inter = res.inter;
        stats.intra16 = res.intra16;
        stats.intra4 = res.intra4;
        stats.changed = res.changed;
        stats.bytes = self.out.len() - out0;
        self.last = stats;
        if self.keep_recons {
            let b = pic_to_yuv(&self.brecon, w, h);
            self.recons.push((gidx, false, b));
        }
    }

    /// Code one I or P picture.
    fn code_frame(&mut self, f: &YuvFrame, hints: &FrameHints, idr: bool, disp: u64, gidx: u64) {
        let (w, h) = (f.width as usize, f.height as usize);
        let (cw, ch) = (w / 2, h / 2);
        if idr {
            self.ref_num = 0;
        }
        self.cur_poc = if self.bf > 0 { Some((2 * disp) as u32) } else { None };
        self.ref_disp = if idr { disp } else { self.p_disp };
        self.p_disp = disp;
        let out0 = self.out.len();
        self.grain = hints.grain.max(0.0);
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
            // A change map of the wrong length is a caller bug; compare every
            // macroblock instead of failing the encode.
            debug_assert!(hints.changed.as_ref().is_none_or(|c| c.len() == nmb), "ak264: changed map must have one entry per macroblock");
            match hints.changed.as_ref().filter(|c| c.len() == nmb) {
                Some(c) => {
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
                    bframes: self.bf > 0,
                    high: self.t8,
                },
                self.level,
            );
            write_nal(&mut self.out, 3, 7, &self.hdr.buf);
            self.hdr.clear();
            write_pps(&mut self.hdr, self.cfg.qp, self.cfg.cabac, self.t8);
            write_nal(&mut self.out, 3, 8, &self.hdr.buf);
        }
        let frame_num = self.ref_num;
        self.ref_num = (self.ref_num + 1) % (1 << LOG2_MAX_FRAME_NUM);
        self.last_p_swapped = any_changed;
        if !any_changed {
            // All-skip P slice; the reference is unchanged.
            let cabac = self.cfg.cabac;
            let w = &mut self.writers[0];
            w.clear();
            write_slice_header(
                w,
                &SliceHdr {
                    first_mb: 0,
                    idr: false,
                    frame_num,
                    idr_id: 0,
                    deblock: self.cfg.deblock,
                    slice_edges: false,
                    cabac,
                    qp_delta: 0,
                    b: false,
                    poc: self.cur_poc,
                },
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
            let mark = self.out.len();
            self.encode_slices(idr, frame_num, &mut stats);
            if self.cfg.deblock && self.writers.len() == 1 {
                crate::deblock::deblock(&mut self.recon, &self.info, self.mbw, self.mbh, &mut self.touched);
            }
            if idr && self.cfg.ssim_floor > 0.0 {
                // An IDR has no history to steer by: measure it, and if it is
                // below the floor code it once more at the estimated QP.
                let s = ssim_luma(&self.cur.y, &self.recon.y, w, h, self.grain);
                let old = self.qp_adj;
                self.floor_update(s, 200.0, 12);
                if self.qp_adj < old {
                    self.out.truncate(mark);
                    stats = FrameStats { idr, ..Default::default() };
                    self.encode_slices(idr, frame_num, &mut stats);
                    if self.cfg.deblock && self.writers.len() == 1 {
                        crate::deblock::deblock(&mut self.recon, &self.info, self.mbw, self.mbh, &mut self.touched);
                    }
                }
            }
            for ((st, inf), &t) in self.stale.iter_mut().zip(&self.info).zip(&self.touched) {
                *st = inf.coded || t;
            }
            self.recon.extend();
            std::mem::swap(&mut self.recon, &mut self.refp);
            if self.cfg.ssim_floor > 0.0 || self.bf > 0 {
                let s = ssim_luma(&self.cur.y, &self.refp.y, w, h, self.grain);
                self.p_ssim = s.0;
                if self.cfg.ssim_floor > 0.0 {
                    self.floor_update(s, 400.0, 4);
                }
            }
            if self.cfg.subpel {
                // only where the reconstruction differs from the old reference
                for (((d, d1), inf), &t) in self.hpel_dirty.iter_mut().zip(self.hpel1_dirty.iter_mut()).zip(&self.info).zip(&self.touched) {
                    let c = t || (inf.coded && (inf.kind != 0 || inf.mv != (0, 0) || inf.part != 0 || inf.cbp != 0));
                    *d |= c;
                    *d1 |= c;
                }
            }
        }
        if idr {
            self.idr_count = self.idr_count.wrapping_add(1);
        }
        let bytes = self.out.len() - out0;
        if self.bf > 0 && !any_changed {
            // all-skip P picture: the reference is the previous one
            self.p_ssim = ssim_luma(&self.cur.y, &self.refp.y, w, h, self.grain).0;
        }
        stats.bytes = bytes;
        self.last = stats;
        if self.keep_recons {
            let b = pic_to_yuv(&self.refp, w, h);
            self.recons.push((gidx, true, b));
        }
    }

    /// Moves the frame QP offset towards the SSIM floor (only while the
    /// frame's luma PSNR is below `FLOOR_PSNR_CAP`) after a frame measured
    /// `s`: down by the shortfall × `gain` (1..=`max_step`), up by one when
    /// there is margin; never above the configured QP or below it by more than
    /// `ssim_max_drop`.
    fn floor_update(&mut self, (s, psnr): (f64, f64), gain: f64, max_step: i32) {
        // + 0.002: this estimate (8×8 blocks, every other one) reads about that
        // much higher than the usual overlapping-window SSIM
        let f = self.cfg.ssim_floor as f64 + 0.002;
        // Above ~45 dB the remaining error is not visible, however low SSIM
        // reads on fine texture: don't spend bits there (e.g. --crf 18).
        if s < f && psnr < FLOOR_PSNR_CAP {
            self.qp_adj -= ((f - s) * gain).ceil().clamp(1.0, max_step as f64) as i32;
        } else if s > f + 0.004 {
            self.qp_adj += 1;
        }
        self.qp_adj = self.qp_adj.clamp(-(self.cfg.ssim_max_drop as i32), 0);
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

    /// AQ strength for the current frame. `aq_strength ≥ 0` is used as given.
    /// Negative means automatic: |aq_strength| when the frame is dominated by
    /// smooth, textured content (renders of 3D scenes, video, soft photographic
    /// gradients: second differences between 0.25 and 2 per sample on at least
    /// 60% of a 4×4-subsampled macroblock grid), else 0. Decided from the
    /// frame's own samples only, so it is deterministic and independent of
    /// chunking and thread count.
    fn aq_for_frame(&self) -> f32 {
        let a = self.cfg.aq_strength;
        if a >= 0.0 {
            return a;
        }
        let y = &self.cur.y;
        let (mut smooth, mut n) = (0u32, 0u32);
        let mut blk = [0u8; 256];
        for my in (1..self.mbh).step_by(4) {
            for mx in (1..self.mbw).step_by(4) {
                let o = y.off(mx as isize * 16, my as isize * 16);
                crate::simd::copy_block::<16>(&mut blk, 0, 16, &y.data, o, y.stride, 16);
                let hf = crate::simd::second_diff16(&blk);
                smooth += (SMOOTH_HF / 8..SMOOTH_HF).contains(&hf) as u32;
                n += 1;
            }
        }
        if smooth * 10 >= n * 6 {
            -a
        } else {
            0.0
        }
    }

    fn encode_slices(&mut self, idr: bool, frame_num: u32, stats: &mut FrameStats) {
        // Sub-pel refinement: keep the half-sample planes of the reference up
        // to date, recomputing only the MB rows that changed.
        if self.cfg.subpel && !idr && self.hpel_dirty.iter().any(|&d| d) {
            let r = &self.refp.y;
            let hp = self.hpel.get_or_insert_with(|| crate::mc::HpelPlanes::new(r));
            update_hpel(hp, r, &self.hpel_dirty, self.mbw, self.mbh);
            self.hpel_dirty.fill(false);
        }
        let ctx = FrameCtx {
            mbw: self.mbw,
            qp: (self.cfg.qp as i32 + self.qp_adj).clamp(0, 51) as u8,
            base_qp: self.cfg.qp,
            qsets: &self.qsets,
            qsets_smooth: &self.qsets_smooth,
            aq: self.aq_for_frame(),
            src: &self.cur,
            refp: &self.refp,
            refp1: &self.refp,
            bslice: false,
            col: &[],
            bdist: (1, 1),
            changed: &self.changed,
            stale: &self.stale,
            hint: &self.hint,
            idr,
            max_mvy: max_mv_y(self.level),
            subpel: self.cfg.subpel,
            skip_t: self.cfg.skip_threshold,
            hpel: if self.cfg.subpel && !idr { self.hpel.as_ref() } else { None },
            hpel1: None,
            cabac: self.cfg.cabac,
            partitions: self.cfg.partitions && self.cfg.cabac,
            t8: self.t8,
            hdr_frame_num: frame_num,
            poc: self.cur_poc,
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

