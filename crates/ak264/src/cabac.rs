//! CABAC arithmetic encoder (9.3.4.2) and residual block coding (9.3.2/9.3.3).

use crate::bits::BitWriter;
use crate::cabac_tables::*;

/// Byte-oriented CABAC encoder. `low` holds the spec's 10-bit codILow window
/// plus `queue + 8` not-yet-output bits above it; output bytes whose value may
/// still change by a carry (0xff runs) are counted in `outstanding`.
pub struct Cabac {
    low: u32,
    range: u32,
    queue: i32,
    outstanding: u32,
    /// Context states: (pStateIdx << 1) | valMPS.
    ctx: [u8; 460],
    pub out: Vec<u8>,
}

/// Residual block categories (ctxBlockCat).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cat {
    LumaDc = 0,
    LumaAc = 1,
    Luma4x4 = 2,
    ChromaDc = 3,
    ChromaAc = 4,
}

const CBF_OFF: [usize; 5] = [0, 4, 8, 12, 16];
const SIG_OFF: [usize; 5] = [0, 15, 29, 44, 47];
const ABS_OFF: [usize; 5] = [0, 10, 20, 30, 39];

impl Cabac {
    /// Initialise contexts for a slice (`p_slice`: cabac_init_idc 0) and the engine.
    pub fn new(qp: u8, p_slice: bool) -> Self {
        let mut ctx = [0u8; 460];
        let model = p_slice as usize;
        let qp = qp.min(51) as i32;
        for (i, c) in ctx.iter_mut().enumerate() {
            let (m, n) = CTX_INIT[i][model];
            let pre = (((m as i32 * qp) >> 4) + n as i32).clamp(1, 126);
            *c = if pre <= 63 { ((63 - pre) << 1) as u8 } else { (((pre - 64) << 1) | 1) as u8 };
        }
        Cabac { low: 0, range: 510, queue: -9, outstanding: 0, ctx, out: Vec::with_capacity(4096) }
    }

    #[inline(always)]
    fn putbyte(&mut self) {
        if self.queue >= 0 {
            let out = self.low >> (self.queue + 10);
            self.low &= (0x400 << self.queue) - 1;
            self.queue -= 8;
            if out & 0xff == 0xff {
                self.outstanding += 1;
            } else {
                let carry = (out >> 8) as u8;
                if let Some(last) = self.out.last_mut() {
                    *last = last.wrapping_add(carry);
                }
                for _ in 0..self.outstanding {
                    self.out.push(0xffu8.wrapping_add(carry));
                }
                self.outstanding = 0;
                self.out.push(out as u8);
            }
        }
    }

    #[inline(always)]
    fn renorm(&mut self) {
        let shift = self.range.leading_zeros() as i32 - 23;
        self.range <<= shift;
        self.low <<= shift;
        self.queue += shift;
        self.putbyte();
    }

    #[inline(always)]
    pub fn decision(&mut self, _w: &mut BitWriter, ctx: usize, bin: u32) {
        let s = self.ctx[ctx];
        let state = (s >> 1) as usize;
        let lps = RANGE_LPS[state][((self.range >> 6) & 3) as usize] as u32;
        self.range -= lps;
        if bin != (s & 1) as u32 {
            self.low += self.range;
            self.range = lps;
            let nmps = (s & 1) ^ (state == 0) as u8;
            self.ctx[ctx] = (TRANS[state][0] << 1) | nmps;
        } else {
            self.ctx[ctx] = (TRANS[state][1] << 1) | (s & 1);
        }
        self.renorm();
    }

    #[inline(always)]
    pub fn bypass(&mut self, _w: &mut BitWriter, bin: u32) {
        self.low <<= 1;
        if bin != 0 {
            self.low += self.range;
        }
        self.queue += 1;
        self.putbyte();
    }

    /// end_of_slice_flag / I_PCM terminate bin. A 1 also flushes the engine (the
    /// final bit written is the rbsp_stop_one_bit); the caller then byte-aligns.
    pub fn terminate(&mut self, _w: &mut BitWriter, bin: u32) {
        self.range -= 2;
        if bin == 0 {
            self.renorm();
            return;
        }
        self.low += self.range;
        // EncodeFlush: range = 2, renormalise (7 bits), then the window's top three
        // bits leave as output with the last one forced to 1 (the stop bit).
        self.range = 2;
        self.renorm();
        self.low |= 1 << 7;
        self.low <<= 3;
        self.queue += 3;
        self.putbyte();
        // pad the final partial byte with zeros
        while self.queue > -8 {
            if self.queue < 0 {
                let pad = -self.queue;
                self.low <<= pad;
                self.queue = 0;
            }
            self.putbyte();
        }
        // no carry can arrive any more
        for _ in 0..self.outstanding {
            self.out.push(0xff);
        }
        self.outstanding = 0;
    }

    /// k-th order Exp-Golomb suffix in bypass bins.
    pub fn eg_bypass(&mut self, w: &mut BitWriter, mut v: u32, mut k: u32) {
        loop {
            if v >= (1 << k) {
                self.bypass(w, 1);
                v -= 1 << k;
                k += 1;
            } else {
                self.bypass(w, 0);
                while k > 0 {
                    k -= 1;
                    self.bypass(w, (v >> k) & 1);
                }
                return;
            }
        }
    }

    /// One mvd component (UEG3, uCoff 9). `ctx0` is 40 (x) or 47 (y); `sum` = |mvdA| + |mvdB|.
    pub fn mvd(&mut self, w: &mut BitWriter, ctx0: usize, v: i32, sum: u32) {
        let inc = if sum < 3 {
            0
        } else if sum > 32 {
            2
        } else {
            1
        };
        let a = v.unsigned_abs();
        if a == 0 {
            self.decision(w, ctx0 + inc, 0);
            return;
        }
        self.decision(w, ctx0 + inc, 1);
        let pre = a.min(9);
        const INC: [usize; 9] = [0, 3, 4, 5, 6, 6, 6, 6, 6];
        for b in 1..pre as usize {
            self.decision(w, ctx0 + INC[b], 1);
        }
        if a < 9 {
            self.decision(w, ctx0 + INC[pre as usize], 0);
        } else {
            self.eg_bypass(w, a - 9, 3);
        }
        self.bypass(w, (v < 0) as u32);
    }

    /// A residual block (`coeffs` in scan order, len 16/15/4) with its coded_block_flag
    /// context increment.
    pub fn residual(&mut self, w: &mut BitWriter, coeffs: &[i32], cat: Cat, cbf_inc: usize) {
        let c = cat as usize;
        let n = coeffs.len();
        let mut last = usize::MAX;
        for i in (0..n).rev() {
            if coeffs[i] != 0 {
                last = i;
                break;
            }
        }
        if last == usize::MAX {
            self.decision(w, 85 + CBF_OFF[c] + cbf_inc, 0);
            return;
        }
        self.decision(w, 85 + CBF_OFF[c] + cbf_inc, 1);
        let (sig, lst) = (105 + SIG_OFF[c], 166 + SIG_OFF[c]);
        for (i, &v) in coeffs.iter().enumerate().take(n - 1) {
            let inc = if cat == Cat::ChromaDc { i.min(2) } else { i };
            if v != 0 {
                self.decision(w, sig + inc, 1);
                self.decision(w, lst + inc, (i == last) as u32);
                if i == last {
                    break;
                }
            } else {
                self.decision(w, sig + inc, 0);
            }
        }
        let base = 227 + ABS_OFF[c];
        let (mut gt1, mut eq1) = (0usize, 0usize);
        let cap = if cat == Cat::ChromaDc { 3 } else { 4 };
        for i in (0..=last).rev() {
            let v = coeffs[i];
            if v == 0 {
                continue;
            }
            let a = v.unsigned_abs() - 1;
            let inc0 = if gt1 != 0 { 0 } else { (1 + eq1).min(4) };
            if a == 0 {
                self.decision(w, base + inc0, 0);
                eq1 += 1;
            } else {
                self.decision(w, base + inc0, 1);
                let ctx = base + 5 + gt1.min(cap);
                let pre = a.min(14);
                for _ in 1..pre {
                    self.decision(w, ctx, 1);
                }
                if a < 14 {
                    self.decision(w, ctx, 0);
                } else {
                    self.eg_bypass(w, a - 14, 0);
                }
                gt1 += 1;
            }
            self.bypass(w, (v < 0) as u32);
        }
    }
}
