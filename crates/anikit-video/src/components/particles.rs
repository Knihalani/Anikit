//! Particles: confetti, sparks, snow and dust.
//!
//! Motion is closed-form (ballistic with linear drag, or wrapped drift), so
//! every frame is a pure function of `t` and frames render in any order on
//! any thread. Layouts are seeded and deterministic.

use super::style::*;
use super::{Component, Ctx, Prepare};
use crate::dl::{Blend, Filter, Op, Path, Rgba, IDENTITY};
use anikit_core::noise::Noise;
use kurbo::Shape;
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

fn palette(cols: &[Color], ctx: &Ctx) -> Vec<Rgba> {
    if cols.is_empty() {
        ctx.theme.palette.clone()
    } else {
        cols.iter().map(|c| c.0).collect()
    }
}

// ------------------------------------------------------------------ confetti

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
    pub colors: Vec<Color>,
    /// Launch speed multiplier.
    pub power: f32,
    pub gravity: f32,
    /// Piece size in pixels.
    pub size: f32,
    /// Seconds before pieces fade out.
    pub life: f32,
    pub seed: u32,
}
impl Default for Confetti {
    fn default() -> Self {
        Confetti { count: 160, mode: "burst".into(), origin: [0.5, 0.6], at: 0.0, colors: Vec::new(), power: 1.0, gravity: 900.0, size: 16.0, life: 3.5, seed: 1 }
    }
}
impl Prepare for Confetti {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if !["burst", "cannons", "rain"].contains(&self.mode.as_str()) {
            return Err(super::Error(format!("confetti mode must be burst, cannons or rain, not {:?}", self.mode)));
        }
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
        let cols = palette(&self.colors, ctx);
        let mut rng = Rng::new(self.seed as u64 * 7 + 3);
        let scale = w.max(h) / 1920.0;
        let mut paths: Vec<Path> = vec![Path::new(); cols.len()];
        let mut shade_paths: Vec<Path> = vec![Path::new(); cols.len()];
        for i in 0..self.count {
            let (r1, r2, r3, r4, r5, r6) = (rng.f(), rng.f(), rng.f(), rng.f(), rng.f(), rng.f());
            let ci = i as usize % cols.len();
            // Per-piece launch.
            let (x0, y0, vx, vy, delay) = match self.mode.as_str() {
                "rain" => (r1 * w, -20.0 - r2 * h * 0.3, (r3 - 0.5) * 120.0, 0.0, r4 * 1.5),
                "cannons" => {
                    let left = i % 2 == 0;
                    let ang = (-48.0 - r1 * 40.0f32).to_radians();
                    let sp = (1100.0 + r2 * 2200.0) * self.power * scale;
                    let (s, c) = ang.sin_cos();
                    (if left { 0.0 } else { w }, h, if left { c * sp } else { -c * sp }, s * sp, r3 * 0.12)
                }
                _ => {
                    let ang = r1 * std::f32::consts::TAU;
                    let sp = (300.0 + r2.sqrt() * 1300.0) * self.power * scale;
                    (self.origin[0] * w, self.origin[1] * h, ang.cos() * sp, ang.sin() * sp - 500.0 * scale * self.power, r3 * 0.05)
                }
            };
            let lt = t - delay;
            if lt <= 0.0 {
                continue;
            }
            let k = 1.6 + r4 * 1.2; // drag
            let g = self.gravity * scale * (0.7 + 0.3 * r5);
            let mut x = ballistic(x0, vx, k, g * 0.0 + 0.0, lt);
            let y = ballistic(y0, vy, k, g, lt);
            // Flutter: sideways sway grows as pieces slow down.
            x += (lt * (3.0 + r5 * 4.0) + r6 * 6.0).sin() * 18.0 * scale * clamp01(lt * 0.8);
            let fade = 1.0 - seg(lt, self.life * (0.75 + 0.25 * r6), 0.6);
            if fade <= 0.0 || y > h + 40.0 || x < -60.0 || x > w + 60.0 {
                continue;
            }
            // Tumbling rectangle: rotation plus a 3D flip (scale on one axis).
            let s = self.size * scale.max(0.4) * (0.6 + 0.8 * r3) * fade.sqrt();
            let rot = r6 * 6.0 + lt * (4.0 + r1 * 8.0) * if i % 3 == 0 { -1.0 } else { 1.0 };
            let flip = (lt * (6.0 + r2 * 9.0) + r4 * 6.0).cos();
            let (hw, hh) = (s * 0.5, s * 0.32 * flip.abs().max(0.08));
            let (sn, cs) = rot.sin_cos();
            let corner = |a: f32, b: f32| ((x + a * cs - b * sn) as f64, (y + a * sn + b * cs) as f64);
            let target = if flip < 0.0 { &mut shade_paths[ci] } else { &mut paths[ci] };
            target.move_to(corner(-hw, -hh));
            target.line_to(corner(hw, -hh));
            target.line_to(corner(hw, hh));
            target.line_to(corner(-hw, hh));
            target.close_path();
        }
        let mut o = Vec::new();
        for (i, c) in cols.iter().enumerate() {
            if !paths[i].is_empty() {
                o.push(fill(std::mem::take(&mut paths[i]), solid(*c)));
            }
            if !shade_paths[i].is_empty() {
                o.push(fill(std::mem::take(&mut shade_paths[i]), solid(shade(*c, -0.3))));
            }
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
    /// Hot core and cooling tail colours.
    pub colors: Vec<Color>,
    pub power: f32,
    pub gravity: f32,
    pub life: f32,
    /// Emission cone: direction (degrees, 0 = right, -90 = up) and spread (degrees).
    pub direction: f32,
    pub spread: f32,
    pub glow: bool,
    pub seed: u32,
}
impl Default for Sparks {
    fn default() -> Self {
        Sparks { count: 90, origin: [0.5, 0.5], bursts: Vec::new(), colors: Vec::new(), power: 1.0, gravity: 600.0, life: 1.1, direction: -90.0, spread: 360.0, glow: true, seed: 2 }
    }
}
impl Prepare for Sparks {}
impl Component for Sparks {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let scale = w.max(h) / 1920.0;
        let hot = self.colors.first().map(|c| c.0).unwrap_or(parse_color("#fff4d6").unwrap());
        let cool = self.colors.get(1).map(|c| c.0).unwrap_or(parse_color("#ff7a2e").unwrap());
        let bursts = if self.bursts.is_empty() { vec![0.0] } else { self.bursts.clone() };
        // Three heat buckets → three strokes.
        let mut buckets = [Path::new(), Path::new(), Path::new()];
        for (bi, &bt) in bursts.iter().enumerate() {
            let lt0 = t - bt;
            if lt0 < 0.0 || lt0 > self.life * 1.6 {
                continue;
            }
            let mut rng = Rng::new((self.seed as u64) << 16 | bi as u64);
            for _ in 0..self.count {
                let (r1, r2, r3, r4) = (rng.f(), rng.f(), rng.f(), rng.f());
                let lt = lt0 - r4 * 0.06;
                let life = self.life * (0.5 + 0.7 * r3);
                if lt <= 0.0 || lt > life {
                    continue;
                }
                let ang = (self.direction + (r1 - 0.5) * self.spread).to_radians();
                let sp = (500.0 + 1400.0 * r2 * r2) * self.power * scale;
                let (vx, vy) = (ang.cos() * sp, ang.sin() * sp);
                let k = 3.2;
                let g = self.gravity * scale;
                let (x0, y0) = (self.origin[0] * w, self.origin[1] * h);
                let (x, y) = (ballistic(x0, vx, k, 0.0, lt), ballistic(y0, vy, k, g, lt));
                // Tail: where it was a short time ago (motion blur streak).
                let dt = 0.035;
                let (tx, ty) = (x - velocity(vx, k, 0.0, lt) * dt, y - velocity(vy, k, g, lt) * dt);
                let heat = 1.0 - lt / life;
                let b = (heat * 3.0).min(2.0) as usize;
                buckets[b].move_to((tx as f64, ty as f64));
                buckets[b].line_to((x as f64, y as f64));
            }
        }
        let mut o = Vec::new();
        let draw_all = |o: &mut Vec<Op>, width_k: f32| {
            for (i, p) in buckets.iter().enumerate() {
                if p.is_empty() {
                    continue;
                }
                let c = mix(cool, hot, i as f32 / 2.0);
                let a = [0.55, 0.85, 1.0][i];
                o.push(stroke_op(p.clone(), solid(alpha(c, a)), (1.2 + i as f32 * 0.9) * width_k * scale.max(0.5), [0.0, 1.0]));
            }
        };
        if buckets.iter().all(|b| b.is_empty()) {
            return o;
        }
        push_layer(&mut o, 1.0, Blend::Add, Vec::new());
        if self.glow {
            push_layer(&mut o, 0.8, Blend::Normal, vec![Filter::Blur(6.0 * scale.max(0.5))]);
            draw_all(&mut o, 3.0);
            o.push(Op::PopLayer);
        }
        draw_all(&mut o, 1.0);
        o.push(Op::PopLayer);
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.bursts.iter().fold(0.0f32, |m, b| m.max(*b)) + self.life * 1.3)
    }
}

// ------------------------------------------------------------------ snow

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Snow {
    pub count: u32,
    pub color: Option<Color>,
    /// Fall speed (px/s at 1080p for the nearest layer).
    pub speed: f32,
    /// Horizontal wind (px/s).
    pub wind: f32,
    pub size: f32,
    pub seed: u32,
}
impl Default for Snow {
    fn default() -> Self {
        Snow { count: 220, color: None, speed: 90.0, wind: 15.0, size: 4.0, seed: 4 }
    }
}
impl Prepare for Snow {}
impl Component for Snow {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let scale = h / 1080.0;
        let c = self.color.map(|c| c.0).unwrap_or(Rgba([1.0, 1.0, 1.0, 1.0]));
        let mut rng = Rng::new(self.seed as u64);
        // Depth layers: far (small, slow, dim), mid, near (big, soft).
        let mut layers = [Path::new(), Path::new(), Path::new()];
        for _ in 0..self.count {
            let (a, b, z, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * std::f32::consts::TAU);
            let depth = 0.35 + 0.65 * z;
            let r = self.size * scale * depth * (0.6 + 0.6 * rng.f());
            let fall = self.speed * scale * depth;
            let x = (a * w + self.wind * depth * t + (t * (0.6 + z) + ph).sin() * 20.0 * scale * depth).rem_euclid(w + 20.0) - 10.0;
            let y = (b * (h + 20.0) + fall * t).rem_euclid(h + 20.0) - 10.0;
            let li = (z * 3.0).min(2.0) as usize;
            layers[li].extend(kurbo::Circle::new((x as f64, y as f64), r.max(0.3) as f64).path_elements(0.2));
        }
        let mut o = Vec::new();
        for (i, p) in layers.into_iter().enumerate() {
            if p.is_empty() {
                continue;
            }
            if i == 2 {
                push_layer(&mut o, 1.0, Blend::Normal, vec![Filter::Blur(2.0 * scale.max(0.5))]);
                o.push(fill(p, solid(alpha(c, 0.9))));
                o.push(Op::PopLayer);
            } else {
                o.push(fill(p, solid(alpha(c, [0.35, 0.6][i]))));
            }
        }
        o
    }
}

// ------------------------------------------------------------------ dust

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dust {
    pub count: u32,
    pub color: Option<Color>,
    pub size: f32,
    pub speed: f32,
    pub glow: bool,
    pub seed: u32,
}
impl Default for Dust {
    fn default() -> Self {
        Dust { count: 90, color: None, size: 3.5, speed: 1.0, glow: true, seed: 6 }
    }
}
impl Prepare for Dust {}
impl Component for Dust {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let scale = h / 1080.0;
        let c = self.color.map(|c| c.0).unwrap_or(parse_color("#ffe9c4").unwrap());
        let n = Noise::new(self.seed);
        let mut rng = Rng::new(self.seed as u64 ^ 0xd057);
        let mut dim = Path::new();
        let mut bright = Path::new();
        for i in 0..self.count {
            let (a, b, z, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * std::f32::consts::TAU);
            let s = (t * 0.05 * self.speed) as f64;
            let fi = i as f64 * 3.17;
            let x = (a * w + n.noise2(fi, s) as f32 * w * 0.15 + t * 6.0 * self.speed * scale).rem_euclid(w);
            let y = (b * h + n.noise2(fi + 40.0, s) as f32 * h * 0.15 - t * 10.0 * self.speed * scale).rem_euclid(h);
            let tw = 0.5 + 0.5 * (t * (0.8 + z * 1.5) + ph).sin();
            let r = self.size * scale * (0.5 + z) * (0.7 + 0.3 * tw);
            let circle = kurbo::Circle::new((x as f64, y as f64), r.max(0.3) as f64);
            if tw > 0.6 {
                bright.extend(circle.path_elements(0.2));
            } else {
                dim.extend(circle.path_elements(0.2));
            }
        }
        let mut o = Vec::new();
        push_layer(&mut o, 1.0, Blend::Screen, Vec::new());
        if self.glow && !bright.is_empty() {
            push_layer(&mut o, 0.9, Blend::Normal, vec![Filter::Blur(6.0 * scale.max(0.5))]);
            o.push(fill(bright.clone(), solid(alpha(c, 0.9))));
            o.push(Op::PopLayer);
        }
        o.push(fill(dim, solid(alpha(c, 0.35))));
        o.push(fill(bright, solid(alpha(c, 0.85))));
        o.push(Op::PopLayer);
        let _ = IDENTITY;
        o
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, text::StubAssets, Theme};
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
    fn deterministic_and_gallery() {
        let th = Theme::default();
        let a = StubAssets::default();
        for (k, p, t) in [
            ("confetti", json!({}), 0.6f32),
            ("confetti", json!({"mode": "cannons"}), 0.9),
            ("sparks", json!({}), 0.25),
            ("snow", json!({}), 2.0),
            ("dust", json!({}), 2.0),
        ] {
            let c = build(k, &p).unwrap();
            let o1 = c.eval(t, 960.0, 540.0, 30.0, &th, &a);
            let o2 = c.eval(t, 960.0, 540.0, 30.0, &th, &a);
            assert_eq!(o1, o2, "{k} is not deterministic");
            assert!(!o1.is_empty(), "{k} drew nothing at {t}");
            preview::validate(&o1).unwrap();
            let name = format!("particles-{k}-{}", p.get("mode").and_then(|m| m.as_str()).unwrap_or("default"));
            preview::snapshot(&name, 960, 540, th.background, o1, &a);
        }
    }
}
