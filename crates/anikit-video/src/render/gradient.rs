//! Large gradient fills, evaluated in float and dithered.
//!
//! vello interpolates gradient stops in premultiplied sRGB (as CSS does) and
//! rounds to 8 bits, which bands on large, dark or slow gradients. Here the same
//! interpolation is evaluated per pixel in float, with a fixed screen-anchored
//! triangular dither of ±½ LSB (the same every frame, so codecs keep it), into
//! a bitmap that the shape is then filled with. The bitmap is cached by paint,
//! transform and position, so a static gradient costs one blit per frame.

use kurbo::Affine;
use vello_cpu::Pixmap;

use crate::dl::{Paint, Rgba};

/// Smallest device area (px) worth a bitmap; smaller fills keep vello's gradient.
pub(crate) const MIN_AREA: f64 = 48.0 * 48.0;

/// Premultiplied stop colours with offsets, sorted.
fn prepared(stops: &[(f32, Rgba)]) -> Vec<(f32, [f32; 4])> {
    let mut v: Vec<(f32, [f32; 4])> = stops
        .iter()
        .map(|(o, c)| {
            let a = c.0[3].clamp(0.0, 1.0);
            (*o, [c.0[0].clamp(0.0, 1.0) * a, c.0[1].clamp(0.0, 1.0) * a, c.0[2].clamp(0.0, 1.0) * a, a])
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

/// The premultiplied colour at parameter `s` (padded outside the stops).
#[inline]
fn at(stops: &[(f32, [f32; 4])], s: f32) -> [f32; 4] {
    let n = stops.len();
    if s <= stops[0].0 {
        return stops[0].1;
    }
    if s >= stops[n - 1].0 {
        return stops[n - 1].1;
    }
    let mut i = 1;
    while i < n - 1 && stops[i].0 < s {
        i += 1;
    }
    let (a, b) = (&stops[i - 1], &stops[i]);
    let f = ((s - a.0) / (b.0 - a.0).max(1e-9)).clamp(0.0, 1.0);
    [a.1[0] + (b.1[0] - a.1[0]) * f, a.1[1] + (b.1[1] - a.1[1]) * f, a.1[2] + (b.1[2] - a.1[2]) * f, a.1[3] + (b.1[3] - a.1[3]) * f]
}

/// Render `paint` (Linear or Radial, local coordinates mapped to the device
/// by `t`) over device pixels `[x0, y0, x0 + w, y0 + h)`. `org` is the device
/// origin's position in the frame, which anchors the dither to frame pixels.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render(paint: &Paint, t: Affine, x0: i64, y0: i64, w: usize, h: usize, org: (i32, i32), pool: Option<&mut Vec<Pixmap>>) -> Option<Pixmap> {
    let inv = t.inverse();
    let c = inv.as_coeffs();
    if !c.iter().all(|v| v.is_finite()) {
        return None;
    }
    // Parameter s at device (x, y) is affine in x along a row: s = s0 + dx·x (linear),
    // or the distance of an affine local point from the centre (radial).
    enum G {
        Linear { p0: [f64; 2], d: [f64; 2] },
        Radial { c: [f64; 2], inv_r: f64 },
    }
    let (g, stops) = match paint {
        Paint::Linear { p0, p1, stops } if stops.len() >= 2 => {
            let d = [(p1[0] - p0[0]) as f64, (p1[1] - p0[1]) as f64];
            let l2 = d[0] * d[0] + d[1] * d[1];
            if l2 < 1e-12 {
                return None;
            }
            (G::Linear { p0: [p0[0] as f64, p0[1] as f64], d: [d[0] / l2, d[1] / l2] }, stops)
        }
        Paint::Radial { c: ctr, r, stops } if stops.len() >= 2 => (G::Radial { c: [ctr[0] as f64, ctr[1] as f64], inv_r: 1.0 / (*r as f64).max(1e-3) }, stops),
        _ => return None,
    };
    let stops = prepared(stops);
    // Colour per parameter step (4096 over 0..1, stops padded outside),
    // premultiplied, 8.8 fixed point, packed as 4 × u16 lanes (r, g, b, a).
    const N: usize = 4096;
    let lut: Vec<u64> = (0..N)
        .map(|i| {
            let c = at(&stops, i as f32 / (N - 1) as f32);
            let q = c.map(|v| ((v * 255.0 * 256.0).round() as u64).min(255 * 256));
            q[0] | q[1] << 16 | q[2] << 32 | q[3] << 48
        })
        .collect();
    let tile = dither_tile_fixed();
    let mut pm = pool.and_then(|p| p.pop()).unwrap_or_else(|| Pixmap::new(1, 1));
    pm.resize(w as u16, h as u16);
    let row = |(j, out): (usize, &mut [u8])| {
        let yd = (y0 + j as i64) as f64 + 0.5;
        // Local point of device (x + ½, yd) = base + x·step.
        let xd0 = x0 as f64 + 0.5;
        let (bx, by) = (c[0] * xd0 + c[2] * yd + c[4], c[1] * xd0 + c[3] * yd + c[5]);
        let mut sbuf = [0i32; 256];
        let trow = &tile[((y0 + j as i64 + org.1 as i64) & 63) as usize * 64..][..64];
        let tx0 = ((x0 + org.0 as i64) & 63) as usize;
        let mut i0 = 0;
        while i0 < w {
            let n = (w - i0).min(256);
            let sb = &mut sbuf[..n];
            let top = (N - 1) as f32;
            // Table index along the row (branch-free: vectorises).
            match g {
                G::Linear { p0, d } => {
                    let s0 = ((bx + c[0] * i0 as f64 - p0[0]) * d[0] + (by + c[1] * i0 as f64 - p0[1]) * d[1]) as f32 * top + 0.5;
                    let ds = (c[0] * d[0] + c[1] * d[1]) as f32 * top;
                    idx_linear(s0, ds, top, sb);
                }
                G::Radial { c: ctr, inv_r } => {
                    let (lx0, ly0) = (((bx + c[0] * i0 as f64) - ctr[0]) as f32, ((by + c[1] * i0 as f64) - ctr[1]) as f32);
                    let (sx, sy, ir) = (c[0] as f32, c[1] as f32, inv_r as f32 * top);
                    idx_radial(lx0, ly0, sx, sy, ir, top, sb);
                }
            }
            let o = &mut out[i0 * 4..(i0 + n) * 4];
            for (k, (px, &si)) in o.chunks_exact_mut(4).zip(sb.iter()).enumerate() {
                // Same dither on every lane: premultiplied colour stays ≤ alpha.
                let v = lut[si as usize & (N - 1)] + trow[(tx0 + i0 + k) & 63] * 0x0001_0001_0001_0001;
                let v = (v >> 8) & 0x00ff_00ff_00ff_00ff;
                let p = (v | v >> 8) & 0x0000_ffff_0000_ffff;
                px.copy_from_slice(&((p | p >> 16) as u32).to_le_bytes());
            }
            i0 += n;
        }
    };
    let data = pm.data_as_u8_slice_mut();
    if w * h >= 1 << 15 {
        use rayon::prelude::*;
        data.par_chunks_mut(w * 4).enumerate().for_each(row);
    } else {
        data.chunks_mut(w * 4).enumerate().for_each(row);
    }
    pm.set_may_have_transparency(true);
    Some(pm)
}

/// 64×64 tile of triangular dither, ±½ LSB, anchored to frame pixels.
fn dither_tile() -> &'static [f32] {
    static T: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    T.get_or_init(|| (0..64 * 64).map(|i| super::post::tri_dither((i % 64) as u32, (i / 64) as u32) * 0.5).collect())
}

/// Lane offsets 0..8 for the blocked loops below.
const LANES: [f32; 8] = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];

/// Table indices along a row of a linear gradient: `s0 + ds·k`, clamped.
fn idx_linear(s0: f32, ds: f32, top: f32, out: &mut [i32]) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: feature checked.
        return unsafe { idx_linear_avx2(s0, ds, top, out) };
    }
    idx_linear_body(s0, ds, top, out)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn idx_linear_avx2(s0: f32, ds: f32, top: f32, out: &mut [i32]) {
    idx_linear_body(s0, ds, top, out)
}

#[inline(always)]
fn idx_linear_body(s0: f32, ds: f32, top: f32, out: &mut [i32]) {
    let mut base = 0.0f32;
    let mut chunks = out.chunks_exact_mut(8);
    for c in &mut chunks {
        for j in 0..8 {
            c[j] = (s0 + ds * (base + LANES[j])).clamp(0.0, top) as i32;
        }
        base += 8.0;
    }
    for (j, v) in chunks.into_remainder().iter_mut().enumerate() {
        *v = (s0 + ds * (base + j as f32)).clamp(0.0, top) as i32;
    }
}

/// Table indices along a row of a radial gradient: distance from the centre × `ir`.
#[allow(clippy::too_many_arguments)]
fn idx_radial(lx0: f32, ly0: f32, sx: f32, sy: f32, ir: f32, top: f32, out: &mut [i32]) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: feature checked.
        return unsafe { idx_radial_avx2(lx0, ly0, sx, sy, ir, top, out) };
    }
    idx_radial_body(lx0, ly0, sx, sy, ir, top, out)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
unsafe fn idx_radial_avx2(lx0: f32, ly0: f32, sx: f32, sy: f32, ir: f32, top: f32, out: &mut [i32]) {
    idx_radial_body(lx0, ly0, sx, sy, ir, top, out)
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn idx_radial_body(lx0: f32, ly0: f32, sx: f32, sy: f32, ir: f32, top: f32, out: &mut [i32]) {
    let mut base = 0.0f32;
    let mut chunks = out.chunks_exact_mut(8);
    for c in &mut chunks {
        for j in 0..8 {
            let k = base + LANES[j];
            let (lx, ly) = (lx0 + sx * k, ly0 + sy * k);
            c[j] = ((lx * lx + ly * ly).sqrt() * ir + 0.5).min(top) as i32;
        }
        base += 8.0;
    }
    for (j, v) in chunks.into_remainder().iter_mut().enumerate() {
        let k = base + j as f32;
        let (lx, ly) = (lx0 + sx * k, ly0 + sy * k);
        *v = ((lx * lx + ly * ly).sqrt() * ir + 0.5).min(top) as i32;
    }
}

/// The dither tile in 8.8 fixed point, plus ½ for rounding.
fn dither_tile_fixed() -> &'static [u64] {
    static T: std::sync::OnceLock<Vec<u64>> = std::sync::OnceLock::new();
    // ½ (rounding) plus the dither, in 1/256 LSB; at most 255 so lanes can't carry.
    T.get_or_init(|| dither_tile().iter().map(|d| ((d + 0.5) * 256.0).round().clamp(0.0, 255.0) as u64).collect())
}

/// Cache key for a gradient bitmap.
pub(crate) fn key(paint: &Paint, t: Affine, rect: [i64; 4], org: (i32, i32)) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match paint {
        Paint::Linear { p0, p1, stops } => {
            0u8.hash(&mut h);
            p0.map(f32::to_bits).hash(&mut h);
            p1.map(f32::to_bits).hash(&mut h);
            for (o, c) in stops {
                o.to_bits().hash(&mut h);
                c.0.map(f32::to_bits).hash(&mut h);
            }
        }
        Paint::Radial { c, r, stops } => {
            1u8.hash(&mut h);
            c.map(f32::to_bits).hash(&mut h);
            r.to_bits().hash(&mut h);
            for (o, c) in stops {
                o.to_bits().hash(&mut h);
                c.0.map(f32::to_bits).hash(&mut h);
            }
        }
        _ => 2u8.hash(&mut h),
    }
    t.as_coeffs().map(f64::to_bits).hash(&mut h);
    rect.hash(&mut h);
    ((org.0 as i64 + rect[0]) & 63, (org.1 as i64 + rect[1]) & 63).hash(&mut h);
    h.finish()
}


