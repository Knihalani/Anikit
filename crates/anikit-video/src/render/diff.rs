//! Changed-macroblock maps for the encoder's frame hints.
//!
//! [`diff_mb`] is exact: a macroblock is "changed" if any of its 16×16 luma
//! or 8×8 Cb/Cr samples differ. Row comparisons are slice equality, which
//! compiles to vectorised `memcmp`.

use super::yuv::YuvFrame;
use kurbo::Rect;

/// One bit per 16×16 macroblock, row-major.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MbMap {
    pub mb_width: usize,
    pub mb_height: usize,
    pub bits: Vec<u64>,
}

impl MbMap {
    pub fn new(mb_width: usize, mb_height: usize) -> Self {
        let mut m = Self::default();
        m.reset(mb_width, mb_height, false);
        m
    }

    /// Resize and fill with `value`, keeping the allocation.
    pub fn reset(&mut self, mb_width: usize, mb_height: usize, value: bool) {
        self.mb_width = mb_width;
        self.mb_height = mb_height;
        self.bits.clear();
        self.bits.resize((mb_width * mb_height).div_ceil(64), if value { !0 } else { 0 });
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        let i = y * self.mb_width + x;
        self.bits[i >> 6] >> (i & 63) & 1 == 1
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize) {
        let i = y * self.mb_width + x;
        self.bits[i >> 6] |= 1 << (i & 63);
    }

    /// Number of changed macroblocks.
    pub fn count(&self) -> usize {
        let n = self.mb_width * self.mb_height;
        (0..n).filter(|&i| self.bits[i >> 6] >> (i & 63) & 1 == 1).count()
    }

    /// Mark every macroblock touched by the device-space rect `r` (inflated by 1 px for AA).
    pub fn mark_rect(&mut self, r: Rect) {
        let r = r.inflate(1.0, 1.0);
        let x0 = (r.x0 / 16.0).floor().max(0.0) as usize;
        let y0 = (r.y0 / 16.0).floor().max(0.0) as usize;
        let x1 = ((r.x1 / 16.0).ceil().max(0.0) as usize).min(self.mb_width);
        let y1 = ((r.y1 / 16.0).ceil().max(0.0) as usize).min(self.mb_height);
        for y in y0..y1 {
            for x in x0..x1 {
                self.set(x, y);
            }
        }
    }
}

/// Exact changed-macroblock map between two frames of the same size.
pub fn diff_mb(prev: &YuvFrame, cur: &YuvFrame) -> MbMap {
    let mut m = MbMap::default();
    diff_mb_into(prev, cur, &mut m);
    m
}

/// As [`diff_mb`], reusing `out`'s allocation. Frames of different sizes mark everything.
pub fn diff_mb_into(prev: &YuvFrame, cur: &YuvFrame, out: &mut MbMap) {
    let (mw, mh) = cur.mb_dims();
    if prev.padded_width != cur.padded_width || prev.padded_height != cur.padded_height {
        out.reset(mw, mh, true);
        return;
    }
    out.reset(mw, mh, false);
    let (ys, cs) = (cur.y_stride, cur.c_stride);
    for my in 0..mh {
        for mx in 0..mw {
            let mut changed = false;
            for r in 0..16 {
                let o = (my * 16 + r) * ys + mx * 16;
                if prev.y[o..o + 16] != cur.y[o..o + 16] {
                    changed = true;
                    break;
                }
            }
            if !changed {
                for r in 0..8 {
                    let o = (my * 8 + r) * cs + mx * 8;
                    if prev.u[o..o + 8] != cur.u[o..o + 8] || prev.v[o..o + 8] != cur.v[o..o + 8] {
                        changed = true;
                        break;
                    }
                }
            }
            if changed {
                out.set(mx, my);
            }
        }
    }
}

/// Conservative changed-macroblock map from display lists: every macroblock
/// under an op that differs between `prev` and `cur` (compared op by op) is
/// marked. Cheaper than [`diff_mb`] but never smaller than it.
pub fn diff_mb_ops(prev: &crate::dl::DisplayList, cur: &crate::dl::DisplayList, out: &mut MbMap) {
    let (mw, mh) = ((cur.width as usize).div_ceil(16), (cur.height as usize).div_ceil(16));
    if prev.width != cur.width || prev.height != cur.height || prev.background != cur.background {
        out.reset(mw, mh, true);
        return;
    }
    out.reset(mw, mh, false);
    // Margins of the filters enclosing each op (a changed op inside a blurred
    // or shadowed layer touches more than its own bounds).
    let mut stack: Vec<[f32; 4]> = Vec::new();
    let mut m = [0f32; 4];
    let n = prev.ops.len().max(cur.ops.len());
    for i in 0..n {
        let (a, b) = (prev.ops.get(i), cur.ops.get(i));
        let layer_op = |o: Option<&crate::dl::Op>| matches!(o, Some(crate::dl::Op::PushLayer { .. } | crate::dl::Op::PopLayer));
        if layer_op(a) || layer_op(b) {
            if a != b {
                // Structure changed: everything after may composite differently.
                out.reset(mw, mh, true);
                return;
            }
            match b {
                Some(crate::dl::Op::PushLayer { filters, .. }) => {
                    stack.push(m);
                    for f in filters {
                        let fm = super::filter::margin(f);
                        for k in 0..4 {
                            m[k] += fm[k];
                        }
                    }
                }
                _ => m = stack.pop().unwrap_or([0.0; 4]),
            }
            continue;
        }
        if a == b {
            continue;
        }
        for o in [a, b].into_iter().flatten() {
            if let Some(r) = super::ops_bounds(std::slice::from_ref(o), kurbo::Affine::IDENTITY) {
                out.mark_rect(Rect::new(r.x0 - m[0] as f64, r.y0 - m[1] as f64, r.x1 + m[2] as f64, r.y1 + m[3] as f64));
            }
        }
    }
}
