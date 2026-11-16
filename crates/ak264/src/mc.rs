//! Luma sub-pixel interpolation (8.4.2.2.1): 6-tap half samples, bilinear quarters.
//!
//! [`Hpel`] caches the integer, half and centre samples of a 16×16 block's
//! neighbourhood so that every quarter-pel position within ±3/4 of the base can
//! be formed cheaply (motion refinement), bit-exactly as a decoder does.

use crate::frame::Plane;

const W: usize = 18; // positions -1..=16
const L: usize = 32; // padded lane count

#[inline(always)]
fn clip(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[inline(always)]
fn tap6(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32) -> i32 {
    a - 5 * b + 20 * c + 20 * d - 5 * e + f
}

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
