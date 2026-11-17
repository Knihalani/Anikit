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
