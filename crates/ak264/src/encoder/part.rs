//! P macroblock partitions: P_L0_16x8, P_L0_8x16 and P_8x8 (four 8×8
//! sub-macroblocks), one reference. Motion vectors are kept per 8×8 quadrant
//! (raster order) in [`MbInfo::mv8`]. CABAC only.

use super::*;

/// Partitions of each partitioning (16×16, 16×8, 8×16, 8×8) as
/// (x, y, width, height) in 8×8 quadrants, in decoding order.
pub(super) const PARTS: [&[(usize, usize, usize, usize)]; 4] =
    [&[(0, 0, 2, 2)], &[(0, 0, 2, 1), (0, 1, 2, 1)], &[(0, 0, 1, 2), (1, 0, 1, 2)], &[(0, 0, 1, 1), (1, 0, 1, 1), (0, 1, 1, 1), (1, 1, 1, 1)]];

/// Quadrants covered by partition `idx` of partitioning `part`.
fn quads(part: usize, idx: usize) -> impl Iterator<Item = usize> {
    let (x, y, w, h) = PARTS[part][idx];
    (y..y + h).flat_map(move |qy| (x..x + w).map(move |qx| qy * 2 + qx))
}

impl MbInfo {
    /// L0 motion vector of 8×8 quadrant `q` (raster).
    #[inline(always)]
    pub(crate) fn qmv(&self, q: usize) -> (i16, i16) {
        if self.part == 0 {
            self.mv
        } else {
            self.mv8[q]
        }
    }
    /// L0 motion vector difference of quadrant `q`.
    #[inline(always)]
    pub(crate) fn qmvd(&self, q: usize) -> (i16, i16) {
        if self.part == 0 {
            self.mvd
        } else {
            self.mvd8[q]
        }
    }
}

/// A partitioned inter decision: partitioning, vector per quadrant, cost
/// (SATD + λ·bits) and the luma prediction.
pub(super) struct PartChoice {
    pub part: usize,
    pub mv: [(i32, i32); 4],
    pub cost: u32,
    pub pred: [u8; 256],
}

#[inline(always)]
fn sad8(a: &[u8; 64], b: &[u8; 64]) -> u32 {
    a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y) as u32).sum()
}

impl<'a, 'b> SliceEnc<'a, 'b> {
    /// Neighbouring 8×8 quadrant at (qx, qy) quadrants from the macroblock's
    /// top-left (P slices): inside the macroblock from `cur` (only quadrants
    /// marked in `ok`), else from an already coded macroblock.
    fn nbq(&self, mbx: usize, mby: usize, qx: isize, qy: isize, cur: &[(i32, i32); 4], ok: &[bool; 4]) -> Nb {
        const NONE: Nb = Nb { avail: false, inter: false, mv: (0, 0) };
        if (0..2).contains(&qx) && (0..2).contains(&qy) {
            let q = (qy * 2 + qx) as usize;
            return if ok[q] { Nb { avail: true, inter: true, mv: cur[q] } } else { NONE };
        }
        let (mx, my) = (mbx as isize + qx.div_euclid(2), mby as isize + qy.div_euclid(2));
        if mx < 0 || mx >= self.c.mbw as isize || my < self.b.mby0 as isize || my > mby as isize || (my == mby as isize && mx >= mbx as isize) {
            return NONE;
        }
        let i = self.info(mx as usize, my as usize);
        if i.kind != 0 {
            return Nb { avail: true, inter: false, mv: (0, 0) };
        }
        let m = i.qmv((qy.rem_euclid(2) * 2 + qx.rem_euclid(2)) as usize);
        Nb { avail: true, inter: true, mv: (m.0 as i32, m.1 as i32) }
    }

    /// Motion vector predictor of partition `idx` (8.4.1.3, refIdx 0), with
    /// the vectors of the macroblock's earlier partitions in `cur` / `ok`.
    pub(super) fn mvp_part(&self, mbx: usize, mby: usize, part: usize, idx: usize, cur: &[(i32, i32); 4], ok: &[bool; 4]) -> (i32, i32) {
        let (px, py, pw, _) = PARTS[part][idx];
        let (px, py, pw) = (px as isize, py as isize, pw as isize);
        let a = self.nbq(mbx, mby, px - 1, py, cur, ok);
        let mut b = self.nbq(mbx, mby, px, py - 1, cur, ok);
        let mut c = self.nbq(mbx, mby, px + pw, py - 1, cur, ok);
        if !c.avail {
            c = self.nbq(mbx, mby, px - 1, py - 1, cur, ok);
        }
        match (part, idx) {
            (1, 0) if b.inter => return b.mv,
            (1, 1) if a.inter => return a.mv,
            (2, 0) if a.inter => return a.mv,
            (2, 1) if c.inter => return c.mv,
            _ => {}
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

    /// Luma prediction of quadrant `q` at quarter-pel `mv`.
    fn pred8(&self, mbx: usize, mby: usize, q: usize, mv: (i32, i32), out: &mut [u8; 64]) {
        let r = self.rp();
        let (x0, y0) = ((mbx * 16 + (q & 1) * 8) as isize, (mby * 16 + (q >> 1) * 8) as isize);
        let mut t = [0u8; 256];
        match self.hp() {
            Some(hp) => hp.pred(&r.y, x0 + (mv.0 >> 2) as isize, y0 + (mv.1 >> 2) as isize, mv.0 & 3, mv.1 & 3, &mut t),
            None => crate::mc::luma_pred(&r.y, x0, y0, mv, &mut t),
        }
        for y in 0..8 {
            out[y * 8..y * 8 + 8].copy_from_slice(&t[y * 16..y * 16 + 8]);
        }
    }

    /// Quadrant prediction through a small cache.
    fn cached8(&self, cache: &mut Vec<(usize, (i32, i32), [u8; 64])>, mbx: usize, mby: usize, q: usize, m: (i32, i32)) -> [u8; 64] {
        if let Some(e) = cache.iter().find(|e| e.0 == q && e.1 == m) {
            return e.2;
        }
        let mut p = [0u8; 64];
        self.pred8(mbx, mby, q, m, &mut p);
        cache.push((q, m, p));
        p
    }

    /// Integer then quarter-pel search for quadrant `q` around `seeds`.
    /// Returns (vector, SAD + λ·MV bits).
    fn search8(&self, mbx: usize, mby: usize, q: usize, src: &[u8; 64], mvp: (i32, i32), seeds: &[(i32, i32)]) -> ((i32, i32), u32) {
        let c = self.c;
        let lambda = self.p.lambda as u32;
        let mvcost = |m: (i32, i32)| lambda * (se_len(m.0 - mvp.0) + se_len(m.1 - mvp.1));
        let r = &self.rp().y;
        let (x0, y0) = ((mbx * 16 + (q & 1) * 8) as i32, (mby * 16 + (q >> 1) * 8) as i32);
        let w16 = (c.mbw * 16) as i32;
        let h16 = r.h as i32;
        let clamp = |m: (i32, i32)| (m.0.clamp(-8 - x0, w16 - x0), m.1.clamp((-8 - y0).max(-c.max_mvy), (h16 - y0).min(c.max_mvy - 1)));
        let eval = |m: (i32, i32)| -> u32 {
            let o = r.off((x0 + m.0) as isize, (y0 + m.1) as isize);
            let mut s = 0u32;
            for y in 0..8 {
                let row = &r.data[o + y * r.stride..o + y * r.stride + 8];
                for x in 0..8 {
                    s += src[y * 8 + x].abs_diff(row[x]) as u32;
                }
            }
            s + mvcost((m.0 * 4, m.1 * 4))
        };
        let mut best = (u32::MAX, (0, 0));
        for &sd in seeds {
            let m = clamp(((sd.0 + 2) >> 2, (sd.1 + 2) >> 2));
            let s = eval(m);
            if s < best.0 {
                best = (s, m);
            }
        }
        const HEX: [(i32, i32); 6] = [(-2, 0), (-1, -2), (1, -2), (2, 0), (1, 2), (-1, 2)];
        const SQ: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];
        for pat in [&HEX[..], &SQ[..]] {
            for _ in 0..8 {
                let cur = best;
                for d in pat {
                    let m = clamp((cur.1 .0 + d.0, cur.1 .1 + d.1));
                    let s = eval(m);
                    if s < best.0 {
                        best = (s, m);
                    }
                }
                if best.1 == cur.1 {
                    break;
                }
            }
        }
        // half- then quarter-pel squares
        let mut bq = (best.0, (best.1 .0 * 4, best.1 .1 * 4));
        if c.subpel {
            let mut p = [0u8; 64];
            for step in [2, 1] {
                let centre = bq.1;
                for d in SQ {
                    let m = (centre.0 + d.0 * step, centre.1 + d.1 * step);
                    self.pred8(mbx, mby, q, m, &mut p);
                    let s = sad8(src, &p) + mvcost(m);
                    if s < bq.0 {
                        bq = (s, m);
                    }
                }
            }
        }
        (bq.1, bq.0)
    }

    /// Try 16×8, 8×16 and 8×8 partitions of a macroblock whose best 16×16
    /// prediction is `it`; the best of them if it beats `it.cost` (+ the bits
    /// of the larger macroblock type).
    pub(super) fn try_partitions(&self, mbx: usize, mby: usize, src_y: &[u8; 256], it: &Inter) -> Option<PartChoice> {
        let lambda = self.p.lambda as u32;
        let mut src8 = [[0u8; 64]; 4];
        for (q, s) in src8.iter_mut().enumerate() {
            let (x0, y0) = ((q & 1) * 8, (q >> 1) * 8);
            for y in 0..8 {
                s[y * 8..y * 8 + 8].copy_from_slice(&src_y[(y0 + y) * 16 + x0..(y0 + y) * 16 + x0 + 8]);
            }
        }
        // 8×8: one search per quadrant, in decoding order (each predictor
        // depends on the earlier quadrants' vectors).
        let mut mv8 = [(0i32, 0i32); 4];
        let mut ok = [false; 4];
        for q in 0..4 {
            let mvp = self.mvp_part(mbx, mby, 3, q, &mv8, &ok);
            let (qx, qy) = ((q & 1) as isize, (q >> 1) as isize);
            let a = self.nbq(mbx, mby, qx - 1, qy, &mv8, &ok);
            let b = self.nbq(mbx, mby, qx, qy - 1, &mv8, &ok);
            let (m, _) = self.search8(mbx, mby, q, &src8[q], mvp, &[it.mv, mvp, a.mv, b.mv, (0, 0)]);
            mv8[q] = m;
            ok[q] = true;
        }
        // Candidate partitionings: each half/column takes the better of its two
        // quadrants' vectors. Quadrant predictions are cached by (quadrant, vector).
        let mut cache: Vec<(usize, (i32, i32), [u8; 64])> = Vec::with_capacity(12);
        let mut best: Option<PartChoice> = None;
        for part in 1..4 {
            let mut mv = [(0i32, 0i32); 4];
            for idx in 0..PARTS[part].len() {
                let mut pick = (u32::MAX, (0, 0));
                for qc in quads(part, idx) {
                    let m = mv8[qc];
                    let mut s = 0;
                    for q in quads(part, idx) {
                        s += sad8(&src8[q], &self.cached8(&mut cache, mbx, mby, q, m));
                    }
                    if s < pick.0 {
                        pick = (s, m);
                    }
                }
                for q in quads(part, idx) {
                    mv[q] = pick.1;
                }
            }
            let mut pred = [0u8; 256];
            for q in 0..4 {
                let p = self.cached8(&mut cache, mbx, mby, q, mv[q]);
                let (x0, y0) = ((q & 1) * 8, (q >> 1) * 8);
                for y in 0..8 {
                    pred[(y0 + y) * 16 + x0..(y0 + y) * 16 + x0 + 8].copy_from_slice(&p[y * 8..y * 8 + 8]);
                }
            }
            // MV bits with the real predictors, in decoding order
            let mut done = [(0i32, 0i32); 4];
            let mut dok = [false; 4];
            let mut bits = 0;
            for idx in 0..PARTS[part].len() {
                let q0 = quads(part, idx).next().unwrap();
                let p = self.mvp_part(mbx, mby, part, idx, &done, &dok);
                bits += se_len(mv[q0].0 - p.0) + se_len(mv[q0].1 - p.1);
                for q in quads(part, idx) {
                    done[q] = mv[q];
                    dok[q] = true;
                }
            }
            let type_bits = if part == 3 { 9 } else { 3 };
            let cost = dsp::satd(src_y, &pred, 16, 16) + lambda * (bits + type_bits);
            if best.as_ref().is_none_or(|b| cost < b.cost) {
                best = Some(PartChoice { part, mv, cost, pred });
            }
        }
        best.filter(|b| b.cost < it.cost)
    }

    /// Code a partitioned P macroblock.
    pub(super) fn code_part(&mut self, mbx: usize, mby: usize, src_y: &[u8; 256], src_c: &[[u8; 64]; 2], pc: &PartChoice) {
        let pred = &pc.pred;
        // chroma: each 4×4 chroma quadrant from its luma quadrant's vector
        let mut predc = [[0u8; 64]; 2];
        let mut t = [0u8; 64];
        for comp in 0..2 {
            for q in 0..4 {
                self.mc_chroma(comp, mbx, mby, pc.mv[q], &mut t);
                let (x0, y0) = ((q & 1) * 4, (q >> 1) * 4);
                for y in 0..4 {
                    predc[comp][(y0 + y) * 8 + x0..(y0 + y) * 8 + x0 + 4].copy_from_slice(&t[(y0 + y) * 8 + x0..(y0 + y) * 8 + x0 + 4]);
                }
            }
        }
        let mut cd = Coded::new();
        let mut recon = [0u8; 256];
        let sad = crate::simd::sad16(src_y, 16, pred, 16);
        let ry = if sad > self.p.zero_sad.0 && crate::simd::max_sad4(src_y, pred, 16, 16) > self.p.zero_sad4.0 {
            luma_inter(src_y, pred, &self.p.qs, &mut cd, &mut recon);
            &recon
        } else {
            pred
        };
        let mut reconc = [[0u8; 64]; 2];
        let zc = self.p.zero_sad.1;
        let (s0, s1) = (crate::simd::sad64(&src_c[0], &predc[0]), crate::simd::sad64(&src_c[1], &predc[1]));
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
        // vector differences per partition (decoding order)
        let part = pc.part;
        let mut done = [(0i32, 0i32); 4];
        let mut dok = [false; 4];
        let mut mvd8 = [(0i32, 0i32); 4];
        let mut mvds = Vec::with_capacity(4);
        for idx in 0..PARTS[part].len() {
            let q0 = quads(part, idx).next().unwrap();
            let p = self.mvp_part(mbx, mby, part, idx, &done, &dok);
            let d = (pc.mv[q0].0 - p.0, pc.mv[q0].1 - p.1);
            mvds.push(d);
            for q in quads(part, idx) {
                done[q] = pc.mv[q];
                dok[q] = true;
                mvd8[q] = d;
            }
        }
        let i = (mby - self.b.mby0) * self.c.mbw + mbx;
        let to16 = |m: (i32, i32)| (m.0 as i16, m.1 as i16);
        self.b.info[i] = MbInfo {
            qp: 0,
            kind: 0,
            skip: false,
            coded: true,
            mv: to16(pc.mv[0]),
            nz: cd.nz,
            nzc: cd.nzc,
            m4: [2; 16],
            cbp: cd.cbp,
            cmode: 0,
            mvd: to16(mvd8[0]),
            dcf: dc_flags(&cd, false),
            refs: 1,
            mv1: (0, 0),
            mvd1: (0, 0),
            direct: false,
            part: part as u8,
            mv8: pc.mv.map(to16),
            mvd8: mvd8.map(to16),
        };
        self.stats.inter += 1;
        self.emit_part(mbx, mby, part, &mvds, &mvd8, &cd);
        self.emit_end(mbx, mby);
    }

    /// |mvd| of the quadrant at (qx, qy) for the CABAC context: earlier
    /// partitions of this macroblock from `cur`, else a coded neighbour.
    fn mvdq(&self, mbx: usize, mby: usize, qx: isize, qy: isize, cur: &[(i32, i32); 4]) -> (u32, u32) {
        if (0..2).contains(&qx) && (0..2).contains(&qy) {
            let d = cur[(qy * 2 + qx) as usize];
            return (d.0.unsigned_abs(), d.1.unsigned_abs());
        }
        let (mx, my) = (mbx as isize + qx.div_euclid(2), mby as isize + qy.div_euclid(2));
        if mx < 0 || my < self.b.mby0 as isize {
            return (0, 0);
        }
        let m = self.info(mx as usize, my as usize);
        let d = m.qmvd((qy.rem_euclid(2) * 2 + qx.rem_euclid(2)) as usize);
        (d.0.unsigned_abs() as u32, d.1.unsigned_abs() as u32)
    }

    fn emit_part(&mut self, mbx: usize, mby: usize, part: usize, mvds: &[(i32, i32)], mvd8: &[(i32, i32); 4], cd: &Coded) {
        let (a, b) = self.nbrs(mbx, mby);
        // contexts of each partition's vector difference (left / above quadrants)
        let mut cur = [(0i32, 0i32); 4];
        let mut sums = Vec::with_capacity(4);
        for idx in 0..PARTS[part].len() {
            let (px, py, _, _) = PARTS[part][idx];
            let (px, py) = (px as isize, py as isize);
            let (la, lb) = (self.mvdq(mbx, mby, px - 1, py, &cur), self.mvdq(mbx, mby, px, py - 1, &cur));
            sums.push((la.0 + lb.0, la.1 + lb.1));
            for q in quads(part, idx) {
                cur[q] = mvd8[q];
            }
        }
        let cb = self.cabac.as_mut().expect("ak264: partitions need CABAC");
        let w = &mut *self.w;
        let inc = a.map_or(0, |m| !m.skip as usize) + b.map_or(0, |m| !m.skip as usize);
        cb.decision(w, 11 + inc, 0);
        cb.decision(w, 14, 0);
        match part {
            1 => {
                cb.decision(w, 15, 1);
                cb.decision(w, 17, 1);
            }
            2 => {
                cb.decision(w, 15, 1);
                cb.decision(w, 17, 0);
            }
            _ => {
                cb.decision(w, 15, 0);
                cb.decision(w, 16, 1);
                for _ in 0..4 {
                    cb.decision(w, 21, 1); // sub_mb_type P_L0_8x8
                }
            }
        }
        for (d, s) in mvds.iter().zip(&sums) {
            cb.mvd(w, 40, d.0, s.0);
            cb.mvd(w, 47, d.1, s.1);
        }
        cabac_cbp(cb, w, &a, &b, cd.cbp);
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
