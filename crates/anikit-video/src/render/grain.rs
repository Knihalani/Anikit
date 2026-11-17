//! Film grain fast path: trailing full-frame tiled-image layers blended with
//! Overlay or Soft Light (how the `grain` component and grain looks draw)
//! are blended straight into the finished frame.
//!
//! Drawn by vello, a non-Normal blend needs the background inside the layer
//! stack, which turns off every backdrop fast path for the frame. Here the rest
//! of the list renders with all fast paths, then each grain layer is one pass
//! of a 256×256 lookup table per channel (the W3C blend in 8-bit sRGB, as
//! vello blends), at exactly the texels vello's nearest sampling would read.

use std::sync::Arc;

use vello_cpu::Pixmap;

use super::View;
use crate::dl::{self, Blend, Op, Paint, Rgba};

/// One grain layer: its tile, integer texel offset, blend and strength.
pub(crate) struct Grain {
    tile: Arc<Pixmap>,
    off: (i64, i64),
    blend: Blend,
    alpha: f32,
}

/// Trailing grain layers of `ops` (in draw order) and where they start, if
/// the frame is opaque and every trailing layer qualifies.
pub(crate) fn tail(ops: &[Op], w: f64, h: f64, bg: Rgba, assets: &View, out: &mut Vec<Grain>) -> Option<usize> {
    out.clear();
    if bg.0[3] < 1.0 {
        return None;
    }
    let mut k = ops.len();
    while k >= 3 {
        let g = match &ops[k - 3..k] {
            [Op::PushLayer { blend, filters, clip: None, opacity, .. }, Op::RoundRect { x, y, w: rw, h: rh, radius, paint: Paint::Image { image, transform: t, opacity: po }, transform: rt }, Op::PopLayer]
                if matches!(blend, Blend::Overlay | Blend::SoftLight)
                    && filters.is_empty()
                    && *rt == dl::IDENTITY
                    && *radius == 0.0
                    && *x <= 0.0
                    && *y <= 0.0
                    && (*x + *rw) as f64 >= w
                    && (*y + *rh) as f64 >= h
                    && t[..4] == [1.0, 0.0, 0.0, 1.0]
                    && t[4].fract() == 0.0
                    && t[5].fract() == 0.0 =>
            {
                let Some(tile) = assets.image(*image) else { break };
                if tile.width() == 0 || tile.height() == 0 || tile.data_as_u8_slice().chunks_exact(4).any(|p| p[3] != 255) {
                    break;
                }
                Grain { tile, off: (t[4] as i64, t[5] as i64), blend: *blend, alpha: (opacity * po).clamp(0.0, 1.0) }
            }
            _ => break,
        };
        out.push(g);
        k -= 3;
    }
    out.reverse();
    (k < ops.len()).then_some(k)
}

/// W3C separable blend of source `s` over backdrop `b` (0..1).
fn mix(blend: Blend, b: f32, s: f32) -> f32 {
    match blend {
        // Overlay(b, s) = HardLight(s, b).
        Blend::Overlay => {
            if b <= 0.5 {
                2.0 * s * b
            } else {
                let t = 2.0 * b - 1.0;
                s + t - s * t
            }
        }
        Blend::SoftLight => {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }
        _ => s,
    }
}

/// Blend `layers` into the opaque RGBA `frame` (`fw` wide), within `region`.
pub(crate) fn apply(layers: &[Grain], frame: &mut [u8], fw: usize, region: [usize; 4]) {
    let [x0, y0, x1, y1] = region;
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    for g in layers {
        // lut[s * 256 + b]: the blended, faded result for backdrop b under source s.
        let mut lut = vec![0u8; 256 * 256];
        for s in 0..256 {
            for b in 0..256 {
                let (bf, sf) = (b as f32 / 255.0, s as f32 / 255.0);
                let v = bf + g.alpha * (mix(g.blend, bf, sf) - bf);
                lut[s * 256 + b] = (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
        let (tw, th) = (g.tile.width() as i64, g.tile.height() as i64);
        let tpx = g.tile.data_as_u8_slice();
        let row = |(y, frow): (usize, &mut [u8])| {
            // Pixel centre (x + ½) + offset lands in texel x + offset.
            let ty = (y as i64 + g.off.1).rem_euclid(th) as usize;
            let trow = &tpx[ty * tw as usize * 4..(ty + 1) * tw as usize * 4];
            for x in x0..x1 {
                let tx = (x as i64 + g.off.0).rem_euclid(tw) as usize * 4;
                let p = &mut frow[x * 4..x * 4 + 3];
                for c in 0..3 {
                    p[c] = lut[trow[tx + c] as usize * 256 + p[c] as usize];
                }
            }
        };
        use rayon::prelude::*;
        frame[y0 * fw * 4..y1 * fw * 4].par_chunks_mut(fw * 4).enumerate().for_each(|(y, r)| row((y + y0, r)));
    }
}
