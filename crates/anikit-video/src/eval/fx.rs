//! Motion-graphics layer effects: repeater, echo (motion trails), motion paths,
//! text on a path, and shape modifiers (round corners, zigzag, pucker/bloat,
//! offset path, twist, wiggle path).
//!
//! Repeater and echo re-emit the same compiled node with a different time and
//! transform, so they cost one property evaluation per copy and no compile work.
//! Static shape modifiers run once at compile time; only `wigglePath` runs per
//! frame (into a pooled path).

use super::{noise1, rotate, scale, translate};
use crate::dl::{self, Affine, Blend, Path};
use kurbo::{ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg, Point, Shape};

/// Render-time repeater.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RepC {
    pub count: u32,
    pub d: [f32; 2],
    pub rot: f32,
    pub scale: [f32; 2],
    pub opacity: f32,
    pub stagger: f32,
    /// Stagger rank of each copy (empty = in order).
    pub ranks: Vec<u32>,
    /// Rotation/scale centre in parent pixels; `None` = the layer's position.
    pub center: Option<[f32; 2]>,
}

impl RepC {
    /// Copy `i`'s transform in parent space (`c` = rotation/scale centre).
    pub fn xf(&self, i: u32, c: [f32; 2]) -> Affine {
        let k = i as f32;
        let m = dl::mul(&translate(c[0], c[1]), &dl::mul(&rotate(self.rot * k), &scale(self.scale[0].powf(k), self.scale[1].powf(k))));
        dl::mul(&m, &translate(self.d[0] * k - c[0], self.d[1] * k - c[1]))
    }
    pub fn alpha(&self, i: u32) -> f32 {
        (1.0 + self.opacity * i as f32).clamp(0.0, 1.0)
    }
}

/// Echo: earlier-time copies.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EchoC {
    pub count: u32,
    pub delay: f32,
    pub decay: f32,
    pub blend: Blend,
}

/// A flattened path with cumulative arc lengths, for constant-speed lookup.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MPath {
    pub pts: Vec<[f32; 2]>,
    pub cum: Vec<f32>,
    pub closed: bool,
}

impl MPath {
    pub fn from_path(p: &Path) -> Option<MPath> {
        let mut pts: Vec<[f32; 2]> = Vec::new();
        let mut closed = false;
        kurbo::flatten(p, 0.25, |el| match el {
            PathEl::MoveTo(q) => {
                if pts.is_empty() {
                    pts.push([q.x as f32, q.y as f32])
                }
            }
            PathEl::LineTo(q) => pts.push([q.x as f32, q.y as f32]),
            PathEl::ClosePath => {
                closed = true;
                if let Some(&f) = pts.first() {
                    pts.push(f)
                }
            }
            _ => {}
        });
        if pts.len() < 2 {
            return None;
        }
        let mut cum = Vec::with_capacity(pts.len());
        let mut s = 0.0f32;
        cum.push(0.0);
        for w in pts.windows(2) {
            s += ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            cum.push(s);
        }
        (s > 0.0).then_some(MPath { pts, cum, closed })
    }
    pub fn len(&self) -> f32 {
        *self.cum.last().unwrap_or(&0.0)
    }
    /// Point and tangent angle (degrees) at arc length `s` (wraps on closed paths, clamps on open ones).
    pub fn at(&self, s: f32) -> ([f32; 2], f32) {
        let l = self.len();
        if !self.closed && (s < 0.0 || s > l) {
            // Past an open end: continue along the end segment's direction.
            let (a, b, base) = if s < 0.0 { (self.pts[0], self.pts[1], 0.0) } else { (self.pts[self.pts.len() - 2], self.pts[self.pts.len() - 1], l) };
            let d = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
            let e = if s < 0.0 { a } else { b };
            let k = (s - base) / d;
            return ([e[0] + (b[0] - a[0]) * k, e[1] + (b[1] - a[1]) * k], (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees());
        }
        let s = if self.closed { s.rem_euclid(l) } else { s };
        let k = match self.cum.binary_search_by(|c| c.partial_cmp(&s).unwrap_or(std::cmp::Ordering::Less)) {
            Ok(i) => i.min(self.pts.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.pts.len() - 2),
        };
        let (a, b) = (self.pts[k], self.pts[k + 1]);
        let seg = (self.cum[k + 1] - self.cum[k]).max(1e-6);
        let u = ((s - self.cum[k]) / seg).clamp(0.0, 1.0);
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
        ([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u], ang)
    }
}

/// Inertia: a damped spring chasing the layer's own animated values.
#[derive(Debug)]
pub(crate) struct InertiaC {
    /// Property slots the spring follows.
    pub slots: Vec<u8>,
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
    /// Spring output per sample (`slots.len()` values each), at `RATE` Hz from the layer start.
    pub table: std::sync::OnceLock<Vec<f32>>,
}

impl InertiaC {
    pub const RATE: f32 = 240.0;

    /// Run the spring over `life` seconds against `target(t)` (the layer's own values at local time t).
    pub fn simulate(&self, life: f32, mut target: impl FnMut(f32) -> [f32; super::prop::N]) -> Vec<f32> {
        let k = self.slots.len();
        let n = (life * Self::RATE).ceil().max(1.0) as usize + 1;
        let mut out = Vec::with_capacity(n * k);
        let first = target(0.0);
        let mut x: Vec<f32> = self.slots.iter().map(|&s| first[s as usize]).collect();
        let mut vel = vec![0.0f32; k];
        let dt = 1.0 / Self::RATE;
        let sub = 4;
        let h = dt / sub as f32;
        let mut prev = x.clone();
        for j in 0..n {
            let t = j as f32 * dt;
            let tv = if j == 0 { first } else { target(t) };
            let goal: Vec<f32> = self.slots.iter().map(|&s| tv[s as usize]).collect();
            if j > 0 {
                // Semi-implicit Euler, the goal interpolated across the substeps.
                for q in 1..=sub {
                    let f = q as f32 / sub as f32;
                    for i in 0..k {
                        let g = prev[i] + (goal[i] - prev[i]) * f;
                        let a = (-self.stiffness * (x[i] - g) - self.damping * vel[i]) / self.mass;
                        vel[i] += a * h;
                        x[i] += vel[i] * h;
                    }
                }
            }
            prev.copy_from_slice(&goal);
            out.extend_from_slice(&x);
        }
        out
    }

    /// Replace the followed slots of `v` with the spring's value at local time `lt`.
    #[inline]
    pub fn apply(&self, table: &[f32], lt: f32, v: &mut [f32; super::prop::N]) {
        let k = self.slots.len();
        let n = table.len() / k.max(1);
        if n == 0 {
            return;
        }
        let f = (lt * Self::RATE).max(0.0);
        let j = (f.floor() as usize).min(n - 1);
        let j1 = (j + 1).min(n - 1);
        let u = (f - j as f32).clamp(0.0, 1.0);
        for (i, &s) in self.slots.iter().enumerate() {
            let a = table[j * k + i];
            let b = table[j1 * k + i];
            v[s as usize] = a + (b - a) * u;
        }
    }
}

/// Per-layer motion-graphics extras (boxed on `Node`, `None` for ordinary layers).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Fx {
    pub repeat: Option<RepC>,
    pub echo: Option<EchoC>,
    /// Motion path (parent pixels); position comes from `progress`.
    pub path: Option<MPath>,
    /// Orient along the path: extra degrees.
    pub orient: Option<f32>,
    /// Text on a path (layer-local pixels).
    pub text_path: Option<MPath>,
    /// Animated noise displacement of the shape outline: (amount, detail, speed, seed).
    pub wiggle: Option<[f32; 4]>,
    /// Text animator with a range selector.
    pub animator: Option<AnimatorC>,
    /// Stroke gradient along the path: chunks of the outline, each with its own two colours.
    pub along: Option<AlongC>,
    /// Extrusion: (dx, dy per step, steps, fade, colour matrix).
    pub extrude: Option<(f32, f32, u32, f32, [f32; 20])>,
    /// Smear strength (stretch along velocity).
    pub smear: Option<f32>,
    /// The outline resampled for `wiggle`.
    pub wiggle_src: Vec<(Vec<Point>, bool)>,
}

/// Text animator (range selector).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AnimatorC {
    pub by: crate::text::UnitKind,
    pub dx: f32,
    pub dy: f32,
    pub rot: f32,
    pub scale: f32,
    pub opacity: f32,
    pub blur: f32,
    /// Target colour (OKLab + alpha).
    pub color: Option<[f32; 4]>,
    /// Variable-font axis targets at full selection.
    pub axes: Vec<(String, f32)>,
    /// Line alignment for axis-driven advance changes: 0 left, 0.5 centre, 1 right.
    pub align: f32,
    pub smooth: f32,
    /// Rank of each unit in selection order (empty = in order).
    pub order: Vec<u32>,
    /// Auto sweep of the offset 0 → 1: (delay, duration, ease id).
    pub auto: Option<(f32, f32, u32)>,
}

impl AnimatorC {
    /// Selection amount 0..1 of a unit at `pos` (0..1) for the range `[lo, hi]`.
    #[inline]
    pub fn amount(&self, pos: f32, lo: f32, hi: f32) -> f32 {
        let edge = |d: f32| {
            if self.smooth <= 1e-4 {
                if d >= 0.0 {
                    1.0
                } else {
                    0.0
                }
            } else {
                // Soft edges fall off outside the range, so [0, 1] selects every unit fully.
                let x = (d / self.smooth + 1.0).clamp(0.0, 1.0);
                x * x * (3.0 - 2.0 * x)
            }
        };
        edge(pos - lo) * edge(hi - pos)
    }
}

/// Selection order ranks: `forward`, `reverse`, `center`, `random` (seeded).
pub(crate) fn order_ranks(n: usize, order: &str, seed: f64) -> Vec<u32> {
    let mut idx: Vec<usize> = (0..n).collect();
    match order {
        "reverse" => idx.reverse(),
        "center" => {
            let c = (n as f64 - 1.0) * 0.5;
            idx.sort_by(|a, b| ((*a as f64 - c).abs()).partial_cmp(&(*b as f64 - c).abs()).unwrap());
        }
        "random" => {
            // Deterministic Fisher-Yates with a hash RNG.
            let mut st = (seed.to_bits() ^ 0x9E37_79B9_7F4A_7C15).wrapping_add(n as u64);
            for i in (1..n).rev() {
                st ^= st << 13;
                st ^= st >> 7;
                st ^= st << 17;
                idx.swap(i, (st % (i as u64 + 1)) as usize);
            }
        }
        _ => return Vec::new(),
    }
    // idx[k] = unit at rank k → rank of each unit.
    let mut rank = vec![0u32; n];
    for (k, &u) in idx.iter().enumerate() {
        rank[u] = k as u32;
    }
    rank
}

/// One chunk of a path-following stroke gradient: (chunk path, start point, end point, start fraction, end fraction, start colour, end colour).
pub(crate) type AlongChunk = (Path, [f32; 2], [f32; 2], f32, f32, crate::dl::Rgba, crate::dl::Rgba);

/// A stroke gradient that follows the path.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct AlongC {
    /// (chunk path, start point, end point, start fraction, end fraction, start colour, end colour).
    pub chunks: Vec<AlongChunk>,
}

impl AlongC {
    /// Split `p` into about `n` equal-length chunks coloured by `stops` (evenly spaced colours).
    pub fn build(p: &Path, stops: &[crate::dl::Rgba], n: usize) -> AlongC {
        let mut chunks = Vec::new();
        let Some(mp) = MPath::from_path(p) else { return AlongC { chunks } };
        let total = mp.len();
        let col = |u: f32| -> crate::dl::Rgba {
            if stops.len() == 1 {
                return stops[0];
            }
            let f = u.clamp(0.0, 1.0) * (stops.len() - 1) as f32;
            let k = (f.floor() as usize).min(stops.len() - 2);
            let t = f - k as f32;
            let (a, b) = (stops[k].0, stops[k + 1].0);
            crate::dl::Rgba([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t])
        };
        let n = n.max(1);
        let mut j = 0usize;
        for c in 0..n {
            let (u0, u1) = (c as f32 / n as f32, (c + 1) as f32 / n as f32);
            let (s0, s1) = (u0 * total, u1 * total);
            let mut path = Path::new();
            let (q0, _) = mp.at(s0);
            path.move_to((q0[0] as f64, q0[1] as f64));
            while j + 1 < mp.cum.len() && mp.cum[j + 1] < s1 {
                j += 1;
                if mp.cum[j] > s0 {
                    path.line_to((mp.pts[j][0] as f64, mp.pts[j][1] as f64));
                }
            }
            let (q1, _) = mp.at(s1);
            path.line_to((q1[0] as f64, q1[1] as f64));
            chunks.push((path, q0, q1, u0, u1, col(u0), col(u1)));
        }
        AlongC { chunks }
    }
}

impl Fx {
    pub fn is_empty(&self) -> bool {
        *self == Fx::default()
    }
}

// ---------------------------------------------------------------- shape modifiers

/// Static outline modifiers, applied in this order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Mods {
    pub round: f64,
    pub zigzag: Option<(f64, u32, bool)>,
    pub pucker: f64,
    pub offset: f64,
    pub twist: f64,
}

/// Split a path into subpaths of points (curves flattened), with a closed flag.
fn polys(p: &Path, tol: f64) -> Vec<(Vec<Point>, bool)> {
    let mut out: Vec<(Vec<Point>, bool)> = Vec::new();
    kurbo::flatten(p, tol, |el| match el {
        PathEl::MoveTo(q) => out.push((vec![q], false)),
        PathEl::LineTo(q) => {
            if let Some(l) = out.last_mut() {
                if l.0.last() != Some(&q) {
                    l.0.push(q)
                }
            }
        }
        PathEl::ClosePath => {
            if let Some(l) = out.last_mut() {
                l.1 = true;
                if l.0.len() > 1 && l.0.first() == l.0.last() {
                    l.0.pop();
                }
            }
        }
        _ => {}
    });
    out.retain(|(v, _)| v.len() > 1);
    out
}

/// Corner points of a path's segments (curves kept as single vertices), for corner-based modifiers.
fn corners(p: &Path) -> Vec<(Vec<Point>, bool)> {
    let mut out: Vec<(Vec<Point>, bool)> = Vec::new();
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(q) => out.push((vec![q], false)),
            PathEl::LineTo(q) | PathEl::QuadTo(_, q) | PathEl::CurveTo(_, _, q) => {
                if let Some(l) = out.last_mut() {
                    l.0.push(q)
                }
            }
            PathEl::ClosePath => {
                if let Some(l) = out.last_mut() {
                    l.1 = true;
                    if l.0.len() > 1 && l.0.first() == l.0.last() {
                        l.0.pop();
                    }
                }
            }
        }
    }
    out.retain(|(v, _)| v.len() > 1);
    out
}

fn has_curves(p: &Path) -> bool {
    p.elements().iter().any(|e| matches!(e, PathEl::QuadTo(..) | PathEl::CurveTo(..)))
}

fn centroid(p: &Path) -> Point {
    let b = p.bounding_box();
    b.center()
}

/// Round every corner by `r` px (curve segments are left as they are).
pub(crate) fn round_corners(p: &Path, r: f64) -> Path {
    if r <= 0.0 || has_curves(p) {
        return p.clone();
    }
    let mut out = Path::new();
    for (pts, closed) in corners(p) {
        let n = pts.len();
        let get = |i: isize| pts[i.rem_euclid(n as isize) as usize];
        for i in 0..n as isize {
            let (a, b, c) = (get(i - 1), get(i), get(i + 1));
            let endpoint = !closed && (i == 0 || i == n as isize - 1);
            if endpoint {
                if i == 0 {
                    out.move_to(b)
                } else {
                    out.line_to(b)
                }
                continue;
            }
            let (da, dc) = ((a - b).hypot(), (c - b).hypot());
            let k = r.min(da * 0.5).min(dc * 0.5);
            let p0 = b + (a - b) * (k / da.max(1e-9));
            let p1 = b + (c - b) * (k / dc.max(1e-9));
            if i == 0 {
                out.move_to(p0)
            } else {
                out.line_to(p0)
            }
            // A cubic that approximates a circular fillet (0.5523 handle).
            let h = 0.5523;
            out.curve_to(p0 + (b - p0) * h, p1 + (b - p1) * h, p1);
        }
        if closed {
            out.close_path();
        }
    }
    out
}

/// Zigzag along the outline: `ridges` per segment (per subpath for curves), `size` px.
pub(crate) fn zigzag(p: &Path, size: f64, ridges: u32, smooth: bool) -> Path {
    let ridges = ridges.max(1) as usize;
    let mut out = Path::new();
    for (segs, closed) in segs_by_subpath(p) {
        if segs.is_empty() {
            continue;
        }
        let lines = segs.iter().all(|s| matches!(s, PathSeg::Line(_)));
        let steps = 2 * ridges * if lines { segs.len() } else { 1 };
        let total: f64 = segs.iter().map(|s| s.arclen(0.1)).sum();
        let m = if closed { steps } else { steps + 1 };
        let mut pts: Vec<Point> = Vec::with_capacity(m);
        for i in 0..m {
            if let Some((q, nrm)) = point_normal(&segs, total * i as f64 / steps as f64) {
                let amp = if !closed && (i == 0 || i == steps) { 0.0 } else if i % 2 == 1 { size } else { -size };
                pts.push(q + nrm * amp);
            }
        }
        if pts.len() < 2 {
            continue;
        }
        if smooth {
            smooth_through(&mut out, &pts, closed);
        } else {
            out.move_to(pts[0]);
            for q in &pts[1..] {
                out.line_to(*q);
            }
            if closed {
                out.close_path();
            }
        }
    }
    out
}

fn segs_by_subpath(p: &Path) -> Vec<(Vec<PathSeg>, bool)> {
    let mut out: Vec<(Vec<PathSeg>, bool)> = Vec::new();
    let mut start = Point::ZERO;
    let mut last = Point::ZERO;
    for el in p.elements() {
        match *el {
            PathEl::MoveTo(q) => {
                out.push((Vec::new(), false));
                start = q;
                last = q;
            }
            PathEl::LineTo(q) => {
                if let Some(l) = out.last_mut() {
                    if q != last {
                        l.0.push(PathSeg::Line(kurbo::Line::new(last, q)))
                    }
                }
                last = q;
            }
            PathEl::QuadTo(a, q) => {
                if let Some(l) = out.last_mut() {
                    l.0.push(PathSeg::Quad(kurbo::QuadBez::new(last, a, q)))
                }
                last = q;
            }
            PathEl::CurveTo(a, b, q) => {
                if let Some(l) = out.last_mut() {
                    l.0.push(PathSeg::Cubic(kurbo::CubicBez::new(last, a, b, q)))
                }
                last = q;
            }
            PathEl::ClosePath => {
                if let Some(l) = out.last_mut() {
                    if last != start {
                        l.0.push(PathSeg::Line(kurbo::Line::new(last, start)))
                    }
                    l.1 = true;
                }
                last = start;
            }
        }
    }
    out
}

fn point_normal(segs: &[PathSeg], mut s: f64) -> Option<(Point, kurbo::Vec2)> {
    for (i, g) in segs.iter().enumerate() {
        let l = g.arclen(0.1);
        if s <= l || i == segs.len() - 1 {
            let u = if l > 0.0 { g.inv_arclen(s.min(l), 0.1) } else { 0.0 };
            let d = tangent(g, u);
            let n = kurbo::Vec2::new(-d.y, d.x);
            let n = if n.hypot() > 1e-9 { n.normalize() } else { kurbo::Vec2::ZERO };
            return Some((g.eval(u), n));
        }
        s -= l;
    }
    None
}

fn tangent(g: &PathSeg, u: f64) -> kurbo::Vec2 {
    match g {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => q.deriv().eval(u).to_vec2(),
        PathSeg::Cubic(c) => c.deriv().eval(u).to_vec2(),
    }
}

/// Catmull-Rom through points, as cubics.
fn smooth_through(out: &mut Path, pts: &[Point], closed: bool) {
    let n = pts.len();
    let get = |i: isize| -> Point {
        if closed {
            pts[i.rem_euclid(n as isize) as usize]
        } else {
            pts[i.clamp(0, n as isize - 1) as usize]
        }
    };
    out.move_to(pts[0]);
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        out.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
    }
    if closed {
        out.close_path();
    }
}

/// Pucker (< 0) / bloat (> 0): vertices move toward/away from the centre, control points the opposite way.
pub(crate) fn pucker_bloat(p: &Path, amt: f64) -> Path {
    if amt == 0.0 {
        return p.clone();
    }
    let c = centroid(p);
    // Rebuild every segment as a cubic: endpoints pulled by −amt, handles pushed by +amt (AE-style).
    let mut out = Path::new();
    for (segs, closed) in segs_by_subpath(p) {
        let Some(first) = segs.first() else { continue };
        let mv = |q: Point, k: f64| c + (q - c) * (1.0 + k);
        out.move_to(mv(first.eval(0.0), -amt));
        for g in &segs {
            let cb = match *g {
                PathSeg::Line(l) => kurbo::CubicBez::new(l.p0, l.p0.lerp(l.p1, 1.0 / 3.0), l.p0.lerp(l.p1, 2.0 / 3.0), l.p1),
                PathSeg::Quad(q) => q.raise(),
                PathSeg::Cubic(c) => c,
            };
            out.curve_to(mv(cb.p1, amt), mv(cb.p2, amt), mv(cb.p3, -amt));
        }
        if closed {
            out.close_path();
        }
    }
    out
}

/// Offset the outline by `d` px (positive = grow) along vertex normals (miter-limited).
pub(crate) fn offset_path(p: &Path, d: f64) -> Path {
    if d == 0.0 {
        return p.clone();
    }
    let mut out = Path::new();
    for (pts, closed) in polys(p, 0.2) {
        let n = pts.len();
        // Orientation: grow means move away from the inside.
        let area: f64 = (0..n).map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        }).sum::<f64>();
        let sgn = if area >= 0.0 { 1.0 } else { -1.0 };
        let get = |i: isize| -> Point {
            if closed {
                pts[i.rem_euclid(n as isize) as usize]
            } else {
                pts[i.clamp(0, n as isize - 1) as usize]
            }
        };
        let nrm = |a: Point, b: Point| {
            let v = b - a;
            let v = if v.hypot() > 1e-9 { v.normalize() } else { kurbo::Vec2::ZERO };
            kurbo::Vec2::new(v.y, -v.x) * sgn
        };
        let mut q = Vec::with_capacity(n);
        for i in 0..n as isize {
            let (a, b, c) = (get(i - 1), get(i), get(i + 1));
            let n1 = if a == b { nrm(b, c) } else { nrm(a, b) };
            let n2 = if b == c { n1 } else { nrm(b, c) };
            let m = n1 + n2;
            let m = if m.hypot() > 1e-9 { m.normalize() } else { n1 };
            let cos = m.dot(n1).max(0.25);
            q.push(b + m * (d / cos));
        }
        out.move_to(q[0]);
        for x in &q[1..] {
            out.line_to(*x);
        }
        if closed {
            out.close_path();
        }
    }
    out
}

/// Twist around the centre: rotation grows with distance from it (`deg` at the farthest point).
pub(crate) fn twist(p: &Path, deg: f64) -> Path {
    if deg == 0.0 {
        return p.clone();
    }
    let c = centroid(p);
    let ps = polys(p, 0.2);
    let rmax = ps.iter().flat_map(|(v, _)| v.iter()).map(|q| (*q - c).hypot()).fold(1e-6, f64::max);
    let mut out = Path::new();
    for (pts, closed) in ps {
        // Resample long edges so the twist bends them.
        let mut dense = Vec::new();
        let n = pts.len();
        let m = if closed { n } else { n - 1 };
        for i in 0..m {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let k = ((b - a).hypot() / 6.0).ceil().clamp(1.0, 64.0) as usize;
            for j in 0..k {
                dense.push(a.lerp(b, j as f64 / k as f64));
            }
        }
        if !closed {
            dense.push(pts[n - 1]);
        }
        for (i, q) in dense.iter().enumerate() {
            let v = *q - c;
            let a = (deg * v.hypot() / rmax).to_radians();
            let (s, co) = a.sin_cos();
            let r = c + kurbo::Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co);
            if i == 0 {
                out.move_to(r)
            } else {
                out.line_to(r)
            }
        }
        if closed {
            out.close_path();
        }
    }
    out
}

impl Mods {
    pub fn is_empty(&self) -> bool {
        *self == Mods::default()
    }
    pub fn apply(&self, p: &Path) -> Path {
        let mut p = p.clone();
        if self.offset != 0.0 {
            p = offset_path(&p, self.offset);
        }
        if self.round > 0.0 {
            p = round_corners(&p, self.round);
        }
        if self.pucker != 0.0 {
            p = pucker_bloat(&p, self.pucker);
        }
        if let Some((s, r, sm)) = self.zigzag {
            p = zigzag(&p, s, r, sm);
        }
        if self.twist != 0.0 {
            p = twist(&p, self.twist);
        }
        p
    }
}

/// Wiggle path: resample each subpath to `detail` points and displace them with smooth noise.
/// Writes into `out` (cleared first); no allocation once `out` has grown.
pub(crate) fn wiggle_into(src: &[(Vec<Point>, bool)], w: [f32; 4], t: f32, out: &mut Path) {
    out.truncate(0);
    let [amount, _detail, speed, seed] = w;
    for (si, (pts, closed)) in src.iter().enumerate() {
        let n = pts.len();
        let disp = |i: usize| -> Point {
            let q = pts[i];
            let base = seed as f64 * 17.0 + si as f64 * 101.0 + i as f64 * 7.31;
            let tt = (t * speed) as f64;
            let dx = noise1(base + tt) * amount as f64;
            let dy = noise1(base + 53.7 + tt) * amount as f64;
            Point::new(q.x + dx, q.y + dy)
        };
        // Smooth through displaced points.
        let get = |i: isize| -> Point {
            if *closed {
                disp(i.rem_euclid(n as isize) as usize)
            } else {
                disp(i.clamp(0, n as isize - 1) as usize)
            }
        };
        out.move_to(get(0));
        let segs = if *closed { n } else { n - 1 };
        for i in 0..segs as isize {
            let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
            out.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
        }
        if *closed {
            out.close_path();
        }
    }
}

/// Resample each subpath to about `detail` evenly spaced points (for `wigglePath`).
pub(crate) fn resample(p: &Path, detail: u32) -> Vec<(Vec<Point>, bool)> {
    let mut out = Vec::new();
    for (segs, closed) in segs_by_subpath(p) {
        if segs.is_empty() {
            continue;
        }
        let total: f64 = segs.iter().map(|s| s.arclen(0.1)).sum();
        let n = detail.max(3) as usize;
        let mut pts = Vec::with_capacity(n + 1);
        let m = if closed { n } else { n + 1 };
        for i in 0..m {
            if let Some((q, _)) = point_normal(&segs, total * i as f64 / n as f64) {
                pts.push(q);
            }
        }
        out.push((pts, closed));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Path {
        let mut p = Path::new();
        p.move_to((0.0, 0.0));
        p.line_to((100.0, 0.0));
        p.line_to((100.0, 100.0));
        p.line_to((0.0, 100.0));
        p.close_path();
        p
    }

    #[test]
    fn animator_selection() {
        let a = AnimatorC { by: Default::default(), dx: 0.0, dy: 0.0, rot: 0.0, scale: 1.0, opacity: 0.0, blur: 0.0, color: None, axes: vec![], align: 0.5, smooth: 0.0, order: vec![], auto: None };
        assert_eq!(a.amount(0.5, 0.0, 1.0), 1.0);
        assert_eq!(a.amount(0.5, 0.6, 1.6), 0.0);
        let r = order_ranks(10, "random", 3.0);
        let mut s = r.clone();
        s.sort();
        assert_eq!(s, (0..10).collect::<Vec<u32>>());
        assert_eq!(r, order_ranks(10, "random", 3.0));
        assert_ne!(r, order_ranks(10, "random", 4.0));
        assert_eq!(order_ranks(5, "center", 0.0)[2], 0);
    }

    #[test]
    fn mpath_constant_speed() {
        let m = MPath::from_path(&square()).unwrap();
        assert!((m.len() - 400.0).abs() < 1e-3);
        let (q, a) = m.at(150.0);
        assert!((q[0] - 100.0).abs() < 1e-3 && (q[1] - 50.0).abs() < 1e-3);
        assert!((a - 90.0).abs() < 1e-3);
        // Closed paths wrap.
        assert_eq!(m.at(450.0).0, m.at(50.0).0);
    }

    #[test]
    fn repeater_transform() {
        let r = RepC { count: 4, d: [10.0, 0.0], rot: 90.0, scale: [1.0, 1.0], opacity: -0.25, stagger: 0.0, ranks: vec![], center: None };
        let m = r.xf(1, [0.0, 0.0]);
        // (0,0) → translated by 10 then rotated 90° about the origin → (0, 10).
        let x = m[0] * 0.0 + m[2] * 0.0 + m[4];
        let y = m[1] * 0.0 + m[3] * 0.0 + m[5];
        assert!(x.abs() < 1e-4 && (y - 10.0).abs() < 1e-4, "{m:?}");
        assert_eq!(r.alpha(2), 0.5);
        assert_eq!(r.alpha(9), 0.0);
    }

    #[test]
    fn modifiers_change_outline() {
        let sq = square();
        let b0 = sq.bounding_box();
        let grown = offset_path(&sq, 10.0).bounding_box();
        assert!((grown.width() - 120.0).abs() < 0.5, "{grown:?}");
        let shrunk = offset_path(&sq, -10.0).bounding_box();
        assert!((shrunk.width() - 80.0).abs() < 0.5, "{shrunk:?}");
        let rc = round_corners(&sq, 20.0);
        assert!(rc.elements().iter().filter(|e| matches!(e, PathEl::CurveTo(..))).count() == 4);
        assert!((rc.area().abs() - sq.area().abs()) < 0.0 && rc.area().abs() > 9000.0);
        let z = zigzag(&sq, 8.0, 5, false);
        assert!(z.elements().len() > 30);
        assert!(z.bounding_box().width() > b0.width());
        let pb = pucker_bloat(&sq, 0.3);
        assert!(pb.elements().len() >= 5);
        let tw = twist(&sq, 90.0);
        assert!(tw.elements().len() > 20);
        let mut w = Path::new();
        wiggle_into(&resample(&sq, 24), [10.0, 24.0, 1.0, 3.0], 0.5, &mut w);
        let mut w2 = Path::new();
        wiggle_into(&resample(&sq, 24), [10.0, 24.0, 1.0, 3.0], 0.5, &mut w2);
        assert_eq!(w, w2, "deterministic");
        assert_ne!(w.bounding_box(), b0);
    }
}
