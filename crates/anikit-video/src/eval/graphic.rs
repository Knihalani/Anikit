//! Graphic-design geometry: superellipses, blobs, arcs (ring sectors) and
//! generated patterns. Everything is built once at compile time as one filled
//! path, so a pattern of thousands of marks costs one fill per frame.

use kurbo::{BezPath, Cap, Join, PathEl, Point, Shape, Stroke, StrokeOpts};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Superellipse `|x/a|^n + |y/b|^n = 1` in the box `w × h`.
pub fn superellipse(w: f64, h: f64, n: f64) -> BezPath {
    let n = n.clamp(0.2, 60.0);
    let (a, b) = (w * 0.5, h * 0.5);
    let e = 2.0 / n;
    let steps = 400;
    let mut p = BezPath::new();
    for i in 0..steps {
        let t = i as f64 / steps as f64 * TAU;
        let (s, c) = t.sin_cos();
        let pt = (a + a * c.signum() * c.abs().powf(e), b + b * s.signum() * s.abs().powf(e));
        if i == 0 {
            p.move_to(pt)
        } else {
            p.line_to(pt)
        }
    }
    p.close_path();
    p
}

/// Small deterministic hash → [0, 1).
pub fn rnd(seed: u32, i: u32) -> f64 {
    let mut x = seed.wrapping_mul(0x9E37_79B9) ^ i.wrapping_mul(0x85EB_CA6B) ^ 0x27d4_eb2f;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x >> 8) as f64 / (1u32 << 24) as f64
}

/// A smooth organic closed outline with `lobes` control points, filling `w × h`.
pub fn blob(w: f64, h: f64, lobes: u32, wobble: f64, seed: u32) -> BezPath {
    let k = lobes.clamp(3, 64) as usize;
    let wob = wobble.clamp(0.0, 1.0);
    let pts: Vec<Point> = (0..k)
        .map(|i| {
            let step = TAU / k as f64;
            let a = -FRAC_PI_2 + i as f64 * step + (rnd(seed, 2 * i as u32) - 0.5) * step * 0.6 * wob;
            let r = 1.0 - wob * 0.55 * rnd(seed, 2 * i as u32 + 1);
            Point::new(a.cos() * r, a.sin() * r)
        })
        .collect();
    // Closed Catmull-Rom → cubic Béziers.
    let mut p = BezPath::new();
    p.move_to(pts[0]);
    for i in 0..k {
        let p0 = pts[(i + k - 1) % k];
        let p1 = pts[i];
        let p2 = pts[(i + 1) % k];
        let p3 = pts[(i + 2) % k];
        let c1 = p1 + (p2 - p0) / 6.0;
        let c2 = p2 - (p3 - p1) / 6.0;
        p.curve_to(c1, c2, p2);
    }
    p.close_path();
    fit_box(p, w, h)
}

/// Scale a path so its bounding box fills `w × h` at the origin.
fn fit_box(mut p: BezPath, w: f64, h: f64) -> BezPath {
    let bb = p.bounding_box();
    if bb.width() > 0.0 && bb.height() > 0.0 {
        p.apply_affine(kurbo::Affine::translate((-bb.x0, -bb.y0)));
        p.apply_affine(kurbo::Affine::scale_non_uniform(w / bb.width(), h / bb.height()));
    }
    p
}

/// A ring sector (or pie wedge when `inner` = 0) in the ellipse filling `w × h`.
/// Angles in degrees, 0 = 12 o'clock, clockwise.
pub fn arc(w: f64, h: f64, start: f64, end: f64, inner: f64) -> BezPath {
    let (cx, cy, rx, ry) = (w * 0.5, h * 0.5, w * 0.5, h * 0.5);
    let inner = inner.clamp(0.0, 0.999);
    let sweep = (end - start).clamp(-360.0, 360.0);
    let a0 = (start - 90.0).to_radians();
    let sw = sweep.to_radians();
    let full = sweep.abs() >= 359.999;
    let mut p = BezPath::new();
    let outer = kurbo::Arc::new((cx, cy), (rx, ry), a0, sw, 0.0);
    let first = outer.path_elements(0.05).next();
    if let Some(PathEl::MoveTo(q)) = first {
        p.move_to(q);
    }
    outer.to_cubic_beziers(0.05, |c1, c2, q| p.curve_to(c1, c2, q));
    if inner > 0.0 {
        let ia = kurbo::Arc::new((cx, cy), (rx * inner, ry * inner), a0 + sw, -sw, 0.0);
        let start_in = Point::new(cx + rx * inner * (a0 + sw).cos(), cy + ry * inner * (a0 + sw).sin());
        if full {
            p.close_path();
            p.move_to(start_in);
        } else {
            p.line_to(start_in);
        }
        ia.to_cubic_beziers(0.05, |c1, c2, q| p.curve_to(c1, c2, q));
    } else if !full {
        p.line_to((cx, cy));
    }
    p.close_path();
    p
}

/// Pattern kinds.
pub const PATTERNS: &[&str] = &["dots", "grid", "lines", "stripes", "checker", "triangles", "diamonds", "crosses", "rings", "waves", "zigzag", "truchet", "arcs", "scales", "bricks"];

/// Parameters of a pattern.
#[derive(Clone, Debug)]
pub struct Pat {
    pub kind: String,
    pub spacing: f64,
    pub mark: Option<f64>,
    pub angle: f64,
    pub seed: u32,
    pub jitter: f64,
    pub ramp: Option<[f64; 2]>,
}

fn default_mark(kind: &str) -> f64 {
    match kind {
        "dots" => 0.42,
        "grid" => 0.06,
        "lines" => 0.25,
        "stripes" => 0.5,
        "checker" => 1.0,
        "triangles" => 0.86,
        "diamonds" => 0.55,
        "crosses" => 0.14,
        "rings" => 0.08,
        "waves" | "zigzag" => 0.14,
        "truchet" => 0.2,
        "arcs" => 1.0,
        "scales" => 0.07,
        "bricks" => 0.08,
        _ => 0.3,
    }
}

/// A pattern filling `w × h` (marks may overhang; clip the layer to its box).
pub fn pattern(w: f64, h: f64, pt: &Pat) -> BezPath {
    let s = pt.spacing.clamp(2.0, 4096.0);
    let mark = pt.mark.unwrap_or_else(|| default_mark(&pt.kind)).max(0.0);
    let (cx, cy) = (w * 0.5, h * 0.5);
    // Work in a rotated frame covering the box's circumscribed square.
    let half = (w * w + h * h).sqrt() * 0.5 + s;
    let n = (half / s).ceil() as i64 + 1;
    let rot = kurbo::Affine::translate((cx, cy)) * kurbo::Affine::rotate(pt.angle.to_radians());
    // Ramp factor along the rotated x axis.
    let ramp = |x: f64| match pt.ramp {
        Some([a, b]) => {
            let f = ((x + w * 0.5) / w.max(1.0)).clamp(0.0, 1.0);
            a + (b - a) * f
        }
        None => 1.0,
    };
    let mut fill = BezPath::new();
    let mut line = BezPath::new();
    let mut idx = 0u32;
    let mut jit = |i: i64, j: i64| {
        idx = idx.wrapping_add(1);
        if pt.jitter > 0.0 {
            let k = ((i as u32) << 16) ^ (j as u32);
            ((rnd(pt.seed, k.wrapping_mul(2)) - 0.5) * pt.jitter * s, (rnd(pt.seed, k.wrapping_mul(2) + 1) - 0.5) * pt.jitter * s)
        } else {
            (0.0, 0.0)
        }
    };
    let lw = |m: f64| m * s;
    let mut stroke_w = lw(mark);
    let poly = |p: &mut BezPath, pts: &[(f64, f64)]| {
        for (k, q) in pts.iter().enumerate() {
            if k == 0 {
                p.move_to(*q)
            } else {
                p.line_to(*q)
            }
        }
        p.close_path();
    };
    let lo = -n;
    let hi = n;
    match pt.kind.as_str() {
        "dots" => {
            for j in lo..=hi {
                for i in lo..=hi {
                    let (jx, jy) = jit(i, j);
                    let x = i as f64 * s + jx;
                    let y = j as f64 * s + jy;
                    let r = 0.5 * mark * s * ramp(x);
                    if r > 0.05 {
                        fill.extend(kurbo::Circle::new((x, y), r).path_elements(0.05));
                    }
                }
            }
        }
        "diamonds" | "checker" | "triangles" | "bricks" | "arcs" => {
            for j in lo..=hi {
                for i in lo..=hi {
                    let (jx, jy) = jit(i, j);
                    let x = i as f64 * s + jx;
                    let y = j as f64 * s + jy;
                    let k = mark * ramp(x);
                    match pt.kind.as_str() {
                        "diamonds" => {
                            let r = 0.5 * s * k;
                            poly(&mut fill, &[(x, y - r), (x + r, y), (x, y + r), (x - r, y)]);
                        }
                        "checker" => {
                            if (i + j).rem_euclid(2) == 0 {
                                let e = 0.5 * s * (1.0 - k.min(1.0));
                                poly(&mut fill, &[(x + e, y + e), (x + s - e, y + e), (x + s - e, y + s - e), (x + e, y + s - e)]);
                            }
                        }
                        "triangles" => {
                            let hh = s * k;
                            let x0 = x + if j.rem_euclid(2) == 0 { 0.0 } else { s * 0.5 };
                            poly(&mut fill, &[(x0, y + hh * 0.5), (x0 + s * 0.5 * k, y - hh * 0.5), (x0 + s * k, y + hh * 0.5)]);
                        }
                        "bricks" => {
                            let g = 0.5 * s * k;
                            let bw = s * 2.0;
                            if i.rem_euclid(2) != 0 {
                                continue;
                            }
                            let x0 = x + if j.rem_euclid(2) == 0 { 0.0 } else { s };
                            poly(&mut fill, &[(x0 + g, y + g), (x0 + bw - g, y + g), (x0 + bw - g, y + s - g), (x0 + g, y + s - g)]);
                        }
                        _ => {
                            // arcs: a quarter disc in a random corner of each cell.
                            let c = (rnd(pt.seed, (((i as u32) << 16) ^ (j as u32)).wrapping_add(7919)) * 4.0) as u32;
                            let (ox, oy) = [(x, y), (x + s, y), (x + s, y + s), (x, y + s)][c as usize % 4];
                            let a0 = [0.0, FRAC_PI_2, PI, -FRAC_PI_2][c as usize % 4];
                            let r = s * k.min(1.0);
                            fill.move_to((ox, oy));
                            let a = kurbo::Arc::new((ox, oy), (r, r), a0, FRAC_PI_2, 0.0);
                            fill.line_to((ox + r * a0.cos(), oy + r * a0.sin()));
                            a.to_cubic_beziers(0.05, |c1, c2, q| fill.curve_to(c1, c2, q));
                            fill.close_path();
                        }
                    }
                }
            }
        }
        "lines" | "stripes" | "grid" | "waves" | "zigzag" => {
            for j in lo..=hi {
                let y = j as f64 * s;
                match pt.kind.as_str() {
                    "waves" | "zigzag" => {
                        let amp = s * 0.3;
                        let wl = s * 2.0;
                        let steps_per = if pt.kind == "waves" { 16 } else { 2 };
                        let x0 = -half - wl;
                        let count = ((2.0 * half + 2.0 * wl) / wl).ceil() as i64 * steps_per;
                        for k in 0..=count {
                            let x = x0 + k as f64 * wl / steps_per as f64;
                            let ph = k as f64 / steps_per as f64 * TAU;
                            let dy = if pt.kind == "waves" { ph.sin() * amp } else if k % 2 == 0 { -amp } else { amp };
                            if k == 0 {
                                line.move_to((x, y + dy))
                            } else {
                                line.line_to((x, y + dy))
                            }
                        }
                    }
                    _ => {
                        if pt.kind == "stripes" {
                            stroke_w = lw(mark);
                        }
                        line.move_to((-half, y));
                        line.line_to((half, y));
                        if pt.kind == "grid" {
                            line.move_to((y, -half));
                            line.line_to((y, half));
                        }
                    }
                }
            }
            if pt.ramp.is_some() && matches!(pt.kind.as_str(), "lines" | "stripes") {
                // Ramped line weight: build each line as a tapered quad instead.
                line = BezPath::new();
                for j in lo..=hi {
                    let y = j as f64 * s;
                    let (a, b) = (0.5 * s * mark * ramp(-half), 0.5 * s * mark * ramp(half));
                    poly(&mut fill, &[(-half, y - a), (half, y - b), (half, y + b), (-half, y + a)]);
                }
            }
        }
        "crosses" => {
            for j in lo..=hi {
                for i in lo..=hi {
                    let (jx, jy) = jit(i, j);
                    let x = i as f64 * s + jx;
                    let y = j as f64 * s + jy;
                    let arm = s * 0.25 * ramp(x);
                    line.move_to((x - arm, y));
                    line.line_to((x + arm, y));
                    line.move_to((x, y - arm));
                    line.line_to((x, y + arm));
                }
            }
        }
        "rings" | "scales" => {
            for j in lo..=hi {
                for i in lo..=hi {
                    let (jx, jy) = jit(i, j);
                    if pt.kind == "rings" {
                        let x = i as f64 * s + jx;
                        let y = j as f64 * s + jy;
                        line.extend(kurbo::Circle::new((x, y), s * 0.35 * ramp(x)).path_elements(0.05));
                    } else {
                        let x = i as f64 * s + if j.rem_euclid(2) == 0 { 0.0 } else { s * 0.5 } + jx;
                        let y = j as f64 * s * 0.5 + jy;
                        let a = kurbo::Arc::new((x, y), (s * 0.5, s * 0.5), 0.0, PI, 0.0);
                        line.move_to((x + s * 0.5, y));
                        a.to_cubic_beziers(0.05, |c1, c2, q| line.curve_to(c1, c2, q));
                    }
                }
            }
        }
        _ => {
            // truchet: two quarter arcs per cell, in one of two orientations.
            for j in lo..=hi {
                for i in lo..=hi {
                    let x = i as f64 * s;
                    let y = j as f64 * s;
                    let flip = rnd(pt.seed, ((i as u32) << 16) ^ (j as u32)) < 0.5;
                    let r = s * 0.5;
                    let corners = if flip { [((x, y), 0.0), ((x + s, y + s), PI)] } else { [((x + s, y), FRAC_PI_2), ((x, y + s), -FRAC_PI_2)] };
                    for (c, a0) in corners {
                        let a = kurbo::Arc::new(c, (r, r), a0, FRAC_PI_2, 0.0);
                        line.move_to((c.0 + r * a0.cos(), c.1 + r * a0.sin()));
                        a.to_cubic_beziers(0.05, |c1, c2, q| line.curve_to(c1, c2, q));
                    }
                }
            }
        }
    }
    if !line.elements().is_empty() && stroke_w > 0.0 {
        let st = Stroke::new(stroke_w).with_caps(Cap::Butt).with_join(Join::Miter);
        let outline = kurbo::stroke(line, &st, &StrokeOpts::default(), 0.05);
        fill.extend(outline);
    }
    fill.apply_affine(rot);
    fill
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_fill_their_box() {
        for p in [superellipse(300.0, 200.0, 4.0), blob(300.0, 200.0, 7, 0.4, 3), arc(300.0, 200.0, 0.0, 360.0, 0.0)] {
            let bb = p.bounding_box();
            assert!(bb.x0 > -1.0 && bb.y0 > -1.0 && bb.x1 < 301.0 && bb.y1 < 201.0 && bb.width() > 290.0, "{bb:?}");
        }
        // Squircle area between ellipse and rectangle.
        let a = superellipse(100.0, 100.0, 4.0).area().abs();
        assert!(a > PI * 2500.0 && a < 10000.0, "{a}");
        // Half ring: half the annulus.
        let r = arc(200.0, 200.0, 0.0, 180.0, 0.5).area().abs();
        let want = 0.5 * PI * (100.0f64.powi(2) - 50.0f64.powi(2));
        assert!((r - want).abs() / want < 0.01, "{r} vs {want}");
    }

    #[test]
    fn patterns_are_deterministic_and_cover() {
        for k in PATTERNS {
            let pt = Pat { kind: k.to_string(), spacing: 40.0, mark: None, angle: 15.0, seed: 2, jitter: 0.0, ramp: None };
            let a = pattern(400.0, 300.0, &pt);
            let b = pattern(400.0, 300.0, &pt);
            assert_eq!(a, b, "{k}");
            let bb = a.bounding_box();
            assert!(bb.x0 <= 0.0 && bb.y0 <= 0.0 && bb.x1 >= 400.0 && bb.y1 >= 300.0, "{k}: {bb:?}");
        }
    }
}

/// A resolved modular grid inside a `w × h` box.
#[derive(Clone, Debug, PartialEq)]
pub struct GridC {
    pub cols: u32,
    pub rows: u32,
    /// top, right, bottom, left
    pub margin: [f32; 4],
    pub gutter: [f32; 2],
    pub baseline: Option<f32>,
    pub size: [f32; 2],
}

impl GridC {
    pub fn col_w(&self) -> f32 {
        (self.size[0] - self.margin[1] - self.margin[3] - (self.cols as f32 - 1.0) * self.gutter[0]) / self.cols as f32
    }
    pub fn row_h(&self) -> f32 {
        (self.size[1] - self.margin[0] - self.margin[2] - (self.rows as f32 - 1.0) * self.gutter[1]) / self.rows as f32
    }
    /// Rect `[x, y, w, h]` of columns `c0..=c1`, rows `r0..=r1` (1-based).
    pub fn rect(&self, c0: u32, c1: u32, r0: u32, r1: u32) -> [f32; 4] {
        let (cw, rh) = (self.col_w(), self.row_h());
        let x = self.margin[3] + (c0 - 1) as f32 * (cw + self.gutter[0]);
        let y = self.margin[0] + (r0 - 1) as f32 * (rh + self.gutter[1]);
        let nc = (c1 - c0 + 1) as f32;
        let nr = (r1 - r0 + 1) as f32;
        [x, y, nc * cw + (nc - 1.0) * self.gutter[0], nr * rh + (nr - 1.0) * self.gutter[1]]
    }
}

/// Parse a cell range: `"3"`, `"1-6"`, `"1-6/2-4"`, `"7/1"` → (c0, c1, r0, r1), 1-based inclusive.
/// A range with no rows spans all rows. `-1` style indices are not supported; `"6-"` means to the end.
pub fn parse_cell(s: &str, cols: u32, rows: u32) -> Result<(u32, u32, u32, u32), String> {
    let range = |t: &str, max: u32| -> Result<(u32, u32), String> {
        let t = t.trim();
        let (a, b) = match t.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (t, t),
        };
        let a: u32 = a.parse().map_err(|_| format!("'{t}' is not a number or range like 2-5"))?;
        let b: u32 = if b.is_empty() { max } else { b.parse().map_err(|_| format!("'{t}' is not a number or range like 2-5"))? };
        if a < 1 || b < a || b > max {
            return Err(format!("'{t}' is outside 1..{max}"));
        }
        Ok((a, b))
    };
    let (c, r) = match s.split_once('/') {
        Some((c, r)) => (c, Some(r)),
        None => (s, None),
    };
    let (c0, c1) = range(c, cols)?;
    let (r0, r1) = match r {
        Some(r) => range(r, rows)?,
        None => (1, rows),
    };
    Ok((c0, c1, r0, r1))
}

#[cfg(test)]
mod grid_tests {
    use super::*;

    #[test]
    fn grid_cells() {
        let g = GridC { cols: 12, rows: 8, margin: [54.0, 96.0, 54.0, 96.0], gutter: [24.0, 24.0], baseline: None, size: [1920.0, 1080.0] };
        let full = g.rect(1, 12, 1, 8);
        assert!((full[0] - 96.0).abs() < 1e-3 && (full[2] - 1728.0).abs() < 1e-3 && (full[3] - 972.0).abs() < 1e-3);
        let a = g.rect(1, 6, 1, 1);
        let b = g.rect(7, 12, 1, 1);
        assert!((b[0] - (a[0] + a[2] + 24.0)).abs() < 1e-3);
        assert_eq!(parse_cell("1-6/2-4", 12, 8), Ok((1, 6, 2, 4)));
        assert_eq!(parse_cell("7/1", 12, 8), Ok((7, 7, 1, 1)));
        assert_eq!(parse_cell("9-", 12, 8), Ok((9, 12, 1, 8)));
        assert!(parse_cell("0-3", 12, 8).is_err() && parse_cell("1-13", 12, 8).is_err());
    }
}
