//! Illustration: hand-made brush strokes.
//!
//! - `brush`: one stroke along an SVG path (`d`) or smoothed `points`, built
//!   as a variable-width outline (centreline offset by ±width/2·pressure) and
//!   filled. Textures: `ink` (crisp, slightly wobbly edges), `dry` (parallel
//!   bristles with gaps that run out along the stroke), `marker` (flat,
//!   translucent, darker where the ends overlap) and `pencil` (many short
//!   jittered graphite strands).
//! - `sketch`: several paths sharing the brush, draw-on and boil settings,
//!   drawn on one after another (`stagger`).
//!
//! Both draw on (`draw` seconds, `delay`, `ease`) with the brush tip intact,
//! and can "boil" (`boil` fps, `boilAmount` px): hand-drawn jitter whose seed
//! changes `boil` times a second, like a redrawn animation drawing.
//! Everything is deterministic by `seed`. Geometry is cached per boil frame
//! and draw progress, so held frames cost nothing.

use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Path, Rgba, IDENTITY};
use serde::Deserialize;
use std::sync::Mutex;

// ------------------------------------------------------------ props

/// Pressure along the stroke: a preset name or values (0..1) spread over the length.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Pressure {
    Preset(String),
    Values(Vec<f32>),
}
impl Default for Pressure {
    fn default() -> Self {
        Pressure::Preset("swell".into())
    }
}

const PRESSURES: &[&str] = &["swell", "taper", "even", "flick"];
const TEXTURES: &[&str] = &["ink", "dry", "marker", "pencil"];
const EASES: &[&str] = &["linear", "in", "out", "inOut", "easeIn", "easeOut", "easeInOut", "ease"];

fn check_pressure(p: &Pressure) -> Result<(), Error> {
    match p {
        Pressure::Preset(s) if !PRESSURES.contains(&s.as_str()) => Err(unknown("pressure", s, PRESSURES)),
        Pressure::Values(v) if v.is_empty() => Err(Error("pressure: give at least one value".into())),
        _ => Ok(()),
    }
}

fn unknown(key: &str, got: &str, options: &[&str]) -> Error {
    let hint = closest(got, options).map(|c| format!(" (did you mean `{c}`?)")).unwrap_or_default();
    Error(format!("{key}: unknown value `{got}`{hint}; expected one of {}", options.join(", ")))
}

fn ease(name: &str, x: f32) -> f32 {
    let x = clamp01(x);
    match name {
        "linear" => x,
        "in" | "easeIn" => ease_in(x),
        "out" | "easeOut" => ease_out(x),
        _ => ease_in_out(x),
    }
}

/// Parse `d` or `points` into centreline polylines (one per subpath), in the
/// path's own units.
fn centrelines(d: &Option<String>, points: &Option<Vec<[f32; 2]>>, fraction: bool) -> Result<Vec<Vec<[f32; 2]>>, Error> {
    let tol = if fraction { 0.0002 } else { 0.25 };
    let mut out = Vec::new();
    if let Some(d) = d {
        let bp = kurbo::BezPath::from_svg(d).map_err(|e| Error(format!("d: invalid SVG path data ({e})")))?;
        let mut cur: Vec<[f32; 2]> = Vec::new();
        kurbo::flatten(bp.iter(), tol, |el| match el {
            kurbo::PathEl::MoveTo(p) => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                cur.push([p.x as f32, p.y as f32]);
            }
            kurbo::PathEl::LineTo(p) => cur.push([p.x as f32, p.y as f32]),
            kurbo::PathEl::ClosePath => {
                if let Some(&f) = cur.first() {
                    cur.push(f);
                }
            }
            _ => {}
        });
        if cur.len() > 1 {
            out.push(cur);
        }
    } else if let Some(pts) = points {
        if pts.len() < 2 {
            return Err(Error("points: give at least two [x, y] points".into()));
        }
        out.push(catmull(pts));
    } else {
        return Err(Error("give the stroke as `d` (SVG path data) or `points` ([[x, y], …])".into()));
    }
    if out.is_empty() {
        return Err(Error("d: the path has no segments".into()));
    }
    Ok(out)
}

/// Catmull-Rom through `pts`, 16 samples per span.
fn catmull(pts: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let n = pts.len();
    let mut o = Vec::with_capacity(n * 16);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(n - 1)];
        for k in 0..16 {
            let t = k as f32 / 16.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
            o.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    o.push(pts[n - 1]);
    o
}

/// Smooth 1D value noise in [-1, 1].
fn vnoise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let i = i as i32 as u32;
    let h = |k: u32| hash01(k.wrapping_mul(0x9e37_79b1) ^ seed.wrapping_mul(0x85eb_ca6b)) * 2.0 - 1.0;
    let u = f * f * (3.0 - 2.0 * f);
    h(i) + (h(i.wrapping_add(1)) - h(i)) * u
}

/// Shared brush look.
#[derive(Clone, Debug)]
struct Brush {
    color: Rgba,
    width: f32,
    pressure: Pressure,
    taper: [f32; 2],
    texture: String,
    opacity: f32,
    seed: u32,
}

/// A resampled centreline: points, unit normals and arc length.
struct Line {
    p: Vec<[f32; 2]>,
    n: Vec<[f32; 2]>,
    s: Vec<f32>,
}

impl Line {
    fn len(&self) -> f32 {
        *self.s.last().unwrap_or(&0.0)
    }
}

/// Resample `poly` (already in px) evenly, displace it by the boil, and
/// compute normals.
fn resample(poly: &[[f32; 2]], boil: f32, bseed: u32) -> Line {
    let mut cum = Vec::with_capacity(poly.len());
    let mut acc = 0.0;
    cum.push(0.0);
    for w in poly.windows(2) {
        acc += ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        cum.push(acc);
    }
    let total = acc.max(1e-3);
    let step = (total / 900.0).max(2.0);
    let n = ((total / step).ceil() as usize).max(2);
    let mut p = Vec::with_capacity(n + 1);
    let mut j = 0;
    for i in 0..=n {
        let s = total * i as f32 / n as f32;
        while j + 2 < cum.len() && cum[j + 1] < s {
            j += 1;
        }
        let seg = (cum[j + 1] - cum[j]).max(1e-6);
        let u = ((s - cum[j]) / seg).clamp(0.0, 1.0);
        let (a, b) = (poly[j], poly[(j + 1).min(poly.len() - 1)]);
        p.push([a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u]);
    }
    let normals = |p: &[[f32; 2]]| -> Vec<[f32; 2]> {
        let m = p.len();
        (0..m)
            .map(|i| {
                let a = p[i.saturating_sub(1)];
                let b = p[(i + 1).min(m - 1)];
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                [-dy / l, dx / l]
            })
            .collect()
    };
    let mut nr = normals(&p);
    if boil > 0.0 {
        // Low-frequency lateral wander plus a little drift along the tangent.
        for i in 0..p.len() {
            let s = total * i as f32 / n as f32;
            let d = boil * (vnoise(s / 70.0, bseed) * 0.8 + vnoise(s / 23.0, bseed ^ 0x51) * 0.35);
            let e = boil * 0.6 * vnoise(s / 90.0, bseed ^ 0xa3);
            p[i][0] += nr[i][0] * d - nr[i][1] * e;
            p[i][1] += nr[i][1] * d + nr[i][0] * e;
        }
        nr = normals(&p);
    }
    let mut s = Vec::with_capacity(p.len());
    let mut acc = 0.0;
    s.push(0.0);
    for w in p.windows(2) {
        acc += ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
        s.push(acc);
    }
    Line { p, n: nr, s }
}

/// Keep the line up to arc length `e`, interpolating the last point.
fn truncate(l: &Line, e: f32) -> Line {
    let k = l.s.partition_point(|&s| s < e).min(l.s.len() - 1);
    let mut out = Line { p: l.p[..k].to_vec(), n: l.n[..k].to_vec(), s: l.s[..k].to_vec() };
    if k == 0 {
        out.p.push(l.p[0]);
        out.n.push(l.n[0]);
        out.s.push(0.0);
    }
    let a = k.saturating_sub(1);
    let seg = (l.s[k] - l.s[a]).max(1e-6);
    let u = ((e - l.s[a]) / seg).clamp(0.0, 1.0);
    out.p.push([l.p[a][0] + (l.p[k][0] - l.p[a][0]) * u, l.p[a][1] + (l.p[k][1] - l.p[a][1]) * u]);
    out.n.push(l.n[k]);
    out.s.push(e.min(l.s[k]));
    out
}

fn pressure_at(p: &Pressure, u: f32) -> f32 {
    let u = clamp01(u);
    match p {
        Pressure::Values(v) => {
            if v.len() == 1 {
                return clamp01(v[0]);
            }
            let x = u * (v.len() - 1) as f32;
            let i = (x.floor() as usize).min(v.len() - 2);
            let f = x - i as f32;
            let f = f * f * (3.0 - 2.0 * f);
            clamp01(v[i] + (v[i + 1] - v[i]) * f)
        }
        Pressure::Preset(s) => match s.as_str() {
            "even" => 1.0,
            "taper" => 1.0 - 0.8 * u.powf(0.8),
            "flick" => {
                let rise = clamp01(u / 0.12);
                (1.0 - u).powf(0.55) * (0.35 + 0.65 * rise * (2.0 - rise))
            }
            // swell
            _ => 0.45 + 0.55 * (std::f32::consts::PI * u).sin().powf(0.7),
        },
    }
}

/// Tip shape for a normalised distance from a stroke end (0 at the tip).
fn tip(x: f32) -> f32 {
    if x >= 1.0 {
        1.0
    } else {
        (clamp01(x) * std::f32::consts::FRAC_PI_2).sin().powf(0.75)
    }
}

/// Fill polygon between `left[i] = p + n·(c + hl)` and `right[i] = p + n·(c − hr)`
/// over samples `i0..=i1`, with round caps.
fn ribbon(path: &mut Path, l: &Line, i0: usize, i1: usize, prof: &dyn Fn(usize) -> (f32, f32, f32)) {
    if i1 <= i0 {
        return;
    }
    let pt = |i: usize, off: f32| -> kurbo::Point {
        let (p, n) = (l.p[i], l.n[i]);
        kurbo::Point::new((p[0] + n[0] * off) as f64, (p[1] + n[1] * off) as f64)
    };
    let cap = |path: &mut Path, i: usize, c: f32, hl: f32, hr: f32, fwd: bool| {
        // Half circle from the left edge to the right edge (or back) around the end.
        let r = (hl + hr) * 0.5;
        if r < 0.6 {
            return;
        }
        let (p, n) = (l.p[i], l.n[i]);
        let mid = c + (hl - hr) * 0.5;
        let (cx, cy) = (p[0] + n[0] * mid, p[1] + n[1] * mid);
        let t = if fwd { [n[1], -n[0]] } else { [-n[1], n[0]] };
        for k in 1..6 {
            let a = std::f32::consts::PI * k as f32 / 6.0;
            let (ca, sa) = (a.cos(), a.sin());
            let sgn = if fwd { 1.0 } else { -1.0 };
            let dx = n[0] * ca * sgn + t[0] * sa;
            let dy = n[1] * ca * sgn + t[1] * sa;
            path.line_to(((cx + dx * r) as f64, (cy + dy * r) as f64));
        }
    };
    let (c0, l0, r0) = prof(i0);
    path.move_to(pt(i0, c0 + l0));
    for i in i0 + 1..=i1 {
        let (c, hl, _) = prof(i);
        path.line_to(pt(i, c + hl));
    }
    let (c1, l1, r1) = prof(i1);
    cap(path, i1, c1, l1, r1, true);
    for i in (i0..=i1).rev() {
        let (c, _, hr) = prof(i);
        path.line_to(pt(i, c - hr));
    }
    cap(path, i0, c0, l0, r0, false);
    path.close_path();
}

/// Build the ops of one stroke whose visible part is `line` (truncated to the
/// draw head); `total` is the full stroke length.
fn stroke_ops(out: &mut Vec<Op>, b: &Brush, line: &Line, total: f32, frame: u32) {
    let m = line.p.len();
    if m < 2 || total <= 0.0 {
        return;
    }
    let head = line.len();
    let hw0 = b.width * 0.5;
    let marker = b.texture == "marker";
    let tk = if marker { 0.12 } else { 1.0 };
    let t0 = (b.taper[0] * tk * total).max(0.5);
    let t1 = (b.taper[1] * tk * total).max(0.5);
    // Base half-width per sample (pressure × tips).
    let hw: Vec<f32> = (0..m)
        .map(|i| {
            let s = line.s[i];
            let w = hw0 * pressure_at(&b.pressure, s / total) * tip(s / t0) * tip((head - s) / t1);
            w.max(0.3)
        })
        .collect();
    let col = |a: f32| solid(Rgba([b.color.0[0], b.color.0[1], b.color.0[2], b.color.0[3] * clamp01(a * b.opacity)]));
    let seed = b.seed;
    match b.texture.as_str() {
        "dry" => {
            // A faint body, then bristles that skip and run out.
            let mut body = Path::new();
            ribbon(&mut body, line, 0, m - 1, &|i| {
                let s = line.s[i];
                (0.0, hw[i] * (0.82 + 0.1 * vnoise(s / 11.0, seed ^ 3)), hw[i] * (0.82 + 0.1 * vnoise(s / 13.0, seed ^ 4)))
            });
            out.push(fill(body, col(0.16)));
            let nb = (b.width / 2.0).clamp(7.0, 22.0) as usize;
            let mut buckets: [Path; 4] = Default::default();
            let mut rng = Rng::new(seed as u64 * 7919 + 17);
            for j in 0..nb {
                let f = -0.95 + 1.9 * (j as f32 + 0.5 + rng.range(-0.35, 0.35)) / nb as f32;
                let thick = rng.range(0.55, 1.5) / nb as f32;
                let end = 1.0 - rng.range(0.0, 0.45) * f.abs().powf(1.3) - rng.range(0.0, 0.08);
                let start = rng.range(0.0, 0.03);
                let freq = rng.range(18.0, 55.0);
                let bs = seed.wrapping_add(j as u32 * 131);
                let alpha_b = rng.range(0.5, 1.0);
                // Walk the samples, splitting into runs where the bristle carries paint.
                let mut i = 0;
                while i < m {
                    let carries = |i: usize| {
                        let u = line.s[i] / total;
                        let thresh = 0.85 - 1.1 * u * u - 0.25 * f.abs();
                        u >= start && u <= end && vnoise(line.s[i] / freq, bs) < thresh
                    };
                    if !carries(i) {
                        i += 1;
                        continue;
                    }
                    let a = i;
                    while i < m && carries(i) {
                        i += 1;
                    }
                    let z = i - 1;
                    if z <= a {
                        continue;
                    }
                    let (sa, sz) = (line.s[a], line.s[z]);
                    let um = (sa + sz) * 0.5 / total;
                    let level = clamp01(alpha_b * (1.05 - 0.55 * um));
                    let bi = ((level * 4.0) as usize).min(3);
                    ribbon(&mut buckets[bi], line, a, z, &|k| {
                        let s = line.s[k];
                        let ends = tip((s - sa) / 6.0) * tip((sz - s) / 9.0);
                        let h = hw[k] * thick * ends * (1.0 + 0.3 * vnoise(s / 7.0, bs ^ 9));
                        (f * hw[k] + 0.6 * vnoise(s / 30.0, bs ^ 5), h, h)
                    });
                }
            }
            for (k, p) in buckets.into_iter().enumerate() {
                if !p.elements().is_empty() {
                    out.push(fill(p, col(0.35 + 0.2 * k as f32)));
                }
            }
        }
        "marker" => {
            let mut body = Path::new();
            ribbon(&mut body, line, 0, m - 1, &|i| (0.0, hw[i], hw[i]));
            out.push(fill(body, col(0.62)));
            // Ink pools where the nib lands and lifts.
            let pool = (hw0 * 1.4).min(total * 0.12);
            let mut ends = Path::new();
            let i0 = line.s.partition_point(|&s| s < pool).min(m - 1);
            ribbon(&mut ends, line, 0, i0, &|i| (0.0, hw[i] * 0.96, hw[i] * 0.96));
            if head > total * 0.999 {
                let i1 = line.s.partition_point(|&s| s < head - pool).min(m - 2);
                ribbon(&mut ends, line, i1, m - 1, &|i| (0.0, hw[i] * 0.96, hw[i] * 0.96));
            }
            out.push(fill(ends, col(0.32)));
            // Streaks where the felt tip drags.
            let mut st = Path::new();
            for (k, f) in [-0.62f32, 0.18, 0.7].iter().enumerate() {
                let ss = seed ^ (k as u32 * 77 + 1);
                ribbon(&mut st, line, 0, m - 1, &|i| {
                    let h = hw[i] * 0.07 * (1.0 + vnoise(line.s[i] / 40.0, ss)).max(0.0);
                    (f * hw[i], h, h)
                });
            }
            out.push(fill(st, Paint::Solid(Rgba([b.color.0[0] * 0.7, b.color.0[1] * 0.7, b.color.0[2] * 0.7, 0.18 * b.opacity]))));
        }
        "pencil" => {
            // Graphite: many short jittered strands; pressure sets their density.
            let ns = (b.width / 1.6).clamp(3.0, 12.0) as usize;
            let mut groups: [Path; 3] = Default::default();
            let mut rng = Rng::new(seed as u64 * 104_729 + frame as u64 * 31 + 5);
            for _ in 0..ns {
                let f = rng.range(-0.85, 0.85);
                let mut s = rng.range(0.0, 10.0);
                while s < head {
                    let len = rng.range(7.0, 24.0);
                    let e = (s + len).min(head);
                    let i0 = line.s.partition_point(|&x| x < s).min(m - 1);
                    let i1 = line.s.partition_point(|&x| x < e).min(m - 1);
                    let press = hw[i0] / hw0.max(0.1);
                    if i1 > i0 && rng.f() < 0.35 + 0.75 * press {
                        let g = &mut groups[(rng.f() * 3.0) as usize % 3];
                        let j0 = rng.range(-0.6, 0.6);
                        let j1 = rng.range(-0.6, 0.6);
                        for i in i0..=i1 {
                            let u = (i - i0) as f32 / (i1 - i0) as f32;
                            let off = f * hw[i] + j0 + (j1 - j0) * u;
                            let q = ((line.p[i][0] + line.n[i][0] * off) as f64, (line.p[i][1] + line.n[i][1] * off) as f64);
                            if i == i0 {
                                g.move_to(q);
                            } else {
                                g.line_to(q);
                            }
                        }
                    }
                    s = e + rng.range(0.5, 4.0);
                }
            }
            for (k, p) in groups.into_iter().enumerate() {
                if !p.elements().is_empty() {
                    let mut st = line_style(0.9 + 0.35 * k as f32);
                    st.cap = 0;
                    out.push(Op::Stroke { path: p, paint: col(0.32 + 0.22 * k as f32), stroke: st, transform: IDENTITY });
                }
            }
        }
        _ => {
            // Ink: crisp with a slight hand wobble on each edge, plus a denser core.
            let mut body = Path::new();
            ribbon(&mut body, line, 0, m - 1, &|i| {
                let s = line.s[i];
                let wl = 1.0 + 0.07 * vnoise(s / 9.0, seed ^ 0x11) + 0.04 * vnoise(s / 2.7, seed ^ 0x12);
                let wr = 1.0 + 0.07 * vnoise(s / 10.0, seed ^ 0x21) + 0.04 * vnoise(s / 3.1, seed ^ 0x22);
                (0.0, hw[i] * wl, hw[i] * wr)
            });
            out.push(fill(body, col(0.94)));
        }
    }
}

fn line_style(w: f32) -> crate::dl::Stroke {
    line(w, [0.0, 1.0])
}

/// Small LRU of built ops keyed by everything that changes them.
#[derive(Default)]
struct Cache(Mutex<Vec<(u64, Vec<Op>)>>);
impl Cache {
    fn get(&self, key: u64, build: impl FnOnce() -> Vec<Op>) -> Vec<Op> {
        if let Ok(c) = self.0.lock() {
            if let Some((_, ops)) = c.iter().find(|(k, _)| *k == key) {
                return ops.clone();
            }
        }
        let ops = build();
        if let Ok(mut c) = self.0.lock() {
            if c.len() >= 6 {
                c.remove(0);
            }
            c.push((key, ops.clone()));
        }
        ops
    }
}

fn boil_frame(t: f32, boil: f32) -> u32 {
    if boil > 0.0 {
        (t * boil).floor().max(0.0) as u32 + 1
    } else {
        0
    }
}

/// Draw one set of centrelines with `b` at progress `k` (0..1 of the length).
fn draw_strokes(out: &mut Vec<Op>, lines: &[Vec<[f32; 2]>], b: &Brush, k: f32, sx: f32, sy: f32, frame: u32, amount: f32) {
    if k <= 0.0 {
        return;
    }
    let bseed = b.seed.wrapping_mul(0x2545_f491).wrapping_add(frame.wrapping_mul(0x9e37_79b9));
    let amt = if frame > 0 { amount } else { 0.0 };
    // Subpaths draw on one after another, in proportion to their lengths.
    let scaled: Vec<Vec<[f32; 2]>> = lines.iter().map(|l| l.iter().map(|p| [p[0] * sx, p[1] * sy]).collect()).collect();
    let rs: Vec<Line> = scaled.iter().enumerate().map(|(i, l)| resample(l, amt, bseed ^ (i as u32 * 7))).collect();
    let all: f32 = rs.iter().map(|l| l.len()).sum::<f32>().max(1e-3);
    let mut budget = k * all;
    for (i, l) in rs.iter().enumerate() {
        if budget <= 0.0 {
            break;
        }
        let len = l.len();
        let e = budget.min(len);
        budget -= len;
        let bi = Brush { seed: b.seed.wrapping_add(i as u32 * 1013), ..b.clone() };
        let vis = truncate(l, e);
        stroke_ops(out, &bi, &vis, len, frame);
    }
}

// ------------------------------------------------------------ brush

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrushStroke {
    /// SVG path data (px in the layer box, or 0–1 with `units: "fraction"`).
    pub d: Option<String>,
    /// Points the stroke passes through (smoothed).
    pub points: Option<Vec<[f32; 2]>>,
    /// "px" (default) or "fraction".
    pub units: String,
    pub color: Option<Color>,
    pub width: f32,
    pub pressure: Pressure,
    /// Tip lengths as fractions of the stroke: [start, end].
    pub taper: [f32; 2],
    /// "ink", "dry", "marker" or "pencil".
    pub texture: String,
    pub opacity: f32,
    pub seed: u32,
    /// Seconds to draw the stroke on (0 = already drawn).
    pub draw: f32,
    pub delay: f32,
    pub ease: String,
    /// Redraws per second of hand jitter (0 = off; 8 or 12 = "on twos").
    pub boil: f32,
    /// Boil amplitude in px.
    pub boil_amount: f32,
    #[serde(skip)]
    lines: Vec<Vec<[f32; 2]>>,
    #[serde(skip)]
    cache: Cache,
}
impl Default for BrushStroke {
    fn default() -> Self {
        BrushStroke {
            d: None,
            points: None,
            units: "px".into(),
            color: None,
            width: 18.0,
            pressure: Pressure::default(),
            taper: [0.12, 0.3],
            texture: "ink".into(),
            opacity: 1.0,
            seed: 1,
            draw: 0.0,
            delay: 0.0,
            ease: "inOut".into(),
            boil: 0.0,
            boil_amount: 1.5,
            lines: Vec::new(),
            cache: Cache::default(),
        }
    }
}

fn check_common(units: &str, texture: &str, ease_name: &str, width: f32, p: &Pressure) -> Result<(), Error> {
    if !["px", "fraction"].contains(&units) {
        return Err(unknown("units", units, &["px", "fraction"]));
    }
    if !TEXTURES.contains(&texture) {
        return Err(unknown("texture", texture, TEXTURES));
    }
    if !EASES.contains(&ease_name) {
        return Err(unknown("ease", ease_name, EASES));
    }
    if width.is_nan() || width <= 0.0 {
        return Err(Error("width: must be > 0".into()));
    }
    check_pressure(p)
}

impl Prepare for BrushStroke {
    fn prepare(&mut self) -> Result<(), Error> {
        check_common(&self.units, &self.texture, &self.ease, self.width, &self.pressure)?;
        self.lines = centrelines(&self.d, &self.points, self.units == "fraction")?;
        Ok(())
    }
}

fn progress(t: f32, delay: f32, draw: f32, ease_name: &str) -> f32 {
    if draw <= 0.0 {
        if t >= delay {
            1.0
        } else {
            0.0
        }
    } else {
        ease(ease_name, (t - delay) / draw)
    }
}

impl Component for BrushStroke {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let k = progress(t, self.delay, self.draw, &self.ease);
        if k <= 0.0 {
            return Vec::new();
        }
        let frame = boil_frame(t, self.boil);
        let (sx, sy) = if self.units == "fraction" { (ctx.width, ctx.height) } else { (1.0, 1.0) };
        let key = fnv(&[&k.to_bits().to_le_bytes(), &frame.to_le_bytes(), &sx.to_bits().to_le_bytes(), &sy.to_bits().to_le_bytes()]);
        let b = Brush {
            color: self.color.map(|c| c.0).unwrap_or(ctx.theme.text),
            width: self.width,
            pressure: self.pressure.clone(),
            taper: self.taper,
            texture: self.texture.clone(),
            opacity: self.opacity,
            seed: self.seed,
        };
        self.cache.get(key, || {
            let mut out = Vec::new();
            draw_strokes(&mut out, &self.lines, &b, k, sx, sy, frame, self.boil_amount);
            out
        })
    }
    fn duration(&self) -> Option<f32> {
        (self.draw > 0.0).then_some(self.delay + self.draw)
    }
}

// ------------------------------------------------------------ sketch

/// One path of a sketch; unset style keys come from the sketch.
#[derive(Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct SketchPath {
    pub d: Option<String>,
    pub points: Option<Vec<[f32; 2]>>,
    pub color: Option<Color>,
    pub width: Option<f32>,
    pub pressure: Option<Pressure>,
    pub taper: Option<[f32; 2]>,
    pub texture: Option<String>,
    pub opacity: Option<f32>,
    pub seed: Option<u32>,
    /// Extra delay for this path (s), on top of its stagger slot.
    pub delay: f32,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sketch {
    pub paths: Vec<SketchPath>,
    pub units: String,
    pub color: Option<Color>,
    pub width: f32,
    pub pressure: Pressure,
    pub taper: [f32; 2],
    pub texture: String,
    pub opacity: f32,
    pub seed: u32,
    /// Seconds to draw each path on (0 = all drawn).
    pub draw: f32,
    pub delay: f32,
    /// Seconds between successive paths starting to draw.
    pub stagger: f32,
    pub ease: String,
    pub boil: f32,
    pub boil_amount: f32,
    #[serde(skip)]
    lines: Vec<Vec<Vec<[f32; 2]>>>,
    #[serde(skip)]
    cache: Cache,
}
impl Default for Sketch {
    fn default() -> Self {
        Sketch {
            paths: Vec::new(),
            units: "px".into(),
            color: None,
            width: 10.0,
            pressure: Pressure::default(),
            taper: [0.12, 0.3],
            texture: "ink".into(),
            opacity: 1.0,
            seed: 1,
            draw: 0.0,
            delay: 0.0,
            stagger: 0.25,
            ease: "inOut".into(),
            boil: 0.0,
            boil_amount: 1.5,
            lines: Vec::new(),
            cache: Cache::default(),
        }
    }
}

impl Prepare for Sketch {
    fn prepare(&mut self) -> Result<(), Error> {
        check_common(&self.units, &self.texture, &self.ease, self.width, &self.pressure)?;
        if self.paths.is_empty() {
            return Err(Error("paths: give at least one {\"d\": …} or {\"points\": …}".into()));
        }
        let frac = self.units == "fraction";
        for (i, p) in self.paths.iter().enumerate() {
            let tex = p.texture.as_deref().unwrap_or(&self.texture);
            check_common(&self.units, tex, &self.ease, p.width.unwrap_or(self.width), p.pressure.as_ref().unwrap_or(&self.pressure))
                .map_err(|e| Error(format!("paths[{i}].{}", e.0)))?;
            self.lines.push(centrelines(&p.d, &p.points, frac).map_err(|e| Error(format!("paths[{i}].{}", e.0)))?);
        }
        Ok(())
    }
}

impl Component for Sketch {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let frame = boil_frame(t, self.boil);
        let (sx, sy) = if self.units == "fraction" { (ctx.width, ctx.height) } else { (1.0, 1.0) };
        let ks: Vec<f32> = self.paths.iter().enumerate().map(|(i, p)| progress(t, self.delay + i as f32 * self.stagger + p.delay, self.draw, &self.ease)).collect();
        let mut key_bytes = Vec::with_capacity(ks.len() * 4 + 12);
        for k in &ks {
            key_bytes.extend_from_slice(&k.to_bits().to_le_bytes());
        }
        key_bytes.extend_from_slice(&frame.to_le_bytes());
        key_bytes.extend_from_slice(&sx.to_bits().to_le_bytes());
        key_bytes.extend_from_slice(&sy.to_bits().to_le_bytes());
        let key = fnv(&[&key_bytes]);
        let base = self.color.map(|c| c.0).unwrap_or(ctx.theme.text);
        self.cache.get(key, || {
            let mut out = Vec::new();
            for (i, p) in self.paths.iter().enumerate() {
                let b = Brush {
                    color: p.color.map(|c| c.0).unwrap_or(base),
                    width: p.width.unwrap_or(self.width),
                    pressure: p.pressure.clone().unwrap_or_else(|| self.pressure.clone()),
                    taper: p.taper.unwrap_or(self.taper),
                    texture: p.texture.clone().unwrap_or_else(|| self.texture.clone()),
                    opacity: p.opacity.unwrap_or(self.opacity),
                    seed: p.seed.unwrap_or(self.seed.wrapping_mul(31).wrapping_add(i as u32 * 7_919)),
                };
                draw_strokes(&mut out, &self.lines[i], &b, ks[i], sx, sy, frame, self.boil_amount);
            }
            out
        })
    }
    fn duration(&self) -> Option<f32> {
        if self.draw <= 0.0 {
            return None;
        }
        let last = self.paths.iter().enumerate().map(|(i, p)| i as f32 * self.stagger + p.delay).fold(0.0, f32::max);
        Some(self.delay + last + self.draw)
    }
}
