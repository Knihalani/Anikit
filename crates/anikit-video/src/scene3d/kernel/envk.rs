//! Environment softboxes evaluated eight at a time.
//!
//! [`Boxes`] is a lane-wise copy of up to eight [`Softbox`]es; [`eval`]
//! computes every box's contribution with the same operations, in the same
//! order, as `Softbox::eval` (rejected boxes contribute exactly zero), as
//! branch-free lane loops that vectorise (AVX2 when available).

use super::super::env::Softbox;
use super::super::math::V3;

pub const LANES: usize = 8;

/// Up to eight softboxes, structure of arrays.
#[derive(Clone, Debug, Default)]
pub struct Boxes {
    pub n: usize,
    c: [[f32; LANES]; 3],
    u: [[f32; LANES]; 3],
    v: [[f32; LANES]; 3],
    hu: [f32; LANES],
    hv: [f32; LANES],
    soft: [f32; LANES],
    falloff: [f32; LANES],
    rad: [[f32; LANES]; 3],
}

impl Boxes {
    /// Lane-wise copy, or `None` for more than eight boxes.
    pub fn new(boxes: &[Softbox]) -> Option<Boxes> {
        if boxes.len() > LANES {
            return None;
        }
        let mut s = Boxes { n: boxes.len(), ..Default::default() };
        for (l, b) in boxes.iter().enumerate() {
            let comps = |a: V3| [a.x, a.y, a.z];
            for k in 0..3 {
                s.c[k][l] = comps(b.c)[k];
                s.u[k][l] = comps(b.u)[k];
                s.v[k][l] = comps(b.v)[k];
                s.rad[k][l] = comps(b.radiance)[k];
            }
            s.hu[l] = b.hu;
            s.hv[l] = b.hv;
            s.soft[l] = b.soft;
            s.falloff[l] = b.falloff;
        }
        Some(s)
    }
}

#[inline(always)]
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline(always)]
fn boxf(x: f32, h: f32, s: f32) -> f32 {
    let ax = x.abs();
    let hard = if ax < h { 1.0 } else { 0.0 };
    let soft = smooth(-h - s, -h + s, ax) - smooth(h - s, h + s, ax);
    if s <= 1e-6 {
        hard
    } else {
        soft
    }
}

simd_fn! {
    /// Each box's radiance towards `d` with extra edge blur `blur` (zero
    /// where `Softbox::eval` returns zero), per channel and lane.
    pub fn eval(b: &Boxes, d: V3, blur: f32) -> [[f32; LANES]; 3] {
        let mut out = [[0.0f32; LANES]; 3];
        let mut mg = [0.0f32; LANES];
        for l in 0..LANES {
            let k = d.x * b.c[0][l] + d.y * b.c[1][l] + d.z * b.c[2][l];
            let x = (d.x * b.u[0][l] + d.y * b.u[1][l] + d.z * b.u[2][l]) / k;
            let y = (d.x * b.v[0][l] + d.y * b.v[1][l] + d.z * b.v[2][l]) / k;
            let s = b.soft[l] + blur;
            let lim = 1.0 + s * 2.0;
            let out_of = (x.abs() > b.hu[l] * lim + s) | (y.abs() > b.hv[l] * lim + s);
            let m = boxf(x, b.hu[l], s) * boxf(y, b.hv[l], s);
            let t = (y / b.hv[l].max(1e-4)).clamp(-1.0, 1.0);
            let g = 1.0 - b.falloff[l] * (t * t);
            let keep = (k > 0.05) & !out_of & (m > 0.0) & (l < b.n);
            mg[l] = if keep { m * g } else { 0.0 };
        }
        for c in 0..3 {
            for l in 0..LANES {
                // Rejected lanes give exactly +0 (radiance × 0, then masked).
                let v = b.rad[c][l] * mg[l];
                out[c][l] = if mg[l] != 0.0 { v } else { 0.0 };
            }
        }
        out
    }
}
