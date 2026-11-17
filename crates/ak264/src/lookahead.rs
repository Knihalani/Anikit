//! Quarter-resolution cost estimates for the B-frame decision: is a group of
//! frames cheaper as P pictures in a chain, or as B pictures between two
//! references? (Like x264's fast B-frame decision.)

/// Luma downsampled by 4×4 box averaging.
#[derive(Clone, Default)]
pub(crate) struct Lowres {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl Lowres {
    pub fn from_luma(y: &[u8], stride: usize, w: usize, h: usize) -> Lowres {
        let (lw, lh) = (w / 4, h / 4);
        let mut data = vec![0u8; lw * lh];
        for ly in 0..lh {
            let rows = [&y[(ly * 4) * stride..], &y[(ly * 4 + 1) * stride..], &y[(ly * 4 + 2) * stride..], &y[(ly * 4 + 3) * stride..]];
            for lx in 0..lw {
                let mut s = 0u32;
                for r in &rows {
                    let p = &r[lx * 4..lx * 4 + 4];
                    s += p[0] as u32 + p[1] as u32 + p[2] as u32 + p[3] as u32;
                }
                data[ly * lw + lx] = ((s + 8) >> 4) as u8;
            }
        }
        Lowres { w: lw, h: lh, data }
    }

    #[inline(always)]
    fn sad8(&self, x: usize, y: usize, r: &Lowres, rx: usize, ry: usize) -> u32 {
        let mut s = 0u32;
        for j in 0..8 {
            let a = &self.data[(y + j) * self.w + x..(y + j) * self.w + x + 8];
            let b = &r.data[(ry + j) * r.w + rx..(ry + j) * r.w + rx + 8];
            for i in 0..8 {
                s += (a[i] as i32 - b[i] as i32).unsigned_abs();
            }
        }
        s
    }

    /// Σ|x − mean| of a block: a stand-in for its intra cost.
    fn intra8(&self, x: usize, y: usize) -> u32 {
        let mut sum = 0u32;
        for j in 0..8 {
            for i in 0..8 {
                sum += self.data[(y + j) * self.w + x + i] as u32;
            }
        }
        let m = ((sum + 32) >> 6) as i32;
        let mut s = 0u32;
        for j in 0..8 {
            for i in 0..8 {
                s += (self.data[(y + j) * self.w + x + i] as i32 - m).unsigned_abs();
            }
        }
        s
    }
}

/// Fixed cost of a block that must code something (header, MV): keeps tiny
/// differences from dominating.
const BLOCK_COST: u32 = 24;

/// Best SAD of block (x, y) of `f` in `r` around candidate vectors, with a
/// small diamond search. Returns (sad, mv).
fn search(f: &Lowres, r: &Lowres, x: usize, y: usize, cands: &[(i32, i32)]) -> (u32, (i32, i32)) {
    let (xmax, ymax) = ((f.w - 8) as i32, (f.h - 8) as i32);
    let ok = |mv: (i32, i32)| {
        let (px, py) = (x as i32 + mv.0, y as i32 + mv.1);
        px >= 0 && py >= 0 && px <= xmax && py <= ymax
    };
    let eval = |mv: (i32, i32)| f.sad8(x, y, r, (x as i32 + mv.0) as usize, (y as i32 + mv.1) as usize);
    let mut best = (u32::MAX, (0, 0));
    for &c in cands {
        if ok(c) {
            let s = eval(c);
            if s < best.0 {
                best = (s, c);
            }
        }
    }
    for _ in 0..8 {
        let cur = best;
        for d in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
            let mv = (cur.1 .0 + d.0, cur.1 .1 + d.1);
            if ok(mv) {
                let s = eval(mv);
                if s < best.0 {
                    best = (s, mv);
                }
            }
        }
        if best.1 == cur.1 {
            break;
        }
    }
    best
}

/// Estimated cost of `f` predicted from `r` (P picture).
pub(crate) fn cost_p(f: &Lowres, r: &Lowres) -> u64 {
    let (bw, bh) = (f.w / 8, f.h / 8);
    let mut mvs = vec![(0i32, 0i32); bw];
    let mut total = 0u64;
    for by in 0..bh {
        let mut left = (0, 0);
        for bx in 0..bw {
            let (x, y) = (bx * 8, by * 8);
            let (s, mv) = search(f, r, x, y, &[(0, 0), left, mvs[bx]]);
            left = mv;
            mvs[bx] = mv;
            let c = s.min(f.intra8(x, y));
            total += if c > 64 { (c + BLOCK_COST) as u64 } else { (c / 4) as u64 };
        }
    }
    total
}

/// Estimated cost of `f` as a B picture between `a` (past) and `b` (future).
pub(crate) fn cost_b(f: &Lowres, a: &Lowres, b: &Lowres) -> u64 {
    let (bw, bh) = (f.w / 8, f.h / 8);
    let mut mvs = vec![((0i32, 0i32), (0i32, 0i32)); bw];
    let mut total = 0u64;
    for by in 0..bh {
        let mut left = ((0, 0), (0, 0));
        for bx in 0..bw {
            let (x, y) = (bx * 8, by * 8);
            let (sa, ma) = search(f, a, x, y, &[(0, 0), left.0, mvs[bx].0]);
            let (sb, mb) = search(f, b, x, y, &[(0, 0), left.1, mvs[bx].1]);
            left = (ma, mb);
            mvs[bx] = (ma, mb);
            // bi-prediction: average of the two best blocks
            let (ax, ay) = ((x as i32 + ma.0) as usize, (y as i32 + ma.1) as usize);
            let (cx, cy) = ((x as i32 + mb.0) as usize, (y as i32 + mb.1) as usize);
            let mut sbi = 0u32;
            for j in 0..8 {
                for i in 0..8 {
                    let p = (a.data[(ay + j) * a.w + ax + i] as u32 + b.data[(cy + j) * b.w + cx + i] as u32 + 1) >> 1;
                    sbi += (f.data[(y + j) * f.w + x + i] as i32 - p as i32).unsigned_abs();
                }
            }
            let c = sa.min(sb).min(sbi).min(f.intra8(x, y));
            total += if c > 64 { (c + BLOCK_COST) as u64 } else { (c / 4) as u64 };
        }
    }
    total
}
