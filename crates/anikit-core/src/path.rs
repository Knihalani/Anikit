//! SVG path geometry: parsing, measuring, sampling, motion-path lookup tables
//! and shape morphing.
//!
//! All path commands (including arcs and smooth curves) are normalised to
//! absolute cubic Bézier segments, which makes every downstream operation
//! (length, sampling, splitting, morphing) uniform.

use alloc::string::String;
use alloc::vec::Vec;

use crate::math::{abs, atan2, cos, fmod_pos, sin, sqrt, tan, DEG, PI, RAD, TAU};
use crate::util::{push_num, Cursor};

/// A cubic segment: control point 1, control point 2, end point. The start
/// point is the previous segment's end (or the sub-path start).
pub type Seg = [f64; 6];

#[derive(Clone, Debug, PartialEq)]
pub struct SubPath {
    pub start: [f64; 2],
    pub segs: Vec<Seg>,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    pub subpaths: Vec<SubPath>,
}

#[inline]
fn line_seg(x0: f64, y0: f64, x1: f64, y1: f64) -> Seg {
    [x0 + (x1 - x0) / 3.0, y0 + (y1 - y0) / 3.0, x0 + (x1 - x0) * 2.0 / 3.0, y0 + (y1 - y0) * 2.0 / 3.0, x1, y1]
}

#[inline]
pub fn cubic_point(p0: [f64; 2], s: &Seg, t: f64) -> [f64; 2] {
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    [a * p0[0] + b * s[0] + c * s[2] + d * s[4], a * p0[1] + b * s[1] + c * s[3] + d * s[5]]
}

#[inline]
pub fn cubic_deriv(p0: [f64; 2], s: &Seg, t: f64) -> [f64; 2] {
    let mt = 1.0 - t;
    let a = 3.0 * mt * mt;
    let b = 6.0 * mt * t;
    let c = 3.0 * t * t;
    [
        a * (s[0] - p0[0]) + b * (s[2] - s[0]) + c * (s[4] - s[2]),
        a * (s[1] - p0[1]) + b * (s[3] - s[1]) + c * (s[5] - s[3]),
    ]
}

/// Split a cubic at `t`, returning the two halves (each with its own start).
pub fn split_cubic(p0: [f64; 2], s: &Seg, t: f64) -> (Seg, [f64; 2], Seg) {
    let l = |a: f64, b: f64| a + (b - a) * t;
    let (x01, y01) = (l(p0[0], s[0]), l(p0[1], s[1]));
    let (x12, y12) = (l(s[0], s[2]), l(s[1], s[3]));
    let (x23, y23) = (l(s[2], s[4]), l(s[3], s[5]));
    let (xa, ya) = (l(x01, x12), l(y01, y12));
    let (xb, yb) = (l(x12, x23), l(y12, y23));
    let (xm, ym) = (l(xa, xb), l(ya, yb));
    ([x01, y01, xa, ya, xm, ym], [xm, ym], [xb, yb, x23, y23, s[4], s[5]])
}

// Gauss–Legendre 8-point nodes/weights on [0, 1].
const GL_X: [f64; 8] = [
    0.019855071751231856, 0.10166676129318664, 0.2372337950418355, 0.4082826787521751, 0.591_717_321_247_825,
    0.7627662049581645, 0.8983332387068134, 0.9801449282487681,
];
const GL_W: [f64; 8] = [
    0.05061426814518813, 0.11119051722668724, 0.15685332293894363, 0.181_341_891_689_181, 0.181_341_891_689_181,
    0.15685332293894363, 0.11119051722668724, 0.05061426814518813,
];

/// Arc length of a cubic segment between parameters `0` and `t`.
pub fn cubic_length(p0: [f64; 2], s: &Seg, t: f64) -> f64 {
    let mut len = 0.0;
    for i in 0..8 {
        let d = cubic_deriv(p0, s, GL_X[i] * t);
        len += GL_W[i] * sqrt(d[0] * d[0] + d[1] * d[1]);
    }
    len * t
}

/// Convert an SVG elliptical arc to cubic segments (appended to `out`).
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
    x1: f64,
    y1: f64,
    mut rx: f64,
    mut ry: f64,
    angle: f64,
    large: bool,
    sweep: bool,
    x2: f64,
    y2: f64,
    out: &mut Vec<Seg>,
) {
    if (x1 == x2 && y1 == y2) || rx == 0.0 || ry == 0.0 {
        if x1 != x2 || y1 != y2 {
            out.push(line_seg(x1, y1, x2, y2));
        }
        return;
    }
    rx = abs(rx);
    ry = abs(ry);
    let phi = angle * RAD;
    let (sp, cp) = (sin(phi), cos(phi));
    let dx = (x1 - x2) / 2.0;
    let dy = (y1 - y2) / 2.0;
    let x1p = cp * dx + sp * dy;
    let y1p = -sp * dx + cp * dy;
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = sqrt(lambda);
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
    let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
    let mut coef = if den == 0.0 { 0.0 } else { sqrt((num / den).max(0.0)) };
    if large == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;
    let cx = cp * cxp - sp * cyp + (x1 + x2) / 2.0;
    let cy = sp * cxp + cp * cyp + (y1 + y2) / 2.0;
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| atan2(ux * vy - uy * vx, ux * vx + uy * vy);
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = ang(1.0, 0.0, ux, uy);
    let mut dtheta = ang(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += TAU;
    }
    let nseg = crate::math::ceil(abs(dtheta) / (PI / 2.0) - 1e-9).max(1.0) as usize;
    let delta = dtheta / nseg as f64;
    let k = 4.0 / 3.0 * tan(delta / 4.0);
    let mut t = theta1;
    let (mut px, mut py) = (x1, y1);
    for i in 0..nseg {
        let (c1, s1) = (cos(t), sin(t));
        let t2 = t + delta;
        let (c2, s2) = (cos(t2), sin(t2));
        // Unit-circle control points, then scale/rotate/translate.
        let e = |x: f64, y: f64| -> (f64, f64) {
            let x = x * rx;
            let y = y * ry;
            (cp * x - sp * y + cx, sp * x + cp * y + cy)
        };
        let (q1x, q1y) = e(c1 - k * s1, s1 + k * c1);
        let (q2x, q2y) = e(c2 + k * s2, s2 - k * c2);
        let (ex, ey) = if i + 1 == nseg { (x2, y2) } else { e(c2, s2) };
        let _ = (px, py);
        out.push([q1x, q1y, q2x, q2y, ex, ey]);
        px = ex;
        py = ey;
        t = t2;
    }
}

impl Path {
    /// Parse SVG path data (`d` attribute). Invalid trailing data is ignored,
    /// matching browser error recovery.
    pub fn parse(d: &str) -> Path {
        let mut c = Cursor::new(d);
        let mut subs: Vec<SubPath> = Vec::new();
        let mut cur = SubPath { start: [0.0, 0.0], segs: Vec::new(), closed: false };
        let (mut x, mut y) = (0.0f64, 0.0f64);
        let (mut sx, mut sy) = (0.0f64, 0.0f64);
        // Last control point for smooth curves, and which kind produced it.
        let (mut lcx, mut lcy) = (0.0f64, 0.0f64);
        let mut last_kind = b' ';
        let mut cmd = b' ';
        let mut started = false;

        let flush = |cur: &mut SubPath, subs: &mut Vec<SubPath>| {
            if !cur.segs.is_empty() || cur.closed {
                subs.push(core::mem::replace(cur, SubPath { start: [0.0, 0.0], segs: Vec::new(), closed: false }));
            }
        };

        loop {
            c.skip_sep();
            let Some(ch) = c.peek() else { break };
            if ch.is_ascii_alphabetic() {
                cmd = ch;
                c.i += 1;
                if cmd == b'z' || cmd == b'Z' {
                    if started {
                        if abs(x - sx) > 1e-9 || abs(y - sy) > 1e-9 {
                            cur.segs.push(line_seg(x, y, sx, sy));
                        }
                        cur.closed = true;
                        flush(&mut cur, &mut subs);
                        cur.start = [sx, sy];
                    }
                    x = sx;
                    y = sy;
                    last_kind = b'Z';
                    continue;
                }
            } else if cmd == b' ' {
                break;
            }
            let rel = cmd.is_ascii_lowercase();
            let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
            macro_rules! num {
                () => {{
                    c.skip_sep();
                    match c.number() {
                        Some(v) => v,
                        None => break,
                    }
                }};
            }
            macro_rules! flag {
                () => {{
                    c.skip_sep();
                    match c.peek() {
                        Some(b'0') => {
                            c.i += 1;
                            false
                        }
                        Some(b'1') => {
                            c.i += 1;
                            true
                        }
                        _ => break,
                    }
                }};
            }
            let upper = cmd.to_ascii_uppercase();
            match upper {
                b'M' => {
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    flush(&mut cur, &mut subs);
                    cur.start = [nx, ny];
                    cur.closed = false;
                    x = nx;
                    y = ny;
                    sx = nx;
                    sy = ny;
                    started = true;
                    // Subsequent pairs are implicit line-tos.
                    cmd = if rel { b'l' } else { b'L' };
                    last_kind = b'M';
                }
                b'L' => {
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    cur.segs.push(line_seg(x, y, nx, ny));
                    x = nx;
                    y = ny;
                    last_kind = b'L';
                }
                b'H' => {
                    let nx = num!() + ox;
                    cur.segs.push(line_seg(x, y, nx, y));
                    x = nx;
                    last_kind = b'L';
                }
                b'V' => {
                    let ny = num!() + oy;
                    cur.segs.push(line_seg(x, y, x, ny));
                    y = ny;
                    last_kind = b'L';
                }
                b'C' => {
                    let x1 = num!() + ox;
                    let y1 = num!() + oy;
                    let x2 = num!() + ox;
                    let y2 = num!() + oy;
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    cur.segs.push([x1, y1, x2, y2, nx, ny]);
                    lcx = x2;
                    lcy = y2;
                    x = nx;
                    y = ny;
                    last_kind = b'C';
                }
                b'S' => {
                    let x2 = num!() + ox;
                    let y2 = num!() + oy;
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    let (x1, y1) = if last_kind == b'C' { (2.0 * x - lcx, 2.0 * y - lcy) } else { (x, y) };
                    cur.segs.push([x1, y1, x2, y2, nx, ny]);
                    lcx = x2;
                    lcy = y2;
                    x = nx;
                    y = ny;
                    last_kind = b'C';
                }
                b'Q' => {
                    let qx = num!() + ox;
                    let qy = num!() + oy;
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    cur.segs.push(quad_to_cubic(x, y, qx, qy, nx, ny));
                    lcx = qx;
                    lcy = qy;
                    x = nx;
                    y = ny;
                    last_kind = b'Q';
                }
                b'T' => {
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    let (qx, qy) = if last_kind == b'Q' { (2.0 * x - lcx, 2.0 * y - lcy) } else { (x, y) };
                    cur.segs.push(quad_to_cubic(x, y, qx, qy, nx, ny));
                    lcx = qx;
                    lcy = qy;
                    x = nx;
                    y = ny;
                    last_kind = b'Q';
                }
                b'A' => {
                    let rx = num!();
                    let ry = num!();
                    let rot = num!();
                    let large = flag!();
                    let sweep = flag!();
                    let nx = num!() + ox;
                    let ny = num!() + oy;
                    arc_to_cubics(x, y, rx, ry, rot, large, sweep, nx, ny, &mut cur.segs);
                    x = nx;
                    y = ny;
                    last_kind = b'A';
                }
                _ => break,
            }
            if upper != b'M' && !started {
                started = true;
                cur.start = [0.0, 0.0];
            }
        }
        flush(&mut cur, &mut subs);
        Path { subpaths: subs }
    }

    /// Serialise as absolute `M ... C ... Z` path data.
    pub fn to_svg(&self, decimals: u32) -> String {
        let mut s = String::new();
        for sp in &self.subpaths {
            write_subpath(&mut s, sp.start, &sp.segs, sp.closed, decimals);
        }
        s
    }

    pub fn segment_count(&self) -> usize {
        self.subpaths.iter().map(|s| s.segs.len()).sum()
    }

    /// Total arc length.
    pub fn length(&self) -> f64 {
        let mut len = 0.0;
        for sp in &self.subpaths {
            let mut p0 = sp.start;
            for s in &sp.segs {
                len += cubic_length(p0, s, 1.0);
                p0 = [s[4], s[5]];
            }
        }
        len
    }

    /// Axis-aligned bounding box `[min_x, min_y, max_x, max_y]` (exact, using
    /// the cubic extrema).
    pub fn bounds(&self) -> [f64; 4] {
        let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        let mut add = |p: [f64; 2]| {
            b[0] = b[0].min(p[0]);
            b[1] = b[1].min(p[1]);
            b[2] = b[2].max(p[0]);
            b[3] = b[3].max(p[1]);
        };
        for sp in &self.subpaths {
            let mut p0 = sp.start;
            add(p0);
            for s in &sp.segs {
                add([s[4], s[5]]);
                for axis in 0..2 {
                    // Roots of the derivative (quadratic) per axis.
                    let (a0, a1, a2, a3) = (p0[axis], s[axis], s[2 + axis], s[4 + axis]);
                    let a = -a0 + 3.0 * a1 - 3.0 * a2 + a3;
                    let bq = 2.0 * (a0 - 2.0 * a1 + a2);
                    let cq = a1 - a0;
                    let mut roots = [f64::NAN; 2];
                    if abs(a) < 1e-12 {
                        if abs(bq) > 1e-12 {
                            roots[0] = -cq / bq;
                        }
                    } else {
                        let disc = bq * bq - 4.0 * a * cq;
                        if disc >= 0.0 {
                            let sd = sqrt(disc);
                            roots[0] = (-bq + sd) / (2.0 * a);
                            roots[1] = (-bq - sd) / (2.0 * a);
                        }
                    }
                    for t in roots {
                        if t > 0.0 && t < 1.0 {
                            add(cubic_point(p0, s, t));
                        }
                    }
                }
                p0 = [s[4], s[5]];
            }
        }
        if b[0] > b[2] {
            return [0.0; 4];
        }
        b
    }

    /// Apply an affine transform `[a, b, c, d, e, f]` (SVG matrix order).
    pub fn transform(&mut self, m: [f64; 6]) {
        let tf = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        for sp in &mut self.subpaths {
            let (x, y) = tf(sp.start[0], sp.start[1]);
            sp.start = [x, y];
            for s in &mut sp.segs {
                for k in 0..3 {
                    let (x, y) = tf(s[k * 2], s[k * 2 + 1]);
                    s[k * 2] = x;
                    s[k * 2 + 1] = y;
                }
            }
        }
    }

    /// Reverse the drawing direction of every subpath.
    pub fn reverse(&mut self) {
        for sp in &mut self.subpaths {
            *sp = reverse_subpath(sp);
        }
    }

    /// Point and tangent angle (degrees) at a fraction `t ∈ [0, 1]` of the
    /// total length. For repeated queries build a [`PathLut`] instead.
    pub fn point_at(&self, t: f64) -> [f64; 3] {
        PathLut::new(self, 512).sample(t)
    }

    // ---- shape constructors -------------------------------------------

    pub fn rect(x: f64, y: f64, w: f64, h: f64, rx: f64, ry: f64) -> Path {
        let rx = rx.min(w / 2.0).max(0.0);
        let ry = ry.min(h / 2.0).max(0.0);
        if rx == 0.0 || ry == 0.0 {
            let mut d = String::new();
            d.push('M');
            for (i, (px, py)) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)].iter().enumerate() {
                if i > 0 {
                    d.push('L');
                }
                push_num(&mut d, *px, 6);
                d.push(' ');
                push_num(&mut d, *py, 6);
            }
            d.push('Z');
            return Path::parse(&d);
        }
        let mut s = String::new();
        let mut p = |cmd: &str, v: &[f64]| {
            s.push_str(cmd);
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                push_num(&mut s, *n, 6);
            }
        };
        p("M", &[x + rx, y]);
        p("H", &[x + w - rx]);
        p("A", &[rx, ry, 0.0, 0.0, 1.0, x + w, y + ry]);
        p("V", &[y + h - ry]);
        p("A", &[rx, ry, 0.0, 0.0, 1.0, x + w - rx, y + h]);
        p("H", &[x + rx]);
        p("A", &[rx, ry, 0.0, 0.0, 1.0, x, y + h - ry]);
        p("V", &[y + ry]);
        p("A", &[rx, ry, 0.0, 0.0, 1.0, x + rx, y]);
        s.push('Z');
        Path::parse(&s)
    }

    pub fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> Path {
        // Four exact quarter arcs → 4 cubics.
        let k = 0.5522847498307936;
        let segs = alloc::vec![
            [cx + rx, cy + ry * k, cx + rx * k, cy + ry, cx, cy + ry],
            [cx - rx * k, cy + ry, cx - rx, cy + ry * k, cx - rx, cy],
            [cx - rx, cy - ry * k, cx - rx * k, cy - ry, cx, cy - ry],
            [cx + rx * k, cy - ry, cx + rx, cy - ry * k, cx + rx, cy],
        ];
        Path { subpaths: alloc::vec![SubPath { start: [cx + rx, cy], segs, closed: true }] }
    }

    pub fn polygon(points: &[f64], closed: bool) -> Path {
        let n = points.len() / 2;
        if n == 0 {
            return Path::default();
        }
        let mut segs = Vec::new();
        for i in 1..n {
            segs.push(line_seg(points[i * 2 - 2], points[i * 2 - 1], points[i * 2], points[i * 2 + 1]));
        }
        if closed && n > 1 {
            segs.push(line_seg(points[n * 2 - 2], points[n * 2 - 1], points[0], points[1]));
        }
        Path { subpaths: alloc::vec![SubPath { start: [points[0], points[1]], segs, closed }] }
    }

    /// Regular star / polygon with `n` points (inner radius == outer radius
    /// gives a regular polygon).
    pub fn star(cx: f64, cy: f64, n: usize, outer: f64, inner: f64, rotation_deg: f64) -> Path {
        let n = n.max(2);
        let mut pts = Vec::with_capacity(n * 4);
        let step = PI / n as f64;
        for i in 0..n * 2 {
            let r = if i % 2 == 0 { outer } else { inner };
            let a = -PI / 2.0 + i as f64 * step + rotation_deg * RAD;
            pts.push(cx + r * cos(a));
            pts.push(cy + r * sin(a));
        }
        Path::polygon(&pts, true)
    }
}

fn quad_to_cubic(x0: f64, y0: f64, qx: f64, qy: f64, x: f64, y: f64) -> Seg {
    [x0 + 2.0 / 3.0 * (qx - x0), y0 + 2.0 / 3.0 * (qy - y0), x + 2.0 / 3.0 * (qx - x), y + 2.0 / 3.0 * (qy - y), x, y]
}

fn reverse_subpath(sp: &SubPath) -> SubPath {
    let mut pts = alloc::vec![sp.start];
    for s in &sp.segs {
        pts.push([s[4], s[5]]);
    }
    let n = sp.segs.len();
    let mut segs = Vec::with_capacity(n);
    for i in (0..n).rev() {
        let s = &sp.segs[i];
        let p = pts[i];
        segs.push([s[2], s[3], s[0], s[1], p[0], p[1]]);
    }
    SubPath { start: *pts.last().unwrap_or(&sp.start), segs, closed: sp.closed }
}

/// Write one subpath as `M x y C ... [Z]`.
pub fn write_subpath(s: &mut String, start: [f64; 2], segs: &[Seg], closed: bool, decimals: u32) {
    s.push('M');
    push_num(s, start[0], decimals);
    s.push(' ');
    push_num(s, start[1], decimals);
    if !segs.is_empty() {
        s.push('C');
        for (i, seg) in segs.iter().enumerate() {
            for (k, v) in seg.iter().enumerate() {
                if i > 0 || k > 0 {
                    s.push(' ');
                }
                push_num(s, *v, decimals);
            }
        }
    }
    if closed {
        s.push('Z');
    }
}

// ---------------------------------------------------------------------------
// Motion path lookup table
// ---------------------------------------------------------------------------

/// Uniform arc-length resampling of a path: `n` samples of `(x, y, angle°)`
/// spaced evenly by distance, so lookups are O(1) with linear interpolation.
#[derive(Clone, Debug, PartialEq)]
pub struct PathLut {
    pub data: Vec<f64>,
    pub length: f64,
}

impl PathLut {
    pub fn new(path: &Path, samples: usize) -> PathLut {
        let samples = samples.max(2);
        // Dense flattening: (cumulative length, subpath, segment, t)
        struct P {
            len: f64,
            x: f64,
            y: f64,
            ang: f64,
        }
        let mut pts: Vec<P> = Vec::new();
        let mut total = 0.0;
        for sp in &path.subpaths {
            let mut p0 = sp.start;
            for s in &sp.segs {
                let seg_len = cubic_length(p0, s, 1.0);
                let n = ((seg_len / 2.0) as usize).clamp(8, 256);
                for k in 0..=n {
                    if k == 0 && !pts.is_empty() {
                        // Do not duplicate the joint point (keeps lengths monotone).
                        if pts.last().map(|l| l.x == p0[0] && l.y == p0[1]).unwrap_or(false) {
                            continue;
                        }
                    }
                    let t = k as f64 / n as f64;
                    let p = cubic_point(p0, s, t);
                    let d = cubic_deriv(p0, s, t);
                    let ang = if d[0] == 0.0 && d[1] == 0.0 {
                        pts.last().map(|l| l.ang).unwrap_or(0.0)
                    } else {
                        atan2(d[1], d[0]) * DEG
                    };
                    if let Some(l) = pts.last() {
                        // Jumps between subpaths do not add length.
                        if k > 0 {
                            total += sqrt((p[0] - l.x) * (p[0] - l.x) + (p[1] - l.y) * (p[1] - l.y));
                        }
                    }
                    pts.push(P { len: total, x: p[0], y: p[1], ang });
                }
                p0 = [s[4], s[5]];
            }
        }
        let mut data = Vec::with_capacity(samples * 3);
        if pts.is_empty() {
            let st = path.subpaths.first().map(|s| s.start).unwrap_or([0.0, 0.0]);
            for _ in 0..samples {
                data.extend_from_slice(&[st[0], st[1], 0.0]);
            }
            return PathLut { data, length: 0.0 };
        }
        // Unwrap angles so interpolation never spins the long way round.
        for i in 1..pts.len() {
            let prev = pts[i - 1].ang;
            let mut a = pts[i].ang;
            while a - prev > 180.0 {
                a -= 360.0;
            }
            while a - prev < -180.0 {
                a += 360.0;
            }
            pts[i].ang = a;
        }
        let mut j = 0;
        for i in 0..samples {
            let target = total * i as f64 / (samples - 1) as f64;
            while j + 1 < pts.len() && pts[j + 1].len < target {
                j += 1;
            }
            let a = &pts[j];
            let b = &pts[(j + 1).min(pts.len() - 1)];
            let span = b.len - a.len;
            let f = if span > 0.0 { ((target - a.len) / span).clamp(0.0, 1.0) } else { 0.0 };
            data.push(a.x + (b.x - a.x) * f);
            data.push(a.y + (b.y - a.y) * f);
            data.push(a.ang + (b.ang - a.ang) * f);
        }
        PathLut { data, length: total }
    }

    /// Choose a sample count appropriate for the path's size.
    pub fn auto(path: &Path) -> PathLut {
        let len = path.length();
        let n = ((len / 1.5) as usize).clamp(64, 4096);
        PathLut::new(path, n)
    }

    #[inline]
    pub fn sample(&self, t: f64) -> [f64; 3] {
        sample_lut(&self.data, t)
    }
}

/// Sample a raw LUT slice (`[x, y, angle] * n`) at progress `t`.
#[inline]
pub fn sample_lut(d: &[f64], t: f64) -> [f64; 3] {
    let n = d.len() / 3;
    if n == 0 {
        return [0.0; 3];
    }
    if n == 1 {
        return [d[0], d[1], d[2]];
    }
    // Clamp-free extrapolation is meaningless on a path: clamp progress.
    let f = crate::math::clamp(t, 0.0, 1.0) * (n - 1) as f64;
    let i = (f as usize).min(n - 2);
    let k = f - i as f64;
    let a = i * 3;
    let b = a + 3;
    [d[a] + (d[b] - d[a]) * k, d[a + 1] + (d[b + 1] - d[a + 1]) * k, fmod_angle(d[a + 2] + (d[b + 2] - d[a + 2]) * k)]
}

#[inline]
fn fmod_angle(a: f64) -> f64 {
    let r = fmod_pos(a + 180.0, 360.0) - 180.0;
    if r == -180.0 {
        180.0
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_commands() {
        let p = Path::parse("M10 10 L20 10 H30 V20 C30 30 40 30 40 20 S50 10 50 20 Q55 30 60 20 T70 20 A5 5 0 0 1 80 20 Z");
        assert_eq!(p.subpaths.len(), 1);
        assert!(p.subpaths[0].closed);
        let last = p.subpaths[0].segs.last().unwrap();
        assert!((last[4] - 10.0).abs() < 1e-9 && (last[5] - 10.0).abs() < 1e-9);
        let rel = Path::parse("m10 10 l10 0 h10 v10");
        let abs_ = Path::parse("M10 10 L20 10 H30 V20");
        assert_eq!(rel, abs_);
        // Implicit lineto after moveto + compact numbers.
        let c = Path::parse("M0,0 10,0 10-10.5.5 1");
        assert_eq!(c.subpaths[0].segs.len(), 3);
    }

    #[test]
    fn lengths_and_bounds() {
        let line = Path::parse("M0 0 L100 0");
        assert!((line.length() - 100.0).abs() < 1e-9);
        let circle = Path::ellipse(0.0, 0.0, 50.0, 50.0);
        assert!((circle.length() - TAU * 50.0).abs() < 0.05, "{}", circle.length());
        let arc_circle = Path::parse("M-50 0 A50 50 0 1 1 50 0 A50 50 0 1 1 -50 0");
        assert!((arc_circle.length() - TAU * 50.0).abs() < 0.1, "{}", arc_circle.length());
        let b = circle.bounds();
        assert!((b[0] + 50.0).abs() < 1e-6 && (b[3] - 50.0).abs() < 1e-6);
    }

    #[test]
    fn lut_is_uniform() {
        let p = Path::parse("M0 0 L100 0 L100 100");
        let lut = PathLut::new(&p, 201);
        assert!((lut.length - 200.0).abs() < 1e-6);
        let m = lut.sample(0.5);
        assert!((m[0] - 100.0).abs() < 1e-6 && m[1].abs() < 1e-6);
        let q = lut.sample(0.25);
        assert!((q[0] - 50.0).abs() < 1e-6 && q[2].abs() < 1e-6);
        let r = lut.sample(0.75);
        assert!((r[1] - 50.0).abs() < 1e-6 && (r[2] - 90.0).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_and_reverse() {
        let p = Path::parse("M0 0 C10 0 20 10 20 20 L0 20 Z");
        let q = Path::parse(&p.to_svg(9));
        for (a, b) in p.subpaths[0].segs.iter().zip(&q.subpaths[0].segs) {
            for k in 0..6 {
                assert!((a[k] - b[k]).abs() < 1e-8);
            }
        }
        let mut r = p.clone();
        r.reverse();
        assert!((r.length() - p.length()).abs() < 1e-9);
        r.reverse();
        assert_eq!(r, p);
    }
}
