//! A rotating dot-matrix globe (the Stripe / GitHub look): land drawn as a
//! grid of dots, an atmosphere glow, great-circle arcs that draw on with
//! travelling pulses, and markers with pulse rings.
//!
//! Everything is vector. The dot lattice (land tested against a built-in
//! set of coarse continent polygons) and the arcs' 3D samples are computed
//! once in [`Prepare`]; each frame only rotates and projects them. Dots are
//! foreshortened into ellipses and batched into a handful of fills by
//! brightness, so a frame is a few dozen ops however many dots there are.

use super::fxkit;
use super::style::*;
use super::{rgba, Component, Ctx, Error, Prepare};
use crate::dl::{FillRule, Op, Paint, Path, IDENTITY};
use kurbo::{BezPath, Point};
use serde::Deserialize;

/// Brightness levels the front dots are batched into.
const LEVELS: usize = 12;
/// Samples along each arc.
const ARC_SAMPLES: usize = 96;
const DOTS: &[&str] = &["land", "all", "none"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Globe {
    /// Great-circle arcs `[lat1, lon1, lat2, lon2]` (degrees) that draw on in turn.
    pub arcs: Vec<[f32; 4]>,
    /// Pulsing markers `[lat, lon]`.
    pub markers: Vec<[f32; 2]>,
    /// Rotation speed (degrees of longitude per second).
    pub spin: f32,
    /// Latitude the camera looks from (degrees).
    pub tilt: f32,
    /// Longitude facing the camera at the start (defaults to the middle of the arcs and markers).
    pub lon: Option<f32>,
    /// Dot and atmosphere colour (defaults to a cool sky blue with a touch of the accent).
    pub color: Option<Color>,
    /// Arc colours: one colour, or a gradient from start to end (defaults to the theme accent warming to white).
    pub arc_color: Vec<Color>,
    /// Marker colour (defaults to the end of the arc gradient).
    pub marker_color: Option<Color>,
    /// `land` (default), `all` (the whole sphere) or `none`.
    pub dots: String,
    /// Dot density (1 = 100 rows from pole to pole).
    pub density: f32,
    /// Draw latitude/longitude lines.
    pub grid: bool,
    /// Atmosphere strength (0 = none).
    pub glow: f32,
    /// Seconds between arcs and how long each takes to draw.
    pub stagger: f32,
    pub draw: f32,
    /// Seconds before the globe fades in.
    pub delay: f32,
    /// Size multiplier (1 = the globe fills 80 % of the box's short side).
    pub scale: f32,
    /// Static dot lattice (unit vectors) and arc samples (globe space).
    #[serde(skip)]
    pts: Vec<[f32; 3]>,
    #[serde(skip)]
    arc_pts: Vec<Vec<[f32; 3]>>,
    #[serde(skip)]
    spacing: f32,
    #[serde(skip)]
    lon0: f32,
}

impl Default for Globe {
    fn default() -> Self {
        Globe {
            arcs: Vec::new(),
            markers: Vec::new(),
            spin: 8.0,
            tilt: 18.0,
            lon: None,
            color: None,
            arc_color: Vec::new(),
            marker_color: None,
            dots: "land".into(),
            density: 1.0,
            grid: false,
            glow: 1.0,
            stagger: 0.45,
            draw: 1.4,
            delay: 0.0,
            scale: 1.0,
            pts: Vec::new(),
            arc_pts: Vec::new(),
            spacing: 0.0,
            lon0: 0.0,
        }
    }
}

/// Unit vector for latitude/longitude in degrees (y up, lon 0 facing +z).
fn unit(lat: f32, lon: f32) -> [f32; 3] {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    [la.cos() * lo.sin(), la.sin(), la.cos() * lo.cos()]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Prepare for Globe {
    fn prepare(&mut self) -> Result<(), Error> {
        for (name, v) in [("density", self.density), ("glow", self.glow), ("stagger", self.stagger), ("draw", self.draw), ("scale", self.scale)] {
            if !v.is_finite() || v < 0.0 {
                return Err(Error(format!("`{name}` must be a non-negative number, got {v}")));
            }
        }
        for (name, v) in [("spin", self.spin), ("tilt", self.tilt), ("delay", self.delay), ("lon", self.lon.unwrap_or(0.0))] {
            if !v.is_finite() {
                return Err(Error(format!("`{name}` must be a finite number")));
            }
        }
        if self.density > 4.0 {
            return Err(Error(format!("`density` must be at most 4, got {}", self.density)));
        }
        let ll_ok = |lat: f32, lon: f32| lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0;
        if let Some(a) = self.arcs.iter().find(|a| !ll_ok(a[0], a[1]) || !ll_ok(a[2], a[3])) {
            return Err(Error(format!("arc {a:?}: expected [lat1, lon1, lat2, lon2] with latitudes in -90..90")));
        }
        if let Some(m) = self.markers.iter().find(|m| !ll_ok(m[0], m[1])) {
            return Err(Error(format!("marker {m:?}: expected [lat, lon] with latitude in -90..90")));
        }
        if !DOTS.contains(&self.dots.as_str()) {
            return Err(Error(match closest(&self.dots, DOTS) {
                Some(c) => format!("unknown dots `{}`; did you mean `{c}`?", self.dots),
                None => format!("unknown dots `{}`; expected one of {}", self.dots, DOTS.join(", ")),
            }));
        }
        // Face the arcs and markers (circular mean of their longitudes).
        self.lon0 = self.lon.unwrap_or_else(|| {
            let (mut s, mut c) = (0.0f32, 0.0f32);
            for l in self.arcs.iter().flat_map(|a| [a[1], a[3]]).chain(self.markers.iter().map(|m| m[1])) {
                s += l.to_radians().sin();
                c += l.to_radians().cos();
            }
            if s == 0.0 && c == 0.0 {
                -30.0
            } else {
                s.atan2(c).to_degrees()
            }
        });
        // Dot lattice: rows of latitude, evenly spaced along each row, odd rows offset.
        let rows = (100.0 * self.density).round().max(8.0) as usize;
        let step = 180.0 / rows as f32;
        self.spacing = step.to_radians();
        self.pts.clear();
        if self.dots != "none" {
            let land = self.dots == "land";
            for j in 0..rows {
                let lat = -90.0 + (j as f32 + 0.5) * step;
                let n = ((360.0 * lat.to_radians().cos() / step).round() as usize).max(1);
                let off = if j % 2 == 0 { 0.0 } else { 0.5 };
                for i in 0..n {
                    let lon = -180.0 + (i as f32 + off) * 360.0 / n as f32;
                    if !land || is_land(lat, lon) {
                        self.pts.push(unit(lat, lon));
                    }
                }
            }
        }
        // Arcs: slerp between the endpoints, lifted by up to a fifth of a radius
        // in proportion to their length.
        self.arc_pts = self
            .arcs
            .iter()
            .map(|a| {
                let (p, q) = (unit(a[0], a[1]), unit(a[2], a[3]));
                let ang = dot3(p, q).clamp(-1.0, 1.0).acos();
                let lift = 0.05 + 0.32 * (ang / std::f32::consts::PI);
                let s = ang.sin();
                (0..ARC_SAMPLES)
                    .map(|k| {
                        let t = k as f32 / (ARC_SAMPLES - 1) as f32;
                        let (wa, wb) = if s < 1e-4 { (1.0 - t, t) } else { (((1.0 - t) * ang).sin() / s, (t * ang).sin() / s) };
                        let v = [wa * p[0] + wb * q[0], wa * p[1] + wb * q[1], wa * p[2] + wb * q[2]];
                        let l = dot3(v, v).sqrt().max(1e-6);
                        let r = 1.0 + lift * (std::f32::consts::PI * t).sin();
                        [v[0] / l * r, v[1] / l * r, v[2] / l * r]
                    })
                    .collect()
            })
            .collect();
        Ok(())
    }
}

/// Rotation into view space: spin about the axis, then tilt towards the camera.
struct View {
    cs: (f32, f32),
    ct: (f32, f32),
    cx: f32,
    cy: f32,
    r: f32,
}

impl View {
    fn rot(&self, p: [f32; 3]) -> [f32; 3] {
        let (s, c) = self.cs;
        let x = c * p[0] - s * p[2];
        let z = s * p[0] + c * p[2];
        let (st, ct) = self.ct;
        [x, ct * p[1] - st * z, st * p[1] + ct * z]
    }
    fn px(&self, v: [f32; 3]) -> (f32, f32) {
        (self.cx + self.r * v[0], self.cy - self.r * v[1])
    }
    /// Screen axes of the tangent plane at unit vector `v` (east, north), in px per unit.
    fn tangent(&self, g: [f32; 3]) -> ([f32; 2], [f32; 2]) {
        let e = {
            let l = (g[0] * g[0] + g[2] * g[2]).sqrt();
            if l < 1e-5 {
                [1.0, 0.0, 0.0]
            } else {
                [g[2] / l, 0.0, -g[0] / l]
            }
        };
        let n = [g[1] * e[2] - g[2] * e[1], g[2] * e[0] - g[0] * e[2], g[0] * e[1] - g[1] * e[0]];
        let (e, n) = (self.rot(e), self.rot(n));
        ([e[0] * self.r, -e[1] * self.r], [-n[0] * self.r, n[1] * self.r])
    }
}

/// Append an ellipse (a unit circle under the 2×2 map `[a, b]` at `c`).
fn ellipse(p: &mut BezPath, c: (f32, f32), a: [f32; 2], b: [f32; 2]) {
    const K: f32 = 0.552_284_7;
    let m = |u: f32, v: f32| Point::new((c.0 + a[0] * u + b[0] * v) as f64, (c.1 + a[1] * u + b[1] * v) as f64);
    p.move_to(m(1.0, 0.0));
    p.curve_to(m(1.0, K), m(K, 1.0), m(0.0, 1.0));
    p.curve_to(m(-K, 1.0), m(-1.0, K), m(-1.0, 0.0));
    p.curve_to(m(-1.0, -K), m(-K, -1.0), m(0.0, -1.0));
    p.curve_to(m(K, -1.0), m(1.0, -K), m(1.0, 0.0));
    p.close_path();
}

fn fill_path(path: Path, paint: Paint) -> Op {
    Op::Fill { path, paint, rule: FillRule::NonZero, transform: IDENTITY }
}

impl Globe {
    fn arc_start(&self, i: usize) -> f32 {
        0.5 + i as f32 * self.stagger
    }
}

impl Component for Globe {
    fn duration(&self) -> Option<f32> {
        if self.arcs.is_empty() {
            None
        } else {
            Some(self.delay + self.arc_start(self.arcs.len() - 1) + self.draw)
        }
    }

    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let local = t - self.delay;
        if local < 0.0 || ctx.width < 2.0 || ctx.height < 2.0 || self.scale <= 0.0 {
            return Vec::new();
        }
        let (w, h) = (ctx.width, ctx.height);
        let intro = ease_out(seg(local, 0.0, 1.2));
        let r = 0.4 * w.min(h) * self.scale * (0.92 + 0.08 * intro);
        let spin = (self.lon0 + self.spin * local - 20.0 * (1.0 - intro)).to_radians();
        let tilt = self.tilt.to_radians();
        let v = View { cs: (-spin).sin_cos(), ct: tilt.sin_cos(), cx: w * 0.5, cy: h * 0.5, r };
        let dark = ctx.theme.dark;
        // Defaults: a cool (Rayleigh) blue atmosphere and dots, arcs in the theme accent warming to white.
        let base = self.color.map(|c| c.0).unwrap_or_else(|| mix(rgba(0.55, 0.75, 1.0, 1.0), ctx.theme.accent, 0.12));
        let acc = ctx.theme.accent;
        let arc_a = self.arc_color.first().map(|c| c.0).unwrap_or_else(|| mix(acc, rgba(1.0, 1.0, 1.0, 1.0), 0.25));
        let arc_b = self.arc_color.get(1).map(|c| c.0).unwrap_or(if self.arc_color.len() == 1 { arc_a } else { fxkit::hot(acc, 0.7) });
        let mark_c = self.marker_color.map(|c| c.0).unwrap_or(arc_b);
        let fade = intro;
        let mut out = Vec::new();
        let (cx, cy) = (v.cx, v.cy);

        // Atmosphere: an outer halo, behind everything.
        if self.glow > 0.0 {
            let g = self.glow.min(2.0);
            let rr = r * 1.32;
            let k = r / rr;
            out.push(fill_path(
                circle_path(cx, cy, rr),
                radial(cx, cy, rr, vec![
                    (0.0, alpha(base, 0.0)),
                    (k * 0.9, alpha(base, 0.10 * g * fade)),
                    (k, alpha(base, 0.42 * g * fade)),
                    (k + (1.0 - k) * 0.25, alpha(base, 0.16 * g * fade)),
                    (k + (1.0 - k) * 0.6, alpha(base, 0.04 * g * fade)),
                    (1.0, alpha(base, 0.0)),
                ]),
            ));
        }

        // Dots on the far side, seen through the body.
        let dot_r = (self.spacing * 0.36).min(0.02);
        let mut back = BezPath::new();
        let mut front: Vec<BezPath> = (0..LEVELS).map(|_| BezPath::new()).collect();
        let light = {
            let l = [-0.45f32, 0.55, 0.7];
            let n = dot3(l, l).sqrt();
            [l[0] / n, l[1] / n, l[2] / n]
        };
        for &p in &self.pts {
            let q = v.rot(p);
            if q[2] < -0.05 {
                if q[2] < -0.25 {
                    let (e, n) = v.tangent(p);
                    let s = dot_r * 0.8;
                    ellipse(&mut back, v.px(q), [e[0] * s, e[1] * s], [n[0] * s, n[1] * s]);
                }
            } else if q[2] > 0.0 {
                let (e, n) = v.tangent(p);
                let lit = 0.35 + 0.65 * dot3(q, light).max(0.0);
                let limb = smooth(q[2] / 0.35);
                let lvl = ((lit * (0.25 + 0.75 * limb)) * LEVELS as f32).clamp(0.0, LEVELS as f32 - 1.0) as usize;
                ellipse(&mut front[lvl], v.px(q), [e[0] * dot_r, e[1] * dot_r], [n[0] * dot_r, n[1] * dot_r]);
            }
        }
        if !back.is_empty() {
            out.push(fill_path(back, solid(alpha(base, 0.16 * fade))));
        }

        // Body: a dark sphere with a soft key light and a fresnel rim.
        let body = if dark { mix(rgba(0.012, 0.018, 0.045, 1.0), base, 0.05) } else { mix(rgba(0.93, 0.95, 0.99, 1.0), base, 0.12) };
        out.push(fill_path(
            circle_path(cx, cy, r),
            radial(cx - r * 0.35, cy - r * 0.4, r * 1.55, vec![
                (0.0, alpha(shade(body, if dark { 0.07 } else { 0.04 }), 0.94 * fade)),
                (0.6, alpha(body, 0.9 * fade)),
                (1.0, alpha(shade(body, if dark { -0.02 } else { -0.08 }), 0.92 * fade)),
            ]),
        ));
        if self.glow > 0.0 {
            let g = self.glow.min(2.0);
            out.push(fill_path(
                circle_path(cx, cy, r),
                radial(cx, cy, r, vec![(0.0, alpha(base, 0.0)), (0.72, alpha(base, 0.0)), (0.93, alpha(base, 0.1 * g * fade)), (1.0, alpha(base, 0.38 * g * fade))]),
            ));
        }

        // Graticule.
        if self.grid {
            let mut gp = BezPath::new();
            let mut line = |pts: &mut dyn Iterator<Item = [f32; 3]>| {
                let mut pen = false;
                for p in pts {
                    let q = v.rot(p);
                    if q[2] < 0.0 {
                        pen = false;
                        continue;
                    }
                    let (x, y) = v.px(q);
                    if pen {
                        gp.line_to((x as f64, y as f64));
                    } else {
                        gp.move_to((x as f64, y as f64));
                        pen = true;
                    }
                }
            };
            for lat in (-75..=75).step_by(15) {
                line(&mut (0..=120).map(|k| unit(lat as f32, -180.0 + 3.0 * k as f32)));
            }
            for lon in (-180..180).step_by(15) {
                line(&mut (0..=60).map(|k| unit(-90.0 + 3.0 * k as f32, lon as f32)));
            }
            out.push(Op::Stroke { path: gp, paint: solid(alpha(base, 0.13 * fade)), stroke: line_style(1.0), transform: IDENTITY });
        }

        // Front dots, dim to bright.
        for (k, p) in front.into_iter().enumerate() {
            if p.is_empty() {
                continue;
            }
            let f = (k as f32 + 0.5) / LEVELS as f32;
            let c = mix(mix(body, base, 0.35), shade(base, 0.12), f);
            out.push(fill_path(p, solid(alpha(c, (0.35 + 0.65 * f) * fade))));
        }

        // Arcs: glow, core, travelling pulse and landing dots.
        let mut glow_ops = Vec::new();
        for (i, samples) in self.arc_pts.iter().enumerate() {
            let p0 = local - self.arc_start(i);
            if p0 <= 0.0 {
                continue;
            }
            let head = ease_in_out(seg(p0, 0.0, self.draw.max(0.01)));
            let a = self.arcs[i];
            let (sa, sb) = (v.px(v.rot(unit(a[0], a[1]))), v.px(v.rot(unit(a[2], a[3]))));
            let paint = |al: f32| Paint::Linear { p0: [sa.0, sa.1], p1: [sb.0, sb.1], stops: vec![(0.0, alpha(arc_a, al)), (1.0, alpha(arc_b, al))] };
            let wid = (r * 0.0075).max(1.4);
            let (path, _) = arc_path(&v, samples, 0.0, head);
            if !path.is_empty() {
                // Soft glow: two wide faint strokes falling off from the core.
                glow_ops.push(Op::Stroke { path: path.clone(), paint: paint(0.07 * fade), stroke: line_style(wid * 8.0), transform: IDENTITY });
                glow_ops.push(Op::Stroke { path: path.clone(), paint: paint(0.16 * fade), stroke: line_style(wid * 3.6), transform: IDENTITY });
                out.push(Op::Stroke { path, paint: paint(0.9 * fade), stroke: line_style(wid), transform: IDENTITY });
            }
            // A comet-like pulse runs along each drawn arc every 2.6 s.
            if head >= 1.0 {
                let ph = ((p0 - self.draw) / 2.6).fract();
                let tip = ease_in_out(ph / 0.8);
                if ph < 0.8 {
                    // Comet: a tail that tapers and fades behind a glowing head.
                    let len = 0.24;
                    let fade_in = ease_out(clamp01(ph / 0.08)) * (1.0 - ease_in(clamp01((ph - 0.7) / 0.1)));
                    let pc = mix(arc_b, rgba(1.0, 1.0, 1.0, 1.0), 0.55);
                    const N: usize = 8;
                    for s in 0..N {
                        let (f0, f1) = (s as f32 / N as f32, (s + 1) as f32 / N as f32);
                        let (pp, _) = arc_path(&v, samples, (tip - len * (1.0 - f0)).max(0.0), (tip - len * (1.0 - f1)).max(0.0));
                        if !pp.is_empty() {
                            let a = f1 * f1;
                            out.push(Op::Stroke { path: pp, paint: solid(alpha(pc, a * fade * fade_in)), stroke: line_style(wid * (0.8 + 1.6 * a)), transform: IDENTITY });
                        }
                    }
                    if let (_, Some((x, y))) = arc_path(&v, samples, tip, tip) {
                        glow_ops.push(fxkit::light(x, y, wid * 9.0, fxkit::gauss(pc, 0.75 * fade * fade_in)));
                        out.push(circle(x, y, wid * 1.5, solid(alpha(rgba(1.0, 1.0, 1.0, 1.0), fade * fade_in))));
                    }
                }
            }
            // Head dot while drawing; landing dots at both ends.
            let (_, head_pt) = arc_path(&v, samples, head, head);
            if let (Some((x, y)), true) = (head_pt, head < 1.0) {
                out.push(circle(x, y, wid * 2.4, solid(alpha(rgba(1.0, 1.0, 1.0, 1.0), 0.95 * fade))));
                glow_ops.push(circle(x, y, wid * 7.0, radial(x, y, wid * 7.0, vec![(0.0, alpha(arc_b, 0.6 * fade)), (1.0, alpha(arc_b, 0.0))])));
            }
            for (k, (lat, lon), c, on) in [(0, (a[0], a[1]), arc_a, 1.0), (1, (a[2], a[3]), arc_b, ease_out(seg(head, 0.92, 0.08)))] {
                let _ = k;
                let g = unit(lat, lon);
                let q = v.rot(g);
                if q[2] > 0.0 && on > 0.0 {
                    let (e, n) = v.tangent(g);
                    let s = wid * 1.8 / r * on;
                    let mut p = BezPath::new();
                    ellipse(&mut p, v.px(q), [e[0] * s, e[1] * s], [n[0] * s, n[1] * s]);
                    out.push(fill_path(p, solid(alpha(c, fade * smooth(q[2] / 0.2)))));
                }
            }
        }

        // Markers: a dot with two expanding rings.
        for (i, m) in self.markers.iter().enumerate() {
            let g = unit(m[0], m[1]);
            let q = v.rot(g);
            if q[2] <= 0.0 {
                continue;
            }
            let pop = spring(local - 0.6 - i as f32 * 0.12, 0.8).max(0.0);
            if pop <= 0.0 {
                continue;
            }
            let vis = smooth(q[2] / 0.25) * fade;
            let (e, n) = v.tangent(g);
            let base_r = (r * 0.012).max(2.5) / r;
            let c = v.px(q);
            let mut dot = BezPath::new();
            let s = base_r * pop;
            ellipse(&mut dot, c, [e[0] * s, e[1] * s], [n[0] * s, n[1] * s]);
            glow_ops.push({
                let gr = base_r * r * 4.0;
                fill_path(circle_path(c.0, c.1, gr), radial(c.0, c.1, gr, vec![(0.0, alpha(mark_c, 0.5 * vis)), (1.0, alpha(mark_c, 0.0))]))
            });
            for k in 0..2 {
                let ph = ((local + k as f32 * 1.0 + i as f32 * 0.37) / 2.0).fract();
                let s = base_r * (1.2 + 3.8 * ease_out(ph));
                let mut ring = BezPath::new();
                ellipse(&mut ring, c, [e[0] * s, e[1] * s], [n[0] * s, n[1] * s]);
                out.push(Op::Stroke { path: ring, paint: solid(alpha(mark_c, 0.85 * (1.0 - ph) * vis * pop.min(1.0))), stroke: line_style((r * 0.0035).max(1.0)), transform: IDENTITY });
            }
            out.push(fill_path(dot, solid(alpha(mix(mark_c, rgba(1.0, 1.0, 1.0, 1.0), 0.35), vis))));
        }

        if !glow_ops.is_empty() {
            // Additive glows under the crisp strokes would be hidden; put them on top, screened.
            out.extend(glow_ops);
        }
        out
    }
}

fn line_style(width: f32) -> crate::dl::Stroke {
    line(width, [0.0, 1.0])
}

fn smooth(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * (3.0 - 2.0 * x)
}

/// The visible part of an arc between fractions `a..b` of its length (hidden
/// where the globe covers it), and the screen point at `b` when visible.
fn arc_path(v: &View, s: &[[f32; 3]], a: f32, b: f32) -> (BezPath, Option<(f32, f32)>) {
    let n = s.len() - 1;
    let at = |f: f32| -> [f32; 3] {
        let x = f.clamp(0.0, 1.0) * n as f32;
        let i = (x.floor() as usize).min(n - 1);
        let t = x - i as f32;
        let (p, q) = (s[i], s[i + 1]);
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t]
    };
    let visible = |q: [f32; 3]| q[2] > 0.0 || q[0] * q[0] + q[1] * q[1] > 1.0;
    let mut path = BezPath::new();
    let mut pen = false;
    let mut pts = vec![a];
    let (ia, ib) = ((a * n as f32).ceil() as usize, (b * n as f32).floor() as usize);
    pts.extend((ia..=ib.min(n)).map(|i| i as f32 / n as f32).filter(|&f| f > a && f < b));
    pts.push(b);
    let mut last = None;
    for f in pts {
        let q = v.rot(at(f));
        if !visible(q) {
            pen = false;
            last = None;
            continue;
        }
        let (x, y) = v.px(q);
        if pen {
            path.line_to((x as f64, y as f64));
        } else {
            path.move_to((x as f64, y as f64));
            pen = true;
        }
        last = Some((x, y));
    }
    if b <= a {
        path = BezPath::new();
    }
    (path, last)
}

// ---------------------------------------------------------------- land

/// Coarse continent outlines `(lon, lat)`, good enough for a dot globe.
const LAND: &[&[(f32, f32)]] = &[
    // North America
    &[
        (-166.0, 68.0), (-156.0, 71.3), (-140.0, 69.8), (-128.0, 70.0), (-115.0, 68.5), (-100.0, 68.0), (-94.0, 71.0), (-85.0, 69.5), (-82.0, 66.0), (-87.0, 64.0),
        (-94.0, 61.0), (-93.0, 58.5), (-88.0, 56.5), (-82.0, 52.5), (-79.0, 55.0), (-77.0, 60.0), (-78.0, 62.5), (-70.0, 61.0), (-64.5, 60.0), (-61.0, 56.0),
        (-56.0, 52.0), (-59.5, 47.5), (-64.0, 46.0), (-66.0, 44.5), (-70.0, 43.0), (-70.5, 41.5), (-74.0, 40.5), (-76.0, 37.0), (-76.0, 35.0), (-81.0, 31.5),
        (-80.0, 27.0), (-80.5, 25.2), (-82.5, 27.0), (-84.0, 30.0), (-89.0, 30.2), (-94.0, 29.5), (-97.5, 27.0), (-97.5, 22.0), (-95.5, 19.0), (-91.0, 19.0),
        (-90.5, 21.0), (-87.0, 21.5), (-88.0, 16.0), (-84.0, 15.5), (-83.5, 11.0), (-79.5, 9.3), (-77.3, 8.5), (-78.5, 7.5), (-81.0, 7.5), (-85.5, 10.5),
        (-87.5, 13.0), (-91.5, 14.0), (-96.0, 15.7), (-101.0, 17.5), (-105.5, 20.0), (-105.5, 23.0), (-109.0, 26.5), (-112.5, 29.5), (-114.5, 31.5), (-112.5, 28.0),
        (-110.0, 23.0), (-112.0, 25.0), (-114.5, 28.0), (-117.0, 32.5), (-120.5, 34.5), (-122.5, 37.5), (-124.2, 40.5), (-124.0, 46.0), (-124.7, 48.5), (-127.0, 51.0),
        (-130.5, 54.5), (-135.0, 58.0), (-140.0, 59.7), (-146.0, 60.5), (-151.0, 60.5), (-153.0, 58.0), (-158.0, 56.5), (-163.5, 55.0), (-158.0, 58.8), (-162.0, 60.0),
        (-165.0, 61.5), (-164.5, 63.0), (-161.0, 64.5), (-166.0, 65.5),
    ],
    // Greenland
    &[(-73.0, 78.0), (-60.0, 82.0), (-35.0, 83.5), (-18.0, 81.5), (-20.0, 76.0), (-19.0, 72.0), (-23.5, 70.0), (-32.0, 68.0), (-40.0, 65.0), (-43.5, 60.0), (-48.0, 61.0), (-51.5, 64.5), (-54.0, 69.0), (-57.0, 74.0), (-66.0, 76.5)],
    // Arctic archipelago and Baffin Island
    &[(-125.0, 72.0), (-117.0, 76.5), (-100.0, 79.0), (-85.0, 82.0), (-65.0, 83.0), (-75.0, 79.0), (-80.0, 76.0), (-92.0, 74.5), (-105.0, 72.5), (-118.0, 70.0)],
    &[(-90.0, 73.0), (-80.0, 73.7), (-72.0, 71.0), (-62.0, 67.0), (-65.0, 63.0), (-72.0, 63.0), (-78.0, 65.0), (-86.0, 70.0)],
    // Cuba, Hispaniola
    &[(-85.0, 22.0), (-80.0, 23.2), (-74.2, 20.2), (-77.5, 19.9), (-82.0, 21.6)],
    &[(-74.5, 19.8), (-68.5, 19.0), (-70.0, 18.2), (-74.3, 18.1)],
    // South America
    &[
        (-77.3, 8.5), (-72.0, 12.0), (-64.0, 10.7), (-60.0, 8.5), (-52.0, 5.0), (-50.0, 0.5), (-44.0, -2.5), (-35.0, -5.5), (-35.0, -9.0), (-39.0, -13.0),
        (-39.5, -18.0), (-41.5, -22.5), (-48.0, -26.0), (-49.0, -29.0), (-53.0, -34.0), (-58.0, -35.0), (-57.0, -38.5), (-62.0, -39.0), (-65.0, -42.0), (-64.0, -46.0),
        (-68.0, -50.5), (-69.0, -53.0), (-67.0, -55.0), (-72.0, -54.0), (-75.0, -50.0), (-74.0, -43.0), (-73.5, -37.0), (-71.5, -30.0), (-70.3, -20.0), (-71.5, -17.0),
        (-76.0, -14.0), (-79.5, -7.5), (-81.2, -4.5), (-80.0, -0.5), (-78.5, 2.5),
    ],
    // Europe and Asia
    &[
        (-9.5, 43.0), (-9.0, 37.0), (-6.0, 36.0), (-2.0, 36.8), (0.0, 39.0), (3.2, 42.0), (4.5, 43.4), (8.0, 44.0), (10.5, 43.8), (12.5, 41.5),
        (16.0, 38.0), (18.5, 40.0), (16.0, 41.8), (13.0, 45.5), (15.5, 44.0), (19.5, 41.8), (20.5, 39.5), (22.5, 36.5), (24.0, 38.0), (23.0, 40.5),
        (26.0, 40.6), (26.5, 38.5), (27.5, 37.0), (30.5, 36.5), (36.0, 36.6), (35.8, 34.0), (34.5, 31.5), (34.9, 29.5), (37.0, 25.5), (39.0, 21.5),
        (42.5, 15.0), (43.5, 12.7), (45.0, 13.0), (49.0, 14.5), (52.2, 16.0), (55.5, 17.8), (57.5, 19.0), (59.8, 22.5), (56.5, 24.5), (56.0, 26.2),
        (54.0, 24.2), (51.5, 24.5), (50.0, 26.5), (48.5, 29.5), (50.5, 29.5), (54.0, 27.0), (57.0, 26.8), (61.5, 25.2), (66.5, 25.4), (68.5, 23.5),
        (70.0, 21.0), (72.8, 19.0), (73.5, 15.5), (74.8, 12.5), (76.5, 8.5), (77.5, 8.0), (80.0, 10.0), (80.2, 13.5), (81.0, 16.0), (84.5, 19.0),
        (87.0, 21.5), (89.5, 22.0), (91.8, 22.5), (94.0, 18.5), (94.5, 16.0), (97.5, 16.5), (98.5, 13.0), (98.5, 8.5), (100.3, 5.0), (103.5, 1.4),
        (104.2, 1.5), (103.5, 4.5), (102.2, 6.2), (100.4, 7.5), (100.0, 12.5), (102.0, 12.2), (104.8, 8.8), (106.8, 10.4), (109.2, 11.8), (108.8, 15.5),
        (106.5, 18.0), (106.0, 19.8), (108.5, 21.6), (111.0, 21.5), (114.0, 22.3), (117.0, 23.5), (119.5, 26.0), (121.5, 28.5), (122.0, 30.5), (121.0, 32.5),
        (120.3, 34.3), (119.2, 35.0), (122.5, 37.0), (121.0, 37.8), (118.8, 38.0), (117.8, 38.8), (119.0, 39.5), (121.5, 40.8), (122.5, 40.3), (121.3, 39.0),
        (124.5, 39.8), (126.0, 37.7), (126.5, 34.5), (129.0, 35.0), (129.5, 36.5), (128.5, 38.5), (127.5, 39.8), (129.8, 41.0), (130.8, 42.5), (133.0, 42.8),
        (135.5, 43.8), (138.5, 47.0), (140.5, 48.5), (140.5, 52.0), (141.5, 53.3), (137.5, 54.0), (135.5, 54.8), (139.0, 57.5), (142.0, 59.0), (146.5, 59.3),
        (151.0, 59.0), (155.0, 59.5), (156.5, 61.5), (160.0, 61.5), (163.0, 61.5), (160.5, 59.0), (156.5, 57.5), (155.5, 55.0), (156.5, 51.0), (158.5, 52.5),
        (162.0, 56.0), (163.5, 58.5), (166.0, 60.0), (172.0, 61.0), (179.0, 62.5), (180.0, 65.0), (180.0, 68.8), (176.0, 69.8), (170.0, 70.0), (161.0, 69.5),
        (152.0, 70.8), (143.0, 72.5), (135.0, 71.5), (128.0, 72.5), (124.0, 73.5), (113.0, 73.8), (110.0, 76.5), (104.0, 77.7), (97.0, 76.0), (88.0, 75.5),
        (81.0, 72.5), (80.5, 73.5), (74.0, 72.8), (73.0, 69.0), (69.0, 68.5), (67.0, 69.5), (60.0, 68.8), (54.0, 68.2), (46.0, 68.0), (44.0, 66.5),
        (41.0, 66.2), (40.5, 64.8), (37.5, 64.0), (35.0, 64.5), (34.5, 66.0), (41.0, 67.5), (37.0, 69.0), (33.0, 69.5), (29.0, 70.0), (29.0, 67.0),
        (30.0, 62.0), (28.5, 60.5), (24.0, 59.6), (22.5, 58.5), (21.5, 57.0), (21.0, 55.5), (19.5, 54.5), (16.0, 54.3), (13.5, 54.2), (11.0, 54.0),
        (10.0, 55.0), (8.5, 57.2), (8.0, 54.5), (6.0, 53.5), (4.5, 52.5), (3.5, 51.3), (1.8, 50.9), (0.0, 49.7), (-1.5, 49.6), (-1.8, 48.6),
        (-4.8, 48.5), (-2.5, 47.3), (-1.2, 46.0), (-1.5, 43.5),
    ],
    // Scandinavia
    &[
        (5.0, 62.0), (5.5, 58.8), (7.5, 58.0), (10.5, 59.2), (11.5, 58.0), (12.8, 55.7), (14.3, 55.5), (16.0, 56.5), (16.7, 58.0), (18.5, 59.5),
        (17.3, 61.0), (17.5, 62.5), (20.5, 63.8), (22.3, 65.7), (25.3, 65.2), (21.5, 63.2), (21.0, 61.0), (22.8, 60.0), (27.5, 60.5), (30.0, 61.5),
        (30.0, 64.5), (29.0, 67.0), (28.5, 70.5), (25.0, 71.2), (19.0, 70.2), (15.0, 68.5), (12.5, 65.8), (10.0, 64.0), (8.0, 63.0),
    ],
    // Great Britain, Ireland, Iceland
    &[(-5.5, 50.0), (1.5, 51.2), (1.7, 52.8), (0.0, 54.0), (-1.8, 55.8), (-2.0, 57.6), (-3.5, 58.6), (-5.0, 58.6), (-6.2, 56.5), (-4.8, 55.0), (-3.2, 54.2), (-4.5, 53.2), (-4.2, 51.6), (-5.3, 51.7)],
    &[(-6.0, 52.0), (-6.0, 54.5), (-8.0, 55.3), (-10.0, 54.2), (-10.2, 51.7)],
    &[(-24.0, 65.5), (-22.0, 66.4), (-16.0, 66.5), (-13.5, 65.0), (-18.0, 63.4), (-22.5, 63.8)],
    // Novaya Zemlya
    &[(52.0, 71.0), (56.0, 75.0), (68.0, 77.0), (60.0, 74.0), (56.0, 71.0)],
    // Africa
    &[
        (-17.0, 21.0), (-16.0, 24.0), (-13.0, 28.0), (-9.8, 31.5), (-6.5, 35.0), (-2.0, 35.2), (3.0, 36.8), (10.0, 37.2), (11.0, 35.0), (10.5, 33.5),
        (15.0, 32.3), (19.5, 30.5), (20.5, 32.5), (25.0, 32.0), (29.0, 31.0), (32.0, 31.3), (34.0, 31.0), (34.0, 28.0), (33.5, 27.5), (35.5, 24.0),
        (37.2, 20.5), (38.5, 18.0), (39.5, 15.5), (41.5, 13.5), (43.3, 12.0), (44.5, 10.5), (51.2, 11.8), (51.0, 10.0), (48.0, 4.5), (44.5, 1.0),
        (41.0, -2.0), (39.0, -6.5), (39.5, -10.0), (40.5, -15.0), (36.0, -19.0), (35.3, -24.0), (32.5, -26.5), (32.0, -29.0), (28.5, -32.7), (25.5, -34.0),
        (20.0, -34.8), (18.3, -33.5), (17.0, -29.5), (15.0, -26.5), (11.8, -18.0), (12.3, -14.0), (13.3, -9.5), (12.0, -5.0), (9.0, -1.0), (9.5, 3.8),
        (8.5, 4.5), (5.5, 4.2), (3.0, 6.3), (-2.0, 4.8), (-5.0, 5.0), (-7.5, 4.4), (-11.5, 6.8), (-13.2, 9.0), (-15.0, 11.0), (-16.8, 13.5),
        (-17.3, 15.0), (-16.3, 19.0),
    ],
    // Madagascar
    &[(44.0, -25.0), (47.0, -25.2), (50.5, -15.5), (49.3, -12.0), (44.0, -16.5), (43.5, -21.5)],
    // Sri Lanka, Taiwan, Hainan
    &[(79.8, 9.8), (81.8, 7.5), (81.0, 6.0), (79.9, 6.5)],
    &[(120.1, 23.0), (121.0, 25.2), (122.0, 25.0), (120.8, 21.9)],
    // Japan
    &[(129.8, 31.2), (131.2, 34.0), (135.0, 33.5), (139.8, 34.9), (140.9, 37.5), (141.9, 40.0), (141.3, 41.4), (140.0, 41.4), (141.0, 45.4), (145.5, 44.2), (143.5, 42.0), (141.0, 41.6), (140.0, 39.5), (139.5, 37.8), (137.0, 37.0), (133.0, 35.5), (130.9, 34.4), (129.7, 33.3)],
    // Sakhalin
    &[(142.0, 46.0), (143.5, 49.0), (143.0, 54.0), (142.5, 54.3), (141.8, 50.0)],
    // Philippines
    &[(120.5, 18.5), (122.3, 18.5), (124.0, 13.5), (126.3, 9.0), (126.0, 6.5), (125.2, 5.8), (122.0, 7.0), (122.8, 10.5), (120.5, 14.0), (120.0, 16.3)],
    // Indonesia: Sumatra, Java, Borneo, Sulawesi, New Guinea
    &[(95.3, 5.6), (97.5, 5.2), (100.3, 2.5), (104.0, -1.5), (106.0, -3.2), (105.8, -5.8), (104.0, -5.0), (101.0, -2.5), (98.5, 1.5)],
    &[(105.2, -6.8), (108.0, -6.2), (111.0, -6.5), (114.5, -7.7), (114.4, -8.7), (108.5, -7.8)],
    &[(109.0, 1.5), (110.0, 2.0), (113.0, 3.2), (116.0, 6.8), (117.5, 6.5), (119.0, 5.0), (117.8, 1.0), (116.5, -1.0), (116.0, -3.5), (114.0, -3.6), (111.0, -3.0), (110.0, -1.5)],
    &[(119.5, -5.5), (120.5, 0.0), (125.0, 1.5), (123.0, -1.0), (121.5, -1.5), (122.0, -4.5), (120.5, -2.5)],
    &[(131.0, -1.5), (134.0, -0.8), (138.0, -1.8), (141.0, -2.6), (145.0, -4.5), (148.0, -6.5), (150.5, -10.5), (147.0, -10.0), (143.5, -9.0), (141.0, -9.2), (138.5, -8.2), (137.5, -5.0), (133.5, -4.0)],
    // Australia
    &[
        (114.0, -22.0), (113.5, -26.0), (115.0, -34.0), (118.0, -35.0), (123.5, -33.9), (126.0, -32.3), (131.0, -31.5), (134.0, -33.0), (135.8, -35.0), (138.0, -35.5),
        (138.0, -33.5), (140.0, -38.0), (143.5, -38.8), (146.5, -39.0), (150.0, -37.5), (151.5, -33.5), (153.5, -28.5), (153.0, -25.0), (150.5, -22.5), (146.5, -19.0),
        (145.5, -15.0), (143.5, -12.0), (142.5, -10.8), (141.6, -13.0), (141.5, -17.0), (139.5, -17.5), (136.0, -15.0), (136.8, -12.2), (132.5, -11.2), (130.0, -13.0),
        (127.0, -14.0), (122.5, -17.0), (119.0, -20.0),
    ],
    // Tasmania, New Zealand
    &[(144.7, -40.7), (148.3, -40.9), (148.0, -43.2), (146.0, -43.6)],
    &[(172.7, -34.4), (174.8, -36.8), (178.5, -37.7), (177.0, -39.3), (175.0, -41.5), (173.0, -41.0), (174.3, -38.5)],
    &[(172.5, -40.5), (174.3, -41.7), (171.2, -44.3), (169.0, -46.6), (166.5, -46.0), (168.5, -44.0), (171.0, -42.0)],
];

/// Inland seas cut out of the land polygons above.
const WATER: &[&[(f32, f32)]] = &[
    // Black Sea, Caspian Sea
    &[(27.5, 42.0), (28.5, 44.5), (30.5, 46.5), (33.5, 46.0), (35.0, 45.2), (38.5, 47.0), (39.5, 44.0), (41.5, 41.5), (36.0, 41.6), (31.0, 41.2)],
    &[(47.0, 45.5), (50.0, 47.0), (53.0, 46.8), (53.0, 42.0), (54.0, 40.0), (53.5, 37.2), (49.0, 37.5), (49.3, 40.5), (47.5, 43.0)],
    // Hudson Bay
    &[(-94.5, 59.5), (-92.5, 57.0), (-87.0, 55.3), (-82.5, 52.8), (-79.2, 54.5), (-77.5, 60.0), (-79.5, 62.5), (-86.0, 64.0), (-93.0, 62.0)],
    // Baltic and Gulf of Bothnia
    &[(18.8, 59.5), (21.0, 60.5), (21.2, 62.5), (24.5, 65.0), (21.0, 64.5), (19.5, 63.0), (17.8, 61.0)],
];

fn in_poly(lon: f32, lat: f32, poly: &[(f32, f32)]) -> bool {
    let mut c = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > lat) != (b.1 > lat) && lon < (b.0 - a.0) * (lat - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
        j = i;
    }
    c
}

/// Rough land test: the outlines above, plus Antarctica below about 70° S.
fn is_land(lat: f32, lon: f32) -> bool {
    let ant = -71.0 + 3.5 * (lon.to_radians() * 2.0).sin() + if (-75.0..-55.0).contains(&lon) { 6.0 } else { 0.0 };
    if lat < ant {
        return true;
    }
    LAND.iter().any(|p| in_poly(lon, lat, p)) && !WATER.iter().any(|p| in_poly(lon, lat, p))
}

#[cfg(test)]
mod tests {
    use super::super::preview::{snapshot, validate};
    use super::super::{build, Theme};
    use super::*;
    use kurbo::Shape;

    fn backdrop(w: f32, h: f32) -> Op {
        Op::Fill {
            path: kurbo::Rect::new(0.0, 0.0, w as f64, h as f64).into_path(0.1),
            paint: radial(w * 0.5, h * 0.5, w * 0.7, vec![(0.0, rgba(0.05, 0.06, 0.1, 1.0)), (1.0, rgba(0.015, 0.015, 0.03, 1.0))]),
            rule: FillRule::NonZero,
            transform: IDENTITY,
        }
    }

    fn cases() -> Vec<(&'static str, serde_json::Value)> {
        use serde_json::json;
        let arcs = json!([[37.8, -122.4, 51.5, -0.1], [40.7, -74.0, -23.5, -46.6], [51.5, -0.1, 1.35, 103.8], [35.7, 139.7, 37.8, -122.4], [48.9, 2.35, 28.6, 77.2]]);
        vec![
            ("default", json!({})),
            ("arcs", json!({"arcs": arcs, "markers": [[37.8, -122.4], [51.5, -0.1], [40.7, -74.0]]})),
            ("grid", json!({"grid": true, "dots": "none", "color": "#3ddc97", "arcs": [[52.5, 13.4, 40.7, -74.0]], "tilt": 30})),
            ("all", json!({"dots": "all", "density": 0.6, "color": "#ffb547", "markers": [[0, 0]]})),
        ]
    }

    #[test]
    fn renders() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        let (w, h) = (800.0, 600.0);
        for (name, params) in cases() {
            let p = build("globe", &params).unwrap_or_else(|e| panic!("{name}: {e}"));
            let end = p.component.duration().unwrap_or(3.0);
            for t in [0.0, end * 0.5, end, end + 1.1] {
                let ops = p.eval(t, w, h, 30.0, &theme, assets);
                validate(&ops).unwrap_or_else(|e| panic!("{name} at {t}: {e}"));
                if t == end * 0.5 || t == end + 1.1 {
                    let mut all = vec![backdrop(w, h)];
                    all.extend(ops);
                    let tag = if t == end * 0.5 { "-mid" } else { "" };
                    let px = snapshot(&format!("globe-{name}{tag}"), w as u32, h as u32, theme.background, all, assets);
                    let bright = px.chunks_exact(4).filter(|c| c[0].max(c[1]).max(c[2]) > 80).count();
                    assert!(bright > 1500 || name == "grid", "{name}: only {bright} bright pixels");
                }
            }
        }
    }

    #[test]
    fn land_mask() {
        // Cities on land, oceans not.
        for (lat, lon) in [(48.9, 2.35), (40.7, -74.5), (-23.5, -46.8), (35.7, 139.5), (-33.9, 151.0), (30.0, 31.0), (55.7, 37.6), (39.9, 116.4), (-1.3, 36.8), (72.0, -40.0)] {
            assert!(is_land(lat, lon), "{lat},{lon} should be land");
        }
        for (lat, lon) in [(0.0, -30.0), (30.0, -150.0), (-30.0, 80.0), (43.0, 35.0), (60.0, -86.0), (0.0, 170.0), (-50.0, -100.0)] {
            assert!(!is_land(lat, lon), "{lat},{lon} should be sea");
        }
        let g = build("globe", &serde_json::Value::Null).unwrap();
        let _ = g;
    }

    #[test]
    fn errors() {
        assert!(build("globe", &serde_json::json!({"arcs": [[95, 0, 0, 0]]})).is_err());
        let e = build("globe", &serde_json::json!({"dots": "lnad"})).err().unwrap();
        assert!(e.0.contains("did you mean `land`"), "{e}");
        let e = build("globe", &serde_json::json!({"arcColour": "#fff"})).err().unwrap();
        assert!(e.0.contains("arcColor"), "{e}");
        assert!(build("globe", &serde_json::json!({"arcColor": ["#fff"]})).is_ok());
    }

    /// `cargo test --release -p anikit-video globe::tests::timing -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        let theme = Theme::default();
        let assets = super::super::preview::host();
        for (name, params) in cases() {
            let p = build("globe", &params).unwrap();
            let n = 60;
            let mut lists = Vec::new();
            let t0 = std::time::Instant::now();
            for i in 0..n {
                lists.push(p.eval(2.0 + i as f32 / 60.0, 1920.0, 1080.0, 60.0, &theme, assets));
            }
            let e = t0.elapsed().as_secs_f64() * 1e3 / n as f64;
            let mut r = crate::render::Renderer::new(std::sync::Arc::new(assets.render_assets(None)));
            let t1 = std::time::Instant::now();
            for ops in &lists {
                r.render(&crate::dl::DisplayList { width: 1920, height: 1080, background: theme.background, ops: ops.clone() });
            }
            let rr = t1.elapsed().as_secs_f64() * 1e3 / n as f64;
            println!("{name:10} eval {e:.2} ms  render {rr:.2} ms  ops {}", lists[0].len());
        }
    }
}
