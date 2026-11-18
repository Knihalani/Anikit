//! Illustration fills and textures: hand-drawn hatching, paper stock with real
//! tooth, and torn-paper collage cut-outs.
//!
//! * `hatch`: hatched, cross-hatched, stippled or scribbled fill of a shape,
//!   with tone gradients, hand-drawn wobble, boil and a line-by-line draw-on.
//! * `paper`: a full-frame paper background (paper, cotton, kraft, canvas,
//!   watercolor, newsprint), computed once, cached and dithered.
//! * `cutout`: cut or torn paper shapes with a fibrous rim, a lifted drop
//!   shadow, gentle hand-placed wobble and an entrance.

use super::bgfx::{fbm2, hash2, lin3, smoothstep, vnoise2, Lin};
use super::style::{alpha, clamp01, ease_out, fnv, hash01, mix, rotate_about, spring, Color, Rng};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{self, Affine, Op, Path, Rgba, IDENTITY};
use kurbo::{PathEl, Point, Shape};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------ shapes

/// A closed shape in box pixels: one of the named shapes fitted to the box, or an SVG path.
fn shape_path(shape: &str, d: Option<&Path>, w: f32, h: f32, seed: u32) -> Path {
    if let Some(p) = d {
        return p.clone();
    }
    let (cx, cy, rx, ry) = (w as f64 * 0.5, h as f64 * 0.5, w as f64 * 0.5, h as f64 * 0.5);
    match shape {
        "circle" => kurbo::Ellipse::new((cx, cy), (rx, ry), 0.0).to_path(0.1),
        "blob" | "star" => {
            let n = if shape == "star" { 10 } else { 48 };
            let mut pts = Vec::with_capacity(n);
            for i in 0..n {
                let a = i as f64 / n as f64 * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                let r = if shape == "star" {
                    if i % 2 == 0 { 1.0 } else { 0.45 }
                } else {
                    let (c, s) = (a.cos() as f32, a.sin() as f32);
                    0.8 + 0.2 * fbm2(c * 1.1 + 3.0, s * 1.1, 2, seed) as f64
                };
                pts.push(Point::new(cx + rx * r * a.cos(), cy + ry * r * a.sin()));
            }
            let mut p = Path::new();
            if shape == "star" {
                p.move_to(pts[0]);
                for q in &pts[1..] {
                    p.line_to(*q);
                }
            } else {
                // Closed Catmull-Rom.
                p.move_to(pts[0]);
                for i in 0..n {
                    let (p0, p1, p2, p3) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
                    p.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
                }
            }
            p.close_path();
            p
        }
        _ => kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).to_path(0.1),
    }
}

/// Flatten a path into closed polylines resampled every `step` pixels.
pub(super) fn rings(path: &Path, step: f32) -> Vec<Vec<(f32, f32)>> {
    let mut raw: Vec<Vec<(f32, f32)>> = Vec::new();
    kurbo::flatten(path.iter(), 0.25, |el| match el {
        PathEl::MoveTo(p) => raw.push(vec![(p.x as f32, p.y as f32)]),
        PathEl::LineTo(p) => {
            if let Some(r) = raw.last_mut() {
                r.push((p.x as f32, p.y as f32));
            }
        }
        _ => {}
    });
    raw.into_iter()
        .filter(|r| r.len() > 2)
        .map(|r| {
            let mut out = Vec::new();
            let n = r.len();
            for i in 0..n {
                let (a, b) = (r[i], r[(i + 1) % n]);
                let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                let k = (len / step).ceil().max(1.0) as usize;
                for j in 0..k {
                    let u = j as f32 / k as f32;
                    out.push((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u));
                }
            }
            out
        })
        .collect()
}

/// A ring displaced along its normal by `off(i, arc)` (positive = outwards for a counter-clockwise ring
/// in y-down space is handled by the caller via the sign of the area).
fn displaced(ring: &[(f32, f32)], off: impl Fn(usize, f32) -> f32) -> Path {
    let n = ring.len();
    let area: f32 = (0..n).map(|i| ring[i].0 * ring[(i + 1) % n].1 - ring[(i + 1) % n].0 * ring[i].1).sum();
    let sign = if area > 0.0 { 1.0 } else { -1.0 };
    let mut p = Path::new();
    let mut arc = 0.0;
    for i in 0..n {
        let (a, b) = (ring[(i + n - 1) % n], ring[(i + 1) % n]);
        let (tx, ty) = (b.0 - a.0, b.1 - a.1);
        let l = (tx * tx + ty * ty).sqrt().max(1e-6);
        // Outward normal for this winding.
        let (nx, ny) = (ty / l * sign, -tx / l * sign);
        let o = off(i, arc);
        let q = Point::new((ring[i].0 + nx * o) as f64, (ring[i].1 + ny * o) as f64);
        if i == 0 {
            p.move_to(q);
        } else {
            p.line_to(q);
        }
        arc += l * 0.5;
    }
    p.close_path();
    p
}

fn parse_d(d: &Option<String>) -> Result<Option<Path>, Error> {
    match d {
        None => Ok(None),
        Some(s) => Path::from_svg(s).map(Some).map_err(|e| Error(format!("bad SVG path `d`: {e}"))),
    }
}

fn check_one(what: &str, v: &str, ok: &[&str]) -> Result<(), Error> {
    if ok.contains(&v) {
        Ok(())
    } else {
        let hint = super::style::closest(v, ok).map(|c| format!(" (did you mean {c:?}?)")).unwrap_or_default();
        Err(Error(format!("{what} {v:?}: use one of {ok:?}{hint}")))
    }
}

fn bbox(p: &Path) -> (f32, f32, f32, f32) {
    let r = kurbo::Shape::bounding_box(p);
    (r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32)
}

fn fill_op(path: Path, c: Rgba, transform: Affine) -> Op {
    Op::Fill { path, paint: dl::Paint::Solid(c), rule: dl::FillRule::NonZero, transform }
}

fn push_clip(out: &mut Vec<Op>, path: Path, transform: Affine) {
    out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: Vec::new(), clip: Some((path, transform)), cache_key: None });
}

// ------------------------------------------------------------------ hatch

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Hatch {
    /// `"rect"` (default), `"circle"` or `"blob"`, fitted to the box.
    pub shape: String,
    /// SVG path in box pixels (overrides `shape`).
    pub d: Option<String>,
    /// `"hatch"` (default), `"crosshatch"`, `"stipple"` or `"scribble"`.
    pub style: String,
    /// Line angle in degrees (default 35).
    pub angle: f32,
    /// Line spacing at full tone, pixels (default 9).
    pub spacing: f32,
    /// Overall darkness 0..1 (default 0.6).
    pub tone: f32,
    /// Tone on the lit side / on the shadow side (a gradient along `light`).
    pub tone_from: Option<f32>,
    pub tone_to: Option<f32>,
    /// Direction the light comes from, degrees (0 = from the right, 90 = from below; default 225 = top-left).
    pub light: f32,
    pub color: Color,
    /// Line width (default 1.6).
    pub width: f32,
    /// Hand-drawn wobble, 0..1 (default 0.5).
    pub jitter: f32,
    /// Draw a hand-drawn outline of the shape.
    pub outline: bool,
    /// Re-seeds the hand-drawn jitter this many times a second (8–12 looks hand-made; 0 = off).
    pub boil: f32,
    pub seed: u32,
    /// Seconds to hatch it on, line by line (0 = all at once).
    pub draw: f32,
    #[serde(skip)]
    path: Option<Path>,
}
impl Default for Hatch {
    fn default() -> Self {
        Hatch {
            shape: "rect".into(),
            d: None,
            style: "hatch".into(),
            angle: 35.0,
            spacing: 9.0,
            tone: 0.6,
            tone_from: None,
            tone_to: None,
            light: 225.0,
            color: Color::hex("#2a2622"),
            width: 1.6,
            jitter: 0.5,
            outline: false,
            boil: 0.0,
            seed: 1,
            draw: 0.0,
            path: None,
        }
    }
}
impl Prepare for Hatch {
    fn prepare(&mut self) -> Result<(), Error> {
        check_one("hatch style", &self.style, &["hatch", "crosshatch", "stipple", "scribble"])?;
        check_one("hatch shape", &self.shape, &["rect", "circle", "blob"])?;
        if self.spacing.is_nan() || self.spacing <= 0.5 {
            return Err(Error("hatch spacing must be > 0.5 px".into()));
        }
        self.path = parse_d(&self.d)?;
        Ok(())
    }
}

/// Hatch, cross-hatch, stipple or scribble lines filling `path` (any coordinates; the
/// lines come back in the same space), with tone running `tone.0` (lit) → `tone.1`
/// (shadow) along `light`. Returns `(path, width, alpha)` per line, and the stipple dots.
pub(super) fn hatch_lines(path: Path, style: &str, angle: f32, spacing: f32, light: f32, tone: (f32, f32), width: f32, jitter: f32, seed: u32) -> (Vec<(Path, f32, f32)>, Path) {
    let h = Hatch { style: style.into(), angle, spacing: spacing.max(0.6), tone: tone.1, tone_from: Some(tone.0), tone_to: Some(tone.1), light, width, jitter, seed, path: Some(path), ..Default::default() };
    let (_, s) = h.strokes(0.0, 0.0, seed);
    (s.lines, s.dots)
}

/// One hand-drawn stroke from `a` to `b`: slight bow, wobble and uneven ends.
fn hand_line(p: &mut Path, a: (f32, f32), b: (f32, f32), jit: f32, rng: &mut Rng) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (nx, ny) = (-dy / len, dx / len);
    let bow = rng.range(-1.0, 1.0) * len * 0.02 * (0.3 + jit);
    let k = ((len / 40.0).ceil() as usize).clamp(2, 12);
    let ph = rng.f() * 100.0;
    let wob = jit * 1.2;
    let at = |u: f32| {
        let o = bow * 4.0 * u * (1.0 - u) + wob * vnoise2(u * len / 50.0 + ph, ph, 7);
        Point::new((a.0 + dx * u + nx * o) as f64, (a.1 + dy * u + ny * o) as f64)
    };
    p.move_to(at(0.0));
    let mut prev = at(0.0);
    for i in 1..=k {
        let q = at(i as f32 / k as f32);
        p.quad_to(prev.midpoint(q), q);
        prev = q;
    }
}

struct Strokes {
    /// (path, width, alpha) per line, in draw order.
    lines: Vec<(Path, f32, f32)>,
    dots: Path,
}

impl Hatch {
    fn tone_at(&self, x: f32, y: f32, bb: (f32, f32, f32, f32)) -> f32 {
        match (self.tone_from, self.tone_to) {
            (None, None) => self.tone,
            (a, b) => {
                let (a, b) = (a.unwrap_or(self.tone * 0.4), b.unwrap_or(self.tone));
                let la = self.light.to_radians();
                // Unit vector towards the light.
                let (lx, ly) = (la.cos(), la.sin());
                let (cx, cy) = ((bb.0 + bb.2) * 0.5, (bb.1 + bb.3) * 0.5);
                let ext = ((bb.2 - bb.0).abs() * lx.abs() + (bb.3 - bb.1).abs() * ly.abs()).max(1.0) * 0.5;
                let u = clamp01(0.5 - ((x - cx) * lx + (y - cy) * ly) / ext * 0.5);
                a + (b - a) * u
            }
        }
    }

    fn strokes(&self, w: f32, h: f32, seed: u32) -> (Path, Strokes) {
        let shape = shape_path(&self.shape, self.path.as_ref(), w, h, self.seed);
        let bb = bbox(&shape);
        let mut rng = Rng::new(seed as u64 * 7919 + 13);
        let jit = self.jitter.max(0.0);
        let mut lines = Vec::new();
        let mut dots = Path::new();
        let (cx, cy) = ((bb.0 + bb.2) * 0.5, (bb.1 + bb.3) * 0.5);
        let rad = 0.5 * ((bb.2 - bb.0).powi(2) + (bb.3 - bb.1).powi(2)).sqrt() + 4.0;
        let s = self.spacing;
        match self.style.as_str() {
            "stipple" => {
                let g = s * 0.55;
                let (nx, ny) = (((bb.2 - bb.0) / g).ceil() as i32 + 1, ((bb.3 - bb.1) / g).ceil() as i32 + 1);
                for j in 0..ny {
                    for i in 0..nx {
                        let hs = (i as u32).wrapping_mul(73856093) ^ (j as u32).wrapping_mul(19349663) ^ seed.wrapping_mul(83492791);
                        let x = bb.0 + (i as f32 + hash01(hs)) * g;
                        let y = bb.1 + (j as f32 + hash01(hs ^ 0x9e37)) * g;
                        let t = self.tone_at(x, y, bb);
                        // Ordered threshold: denser dots in shadow.
                        if hash01(hs ^ 0x51ed) < t * 1.05 {
                            let r = self.width * (0.55 + 0.4 * hash01(hs ^ 0xabc)) * (0.8 + 0.4 * t);
                            dots.extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.2));
                        }
                    }
                }
            }
            "scribble" => {
                // One continuous line of small loops, sweeping back and forth across the shape.
                let a = self.angle.to_radians();
                let (ux, uy) = (a.cos(), a.sin());
                let (vx, vy) = (-uy, ux);
                let mut pts = Vec::new();
                let mut v = -rad;
                let mut dir = 1.0f32;
                let loop_r = s * 1.5;
                let nloops = (2.0 * rad / (loop_r * 1.5)).ceil().max(4.0) as usize;
                let per = 14usize;
                while v < rad && pts.len() < 60000 {
                    let tc = self.tone_at(cx + vx * v, cy + vy * v, bb).max(0.08);
                    let sv = s / tc.powf(0.8);
                    for i in 0..nloops * per {
                        let f = i as f32 / (nloops * per) as f32;
                        let ph = f * nloops as f32 * std::f32::consts::TAU + rng.range(-0.15, 0.15) * jit;
                        let lr = loop_r * (0.7 + 0.5 * vnoise2(f * 9.0, v * 0.05, seed));
                        let uu = dir * (-rad + 2.0 * rad * f) + lr * ph.cos();
                        let vv = v + sv * f + lr * 0.8 * ph.sin();
                        pts.push((cx + ux * uu + vx * vv, cy + uy * uu + vy * vv));
                    }
                    v += sv;
                    dir = -dir;
                }
                let mut p = Path::new();
                if let Some(f) = pts.first() {
                    p.move_to((f.0 as f64, f.1 as f64));
                    for q in pts.iter().skip(1) {
                        p.line_to((q.0 as f64, q.1 as f64));
                    }
                }
                lines.push((p, self.width, 0.9));
            }
            _ => {
                let layers: &[(f32, f32)] = if self.style == "crosshatch" { &[(0.0, 0.0), (90.0, 0.45), (45.0, 0.78)] } else { &[(0.0, 0.0)] };
                for (li, &(da, gate)) in layers.iter().enumerate() {
                    let a = (self.angle + da + rng.range(-4.0, 4.0)).to_radians();
                    let (ux, uy) = (a.cos(), a.sin());
                    let (vx, vy) = (-uy, ux);
                    let n = (2.0 * rad / s).ceil() as i32;
                    for k in 0..=n {
                        let v = -rad + k as f32 * s + rng.range(-0.25, 0.25) * s * jit;
                        // Break each line into hand strokes of uneven length.
                        let mut u = -rad;
                        while u < rad {
                            let seg = rng.range(0.15, 0.45) * rad.max(60.0);
                            let u1 = (u + seg).min(rad);
                            let (mx, my) = (cx + ux * (u + u1) * 0.5 + vx * v, cy + uy * (u + u1) * 0.5 + vy * v);
                            let t = self.tone_at(mx, my, bb);
                            // Bit-reversed order over lines so dark areas get every line, light every 2nd/4th.
                            let order = (k as u32).reverse_bits() as f32 / u32::MAX as f32;
                            if t > 0.02 && (t - gate) / (1.0 - gate).max(1e-3) > order * 0.98 {
                                let a0 = u + rng.range(-0.08, 0.1) * seg;
                                let a1 = u1 - rng.range(-0.05, 0.15) * seg;
                                let mut p = Path::new();
                                hand_line(&mut p, (cx + ux * a0 + vx * v, cy + uy * a0 + vy * v), (cx + ux * a1 + vx * v, cy + uy * a1 + vy * v), jit, &mut rng);
                                let pressure = rng.range(0.7, 1.15);
                                lines.push((p, self.width * pressure * if li > 0 { 0.85 } else { 1.0 }, rng.range(0.65, 1.0)));
                            }
                            u = u1 + rng.range(-0.04, 0.06) * seg;
                        }
                    }
                }
            }
        }
        (shape, Strokes { lines, dots })
    }
}

impl Component for Hatch {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let frame = if self.boil > 0.0 { (t * self.boil).floor() as u32 } else { 0 };
        let seed = self.seed.wrapping_mul(977).wrapping_add(frame.wrapping_mul(131));
        let (shape, st) = self.strokes(ctx.width, ctx.height, seed);
        let prog = if self.draw > 0.0 { clamp01(t / self.draw) } else { 1.0 };
        let mut out = Vec::new();
        push_clip(&mut out, shape.clone(), IDENTITY);
        let col = self.color.0;
        if !st.dots.elements().is_empty() {
            // Stipple appears in a sweep.
            let op = if prog < 1.0 { prog } else { 1.0 };
            out.push(fill_op(st.dots.clone(), alpha(col, ease_out(op)), IDENTITY));
        }
        let n = st.lines.len();
        let shown = prog * n as f32;
        // Finished lines are merged per pressure bucket (few ops); the one being drawn is trimmed.
        let mut buckets: Vec<(Path, f32, f32)> = (0..4).map(|b| (Path::new(), self.width * (0.75 + 0.12 * b as f32), 0.0)).collect();
        for (i, (p, wd, a)) in st.lines.iter().enumerate() {
            let f = shown - i as f32;
            if f <= 0.0 {
                break;
            }
            if f >= 1.0 && n > 1 {
                let b = (((wd / self.width) - 0.7) / 0.12).clamp(0.0, 3.0) as usize;
                buckets[b].0.extend(p.iter());
                buckets[b].2 += a;
            } else {
                out.push(Op::Stroke { path: p.clone(), paint: dl::Paint::Solid(alpha(col, *a)), stroke: super::style::line(*wd, [0.0, f.min(1.0)]), transform: IDENTITY });
            }
        }
        let cnt = st.lines.len().max(1) as f32;
        for (p, wd, a) in buckets {
            if !p.elements().is_empty() {
                let avg = (a / cnt * 4.0).clamp(0.6, 0.95);
                out.push(Op::Stroke { path: p, paint: dl::Paint::Solid(alpha(col, avg)), stroke: super::style::line(wd, [0.0, 1.0]), transform: IDENTITY });
            }
        }
        out.push(Op::PopLayer);
        if self.outline {
            let mut rng = Rng::new(seed as u64 + 5);
            let ph = rng.f() * 50.0;
            for r in rings(&shape, 6.0) {
                let j = self.jitter;
                let p = displaced(&r, |i, _| j * 1.6 * vnoise2(i as f32 * 0.15 + ph, ph, 3));
                out.push(Op::Stroke { path: p, paint: dl::Paint::Solid(col), stroke: super::style::line(self.width * 1.6, [0.0, ease_out(clamp01(prog * 1.5))]), transform: IDENTITY });
            }
        }
        out
    }
    fn duration(&self) -> Option<f32> {
        (self.draw > 0.0).then_some(self.draw)
    }
}

// ------------------------------------------------------------------ paper

pub const PAPER_KINDS: &[&str] = &["paper", "cotton", "kraft", "canvas", "watercolor", "newsprint"];

fn paper_default(kind: &str) -> Rgba {
    Color::hex(match kind {
        "cotton" => "#f5f1e8",
        "kraft" => "#b08a5f",
        "canvas" => "#e8e0cf",
        "watercolor" => "#f3efe4",
        "newsprint" => "#e2dccd",
        _ => "#f2ede2",
    })
    .0
}

/// Paper stock as a linear-light field: albedo mottling, fibres and flecks, and a
/// height field (tooth / weave) lit from the top left.
pub fn paper_field(kind: &str, color: Rgba, tooth: f32, fibers: f32, vignette: f32, seed: u32, w: usize, h: usize) -> Lin {
    let n = w * h;
    let mut hgt = vec![0.0f32; n];
    let mut alb = vec![0.0f32; n];
    let sd = seed.wrapping_mul(2654435761);
    // Height field (tooth).
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32, y as f32);
            let i = y * w + x;
            let fine = hash2(x as i32, y as i32, sd) * 2.0 - 1.0;
            let v = match kind {
                "canvas" => {
                    let p = 5.0;
                    let wx = fx + 1.2 * vnoise2(fx * 0.02, fy * 0.02, sd ^ 3);
                    let wy = fy + 1.2 * vnoise2(fx * 0.02 + 9.0, fy * 0.02, sd ^ 4);
                    let (u, v) = (wx / p, wy / p);
                    let (iu, iv) = (u.floor(), v.floor());
                    let (fu, fv) = (u - iu, v - iv);
                    let over = ((iu as i32 + iv as i32) & 1) == 0;
                    let s1 = (std::f32::consts::PI * fu).sin();
                    let s2 = (std::f32::consts::PI * fv).sin();
                    // Thread thickness varies (slubs).
                    let slub_w = 0.8 + 0.35 * vnoise2(iu * 0.7, fy * 0.03, sd ^ 5);
                    let slub_h = 0.8 + 0.35 * vnoise2(fx * 0.03, iv * 0.7, sd ^ 6);
                    let warp = s1.powf(0.6) * slub_w * (0.75 + 0.25 * (std::f32::consts::PI * (v * 0.5).fract()).sin());
                    let weft = s2.powf(0.6) * slub_h * (0.75 + 0.25 * (std::f32::consts::PI * (u * 0.5).fract()).sin());
                    let base = if over { warp * 1.0 + weft * 0.25 } else { weft * 1.0 + warp * 0.25 };
                    base * 1.4 + 0.15 * fine
                }
                "watercolor" => {
                    // Cold-press: rounded bumps of ~8–20 px plus a finer grain.
                    let b = fbm2(fx / 11.0, fy / 11.0, 2, sd ^ 7) * 0.5 + 0.5;
                    let b2 = fbm2(fx / 4.5 + 3.1, fy / 4.5, 2, sd ^ 8);
                    let b3 = fbm2(fx / 28.0, fy / 28.0, 2, sd ^ 9);
                    b * b * 1.6 + b2 * 0.3 + b3 * 0.45 + 0.1 * fine
                }
                "newsprint" => 0.35 * vnoise2(fx / 1.6, fy / 1.6, sd ^ 10) + 0.25 * fine + 0.2 * vnoise2(fx / 5.0, fy / 5.0, sd ^ 11),
                "kraft" => 0.4 * vnoise2(fx / 2.2, fy / 2.2, sd ^ 12) + 0.3 * vnoise2(fx / 7.0, fy / 3.0, sd ^ 13) + 0.2 * fine,
                "cotton" => 0.45 * vnoise2(fx / 3.0, fy / 3.0, sd ^ 14) + 0.35 * vnoise2(fx / 8.0, fy / 8.0, sd ^ 15) + 0.15 * fine,
                _ => 0.5 * vnoise2(fx / 2.0, fy / 2.0, sd ^ 16) + 0.3 * vnoise2(fx / 6.0, fy / 6.0, sd ^ 17) + 0.15 * fine,
            };
            hgt[i] = v;
            // Albedo mottling (sizing, formation clouds).
            let (mf, ma) = match kind {
                "watercolor" => (1.0 / 140.0, 0.055),
                "kraft" => (1.0 / 60.0, 0.06),
                "newsprint" => (1.0 / 50.0, 0.045),
                "canvas" => (1.0 / 200.0, 0.03),
                _ => (1.0 / 90.0, 0.025),
            };
            let mut m = fbm2(fx * mf, fy * mf, 3, sd ^ 20) * ma;
            if kind == "watercolor" {
                // Blotchy sizing: soft-edged slightly darker patches.
                let b = fbm2(fx / 260.0 + 4.0, fy / 260.0, 3, sd ^ 21);
                m -= 0.035 * smoothstep(0.05, 0.35, b);
            }
            alb[i] = m;
        }
    }
    // Fibres and flecks, splatted along short curved paths.
    let mut rng = Rng::new(seed as u64 * 31 + 7);
    let area = (w * h) as f32;
    let (count, len, dark, hamp, fleck) = match kind {
        "cotton" => (area / 260.0, (14.0, 50.0), 0.07, 0.5, 0.0),
        "kraft" => (area / 300.0, (6.0, 30.0), -0.16, 0.3, area / 700.0),
        "newsprint" => (area / 450.0, (4.0, 18.0), -0.14, 0.15, area / 1200.0),
        "watercolor" => (area / 1200.0, (10.0, 40.0), 0.05, 0.35, 0.0),
        "canvas" => (area / 3000.0, (6.0, 20.0), 0.06, 0.3, 0.0),
        _ => (area / 800.0, (8.0, 35.0), 0.06, 0.35, area / 20000.0),
    };
    let count = (count * fibers * 2.0) as usize;
    let splat = |x: f32, y: f32, a: f32, ha: f32, alb: &mut [f32], hgt: &mut [f32]| {
        let (ix, iy) = (x.floor() as isize, y.floor() as isize);
        let (fx, fy) = (x - ix as f32, y - iy as f32);
        for (dx, dy, wgt) in [(0, 0, (1.0 - fx) * (1.0 - fy)), (1, 0, fx * (1.0 - fy)), (0, 1, (1.0 - fx) * fy), (1, 1, fx * fy)] {
            let (px, py) = (ix + dx, iy + dy);
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                let j = py as usize * w + px as usize;
                alb[j] += a * wgt;
                hgt[j] += ha * wgt;
            }
        }
    };
    for _ in 0..count {
        let (mut x, mut y) = (rng.f() * w as f32, rng.f() * h as f32);
        let mut a = rng.f() * std::f32::consts::TAU;
        let l = rng.range(len.0, len.1);
        let curl = rng.range(-0.12, 0.12);
        let strength = rng.range(0.4, 1.0) * if rng.f() < 0.3 { -0.6 } else { 1.0 };
        let mut s = 0.0;
        while s < l {
            let fade = (std::f32::consts::PI * s / l).sin();
            splat(x, y, dark * strength * fade, hamp * fade, &mut alb, &mut hgt);
            x += a.cos() * 0.7;
            y += a.sin() * 0.7;
            a += curl + rng.range(-0.08, 0.08);
            s += 0.7;
        }
    }
    let flecks = (fleck * fibers * 2.0) as usize;
    for _ in 0..flecks {
        let (x, y) = (rng.f() * w as f32, rng.f() * h as f32);
        let r = rng.range(0.6, 2.2);
        let d = rng.range(-0.35, -0.12);
        let k = (r * 2.0).ceil() as i32;
        for dy in -k..=k {
            for dx in -k..=k {
                let dd = ((dx * dx + dy * dy) as f32).sqrt();
                if dd < r * (0.8 + 0.4 * hash01((dx * 7 + dy * 13) as u32 ^ x as u32)) {
                    splat(x + dx as f32, y + dy as f32, d, 0.0, &mut alb, &mut hgt);
                }
            }
        }
    }
    // Light it: emboss from the top left, then albedo and vignette.
    let base = lin3(color);
    let tooth_k = tooth * match kind {
        "canvas" => 0.30,
        "watercolor" => 0.30,
        _ => 0.24,
    };
    let mut out = Lin::new(w, h);
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let diag = (cx * cx + cy * cy).sqrt();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (x0, x1) = (x.saturating_sub(1), (x + 1).min(w - 1));
            let (y0, y1) = (y.saturating_sub(1), (y + 1).min(h - 1));
            let gx = hgt[y * w + x1] - hgt[y * w + x0];
            let gy = hgt[y1 * w + x] - hgt[y0 * w + x];
            let lit = 1.0 - tooth_k * (gx + gy) * 0.7 + tooth_k * 0.25 * hgt[i];
            let r = (((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / diag).powi(2);
            let vig = 1.0 - vignette * 0.35 * r;
            let k = (1.0 + alb[i]) * lit * vig;
            // Darker fibres lean warm, as real pulp does.
            let warm = alb[i].min(0.0) * 0.4;
            out.px[i * 3] = (base[0] * k).max(0.0);
            out.px[i * 3 + 1] = (base[1] * k * (1.0 + warm * 0.3)).max(0.0);
            out.px[i * 3 + 2] = (base[2] * k * (1.0 + warm)).max(0.0);
        }
    }
    out
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Paper {
    /// `"paper"` (default), `"cotton"`, `"kraft"`, `"canvas"`, `"watercolor"` or `"newsprint"`.
    /// (`kind` is the component name in a spec layer, so the stock is `stock`.)
    pub stock: String,
    /// Stock colour (default per kind).
    pub color: Option<Color>,
    /// Surface relief 0..1 (default 0.6).
    pub tooth: f32,
    /// Fibres and flecks 0..1 (default 0.5).
    pub fibers: f32,
    /// Edge darkening 0..1 (default 0.3).
    pub vignette: f32,
    pub seed: u32,
}
impl Default for Paper {
    fn default() -> Self {
        Paper { stock: "paper".into(), color: None, tooth: 0.6, fibers: 0.5, vignette: 0.3, seed: 1 }
    }
}
impl Prepare for Paper {
    fn prepare(&mut self) -> Result<(), Error> {
        check_one("paper stock", &self.stock, PAPER_KINDS)
    }
}

/// The cached paper image for a box (straight RGBA8, opaque).
fn paper_image(ctx: &Ctx, kind: &str, color: Rgba, tooth: f32, fibers: f32, vignette: f32, seed: u32, w: usize, h: usize) -> dl::ImageId {
    let key = fnv(&[b"illus-paper", kind.as_bytes(), &bytemuck_f(&[color.0[0], color.0[1], color.0[2], tooth, fibers, vignette]), &seed.to_le_bytes(), &(w as u32).to_le_bytes(), &(h as u32).to_le_bytes()]);
    ctx.assets.image(key, w as u32, h as u32, &mut || paper_field(kind, color, tooth, fibers, vignette, seed, w, h).to_rgba8())
}

fn bytemuck_f(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

impl Component for Paper {
    fn eval(&self, _t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width.ceil().clamp(4.0, 4096.0) as usize, ctx.height.ceil().clamp(4.0, 4096.0) as usize);
        let color = self.color.map(|c| c.0).unwrap_or_else(|| paper_default(&self.stock));
        let id = paper_image(ctx, &self.stock, color, self.tooth, self.fibers, self.vignette, self.seed, w, h);
        vec![Op::Image { image: id, dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: IDENTITY }]
    }
}

// ------------------------------------------------------------------ cutout

type CachedEdges = ((u32, u32), Arc<(Path, Path, Path)>);

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cutout {
    /// `"rect"` (default), `"circle"`, `"blob"` or `"star"`, fitted to the box.
    pub shape: String,
    /// SVG path in box pixels (overrides `shape`).
    pub d: Option<String>,
    /// Paper colour (default a warm red); ignored with `texture`.
    pub color: Color,
    /// `"paper"`, `"kraft"` or `"newsprint"`: fill with that stock instead of a flat colour.
    pub texture: Option<String>,
    /// Torn, deckled edge with a lighter fibrous rim, 0..1 (default 0 = cleanly cut).
    pub torn: f32,
    /// Lifted drop shadow strength 0..1 (default 0.5).
    pub shadow: f32,
    /// Rotation in degrees.
    pub rotate: f32,
    /// Gentle hand-placed jitter 0..1 (default 0.3).
    pub wobble: f32,
    /// Re-place the wobble this many times a second (0 = smooth drift).
    pub boil: f32,
    /// `"drop"` (default), `"slide"` or `"none"`.
    pub enter: String,
    /// Seconds before the entrance.
    pub delay: f32,
    pub seed: u32,
    #[serde(skip)]
    path: Option<Path>,
    #[serde(skip)]
    edges: Mutex<Option<CachedEdges>>,
}
impl Default for Cutout {
    fn default() -> Self {
        Cutout {
            shape: "rect".into(),
            d: None,
            color: Color::hex("#d4553a"),
            texture: None,
            torn: 0.0,
            shadow: 0.5,
            rotate: 0.0,
            wobble: 0.3,
            boil: 0.0,
            enter: "drop".into(),
            delay: 0.0,
            seed: 1,
            path: None,
            edges: Mutex::new(None),
        }
    }
}
impl Prepare for Cutout {
    fn prepare(&mut self) -> Result<(), Error> {
        check_one("cutout shape", &self.shape, &["rect", "circle", "blob", "star"])?;
        check_one("cutout enter", &self.enter, &["drop", "slide", "none"])?;
        if let Some(t) = &self.texture {
            check_one("cutout texture", t, &["paper", "kraft", "newsprint"])?;
        }
        self.path = parse_d(&self.d)?;
        *self.edges.get_mut().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
}

impl Cutout {
    /// Outer torn edge, inner (coloured) edge and rim fibres, in box pixels.
    fn edges(&self, w: f32, h: f32) -> Arc<(Path, Path, Path)> {
        let key = (w.to_bits(), h.to_bits());
        if let Ok(g) = self.edges.lock() {
            if let Some((_, edges)) = g.as_ref().filter(|(cached_key, _)| *cached_key == key) {
                return edges.clone();
            }
        }
        let edges = Arc::new(self.make_edges(w, h));
        if let Ok(mut g) = self.edges.lock() {
            *g = Some((key, edges.clone()));
        }
        edges
    }

    fn make_edges(&self, w: f32, h: f32) -> (Path, Path, Path) {
        let shape = shape_path(&self.shape, self.path.as_ref(), w, h, self.seed);
        if self.torn <= 0.001 {
            return (shape.clone(), shape, Path::new());
        }
        let tn = self.torn;
        let sd = self.seed.wrapping_mul(7717);
        let mut outer = Path::new();
        let mut inner = Path::new();
        let mut fib = Path::new();
        let mut rng = Rng::new(self.seed as u64 + 99);
        for r in rings(&shape, 2.0) {
            // Irregular tear: big wander plus fine ragged teeth.
            let tear = |i: usize, sx: u32| {
                let u = i as f32 * 2.0;
                -tn * (4.0 + 12.0 * (fbm2(u / 90.0, 0.5, 4, sd ^ sx) * 0.5 + 0.5) + 2.5 * vnoise2(u / 3.0, 2.0, sd ^ sx ^ 1).abs() + 1.2 * vnoise2(u / 1.2, 5.0, sd ^ sx ^ 2).abs())
            };
            let o = displaced(&r, |i, _| tear(i, 1) + tn * 2.0);
            let rim = |i: usize| tear(i, 1) + tn * 2.0 - tn * (2.0 + 9.0 * (vnoise2(i as f32 * 2.0 / 40.0, 7.0, sd ^ 3) * 0.5 + 0.5).powi(2)) - tn * 1.8 * vnoise2(i as f32 * 2.0 / 2.5, 1.0, sd ^ 4).abs();
            let inn = displaced(&r, |i, _| rim(i));
            outer.extend(o.iter());
            inner.extend(inn.iter());
            // Short fibres poking out of the torn rim.
            let n = r.len();
            let area: f32 = (0..n).map(|i| r[i].0 * r[(i + 1) % n].1 - r[(i + 1) % n].0 * r[i].1).sum();
            let sign = if area > 0.0 { 1.0 } else { -1.0 };
            for i in (0..n).step_by(2) {
                if rng.f() > 0.6 * tn {
                    continue;
                }
                let (a, b) = (r[(i + n - 1) % n], r[(i + 1) % n]);
                let (tx, ty) = (b.0 - a.0, b.1 - a.1);
                let l = (tx * tx + ty * ty).sqrt().max(1e-6);
                let (nx, ny) = (ty / l * sign, -tx / l * sign);
                let e = tear(i, 1) + tn * 2.0;
                let (px, py) = (r[i].0 + nx * (e - 0.5), r[i].1 + ny * (e - 0.5));
                let ang = ny.atan2(nx) + rng.range(-0.9, 0.9);
                let fl = rng.range(1.5, 6.0) * tn;
                fib.move_to((px as f64, py as f64));
                fib.quad_to(((px + ang.cos() * fl * 0.5 + rng.range(-1.0, 1.0)) as f64, (py + ang.sin() * fl * 0.5 + rng.range(-1.0, 1.0)) as f64), ((px + ang.cos() * fl) as f64, (py + ang.sin() * fl) as f64));
            }
        }
        (outer, inner, fib)
    }
}

impl Component for Cutout {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let te = t - self.delay;
        if te < 0.0 && self.enter != "none" {
            return Vec::new();
        }
        let edges = self.edges(w, h);
        let (outer, inner, fib) = &*edges;
        // Entrance.
        let (mut dx, mut dy, mut sc, mut op, mut lift) = (0.0, 0.0, 1.0, 1.0, 0.0);
        match self.enter.as_str() {
            "drop" => {
                let p = spring(te.max(0.0), 1.4);
                sc = 1.0 + 0.12 * (1.0 - p);
                lift = 1.0 - clamp01(te / 0.45);
                op = clamp01(te / 0.12);
            }
            "slide" => {
                let p = ease_out(clamp01(te / 0.7));
                dx = -(w * 0.35 + 80.0) * (1.0 - p);
                dy = 20.0 * (1.0 - p);
                op = clamp01(te / 0.15);
            }
            _ => {}
        }
        // Wobble: a small drift (smooth) or a re-placed jitter (boil).
        let wt = if self.boil > 0.0 { (t * self.boil).floor() / self.boil.max(1e-3) * 3.7 } else { t * 0.35 };
        let s = self.seed;
        let wb = self.wobble;
        let rot = self.rotate.to_radians() + wb * 0.02 * vnoise2(wt, 0.3, s ^ 1) + if self.enter == "slide" { 0.15 * (1.0 - ease_out(clamp01(te / 0.7))) } else { 0.0 };
        dx += wb * 2.0 * vnoise2(wt, 1.7, s ^ 2);
        dy += wb * 2.0 * vnoise2(wt, 3.1, s ^ 3);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let m: Affine = dl::mul(&[sc, 0.0, 0.0, sc, cx - sc * cx + dx, cy - sc * cy + dy], &rotate_about(rot, cx, cy));
        let mut out = Vec::new();
        let faded = op < 0.999;
        if faded {
            out.push(Op::PushLayer { opacity: op, blend: dl::Blend::Normal, filters: Vec::new(), clip: None, cache_key: None });
        }
        // Lifted paper shadow: offset down-right, larger while it is still in the air.
        if self.shadow > 0.0 {
            let sh = self.shadow;
            let (sdx, sdy) = (3.0 + 10.0 * lift, 5.0 + 16.0 * lift);
            out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: vec![dl::Filter::Shadow { dx: sdx, dy: sdy, blur: 6.0 + 14.0 * lift, color: Rgba([0.12, 0.08, 0.05, (0.7 * sh) * (1.0 - 0.4 * lift)]) }], clip: None, cache_key: None });
            out.push(fill_op(outer.clone(), Rgba([0.0, 0.0, 0.0, 1.0]), m));
            out.push(Op::PopLayer);
        }
        // The light, fibrous core showing at a tear.
        let base = match self.texture.as_deref() {
            Some(k) => paper_default(k),
            None => self.color.0,
        };
        if self.torn > 0.001 {
            let rim = mix(base, Rgba([0.98, 0.965, 0.93, 1.0]), 0.88);
            out.push(fill_op(outer.clone(), rim, m));
            out.push(Op::Stroke { path: fib.clone(), paint: dl::Paint::Solid(alpha(rim, 0.9)), stroke: super::style::line(0.7, [0.0, 1.0]), transform: m });
        }
        // The face of the paper.
        push_clip(&mut out, inner.clone(), m);
        let (x0, y0, x1, y1) = bbox(outer);
        let (bw, bh) = ((x1 - x0).ceil().max(4.0) as usize + 2, (y1 - y0).ceil().max(4.0) as usize + 2);
        let (kind, col) = match self.texture.as_deref() {
            Some(k) => (k, base),
            None => ("paper", base),
        };
        // Flat colours still get a faint paper grain so they don't look vector-flat.
        let (tooth, fibers) = if self.texture.is_some() { (0.8, 0.6) } else { (0.7, 0.35) };
        let id = paper_image(ctx, kind, col, tooth, fibers, 0.0, s, bw.min(4096), bh.min(4096));
        out.push(Op::Image { image: id, dst: [x0 - 1.0, y0 - 1.0, bw as f32, bh as f32], opacity: 1.0, transform: m });
        out.push(Op::PopLayer);
        if faded {
            out.push(Op::PopLayer);
        }
        out
    }
    fn duration(&self) -> Option<f32> {
        (self.enter != "none").then_some(self.delay + 0.8)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{preview, Component, Ctx, Theme};
    use super::*;

    fn torn_cutout() -> Cutout {
        let mut cutout = Cutout { torn: 0.8, ..Cutout::default() };
        cutout.prepare().unwrap();
        cutout
    }

    #[test]
    fn torn_edges_match_fresh_cutout_across_times_and_sizes() {
        let theme = Theme::default();
        let assets = preview::host();
        let ctx = Ctx { width: 600.0, height: 400.0, fps: 30.0, theme: &theme, assets };
        let resized_ctx = Ctx { width: 800.0, height: 500.0, ..ctx };
        let cutout = torn_cutout();

        for t in [0.2, 1.0, 2.5] {
            assert_eq!(cutout.eval(t, &ctx), torn_cutout().eval(t, &ctx));
        }
        assert_eq!(cutout.eval(1.0, &resized_ctx), torn_cutout().eval(1.0, &resized_ctx));
    }

    #[test]
    fn edges_refresh_after_reprepare() {
        let theme = Theme::default();
        let assets = preview::host();
        let ctx = Ctx { width: 600.0, height: 400.0, fps: 30.0, theme: &theme, assets };
        let mut cutout = torn_cutout();
        cutout.eval(1.0, &ctx);

        cutout.shape = "circle".into();
        cutout.seed = 7;
        cutout.prepare().unwrap();

        let mut fresh = Cutout { torn: 0.8, shape: "circle".into(), seed: 7, ..Cutout::default() };
        fresh.prepare().unwrap();
        assert_eq!(cutout.eval(1.0, &ctx), fresh.eval(1.0, &ctx));
    }
}
