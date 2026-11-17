//! Smooth backdrops: the leading run of top-level ops that has no visible
//! edges inside the frame (full-frame gradient or solid fills, upscaled
//! images, gradient "glows" whose alpha reaches zero before their path edge)
//! is rendered at ¼ resolution and bilinearly upscaled. Smooth fields lose
//! nothing visible, and three full-frame radial gradients cost a sixteenth.

use kurbo::{Affine, BezPath, ParamCurveNearest, PathEl, Point, Rect, Shape};

use super::{affine, op_ok};
use crate::dl::{Op, Paint, Rgba};

/// Downscale factor of the low-resolution backdrop.
pub const FACTOR: u32 = 4;

/// Smallest on-screen distance (px) between two different stop colours we
/// accept: closer stops form a visible edge.
const MIN_STOP_GAP_PX: f64 = 24.0;

fn uniform_scale(t: &Affine) -> Option<f64> {
    let [a, b, c, d, _, _] = t.as_coeffs();
    (b == 0.0 && c == 0.0 && a > 0.0 && (a - d).abs() <= 1e-6 * a).then_some(a)
}

fn stops_smooth(stops: &[(f32, Rgba)], len_px: f64) -> bool {
    stops.windows(2).all(|w| w[0].1 == w[1].1 || ((w[1].0 - w[0].0) as f64) * len_px >= MIN_STOP_GAP_PX)
}

/// Offset beyond which a stop list is fully transparent.
fn zero_from(stops: &[(f32, Rgba)]) -> Option<f32> {
    let i = stops.iter().rposition(|s| s.1 .0[3] > 0.0)?;
    stops.get(i + 1).map(|s| s.0)
}

/// Whether `paint` (under device scale `s`) is smooth.
fn paint_smooth(p: &Paint, s: f64) -> bool {
    match p {
        Paint::Solid(_) => true,
        Paint::Linear { p0, p1, stops } => {
            let len = ((p1[0] - p0[0]) as f64).hypot((p1[1] - p0[1]) as f64) * s;
            stops_smooth(stops, len)
        }
        Paint::Radial { r, stops, .. } => stops_smooth(stops, *r as f64 * s),
        Paint::Image { .. } => false,
    }
}

/// Whether `r` (device) covers the whole frame.
fn covers(r: Rect, w: f64, h: f64) -> bool {
    r.x0 <= 0.0 && r.y0 <= 0.0 && r.x1 >= w && r.y1 >= h
}

/// Whether `path` is an axis-aligned rectangle.
fn path_rect(path: &BezPath) -> Option<Rect> {
    let mut pts: Vec<Point> = Vec::with_capacity(5);
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) if pts.is_empty() => pts.push(p),
            PathEl::LineTo(p) => pts.push(p),
            PathEl::ClosePath => {}
            _ => return None,
        }
    }
    if pts.len() == 5 && pts[4] == pts[0] {
        pts.pop();
    }
    if pts.len() != 4 {
        return None;
    }
    let bb = path.bounding_box();
    pts.iter().all(|p| (p.x == bb.x0 || p.x == bb.x1) && (p.y == bb.y0 || p.y == bb.y1)).then_some(bb)
}

/// Whether a radial paint is transparent everywhere outside `path` (its
/// visible disk lies inside the path), so the path's edge cannot be seen.
fn edge_hidden(path: &BezPath, paint: &Paint) -> bool {
    let Paint::Radial { c, r, stops } = paint else { return false };
    let Some(tz) = zero_from(stops) else { return true }; // fully transparent
    let c = Point::new(c[0] as f64, c[1] as f64);
    let need = tz as f64 * *r as f64;
    path.winding(c) != 0 && path.segments().all(|s| s.nearest(c, 1e-3).distance_sq.sqrt() >= need)
}

/// Number of leading top-level ops of `ops` forming a smooth backdrop for a
/// `w × h` frame, and whether it's worth rendering at low resolution.
pub fn prefix(ops: &[Op], w: f64, h: f64, image_size: &dyn Fn(crate::dl::ImageId) -> Option<(u32, u32)>) -> usize {
    let mut n = 0;
    let mut area = 0.0;
    for op in ops {
        if !op_ok(op, Affine::IDENTITY) {
            break;
        }
        let ok = match op {
            Op::RoundRect { x, y, w: rw, h: rh, radius, paint, transform } => {
                let t = affine(transform);
                let dev = t.transform_rect_bbox(Rect::new(*x as f64, *y as f64, (*x + *rw) as f64, (*y + *rh) as f64));
                // Rounded corners must lie outside the frame.
                uniform_scale(&t).is_some_and(|s| paint_smooth(paint, s) && covers(dev.inflate(-(radius.max(0.0) as f64) * s, -(radius.max(0.0) as f64) * s), w, h))
                    && {
                        area += dev.area().min(w * h);
                        true
                    }
            }
            Op::Fill { path, paint, transform, .. } => {
                let t = affine(transform);
                uniform_scale(&t).is_some_and(|s| paint_smooth(paint, s))
                    && (path_rect(path).is_some_and(|r| covers(t.transform_rect_bbox(r), w, h)) || edge_hidden(path, paint))
                    && {
                        area += t.transform_rect_bbox(path.bounding_box()).intersect(Rect::new(0.0, 0.0, w, h)).area();
                        true
                    }
            }
            Op::Image { image, dst, opacity, transform } => {
                // Low-res source images (fields) blown up ≥ 4×.
                let t = affine(transform);
                let dev = t.transform_rect_bbox(Rect::new(dst[0] as f64, dst[1] as f64, (dst[0] + dst[2]) as f64, (dst[1] + dst[3]) as f64));
                let low_res = image_size(*image).is_some_and(|(iw, ih)| dev.width() >= 4.0 * iw as f64 && dev.height() >= 4.0 * ih as f64);
                *opacity >= 1.0 && low_res && uniform_scale(&t).is_some() && covers(dev, w, h) && {
                    area += w * h;
                    true
                }
            }
            _ => false,
        };
        if !ok {
            break;
        }
        n += 1;
    }
    // Worth it once the run paints a frame's worth of pixels with at least one
    // gradient or image (a solid full-frame fill alone is already cheap).
    let rich = ops[..n].iter().any(|o| match o {
        Op::Fill { paint, .. } | Op::RoundRect { paint, .. } => !matches!(paint, Paint::Solid(_)),
        _ => true,
    });
    if rich && area >= w * h {
        n
    } else {
        0
    }
}

/// One smooth op as a per-pixel colour field.
struct FieldOp {
    /// Device → paint coordinates.
    inv: Affine,
    kind: u8, // 0 solid, 1 linear, 2 radial
    a: [f64; 2],
    d: [f64; 2],
    inv_len2: f64,
    /// Premultiplied straight-sRGB colour ramp over t ∈ 0..1.
    ramp: Vec<[f32; 4]>,
}

const RAMP: usize = 1024;

fn ramp(stops: &[(f32, Rgba)]) -> Vec<[f32; 4]> {
    (0..=RAMP)
        .map(|i| {
            let t = i as f32 / RAMP as f32;
            let c = match stops.iter().position(|s| s.0 >= t) {
                None => stops.last().map_or([0.0; 4], |s| s.1 .0),
                Some(0) => stops[0].1 .0,
                Some(k) => {
                    let (a, b) = (&stops[k - 1], &stops[k]);
                    let f = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 1.0 };
                    std::array::from_fn(|j| a.1 .0[j] + (b.1 .0[j] - a.1 .0[j]) * f)
                }
            };
            [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
        })
        .collect()
}

/// Whether [`render_field`] can evaluate these ops (gradients and solids only).
pub fn field_ok(ops: &[Op]) -> bool {
    ops.iter().all(|o| match o {
        Op::Fill { paint, .. } | Op::RoundRect { paint, .. } => !matches!(paint, Paint::Image { .. }),
        _ => false,
    })
}

/// Evaluate a smooth prefix (see [`prefix`]) as unbounded colour fields in
/// float, composite source-over onto `bg`, and write dithered 8-bit RGBA:
/// no 8-bit steps inside the gradients, so dark backgrounds don't band.
pub fn render_field(ops: &[Op], w: usize, h: usize, bg: Rgba, out: &mut [u8]) {
    assert!(out.len() >= w * h * 4);
    let fields: Vec<FieldOp> = ops
        .iter()
        .filter_map(|o| {
            let (paint, t) = match o {
                Op::Fill { paint, transform, .. } | Op::RoundRect { paint, transform, .. } => (paint, affine(transform)),
                _ => return None,
            };
            let inv = t.inverse();
            Some(match paint {
                Paint::Solid(c) => FieldOp { inv, kind: 0, a: [0.0; 2], d: [0.0; 2], inv_len2: 0.0, ramp: ramp(&[(0.0, *c)]) },
                Paint::Linear { p0, p1, stops } => {
                    let d = [(p1[0] - p0[0]) as f64, (p1[1] - p0[1]) as f64];
                    let l2 = d[0] * d[0] + d[1] * d[1];
                    FieldOp { inv, kind: 1, a: [p0[0] as f64, p0[1] as f64], d, inv_len2: if l2 > 0.0 { 1.0 / l2 } else { 0.0 }, ramp: ramp(stops) }
                }
                Paint::Radial { c, r, stops } => {
                    FieldOp { inv, kind: 2, a: [c[0] as f64, c[1] as f64], d: [0.0; 2], inv_len2: 1.0 / (*r as f64).max(1e-6), ramp: ramp(stops) }
                }
                Paint::Image { .. } => return None,
            })
        })
        .collect();
    let bgp = [bg.0[0] * bg.0[3], bg.0[1] * bg.0[3], bg.0[2] * bg.0[3], bg.0[3]];
    use rayon::prelude::*;
    out.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for (x, px) in row.chunks_exact_mut(4).enumerate() {
            let p = kurbo::Point::new(x as f64 + 0.5, y as f64 + 0.5);
            let mut acc = bgp;
            for f in &fields {
                let t = match f.kind {
                    0 => 0.0,
                    1 => {
                        let q = f.inv * p;
                        ((q.x - f.a[0]) * f.d[0] + (q.y - f.a[1]) * f.d[1]) * f.inv_len2
                    }
                    _ => {
                        let q = f.inv * p;
                        ((q.x - f.a[0]).hypot(q.y - f.a[1])) * f.inv_len2
                    }
                };
                let s = (t.clamp(0.0, 1.0) * RAMP as f64) as f32;
                let i = (s as usize).min(RAMP - 1);
                let fr = s - i as f32;
                let (c0, c1) = (f.ramp[i], f.ramp[(i + 1).min(RAMP)]);
                let c: [f32; 4] = std::array::from_fn(|k| c0[k] + (c1[k] - c0[k]) * fr);
                let ia = 1.0 - c[3];
                for k in 0..4 {
                    acc[k] = c[k] + acc[k] * ia;
                }
            }
            let d = super::post::tri_dither(x as u32, y as u32) * 0.5;
            for k in 0..3 {
                px[k] = (acc[k] * 255.0 + d + 0.5).clamp(0.0, 255.0) as u8;
            }
            px[3] = (acc[3] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    });
}
