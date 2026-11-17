//! Particles: confetti, sparks, snow and dust.
//!
//! Every particle has a depth: far ones are small, dim and slightly soft,
//! near ones are large and thrown out of focus (they are drawn into a
//! blurred layer, so they read as real bokeh). Confetti pieces are 3D
//! cards tumbling about their own axis, lit by a key light (diffuse plus a
//! specular flash, strong for `foil`), and smear along their motion when
//! fast. Sparks are tapered streaks whose colour follows black-body
//! temperature as they cool.
//!
//! Motion is closed-form (ballistic with linear drag, or wrapped drift),
//! so every frame is a pure function of `t` and frames render in any order
//! on any thread. Layouts are seeded and deterministic.

use super::fxkit::{self, gauss, hot, kelvin, light};
use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, Op, Path, Rgba};
use anikit_core::noise::Noise;
use std::f32::consts::TAU;
use serde::Deserialize;

/// Position after `t` s for initial velocity `v0`, drag `k` (1/s) and gravity `g` (px/s²).
#[inline]
fn ballistic(p0: f32, v0: f32, k: f32, g: f32, t: f32) -> f32 {
    if k < 1e-4 {
        return p0 + v0 * t + 0.5 * g * t * t;
    }
    let e = (-k * t).exp();
    p0 + (v0 - g / k) * (1.0 - e) / k + g * t / k
}

#[inline]
fn velocity(v0: f32, k: f32, g: f32, t: f32) -> f32 {
    if k < 1e-4 {
        return v0 + g * t;
    }
    let e = (-k * t).exp();
    (v0 - g / k) * e + g / k
}

/// Depth bands: far (small and dim), mid (sharp), near (out of focus).
const BANDS: usize = 3;

fn band(z: f32, depth: f32) -> usize {
    if depth <= 0.0 {
        1
    } else if z > 1.0 - 0.17 * depth {
        2
    } else if z < 0.22 * depth {
        0
    } else {
        1
    }
}

/// Blur (px at 1080p) of each band.
fn band_blur(depth: f32) -> [f32; BANDS] {
    [0.0, 0.0, 5.5 * depth]
}

fn rot3(axis: [f32; 3], ang: f32, v: [f32; 3]) -> [f32; 3] {
    // Rodrigues' rotation.
    let (s, c) = ang.sin_cos();
    let d = axis[0] * v[0] + axis[1] * v[1] + axis[2] * v[2];
    let x = [axis[1] * v[2] - axis[2] * v[1], axis[2] * v[0] - axis[0] * v[2], axis[0] * v[1] - axis[1] * v[0]];
    [0, 1, 2].map(|i| v[i] * c + x[i] * s + axis[i] * d * (1.0 - c))
}

fn norm3(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Convex hull (gift wrap) of a few points, appended to `p` as a polygon.
fn hull(p: &mut Path, pts: &[(f32, f32)]) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let mut start = 0;
    for i in 1..n {
        if pts[i].0 < pts[start].0 || (pts[i].0 == pts[start].0 && pts[i].1 < pts[start].1) {
            start = i;
        }
    }
    let mut cur = start;
    let mut first = true;
    for _ in 0..=n {
        let q = pts[cur];
        if first {
            p.move_to((q.0 as f64, q.1 as f64));
            first = false;
        } else {
            p.line_to((q.0 as f64, q.1 as f64));
        }
        let mut next = (cur + 1) % n;
        for j in 0..n {
            let (a, b, c) = (pts[cur], pts[next], pts[j]);
            let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            if cross < 0.0 {
                next = j;
            }
        }
        cur = next;
        if cur == start {
            break;
        }
    }
    p.close_path();
}

// ------------------------------------------------------------------ confetti

const CONFETTI_MODES: &[&str] = &["burst", "cannons", "rain"];
const MATERIALS: &[&str] = &["paper", "foil", "mixed"];
const SHAPES: &[&str] = &["mixed", "strips", "squares", "circles"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Confetti {
    pub count: u32,
    /// "burst" from `origin`, "cannons" from both bottom corners, or "rain" from the top.
    pub mode: String,
    /// Burst origin as a fraction of the box (default: centre, slightly low).
    pub origin: [f32; 2],
    /// When it fires (s).
    pub at: f32,
    /// Piece colours (default: the theme palette).
    pub colors: Vec<Color>,
    /// `paper` (matte, soft sheen), `foil` (metallic flashes) or `mixed`.
    pub material: String,
    /// `mixed`, `strips`, `squares` or `circles`.
    pub shape: String,
    /// Launch speed multiplier.
    pub power: f32,
    pub gravity: f32,
    /// Piece size in pixels.
    pub size: f32,
    /// Seconds before pieces fade out.
    pub life: f32,
    /// How fast pieces tumble (1 = natural).
    pub spin: f32,
    /// Depth of field: near pieces blur, far ones shrink (0 = flat, all sharp).
    pub depth: f32,
    /// Motion smear for fast pieces (0 = none, 1 = a half-frame shutter).
    pub blur: f32,
    pub seed: u32,
}
impl Default for Confetti {
    fn default() -> Self {
        Confetti {
            count: 180,
            mode: "burst".into(),
            origin: [0.5, 0.6],
            at: 0.0,
            colors: Vec::new(),
            material: "paper".into(),
            shape: "mixed".into(),
            power: 1.0,
            gravity: 900.0,
            size: 22.0,
            life: 3.5,
            spin: 1.0,
            depth: 1.0,
            blur: 1.0,
            seed: 1,
        }
    }
}
impl Prepare for Confetti {
    fn prepare(&mut self) -> Result<(), Error> {
        fxkit::choice("confetti mode", &self.mode, CONFETTI_MODES)?;
        fxkit::choice("material", &self.material, MATERIALS)?;
        fxkit::choice("shape", &self.shape, SHAPES)?;
        if self.count > 5000 {
            return Err(Error("count must be at most 5000".into()));
        }
        self.depth = self.depth.clamp(0.0, 2.0);
        self.blur = self.blur.clamp(0.0, 4.0);
        Ok(())
    }
}

impl Component for Confetti {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = t - self.at;
        if t < 0.0 {
            return Vec::new();
        }
        let (w, h) = (ctx.width, ctx.height);
        let cols = fxkit::colors_or(&self.colors, || ctx.theme.palette.clone());
        let nc = cols.len();
        let mut rng = Rng::new(self.seed as u64 * 7 + 3);
        let scale = w.max(h) / 1920.0;
        let k = h / 1080.0;
        // One gradient-filled op per piece, grouped by depth band.
        let mut bands: [Vec<Op>; BANDS] = Default::default();
        let light_dir = norm3([-0.45, -0.7, 0.75]);
        let half = norm3([light_dir[0], light_dir[1], light_dir[2] + 1.0]);
        let shutter = 0.5 / ctx.fps.max(1.0) * self.blur;
        for i in 0..self.count {
            let r: [f32; 12] = std::array::from_fn(|_| rng.f());
            let ci = i as usize % nc;
            let z = r[7];
            // Per-piece launch.
            let (x0, y0, vx, vy, delay) = match self.mode.as_str() {
                "rain" => (r[0] * w, -20.0 - r[1] * h * 0.4, (r[2] - 0.5) * 120.0 * scale, 0.0, r[3] * 1.8),
                "cannons" => {
                    let left = i % 2 == 0;
                    let ang = (-48.0 - r[0] * 40.0f32).to_radians();
                    let sp = (1100.0 + r[1] * 2200.0) * self.power * scale;
                    let (s, c) = ang.sin_cos();
                    (if left { 0.0 } else { w }, h, if left { c * sp } else { -c * sp }, s * sp, r[2] * 0.12)
                }
                _ => {
                    let ang = r[0] * TAU;
                    let sp = (300.0 + r[1].sqrt() * 1300.0) * self.power * scale;
                    (self.origin[0] * w, self.origin[1] * h, ang.cos() * sp, ang.sin() * sp - 500.0 * scale * self.power, r[2] * 0.05)
                }
            };
            let lt = t - delay;
            if lt <= 0.0 {
                continue;
            }
            let drag = 1.6 + r[3] * 1.2;
            let g = self.gravity * scale * (0.7 + 0.3 * r[4]);
            // Flutter: sideways sway grows as pieces slow down.
            let sway = |lt: f32| (lt * (3.0 + r[4] * 4.0) + r[5] * 6.0).sin() * 18.0 * scale * clamp01(lt * 0.8);
            let pos = |lt: f32| (ballistic(x0, vx, drag, 0.0, lt) + sway(lt), ballistic(y0, vy, drag, g, lt));
            let (x, y) = pos(lt);
            let fade = 1.0 - seg(lt, self.life * (0.75 + 0.25 * r[5]), 0.6);
            if fade <= 0.0 || y > h + 60.0 || x < -80.0 || x > w + 80.0 {
                continue;
            }
            // Perspective: near pieces are bigger.
            let persp = if self.depth > 0.0 { 0.6 + 1.1 * z * z } else { 1.0 };
            let s = self.size * scale.max(0.4) * (0.65 + 0.7 * r[6]) * persp;
            let shape = match self.shape.as_str() {
                "strips" => 0,
                "squares" => 1,
                "circles" => 2,
                _ => [0, 0, 1, 2][(r[8] * 4.0) as usize % 4],
            };
            let (pw, ph) = match shape {
                0 => (s, s * (0.36 + 0.14 * r[9])),
                1 => (s * 0.72, s * 0.72),
                _ => (s * 0.62, s * 0.62),
            };
            // 3D tumble about a random axis; spin slows a little with drag.
            let axis = norm3([r[9] - 0.5, r[10] - 0.5, r[11] - 0.5 + 0.2]);
            let w0 = (5.0 + 9.0 * r[10]) * self.spin * if i % 2 == 0 { 1.0 } else { -1.0 };
            let ang = r[11] * TAU + w0 * (lt * 0.7 + 0.3 * (1.0 - (-lt * 2.0).exp()) / 2.0);
            let (u, v) = (rot3(axis, ang, [1.0, 0.0, 0.0]), rot3(axis, ang, [0.0, 1.0, 0.0]));
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let front = n[2] >= 0.0;
            let nn = if front { n } else { [-n[0], -n[1], -n[2]] };
            let diffuse = dot3(nn, light_dir).max(0.0);
            let foil = match self.material.as_str() {
                "foil" => true,
                "mixed" => r[8] > 0.6,
                _ => false,
            };
            let lit = if foil { 0.3 + 0.7 * diffuse } else { 0.32 + 0.85 * diffuse } * if front { 1.0 } else { 0.8 };
            let spec = dot3(nn, half).max(0.0).powf(if foil { 10.0 } else { 28.0 }) * if foil { 1.2 } else { 0.6 };
            let b = band(z, self.depth);
            let fadek = fade.sqrt();
            let (hu, hv) = (pw * 0.5 * fadek, ph * 0.5 * fadek);
            // Projected corners (orthographic: drop z).
            let corner = |cx: f32, cy: f32, a: f32, bb: f32| (cx + u[0] * a + v[0] * bb, cy + u[1] * a + v[1] * bb);
            let (px, py) = pos(lt - shutter);
            let moved = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            let mut path = Path::new();
            if shape == 2 {
                let q: Vec<(f32, f32)> = (0..14).map(|j| {
                    let a = j as f32 * TAU / 14.0;
                    corner(x, y, a.cos() * hu, a.sin() * hu)
                }).collect();
                hull(&mut path, &q);
            } else {
                let q = [corner(x, y, -hu, -hv), corner(x, y, hu, -hv), corner(x, y, hu, hv), corner(x, y, -hu, hv)];
                path.move_to((q[0].0 as f64, q[0].1 as f64));
                for c in &q[1..] {
                    path.line_to((c.0 as f64, c.1 as f64));
                }
                path.close_path();
            }
            // Paper curls a little, so light falls off across the piece; a
            // specular band slides across it as it turns (strong on foil).
            let base = cols[ci];
            let face = if front { base } else { mix(base, shade(base, -0.2), 0.5) };
            let col = |l: f32| if l < 1.0 { shade(face, -(1.0 - l) * 0.8) } else { shade(face, ((l - 1.0) * 1.2).min(0.6)) };
            let sweep = 0.5 + 0.45 * (ang * 1.7 + r[5] * 5.0).sin();
            let curl = 0.3 + 0.2 * r[6];
            let hl = mix(col(lit + 0.1), Rgba([1.0; 4]), spec.min(1.0) * if foil { 0.85 } else { 0.5 });
            let stops = vec![
                (0.0, col(lit - curl)),
                ((sweep - 0.28).max(0.01), col(lit)),
                (sweep, hl),
                ((sweep + 0.28).min(0.99), col(lit - curl * 0.3)),
                (1.0, col(lit - curl * 0.8)),
            ];
            let (p0, p1) = (corner(x, y, -hu, -hv * 0.6), corner(x, y, hu, hv * 0.6));
            let paint = crate::dl::Paint::Linear { p0: [p0.0, p0.1], p1: [p1.0, p1.1], stops };
            // Smear: the swept hull of the piece over the shutter, faint.
            if moved > s * 0.35 && self.blur > 0.0 {
                let mut pts = Vec::with_capacity(8);
                for (a, bb) in [(-hu, -hv), (hu, -hv), (hu, hv), (-hu, hv)] {
                    pts.push(corner(x, y, a, bb));
                    pts.push(corner(px, py, a, bb));
                }
                let mut sp = Path::new();
                hull(&mut sp, &pts);
                bands[b].push(fill(sp, solid(alpha(col(lit), (0.45 * s / (s + moved)).min(0.4)))));
            }
            bands[b].push(fill(path, paint));
        }
        let mut o = Vec::new();
        let blur = band_blur(self.depth);
        for (b, ops) in bands.into_iter().enumerate() {
            if ops.is_empty() {
                continue;
            }
            let dim = if b == 0 { 0.85 } else { 1.0 };
            let pushed = fxkit::push_blur(&mut o, blur[b] * k, Blend::Normal);
            o.extend(super::with_opacity(ops, dim));
            fxkit::pop_if(&mut o, pushed);
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.at + self.life + 2.1)
    }
}

// ------------------------------------------------------------------ sparks

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sparks {
    pub count: u32,
    /// Origin as a fraction of the box.
    pub origin: [f32; 2],
    /// Burst times (s); empty = one burst at 0.
    pub bursts: Vec<f32>,
    /// Hot and cool colours (default: black-body colours from `temperature`).
    pub colors: Vec<Color>,
    /// Temperature of a fresh spark in kelvin (6500 white-hot, 2500 orange embers).
    pub temperature: f32,
    pub power: f32,
    pub gravity: f32,
    pub life: f32,
    /// Emission cone: direction (degrees, 0 = right, -90 = up) and spread (degrees).
    pub direction: f32,
    pub spread: f32,
    /// Trail length (1 ≈ 45 ms of motion; 3 = long light trails; 0 = dots).
    pub streak: f32,
    pub glow: bool,
    /// Depth of field: near sparks blur into soft bokeh (0 = all sharp).
    pub depth: f32,
    /// A bright flash at the origin when a burst fires.
    pub flash: bool,
    pub seed: u32,
}
impl Default for Sparks {
    fn default() -> Self {
        Sparks {
            count: 140,
            origin: [0.5, 0.5],
            bursts: Vec::new(),
            colors: Vec::new(),
            temperature: 4300.0,
            power: 1.0,
            gravity: 600.0,
            life: 1.2,
            direction: -90.0,
            spread: 360.0,
            streak: 1.6,
            glow: true,
            depth: 1.0,
            flash: true,
            seed: 2,
        }
    }
}
impl Prepare for Sparks {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.count > 5000 {
            return Err(Error("count must be at most 5000".into()));
        }
        self.temperature = self.temperature.clamp(1200.0, 12000.0);
        self.depth = self.depth.clamp(0.0, 2.0);
        self.streak = self.streak.clamp(0.0, 12.0);
        Ok(())
    }
}

/// Heat levels sparks are batched into.
const HEAT: usize = 8;

impl Component for Sparks {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let scale = w.max(h) / 1920.0;
        let k = h / 1080.0;
        let custom = (self.colors.first().map(|c| c.0), self.colors.get(1).map(|c| c.0));
        let color_at = |heat: f32| -> Rgba {
            match custom {
                (Some(a), Some(b)) => mix(b, a, heat),
                (Some(a), None) => mix(shade(a, -0.4), hot(a, 0.5), heat),
                _ => kelvin(lerp(1300.0, self.temperature, heat.powf(0.8))),
            }
        };
        let bursts = if self.bursts.is_empty() { vec![0.0] } else { self.bursts.clone() };
        let shutter = 0.045 * self.streak;
        let mut core: Vec<Path> = vec![Path::new(); BANDS * HEAT];
        let mut halo: Vec<Path> = vec![Path::new(); BANDS * HEAT];
        let mut flashes = Vec::new();
        for (bi, &bt) in bursts.iter().enumerate() {
            let lt0 = t - bt;
            if lt0 < 0.0 || lt0 > self.life * 1.6 {
                continue;
            }
            let (x0, y0) = (self.origin[0] * w, self.origin[1] * h);
            if self.flash && lt0 < 0.25 {
                flashes.push((x0, y0, 1.0 - lt0 / 0.25));
            }
            let mut rng = Rng::new((self.seed as u64) << 16 | bi as u64);
            for _ in 0..self.count {
                let r: [f32; 7] = std::array::from_fn(|_| rng.f());
                let lt = lt0 - r[3] * 0.06;
                let life = self.life * (0.45 + 0.75 * r[2]);
                if lt <= 0.0 || lt > life {
                    continue;
                }
                let z = r[4];
                let ang = (self.direction + (r[0] - 0.5) * self.spread).to_radians();
                let sp = (450.0 + 1500.0 * r[1] * r[1]) * self.power * scale * (0.7 + 0.5 * z);
                let (vx, vy) = (ang.cos() * sp, ang.sin() * sp);
                let drag = 2.6 + 1.2 * r[5];
                let g = self.gravity * scale;
                let at = |lt: f32| (ballistic(x0, vx, drag, 0.0, lt), ballistic(y0, vy, drag, g, lt));
                let (x, y) = at(lt);
                // Trail: where it was over the shutter (longer when fast).
                let ts = shutter.min(lt);
                let trail = [at(lt - ts), at(lt - ts * 0.66), at(lt - ts * 0.33), (x, y)];
                let vel = (velocity(vx, drag, 0.0, lt).powi(2) + velocity(vy, drag, g, lt).powi(2)).sqrt();
                let heat = (1.0 - lt / life).powf(0.9);
                // A flicker as the spark sputters.
                let fl = 0.75 + 0.25 * ((lt * (30.0 + 25.0 * r[6])) + r[6] * 9.0).sin();
                let heat_l = clamp01(heat * fl);
                let b = band(z, self.depth);
                let lvl = ((heat_l * HEAT as f32) as usize).min(HEAT - 1);
                let wd = (1.3 + 2.0 * z) * scale.max(0.5) * (0.55 + 0.45 * heat);
                let wtail = wd * 0.25;
                let _ = vel;
                fxkit::trail_path(&mut core[b * HEAT + lvl], &trail, wtail, wd);
                if self.glow && heat_l > 0.25 {
                    fxkit::trail_path(&mut halo[b * HEAT + lvl], &trail, wd * 1.5, wd * 4.0);
                }
            }
        }
        if core.iter().all(|p| p.is_empty()) && flashes.is_empty() {
            return Vec::new();
        }
        let mut o = Vec::new();
        fxkit::push_light(&mut o, 1.0, ctx.theme.dark);
        for (x, y, f) in flashes {
            let c = color_at(1.0);
            o.push(light(x, y, 320.0 * k, fxkit::bloom(c, f * f, 0.06)));
            o.push(light(x, y, 50.0 * k, gauss(hot(c, 0.7), f.sqrt())));
        }
        let blur = band_blur(self.depth);
        // Glow: the wide trails, blurred together into one soft bloom.
        if self.glow && halo.iter().any(|p| !p.is_empty()) {
            push_layer(&mut o, 1.0, Blend::Add, vec![crate::dl::Filter::Blur(7.0 * k)]);
            for b in 0..BANDS {
                for l in 0..HEAT {
                    let heat = (l as f32 + 0.5) / HEAT as f32;
                    fxkit::fill_solid(&mut o, std::mem::take(&mut halo[b * HEAT + l]), alpha(color_at(heat), 0.25 + 0.35 * heat));
                }
            }
            o.push(Op::PopLayer);
        }
        for b in 0..BANDS {
            let pushed = fxkit::push_blur(&mut o, blur[b] * k * 1.4, Blend::Add);
            for l in 0..HEAT {
                let heat = (l as f32 + 0.5) / HEAT as f32;
                let c = color_at(heat);
                let core_c = if heat > 0.6 { hot(c, (heat - 0.6) * 1.2) } else { c };
                fxkit::fill_solid(&mut o, std::mem::take(&mut core[b * HEAT + l]), alpha(core_c, 0.45 + 0.55 * heat));
            }
            fxkit::pop_if(&mut o, pushed);
        }
        o.push(Op::PopLayer);
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.bursts.iter().fold(0.0f32, |m, b| m.max(*b)) + self.life * 1.3)
    }
}

// ------------------------------------------------------------------ snow

const SNOW_SHAPES: &[&str] = &["soft", "crystal"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Snow {
    pub count: u32,
    /// Flake colour (default: white on dark themes, the theme's text colour on light ones).
    pub color: Option<Color>,
    /// Fall speed (px/s at 1080p for the nearest layer).
    pub speed: f32,
    /// Horizontal wind (px/s).
    pub wind: f32,
    /// How much flakes drift and swirl (0 = straight down).
    pub turbulence: f32,
    pub size: f32,
    /// `soft` round flakes, or `crystal` (six-armed flakes up close).
    pub shape: String,
    /// Depth of field: near flakes blur into bokeh (0 = all sharp).
    pub depth: f32,
    pub seed: u32,
}
impl Default for Snow {
    fn default() -> Self {
        Snow { count: 320, color: None, speed: 90.0, wind: 15.0, turbulence: 1.0, size: 4.0, shape: "soft".into(), depth: 1.0, seed: 4 }
    }
}
impl Prepare for Snow {
    fn prepare(&mut self) -> Result<(), Error> {
        fxkit::choice("snow shape", &self.shape, SNOW_SHAPES)?;
        if self.count > 20000 {
            return Err(Error("count must be at most 20000".into()));
        }
        self.depth = self.depth.clamp(0.0, 2.0);
        Ok(())
    }
}

/// Six-armed snow crystal appended to `p`.
fn crystal(p: &mut Path, x: f32, y: f32, r: f32, rot: f32) {
    let arm = r * 0.16;
    for j in 0..6 {
        let a = rot + j as f32 * TAU / 6.0;
        let (s, c) = a.sin_cos();
        let pt = |u: f32, v: f32| ((x + u * c - v * s) as f64, (y + u * s + v * c) as f64);
        p.move_to(pt(0.0, -arm * 0.5));
        p.line_to(pt(r, -arm * 0.3));
        p.line_to(pt(r * 1.05, 0.0));
        p.line_to(pt(r, arm * 0.3));
        p.line_to(pt(0.0, arm * 0.5));
        p.close_path();
        // Side branches.
        for (f, l) in [(0.45, 0.38), (0.7, 0.26)] {
            for sgn in [-1.0f32, 1.0] {
                let bx = r * f;
                let (ex, ey) = (bx + l * r * 0.6, sgn * l * r * 0.8);
                p.move_to(pt(bx - arm * 0.3, 0.0));
                p.line_to(pt(ex, ey));
                p.line_to(pt(bx + arm * 0.5, 0.0));
                p.close_path();
            }
        }
    }
    fxkit::dot(p, x, y, arm * 0.9);
}

impl Component for Snow {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let dark = ctx.theme.dark;
        let c = self.color.map(|c| c.0).unwrap_or(if dark { Rgba([0.96, 0.98, 1.0, 1.0]) } else { ctx.theme.muted });
        let n = Noise::new(self.seed);
        let mut rng = Rng::new(self.seed as u64);
        const LV: usize = 4;
        let mut paths: Vec<Path> = vec![Path::new(); BANDS * LV];
        let mut glints = Vec::new();
        for i in 0..self.count {
            let (a, b, z0, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * TAU);
            let rs = rng.f();
            let z = z0;
            let depth = 0.25 + 0.75 * z;
            let r = self.size * k * depth * depth * (0.6 + 0.7 * rs) * if self.depth > 0.0 { 1.0 + 1.6 * clamp01((z - 0.82) / 0.18) } else { 1.0 };
            let fall = self.speed * k * depth;
            let ts = (t * 0.15) as f64;
            let fi = i as f64 * 0.917;
            let tx = self.turbulence * (n.noise2(fi, ts) as f32 * 60.0 + (t * (0.6 + z) + ph).sin() * 14.0) * k * depth;
            let ty = self.turbulence * n.noise2(fi + 31.0, ts) as f32 * 30.0 * k * depth;
            let pad = 40.0 * k;
            let x = (a * (w + 2.0 * pad) + self.wind * k * depth * t + tx).rem_euclid(w + 2.0 * pad) - pad;
            let y = (b * (h + 2.0 * pad) + fall * t + ty).rem_euclid(h + 2.0 * pad) - pad;
            let bd = band(z, self.depth);
            let lv = (((0.35 + 0.65 * depth) * (0.7 + 0.3 * rs)) * LV as f32).min(LV as f32 - 1.0) as usize;
            if self.shape == "crystal" && bd == 1 && z > 0.45 {
                crystal(&mut paths[bd * LV + lv], x, y, r * 2.2 + 2.0 * k, ph + t * (0.3 + rs) * if i % 2 == 0 { 1.0 } else { -1.0 });
            } else {
                fxkit::dot(&mut paths[bd * LV + lv], x, y, r.max(0.4));
            }
            // An occasional glint as a flake turns towards the light.
            let gl = ((t * (0.9 + 1.7 * rs) + ph).sin() * 0.5 + 0.5).powi(24);
            if gl > 0.3 && bd == 1 && z > 0.3 {
                glints.push((x, y, r, gl));
            }
        }
        let mut o = Vec::new();
        let blur = band_blur(self.depth);
        for b in 0..BANDS {
            let pushed = fxkit::push_blur(&mut o, blur[b] * k * if b == 2 { 1.3 } else { 1.0 }, Blend::Normal);
            for l in 0..LV {
                let a = [0.3, 0.5, 0.72, 0.92][l] * if b == 2 { 0.75 } else { 1.0 };
                fxkit::fill_solid(&mut o, std::mem::take(&mut paths[b * LV + l]), alpha(c, a));
            }
            fxkit::pop_if(&mut o, pushed);
        }
        if !glints.is_empty() && dark {
            fxkit::push_light(&mut o, 1.0, true);
            for (x, y, r, g) in glints {
                o.push(light(x, y, r * 5.0, gauss(c, 0.5 * g)));
                o.push(fxkit::needle(x, y, 0.0, r * 5.0, r * 0.5, c, 0.6 * g));
                o.push(fxkit::needle(x, y, std::f32::consts::FRAC_PI_2, r * 4.0, r * 0.5, c, 0.6 * g));
            }
            o.push(Op::PopLayer);
        }
        o
    }
}

// ------------------------------------------------------------------ dust

const DUST_LOOKS: &[&str] = &["motes", "bokeh", "fireflies"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dust {
    pub count: u32,
    /// Mote colour (default: warm light on dark themes, the muted text colour on light ones).
    pub color: Option<Color>,
    /// `motes` (fine specks in light, with a few soft out-of-focus ones),
    /// `bokeh` (big soft discs) or `fireflies` (glowing points that pulse).
    pub look: String,
    pub size: f32,
    pub speed: f32,
    /// Glow halos around bright motes.
    pub glow: bool,
    /// Depth of field (0 = all sharp).
    pub depth: f32,
    pub seed: u32,
}
impl Default for Dust {
    fn default() -> Self {
        Dust { count: 140, color: None, look: "motes".into(), size: 3.0, speed: 1.0, glow: true, depth: 1.0, seed: 6 }
    }
}
impl Prepare for Dust {
    fn prepare(&mut self) -> Result<(), Error> {
        fxkit::choice("dust look", &self.look, DUST_LOOKS)?;
        if self.count > 20000 {
            return Err(Error("count must be at most 20000".into()));
        }
        self.depth = self.depth.clamp(0.0, 2.0);
        Ok(())
    }
}
impl Component for Dust {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let dark = ctx.theme.dark;
        let look = self.look.as_str();
        let c = self.color.map(|c| c.0).unwrap_or_else(|| match (dark, look) {
            (false, _) => ctx.theme.muted,
            (_, "fireflies") => Rgba([0.85, 1.0, 0.55, 1.0]),
            _ => hot(kelvin(4300.0), 0.25),
        });
        let n = Noise::new(self.seed);
        let mut rng = Rng::new(self.seed as u64 ^ 0xd057);
        const LV: usize = 5;
        let mut sharp: Vec<Path> = vec![Path::new(); BANDS * LV];
        let mut discs = Vec::new();
        let mut halos = Vec::new();
        for i in 0..self.count {
            let (a, b, z, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * TAU);
            let rs = rng.f();
            let s = (t * 0.05 * self.speed) as f64;
            let fi = i as f64 * 3.17;
            let par = 0.4 + 0.8 * z;
            let x = (a * w + n.noise2(fi, s) as f32 * w * 0.12 * par + t * 6.0 * self.speed * k * par).rem_euclid(w);
            let y = (b * h + n.noise2(fi + 40.0, s) as f32 * h * 0.12 * par - t * 9.0 * self.speed * k * par).rem_euclid(h);
            let (rate, sharpness) = if look == "fireflies" { (0.5 + 0.7 * rs, 3) } else { (0.6 + 1.2 * rs, 1) };
            let tw = (0.5 + 0.5 * (t * rate * self.speed + ph).sin()).powi(sharpness);
            let bokeh = look == "bokeh" || (self.depth > 0.0 && z > 1.0 - 0.07 * self.depth);
            if bokeh {
                let r = self.size * k * (3.0 + 13.0 * rs * rs) * if look == "bokeh" { 1.0 + z } else { 1.0 };
                discs.push((x, y, r, (0.05 + 0.1 * tw) * (1.2 - 0.6 * rs), rs));
                continue;
            }
            let r = self.size * k * (0.4 + 0.9 * z) * (0.75 + 0.25 * tw);
            let bd = band(z, self.depth);
            let lv = ((tw * (0.4 + 0.6 * z)) * LV as f32).min(LV as f32 - 1.0) as usize;
            fxkit::dot(&mut sharp[bd * LV + lv], x, y, r.max(0.35));
            if self.glow && (tw > 0.65 || look == "fireflies") {
                halos.push((x, y, r, tw));
            }
        }
        let mut o = Vec::new();
        fxkit::push_light(&mut o, 1.0, dark);
        for (x, y, r, a, rs) in discs {
            // A bokeh disc: flat with a slightly brighter rim and a hint of
            // chromatic fringe.
            let fringe = mix(c, Rgba([0.6, 0.8, 1.0, 1.0]), 0.3 * rs);
            o.push(light(x, y, r, vec![(0.0, alpha(c, a * 0.5)), (0.6, alpha(c, a * 0.6)), (0.84, alpha(c, a * 0.9)), (0.93, alpha(fringe, a)), (1.0, alpha(fringe, 0.0))]));
        }
        for (x, y, r, tw) in halos {
            let (hr, ha) = if look == "fireflies" { (r * 10.0, 0.55 * tw) } else { (r * 7.0, 0.4 * tw) };
            o.push(light(x, y, hr, fxkit::bloom(c, ha, 0.12)));
        }
        let blur = band_blur(self.depth);
        for b in 0..BANDS {
            let pushed = fxkit::push_blur(&mut o, blur[b] * k, Blend::Normal);
            for l in 0..LV {
                let a = 0.22 + 0.78 * (l as f32 + 0.5) / LV as f32;
                let col = if look == "fireflies" && l >= 3 { hot(c, 0.5) } else { c };
                fxkit::fill_solid(&mut o, std::mem::take(&mut sharp[b * LV + l]), alpha(col, a));
            }
            fxkit::pop_if(&mut o, pushed);
        }
        o.push(Op::PopLayer);
        o
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use super::*;
    use serde_json::json;

    #[test]
    fn ballistic_matches_integration() {
        let (mut p, mut v) = (0.0f32, 100.0f32);
        let (k, g, dt) = (2.0, 500.0, 1e-4);
        for _ in 0..10000 {
            v += (g - k * v) * dt;
            p += v * dt;
        }
        assert!((ballistic(0.0, 100.0, k, g, 1.0) - p).abs() < 0.5);
        assert!((velocity(100.0, k, g, 1.0) - v).abs() < 0.5);
    }

    #[test]
    fn hull_is_convex() {
        use kurbo::Shape;
        let mut p = Path::new();
        hull(&mut p, &[(0.0, 0.0), (1.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.0, 1.0)]);
        assert!((p.area().abs() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn deterministic_and_gallery() {
        let th = Theme::default();
        let a = preview::host();
        for (k, p, t) in [
            ("confetti", json!({}), 0.6f32),
            ("confetti", json!({"mode": "cannons", "material": "foil"}), 0.9),
            ("sparks", json!({}), 0.25),
            ("sparks", json!({"temperature": 2400, "streak": 4, "mode": null}), 0.4),
            ("snow", json!({"shape": "crystal"}), 2.0),
            ("dust", json!({}), 2.0),
            ("dust", json!({"look": "fireflies"}), 2.0),
        ] {
            let mut p = p;
            if let Some(o) = p.as_object_mut() {
                o.retain(|_, v| !v.is_null());
            }
            let c = build(k, &p).unwrap();
            let o1 = c.eval(t, 960.0, 540.0, 30.0, &th, a);
            let o2 = c.eval(t, 960.0, 540.0, 30.0, &th, a);
            assert_eq!(o1, o2, "{k} is not deterministic");
            assert!(!o1.is_empty(), "{k} drew nothing at {t}");
            preview::validate(&o1).unwrap();
            let name = format!("particles-{k}-{}", p.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join("-")).unwrap_or_default());
            preview::snapshot(&name, 960, 540, th.background, o1, a);
        }
        assert!(build("confetti", &json!({"material": "foill"})).err().unwrap().0.contains("foil"));
        assert!(build("dust", &json!({"look": "boke"})).err().unwrap().0.contains("bokeh"));
    }
}
