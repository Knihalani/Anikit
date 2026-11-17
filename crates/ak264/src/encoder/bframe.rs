//! B slices: non-reference B pictures with 16×16 L0 / L1 / bi-predicted
//! macroblocks, spatial direct prediction (B_Direct_16x16 and B_Skip) and intra
//! 16×16. CABAC only.

use super::*;
use super::t8;

/// Prediction of a B macroblock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum BMode {
    Direct,
    L0,
    L1,
    Bi,
}

#[inline(always)]
fn avg_into(a: &mut [u8], b: &[u8]) {
    for (x, &y) in a.iter_mut().zip(b) {
        *x = ((*x as u16 + y as u16 + 1) >> 1) as u8;
    }
}

impl<'a, 'b> SliceEnc<'a, 'b> {
    /// 16×16 motion vector predictor in list `l` (8.4.1.3, refIdx 0).
    pub(super) fn mvp_l(&self, mbx: usize, mby: usize, l: usize) -> (i32, i32) {
        let (x, y) = (mbx as isize, mby as isize);
        let a = self.nbl(x - 1, y, l, 1);
        let mut b = self.nbl(x, y - 1, l, 2);
        let mut c = self.nbl(x + 1, y - 1, l, 2);
        if !c.avail {
            c = self.nbl(x - 1, y - 1, l, 3);
        }
        if !b.avail && !c.avail && a.avail {
            b = a;
            c = a;
        }
        let n = a.inter as u32 + b.inter as u32 + c.inter as u32;
        if n == 1 {
            if a.inter {
                a.mv
            } else if b.inter {
                b.mv
            } else {
                c.mv
            }
        } else {
            (median(a.mv.0, b.mv.0, c.mv.0), median(a.mv.1, b.mv.1, c.mv.1))
        }
    }

    /// Spatial direct prediction (8.4.1.2.2) of the macroblock: lists used and
    /// the L0 / L1 motion vectors. The co-located macroblock (in the L1
    /// reference, a P picture of 16×16 partitions) is uniform, so one result
    /// holds for all four 8×8 blocks.
    pub(super) fn direct_spatial(&self, mbx: usize, mby: usize) -> (u8, (i32, i32), (i32, i32)) {
        let (x, y) = (mbx as isize, mby as isize);
        let mut refs = 0u8;
        for l in 0..2 {
            let a = self.nbl(x - 1, y, l, 1);
            let b = self.nbl(x, y - 1, l, 2);
            let mut c = self.nbl(x + 1, y - 1, l, 2);
            if !c.avail {
                c = self.nbl(x - 1, y - 1, l, 3);
            }
            if a.inter || b.inter || c.inter {
                refs |= 1 << l;
            }
        }
        if refs == 0 {
            return (3, (0, 0), (0, 0));
        }
        let col = &self.c.col[mby * self.c.mbw + mbx];
        let col_zero = col.kind == 0 && col.mv.0.abs() <= 1 && col.mv.1.abs() <= 1;
        let mut mv = [(0, 0); 2];
        for (l, m) in mv.iter_mut().enumerate() {
            if (refs >> l) & 1 != 0 && !col_zero {
                *m = self.mvp_l(mbx, mby, l);
            }
        }
        (refs, mv[0], mv[1])
    }

    /// Can a 16×16 prediction at `mv` be formed from the padded reference?
    pub(super) fn mv_in_range(&self, mbx: usize, mby: usize, mv: (i32, i32)) -> bool {
        let c = self.c;
        let w16 = (c.mbw * 16) as i32 * 4;
        let h16 = c.refp.y.h as i32 * 4;
        let (bx, by) = (mbx as i32 * 64 + mv.0, mby as i32 * 64 + mv.1);
        !(bx < -64 || bx > w16 || by < -64 || by > h16 || mv.1.abs() >= c.max_mvy * 4)
    }

    /// Luma and chroma prediction from list `l` at `mv`.
    fn pred_list(&mut self, l: usize, mbx: usize, mby: usize, mv: (i32, i32), y: &mut [u8; 256], cc: &mut [[u8; 64]; 2]) {
        let r = if l == 0 { self.c.refp } else { self.c.refp1 };
        let hp = if l == 0 { self.c.hpel } else { self.c.hpel1 };
        let (x0, y0) = (mbx as isize * 16, mby as isize * 16);
        match hp {
            Some(hp) => hp.pred(&r.y, x0 + (mv.0 >> 2) as isize, y0 + (mv.1 >> 2) as isize, mv.0 & 3, mv.1 & 3, y),
            None => crate::mc::luma_pred(&r.y, x0, y0, mv, y),
        }
        let s = self.rsel;
        self.rsel = l;
        self.mc_chroma(0, mbx, mby, mv, &mut cc[0]);
        self.mc_chroma(1, mbx, mby, mv, &mut cc[1]);
        self.rsel = s;
    }

    /// Prediction of a macroblock using lists `refs` (bi-prediction averages).
    pub(super) fn pred_b(&mut self, refs: u8, mbx: usize, mby: usize, mv0: (i32, i32), mv1: (i32, i32), y: &mut [u8; 256], cc: &mut [[u8; 64]; 2]) {
        match refs {
            1 => self.pred_list(0, mbx, mby, mv0, y, cc),
            2 => self.pred_list(1, mbx, mby, mv1, y, cc),
            _ => {
                self.pred_list(0, mbx, mby, mv0, y, cc);
                let mut y1 = [0u8; 256];
                let mut c1 = [[0u8; 64]; 2];
                self.pred_list(1, mbx, mby, mv1, &mut y1, &mut c1);
                avg_into(y, &y1);
                avg_into(&mut cc[0], &c1[0]);
                avg_into(&mut cc[1], &c1[1]);
            }
        }
    }

    /// A macroblock identical in the B picture and both references' sources:
    /// B_Skip when direct prediction is the co-located block, else a 16×16
    /// copy (MV 0) from the reference with the cheaper MV difference.
    pub(super) fn code_b_static(&mut self, mbx: usize, mby: usize) {
        self.mb_qp = self.c.qp;
        self.p = &self.c.qsets[self.mb_qp as usize];
        let (refs, mv0, mv1) = self.direct_spatial(mbx, mby);
        // (a partitioned co-located macroblock would make direct prediction
        // per 8×8 block: not used)
        let zero = (refs & 1 == 0 || mv0 == (0, 0)) && (refs & 2 == 0 || mv1 == (0, 0)) && self.c.col[mby * self.c.mbw + mbx].part == 0;
        let mut py = [0u8; 256];
        let mut pc = [[0u8; 64]; 2];
        let cd = Coded::new();
        if zero {
            self.pred_b(refs, mbx, mby, mv0, mv1, &mut py, &mut pc);
            self.finish_b(mbx, mby, BMode::Direct, refs, mv0, mv1, &py, &pc, &cd);
            return;
        }
        let (p0, p1) = (self.mvp_l(mbx, mby, 0), self.mvp_l(mbx, mby, 1));
        let bits = |p: (i32, i32)| se_len(-p.0) + se_len(-p.1);
        let (mode, refs) = if bits(p1) < bits(p0) { (BMode::L1, 2) } else { (BMode::L0, 1) };
        self.pred_b(refs, mbx, mby, (0, 0), (0, 0), &mut py, &mut pc);
        self.finish_b(mbx, mby, mode, refs, (0, 0), (0, 0), &py, &pc, &cd);
    }

    /// Mode decision and coding of a changed B macroblock.
    pub(super) fn encode_mb_b(&mut self, mbx: usize, mby: usize, src_y: &[u8; 256], src_c: &[[u8; 64]; 2]) {
        let c = self.c;
        let (off, smooth) = if c.aq > 0.0 { aq_offset(src_y, src_c, c.aq) } else { (0, false) };
        self.mb_qp = (c.qp as i32 + off).clamp(0, 51) as u8;
        self.p = if smooth { &c.qsets_smooth[self.mb_qp as usize] } else { &c.qsets[self.mb_qp as usize] };
        let lambda = self.p.lambda as u32;
        let mbi = mby * c.mbw + mbx;
        self.stats.changed += 1;

        // Direct (and B_Skip when nothing is left to code).
        let (drefs, dmv0, dmv1) = self.direct_spatial(mbx, mby);
        let direct_ok = c.col[mbi].part == 0
            && (drefs & 1 == 0 || self.mv_in_range(mbx, mby, dmv0))
            && (drefs & 2 == 0 || self.mv_in_range(mbx, mby, dmv1));
        let mut dpy = [0u8; 256];
        let mut dpc = [[0u8; 64]; 2];
        let mut best = (u32::MAX, BMode::Direct);
        if direct_ok {
            self.pred_b(drefs, mbx, mby, dmv0, dmv1, &mut dpy, &mut dpc);
            let sad = crate::simd::sad16(src_y, 16, &dpy, 16);
            if sad <= lambda * 16 {
                self.code_b(mbx, mby, src_y, src_c, BMode::Direct, drefs, dmv0, dmv1, &dpy, &dpc);
                return;
            }
            best = (dsp::satd(src_y, &dpy, 16, 16) + lambda, BMode::Direct);
        }

        // One search per list, seeded with the direct vectors and the
        // co-located P vector scaled to this picture's distance from each
        // reference; no long-range rings (the P picture searched those).
        let col = &c.col[mbi];
        let (tb, td) = c.bdist;
        let colmv = if col.kind == 0 { (col.mv.0 as i32, col.mv.1 as i32) } else { (0, 0) };
        let scaled = [(colmv.0 * tb / td, colmv.1 * tb / td), (colmv.0 * (tb - td) / td, colmv.1 * (tb - td) / td)];
        self.no_rings = true;
        let mut its: [Option<Inter>; 2] = [None, None];
        for (l, dmv) in [(0usize, dmv0), (1, dmv1)] {
            self.rsel = l;
            self.extra_cand = Some(scaled[l]);
            let mvp = self.mvp_l(mbx, mby, l);
            let mut it = self.motion_search(mbx, mby, src_y, mvp, dmv, mbi, false, None);
            if c.subpel && it.cost > lambda * 8 {
                self.refine_subpel(mbx, mby, src_y, &mut it);
            }
            let cost = it.cost + lambda * 3;
            if cost < best.0 {
                best = (cost, if l == 0 { BMode::L0 } else { BMode::L1 });
            }
            its[l] = Some(it);
        }
        self.rsel = 0;
        self.extra_cand = None;
        self.no_rings = false;
        let (i0, i1) = (its[0].take().unwrap(), its[1].take().unwrap());
        let mvc = |it: &Inter| lambda * (se_len(it.mv.0 - it.mvp.0) + se_len(it.mv.1 - it.mvp.1));
        let mut bpy = i0.pred;
        avg_into(&mut bpy, &i1.pred);
        let cbi = dsp::satd(src_y, &bpy, 16, 16) + mvc(&i0) + mvc(&i1) + lambda * 5;
        if cbi < best.0 {
            best = (cbi, BMode::Bi);
        }

        // Intra 16×16 when no prediction works (new content).
        if best.0 > lambda * 64 {
            let e16 = self.edges16(mbx, mby);
            let mut b16 = (u32::MAX, 2u8);
            for m in 0..4u8 {
                if Self::intra16_modes_ok(&e16, m) {
                    let s = intra::sad16_mode(&e16, m, src_y);
                    if s < b16.0 {
                        b16 = (s, m);
                    }
                }
            }
            let mut pred16 = [0u8; 256];
            intra::pred16(&e16, b16.1, &mut pred16);
            let cost16 = dsp::satd(src_y, &pred16, 16, 16) + lambda * 10;
            if cost16 < best.0 {
                let mut cd = Coded::new();
                self.finish_intra(mbx, mby, src_c, &mut cd, Kind::I16(b16.1), None, Some((src_y, &pred16)));
                return;
            }
        }

        let mut py = [0u8; 256];
        let mut pc = [[0u8; 64]; 2];
        match best.1 {
            BMode::Direct => self.code_b(mbx, mby, src_y, src_c, BMode::Direct, drefs, dmv0, dmv1, &dpy, &dpc),
            BMode::L0 => {
                self.pred_b(1, mbx, mby, i0.mv, (0, 0), &mut py, &mut pc);
                self.code_b(mbx, mby, src_y, src_c, BMode::L0, 1, i0.mv, (0, 0), &py, &pc);
            }
            BMode::L1 => {
                self.pred_b(2, mbx, mby, (0, 0), i1.mv, &mut py, &mut pc);
                self.code_b(mbx, mby, src_y, src_c, BMode::L1, 2, (0, 0), i1.mv, &py, &pc);
            }
            BMode::Bi => {
                self.pred_b(3, mbx, mby, i0.mv, i1.mv, &mut py, &mut pc);
                self.code_b(mbx, mby, src_y, src_c, BMode::Bi, 3, i0.mv, i1.mv, &py, &pc);
            }
        }
    }

    /// Residual coding of a B macroblock with prediction (`py`, `pc`).
    fn code_b(
        &mut self,
        mbx: usize,
        mby: usize,
        src_y: &[u8; 256],
        src_c: &[[u8; 64]; 2],
        mode: BMode,
        refs: u8,
        mv0: (i32, i32),
        mv1: (i32, i32),
        py: &[u8; 256],
        pc: &[[u8; 64]; 2],
    ) {
        let mut cd = Coded::new();
        let mut recon = [0u8; 256];
        let sad = crate::simd::sad16(src_y, 16, py, 16);
        let ry = if sad > self.p.zero_sad.0 && crate::simd::max_sad4(src_y, py, 16, 16) > self.p.zero_sad4.0 {
            self.luma_resid(src_y, py, &mut cd, &mut recon);
            recon
        } else {
            *py
        };
        let mut reconc = [[0u8; 64]; 2];
        let zc = self.p.zero_sad.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &pc[0]), crate::simd::sad64(&src_c[1], &pc[1]));
        let dc_ok = s0 <= self.p.zero_sad.2 && s1 <= self.p.zero_sad.2;
        let rc = if (s0 > zc || s1 > zc)
            && !(dc_ok
                && crate::simd::max_sad4(&src_c[0], &pc[0], 8, 8) <= self.p.zero_sad4.1
                && crate::simd::max_sad4(&src_c[1], &pc[1], 8, 8) <= self.p.zero_sad4.1)
        {
            chroma_resid(src_c, pc, &self.p.qc, &self.p.qcs, false, self.dc_max(), &mut cd, &mut reconc);
            reconc
        } else {
            *pc
        };
        self.finish_b(mbx, mby, mode, refs, mv0, mv1, &ry, &rc, &cd);
    }

    /// Store the reconstruction and macroblock info, then emit the macroblock.
    fn finish_b(
        &mut self,
        mbx: usize,
        mby: usize,
        mode: BMode,
        refs: u8,
        mv0: (i32, i32),
        mv1: (i32, i32),
        ry: &[u8; 256],
        rc: &[[u8; 64]; 2],
        cd: &Coded,
    ) {
        self.store_recon(mbx, mby, ry, rc);
        let skip = mode == BMode::Direct && cd.cbp == 0;
        let explicit = mode != BMode::Direct;
        let mvd0 = if explicit && refs & 1 != 0 {
            let p = self.mvp_l(mbx, mby, 0);
            (mv0.0 - p.0, mv0.1 - p.1)
        } else {
            (0, 0)
        };
        let mvd1 = if explicit && refs & 2 != 0 {
            let p = self.mvp_l(mbx, mby, 1);
            (mv1.0 - p.0, mv1.1 - p.1)
        } else {
            (0, 0)
        };
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        self.b.info[i] = MbInfo {
            qp: 0,
            kind: 0,
            skip,
            coded: true,
            mv: if refs & 1 != 0 { (mv0.0 as i16, mv0.1 as i16) } else { (0, 0) },
            nz: cd.nz,
            nzc: cd.nzc,
            m4: [2; 16],
            cbp: cd.cbp,
            cmode: 0,
            mvd: (mvd0.0 as i16, mvd0.1 as i16),
            dcf: dc_flags(cd, false),
            refs,
            mv1: if refs & 2 != 0 { (mv1.0 as i16, mv1.1 as i16) } else { (0, 0) },
            mvd1: (mvd1.0 as i16, mvd1.1 as i16),
            direct: mode == BMode::Direct,
            part: 0,
            mv8: [(0, 0); 4],
            mvd8: [(0, 0); 4],
            t8: cd.t8 && cd.cbp & 15 != 0,
        };
        if skip {
            self.stats.skip += 1;
        } else {
            self.stats.inter += 1;
        }
        self.emit_b(mbx, mby, mode, refs, mvd0, mvd1, cd, skip);
        self.emit_end(mbx, mby);
    }

    fn emit_b(&mut self, mbx: usize, mby: usize, mode: BMode, refs: u8, mvd0: (i32, i32), mvd1: (i32, i32), cd: &Coded, skip: bool) {
        let (a, b) = self.nbrs(mbx, mby);
        let cb = self.cabac.as_mut().expect("ak264: B slices need CABAC");
        let w = &mut *self.w;
        let inc = a.map_or(0, |m| !m.skip as usize) + b.map_or(0, |m| !m.skip as usize);
        if skip {
            cb.decision(w, 24 + inc, 1);
            return;
        }
        cb.decision(w, 24 + inc, 0);
        let tinc = a.map_or(0, |m| !m.direct as usize) + b.map_or(0, |m| !m.direct as usize);
        match mode {
            BMode::Direct => cb.decision(w, 27 + tinc, 0),
            BMode::L0 | BMode::L1 => {
                cb.decision(w, 27 + tinc, 1);
                cb.decision(w, 27 + 3, 0);
                cb.decision(w, 27 + 5, (mode == BMode::L1) as u32);
            }
            BMode::Bi => {
                cb.decision(w, 27 + tinc, 1);
                cb.decision(w, 27 + 3, 1);
                cb.decision(w, 27 + 4, 0);
                cb.decision(w, 27 + 5, 0);
                cb.decision(w, 27 + 5, 0);
                cb.decision(w, 27 + 5, 0);
            }
        }
        if mode != BMode::Direct {
            if refs & 1 != 0 {
                let sx = a.map_or(0, |m| m.mvd.0.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd.0.unsigned_abs() as u32);
                let sy = a.map_or(0, |m| m.mvd.1.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd.1.unsigned_abs() as u32);
                cb.mvd(w, 40, mvd0.0, sx);
                cb.mvd(w, 47, mvd0.1, sy);
            }
            if refs & 2 != 0 {
                let sx = a.map_or(0, |m| m.mvd1.0.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd1.0.unsigned_abs() as u32);
                let sy = a.map_or(0, |m| m.mvd1.1.unsigned_abs() as u32) + b.map_or(0, |m| m.mvd1.1.unsigned_abs() as u32);
                cb.mvd(w, 40, mvd1.0, sx);
                cb.mvd(w, 47, mvd1.1, sy);
            }
        }
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
}
