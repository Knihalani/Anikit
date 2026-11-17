//! Luma sub-pixel interpolation (8.4.2.2.1): 6-tap half samples, bilinear quarters.
//!
//! [`Hpel`] caches the integer, half and centre samples of a 16×16 block's
//! neighbourhood so that every quarter-pel position within ±3/4 of the base can
//! be formed cheaply (motion refinement), bit-exactly as a decoder does.

use crate::frame::Plane;

const W: usize = 18; // positions -1..=16
const L: usize = 32; // padded lane count

pub struct Hpel {
    g: [[u8; L]; W], // G(x, y)          [y+1][x+1]
    b: [[u8; L]; W], // half between x and x+1 at row y
    h: [[u8; L]; W], // half between y and y+1 at column x
    j: [[u8; L]; W], // centre
}

impl Hpel {
    /// Neighbourhood of the block whose integer top-left sample is (x, y) in `p`.
    pub fn new(p: &Plane, x: isize, y: isize) -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            if crate::simd::level() >= 2 {
                #[target_feature(enable = "avx2")]
                unsafe fn f(p: &Plane, x: isize, y: isize) -> Hpel {
                    Hpel::new_impl(p, x, y)
                }
                // SAFETY: AVX2 detected at run time.
                return unsafe { f(p, x, y) };
            }
        }
        Self::new_impl(p, x, y)
    }

    #[inline(always)]
    fn new_impl(p: &Plane, x: isize, y: isize) -> Self {
        let mut hp = Hpel { g: [[0; L]; W], b: [[0; L]; W], h: [[0; L]; W], j: [[0; L]; W] };
        // 23 source rows (y-3 ..= y+19); columns x-3 .. x-3+L+5 as i16
        let mut r = [[0i16; L + 8]; 23];
        let avail = p.data.len();
        for (k, row) in r.iter_mut().enumerate() {
            let o = p.off(x - 3, y - 3 + k as isize);
            let n = (L + 8).min(avail - o);
            let src = &p.data[o..o + n];
            for i in 0..n {
                row[i] = src[i] as i16;
            }
        }
        for yy in 0..W {
            let src = &r[yy + 2];
            let dst = &mut hp.g[yy];
            for c in 0..L {
                dst[c] = src[c + 2] as u8;
            }
        }
        // unrounded horizontal halves b1 for rows -3..=18 (22), lane c = column c-1
        let mut b1 = [[0i16; L]; 22];
        for k in 0..22 {
            let row = &r[k];
            let out = &mut b1[k];
            for c in 0..L {
                out[c] = row[c] - 5 * row[c + 1] + 20 * row[c + 2] + 20 * row[c + 3] - 5 * row[c + 4] + row[c + 5];
            }
        }
        for yy in 0..W {
            let src = &b1[yy + 2];
            let dst = &mut hp.b[yy];
            for c in 0..L {
                dst[c] = ((src[c] + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for yy in 0..W {
            let k = yy + 2;
            let (r0, r1, r2, r3, r4, r5) = (&r[k - 2], &r[k - 1], &r[k], &r[k + 1], &r[k + 2], &r[k + 3]);
            let dst = &mut hp.h[yy];
            for c in 0..L {
                let i = c + 2;
                let v = r0[i] - 5 * r1[i] + 20 * r2[i] + 20 * r3[i] - 5 * r4[i] + r5[i];
                dst[c] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for yy in 0..W - 1 {
            let k = yy + 2;
            let (r0, r1, r2, r3, r4, r5) = (&b1[k - 2], &b1[k - 1], &b1[k], &b1[k + 1], &b1[k + 2], &b1[k + 3]);
            let dst = &mut hp.j[yy];
            for c in 0..L {
                let v = r0[c] as i32 - 5 * r1[c] as i32 + 20 * r2[c] as i32 + 20 * r3[c] as i32 - 5 * r4[c] as i32
                    + r5[c] as i32;
                dst[c] = ((v + 512) >> 10).clamp(0, 255) as u8;
            }
        }
        hp
    }

    /// Prediction at quarter-pel offset (dx, dy) ∈ [-3, 3]² from the base.
    pub fn pred(&self, dx: i32, dy: i32, out: &mut [u8; 256]) {
        let (ix, fx) = (dx >> 2, dx & 3);
        let (iy, fy) = (dy >> 2, dy & 3);
        let ox = (ix + 1) as usize;
        let oy = (iy + 1) as usize;
        let (g, b, h, j) = (&self.g, &self.b, &self.h, &self.j);
        type P<'a> = (&'a [[u8; L]; W], usize, usize);
        let (p1, p2): (P, Option<P>) = match (fx, fy) {
            (0, 0) => ((g, 0, 0), None),
            (1, 0) => ((g, 0, 0), Some((b, 0, 0))),
            (2, 0) => ((b, 0, 0), None),
            (3, 0) => ((b, 0, 0), Some((g, 0, 1))),
            (0, 1) => ((g, 0, 0), Some((h, 0, 0))),
            (0, 2) => ((h, 0, 0), None),
            (0, 3) => ((h, 0, 0), Some((g, 1, 0))),
            (1, 1) => ((b, 0, 0), Some((h, 0, 0))),
            (3, 1) => ((b, 0, 0), Some((h, 0, 1))),
            (1, 3) => ((h, 0, 0), Some((b, 1, 0))),
            (3, 3) => ((h, 0, 1), Some((b, 1, 0))),
            (2, 1) => ((b, 0, 0), Some((j, 0, 0))),
            (2, 3) => ((j, 0, 0), Some((b, 1, 0))),
            (1, 2) => ((h, 0, 0), Some((j, 0, 0))),
            (3, 2) => ((j, 0, 0), Some((h, 0, 1))),
            _ => ((j, 0, 0), None),
        };
        for y in 0..16 {
            let a: [u8; 16] = p1.0[oy + y + p1.1][ox + p1.2..ox + p1.2 + 16].try_into().unwrap();
            let o: &mut [u8; 16] = (&mut out[y * 16..y * 16 + 16]).try_into().unwrap();
            match p2 {
                None => *o = a,
                Some(q) => {
                    let c: [u8; 16] = q.0[oy + y + q.1][ox + q.2..ox + q.2 + 16].try_into().unwrap();
                    *o = crate::simd::avg16(&a, &c);
                }
            }
        }
    }
}

/// Luma prediction of the 16×16 block at integer (x, y) displaced by a quarter-pel `mv`.
pub fn luma_pred(p: &Plane, x: isize, y: isize, mv: (i32, i32), out: &mut [u8; 256]) {
    let bx = x + (mv.0 >> 2) as isize;
    let by = y + (mv.1 >> 2) as isize;
    if mv.0 & 3 == 0 && mv.1 & 3 == 0 {
        let o = p.off(bx, by);
        for r in 0..16 {
            out[r * 16..r * 16 + 16].copy_from_slice(&p.data[o + r * p.stride..o + r * p.stride + 16]);
        }
        return;
    }
    Hpel::new(p, bx, by).pred(mv.0 & 3, mv.1 & 3, out);
}

/// Half-sample planes of a whole reference picture (b: horizontal, h: vertical,
/// j: centre), same geometry as the luma plane. Built once per reference when
/// many macroblocks refine; predictions are identical to [`Hpel`].
pub struct HpelPlanes {
    pub b: Plane,
    pub h: Plane,
    pub j: Plane,
    b1: Vec<i16>,
}

impl HpelPlanes {
    pub fn new(r: &Plane) -> Self {
        HpelPlanes { b: r.clone(), h: r.clone(), j: r.clone(), b1: vec![0; r.data.len()] }
    }

    /// Recompute from the (border-extended) reference luma plane.
    pub fn compute(&mut self, r: &Plane) {
        #[cfg(target_arch = "x86_64")]
        {
            if crate::simd::level() >= 2 {
                #[target_feature(enable = "avx2")]
                unsafe fn f(s: &mut HpelPlanes, r: &Plane) {
                    s.compute_impl(r)
                }
                // SAFETY: AVX2 detected at run time.
                return unsafe { f(self, r) };
            }
        }
        self.compute_impl(r)
    }

    #[inline(always)]
    fn compute_impl(&mut self, r: &Plane) {
        let s = r.stride;
        let rows = r.data.len() / s;
        let d = &r.data;
        // columns 2..s-3 of every row get b; rows 2..rows-3 get h and j
        let mut b1 = std::mem::take(&mut self.b1);
        b1.resize(rows * s, 0);
        for y in 0..rows {
            let row = &d[y * s..y * s + s];
            let o = &mut b1[y * s..y * s + s];
            let bo = &mut self.b.data[y * s..y * s + s];
            for x in 2..s - 3 {
                let v = row[x - 2] as i16 - 5 * row[x - 1] as i16 + 20 * row[x] as i16 + 20 * row[x + 1] as i16
                    - 5 * row[x + 2] as i16
                    + row[x + 3] as i16;
                o[x] = v;
                bo[x] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
        }
        for y in 2..rows - 3 {
            let r0 = &d[(y - 2) * s..(y - 1) * s];
            let r1 = &d[(y - 1) * s..y * s];
            let r2 = &d[y * s..(y + 1) * s];
            let r3 = &d[(y + 1) * s..(y + 2) * s];
            let r4 = &d[(y + 2) * s..(y + 3) * s];
            let r5 = &d[(y + 3) * s..(y + 4) * s];
            let ho = &mut self.h.data[y * s..(y + 1) * s];
            for x in 0..s {
                let v = r0[x] as i16 - 5 * r1[x] as i16 + 20 * r2[x] as i16 + 20 * r3[x] as i16 - 5 * r4[x] as i16
                    + r5[x] as i16;
                ho[x] = ((v + 16) >> 5).clamp(0, 255) as u8;
            }
            let (c0, c1, c2) = (&b1[(y - 2) * s..(y - 1) * s], &b1[(y - 1) * s..y * s], &b1[y * s..(y + 1) * s]);
            let (c3, c4, c5) = (&b1[(y + 1) * s..(y + 2) * s], &b1[(y + 2) * s..(y + 3) * s], &b1[(y + 3) * s..(y + 4) * s]);
            let jo = &mut self.j.data[y * s..(y + 1) * s];
            for x in 0..s {
                let v = c0[x] as i32 - 5 * c1[x] as i32 + 20 * c2[x] as i32 + 20 * c3[x] as i32 - 5 * c4[x] as i32
                    + c5[x] as i32;
                jo[x] = ((v + 512) >> 10).clamp(0, 255) as u8;
            }
        }
        self.b1 = b1;
    }

    /// Prediction of the block whose integer top-left sample is (x, y) at the
    /// quarter-pel fraction (fx, fy) ∈ [0, 3]².
    pub fn pred(&self, g: &Plane, x: isize, y: isize, fx: i32, fy: i32, out: &mut [u8; 256]) {
        let s = g.stride;
        let o = g.off(x, y);
        let (gd, b, h, j) = (&g.data, &self.b.data, &self.h.data, &self.j.data);
        type P<'a> = (&'a [u8], usize);
        let (p1, p2): (P, Option<P>) = match (fx, fy) {
            (0, 0) => ((gd, 0), None),
            (1, 0) => ((gd, 0), Some((b, 0))),
            (2, 0) => ((b, 0), None),
            (3, 0) => ((b, 0), Some((gd, 1))),
            (0, 1) => ((gd, 0), Some((h, 0))),
            (0, 2) => ((h, 0), None),
            (0, 3) => ((h, 0), Some((gd, s))),
            (1, 1) => ((b, 0), Some((h, 0))),
            (3, 1) => ((b, 0), Some((h, 1))),
            (1, 3) => ((h, 0), Some((b, s))),
            (3, 3) => ((h, 1), Some((b, s))),
            (2, 1) => ((b, 0), Some((j, 0))),
            (2, 3) => ((j, 0), Some((b, s))),
            (1, 2) => ((h, 0), Some((j, 0))),
            (3, 2) => ((j, 0), Some((h, 1))),
            _ => ((j, 0), None),
        };
        for r in 0..16 {
            let i = o + r * s;
            let a: [u8; 16] = p1.0[i + p1.1..i + p1.1 + 16].try_into().unwrap();
            let dst: &mut [u8; 16] = (&mut out[r * 16..r * 16 + 16]).try_into().unwrap();
            *dst = match p2 {
                None => a,
                Some(q) => crate::simd::avg16(&a, q.0[i + q.1..i + q.1 + 16].try_into().unwrap()),
            };
        }
    }
}

#[cfg(test)]
mod planes_test {
    use super::*;
    #[test]
    fn planes_match_window() {
        let mut p = Plane::new(64, 48, 32);
        for (i, v) in p.data.iter_mut().enumerate() {
            *v = ((i * 37 + i / 97 * 13) % 251) as u8;
        }
        p.extend();
        let mut hp = HpelPlanes::new(&p);
        hp.compute(&p);
        for (x, y) in [(0isize, 0isize), (-17, -17), (40, 20), (48, 32), (-5, 30)] {
            let w = Hpel::new(&p, x, y);
            for dy in -3..=3 {
                for dx in -3..=3 {
                    let (mut a, mut b) = ([0u8; 256], [0u8; 256]);
                    w.pred(dx, dy, &mut a);
                    hp.pred(&p, x + (dx >> 2) as isize, y + (dy >> 2) as isize, dx & 3, dy & 3, &mut b);
                    assert_eq!(a, b, "({x},{y}) d=({dx},{dy})");
                }
            }
        }
    }
}
