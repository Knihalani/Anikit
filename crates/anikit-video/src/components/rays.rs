//! Light: `god-rays` (volumetric beams from a point, with dust in the
//! light) and `lens-flare` (anamorphic streak, halo and ghosts along the axis
//! through the frame centre). Both are a few dozen gradient-filled vector
//! ops in a screen-blended layer: no pixel work.

use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, Op, Paint, Path, Rgba};
use anikit_core::noise::Noise;
use kurbo::Shape;
use serde::Deserialize;
use std::f32::consts::TAU;

// ------------------------------------------------------------------ god rays

struct Ray {
    /// Offset in the fan (-0.5..0.5).
    u: f32,
    /// Angular width (degrees).
    width: f32,
    /// Length multiplier.
    len: f32,
    base: f32,
    sway: f32,
    phase: f32,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GodRays {
    /// Light source as fractions of the box (may be outside 0..1).
    pub at: [f32; 2],
    pub color: Option<Color>,
    /// Number of beams.
    pub count: u32,
    /// Direction of the fan in degrees (90 = straight down).
    pub angle: f32,
    /// Fan width in degrees (360 = all around).
    pub spread: f32,
    /// Beam length as a fraction of the box diagonal.
    pub length: f32,
    /// Overall brightness.
    pub intensity: f32,
    /// Rotation of the fan in degrees per second.
    pub spin: f32,
    /// Dust motes floating in the beams (0 = none).
    pub dust: u32,
    pub seed: u32,
    #[serde(skip)]
    rays: Vec<Ray>,
}

impl Default for GodRays {
    fn default() -> Self {
        GodRays { at: [0.32, -0.08], color: None, count: 14, angle: 62.0, spread: 70.0, length: 1.15, intensity: 0.8, spin: 1.5, dust: 120, seed: 4, rays: Vec::new() }
    }
}

impl Prepare for GodRays {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.count > 200 {
            return Err(Error("count must be at most 200".into()));
        }
        let mut rng = Rng::new(self.seed as u64 * 53 + 1);
        let n = self.count.max(1);
        self.rays = (0..self.count)
            .map(|i| Ray {
                u: (i as f32 + 0.15 + 0.7 * rng.f()) / n as f32 - 0.5,
                width: (if rng.f() < 0.3 { rng.range(7.0, 13.0) } else { rng.range(2.0, 6.5) }) * (self.spread / 70.0).clamp(0.3, 3.0),
                len: rng.range(0.7, 1.0),
                base: rng.range(0.25, 1.0),
                sway: rng.range(0.6, 2.2),
                phase: rng.f() * TAU,
            })
            .collect();
        Ok(())
    }
}

impl GodRays {
    /// (angle rad, half-width rad, alpha, length px) of each beam at `t`.
    fn beams(&self, t: f32, diag: f32) -> Vec<(f32, f32, f32, f32)> {
        let n = Noise::new(self.seed);
        self.rays
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let a = self.angle + r.u * self.spread + self.spin * t + r.sway * (t * 0.25 + r.phase).sin();
                let fl = 0.55 + 0.45 * n.noise2(i as f64 * 3.1, (t * 0.35) as f64) as f32;
                (a.to_radians(), (r.width * 0.5).to_radians(), self.intensity * r.base * fl.max(0.1), self.length * diag * r.len)
            })
            .collect()
    }
}

fn wedge(x: f32, y: f32, ang: f32, half: f32, len: f32) -> Path {
    let mut p = Path::new();
    p.move_to((x as f64, y as f64));
    for a in [ang - half, ang + half] {
        p.line_to(((x + a.cos() * len) as f64, (y + a.sin() * len) as f64));
    }
    p.close_path();
    p
}

impl Component for GodRays {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let c = self.color.map(|c| c.0).unwrap_or(Rgba([1.0, 0.84, 0.6, 1.0]));
        let (sx, sy) = (self.at[0] * w, self.at[1] * h);
        let diag = (w * w + h * h).sqrt();
        let beams = self.beams(t, diag);
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        // Ambient bloom around the source.
        let br = diag * 0.45;
        out.push(rect(0.0, 0.0, w, h, 0.0, radial(sx, sy, br, vec![(0.0, alpha(c, 0.45 * self.intensity)), (0.3, alpha(c, 0.14 * self.intensity)), (1.0, alpha(c, 0.0))])));
        for &(a, half, al, len) in &beams {
            let (dx, dy) = (a.cos(), a.sin());
            let paint = |al: f32| Paint::Linear { p0: [sx, sy], p1: [sx + dx * len, sy + dy * len], stops: vec![(0.0, alpha(c, al)), (0.25, alpha(c, al * 0.6)), (0.7, alpha(c, al * 0.18)), (1.0, alpha(c, 0.0))] };
            // Nested wedges approximate a soft, gaussian-ish beam profile.
            for (wk, ak) in [(1.0, 0.09), (0.72, 0.11), (0.48, 0.13), (0.26, 0.15), (0.1, 0.12)] {
                out.push(fill(wedge(sx, sy, a, half * wk, len), paint(al * ak)));
            }
        }
        if self.dust > 0 {
            let mut rng = Rng::new(self.seed as u64 ^ 0xd05e);
            let mut paths = [Path::new(), Path::new(), Path::new()];
            for _ in 0..self.dust {
                let (a, b, z, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * TAU);
                let x = (a * w + t * 9.0 * k * (0.3 + z) + (t * 0.5 + ph).sin() * 14.0 * k).rem_euclid(w);
                let y = (b * h - t * 5.0 * k * (0.5 + z) + (t * 0.37 + ph).cos() * 10.0 * k).rem_euclid(h);
                let (dx, dy) = (x - sx, y - sy);
                let d = (dx * dx + dy * dy).sqrt();
                let ang = dy.atan2(dx);
                let mut lit = 0.0f32;
                for &(ba, half, al, len) in &beams {
                    let mut da = (ang - ba).rem_euclid(TAU);
                    if da > TAU * 0.5 {
                        da -= TAU;
                    }
                    let g = (-(da / (half * 0.8)).powi(2)).exp() * al * (1.0 - d / len).max(0.0);
                    lit = lit.max(g);
                }
                let lit = lit * (0.6 + 0.4 * (t * (1.0 + z) + ph).sin());
                if lit < 0.03 {
                    continue;
                }
                let bi = ((lit * 4.0) as usize).min(2);
                let r = (1.0 + 2.6 * z * z) * k;
                paths[bi].extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.2));
            }
            for (i, p) in paths.into_iter().enumerate() {
                if !p.is_empty() {
                    out.push(fill(p, solid(alpha(shade(c, 0.3), [0.3, 0.6, 0.95][i]))));
                }
            }
        }
        out.push(Op::PopLayer);
        out
    }
}

// ------------------------------------------------------------------ lens flare

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LensFlare {
    /// Light position as fractions of the box.
    pub at: [f32; 2],
    /// Positions to travel through (fractions); overrides `at`.
    pub path: Vec<[f32; 2]>,
    /// Arrival time of each `path` point (s); default spreads them over `duration`.
    pub times: Vec<f32>,
    /// Seconds to travel the whole `path` when `times` is omitted.
    pub duration: f32,
    /// Main tint of the glow.
    pub color: Option<Color>,
    /// Colour of the anamorphic streak.
    pub streak_color: Option<Color>,
    /// Streak length as a fraction of the box width (0 = none).
    pub streak: f32,
    pub intensity: f32,
    /// Number of ghosts along the axis (0 = none).
    pub ghosts: u32,
    pub seed: u32,
}

impl Default for LensFlare {
    fn default() -> Self {
        LensFlare { at: [0.3, 0.3], path: Vec::new(), times: Vec::new(), duration: 4.0, color: None, streak_color: None, streak: 1.0, intensity: 1.0, ghosts: 7, seed: 2 }
    }
}

impl Prepare for LensFlare {
    fn prepare(&mut self) -> Result<(), Error> {
        if !self.times.is_empty() {
            if self.times.len() != self.path.len() {
                return Err(Error(format!("times has {} entries but path has {}", self.times.len(), self.path.len())));
            }
            if self.times.windows(2).any(|w| w[1] < w[0]) {
                return Err(Error("times must be increasing".into()));
            }
        }
        if self.ghosts > 32 {
            return Err(Error("ghosts must be at most 32".into()));
        }
        Ok(())
    }
}

impl LensFlare {
    fn pos(&self, t: f32) -> [f32; 2] {
        let n = self.path.len();
        if n == 0 {
            return self.at;
        }
        if n == 1 {
            return self.path[0];
        }
        let time = |i: usize| if self.times.is_empty() { self.duration * i as f32 / (n - 1) as f32 } else { self.times[i] };
        if t <= time(0) {
            return self.path[0];
        }
        for i in 0..n - 1 {
            let (t0, t1) = (time(i), time(i + 1));
            if t <= t1 {
                let e = ease_in_out(seg(t, t0, t1 - t0));
                let (a, b) = (self.path[i], self.path[i + 1]);
                return [lerp(a[0], b[0], e), lerp(a[1], b[1], e)];
            }
        }
        self.path[n - 1]
    }
}

/// An ellipse made of a unit circle with a radial paint, scaled per axis:
/// the gradient stretches with it.
fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, stops: Vec<(f32, Rgba)>) -> Op {
    Op::Fill { path: circle_path(0.0, 0.0, 1.0), paint: radial(0.0, 0.0, 1.0, stops), rule: crate::dl::FillRule::NonZero, transform: [rx, 0.0, 0.0, ry, cx, cy] }
}

impl Component for LensFlare {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let p = self.pos(t);
        let (lx, ly) = (p[0] * w, p[1] * h);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let c = self.color.map(|c| c.0).unwrap_or(Rgba([1.0, 0.86, 0.66, 1.0]));
        let sc = self.streak_color.map(|c| c.0).unwrap_or(Rgba([0.45, 0.7, 1.0, 1.0]));
        // Gentle flicker, like a real light through glass.
        let fl = 0.94 + 0.06 * ((t * 7.3).sin() * 0.6 + (t * 12.1 + 1.0).sin() * 0.4);
        let k = self.intensity * fl;
        let u = h;
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        // Wide halo and hot core.
        out.push(circle(lx, ly, u * 0.42, radial(lx, ly, u * 0.42, vec![(0.0, alpha(c, 0.5 * k)), (0.2, alpha(c, 0.2 * k)), (1.0, alpha(c, 0.0))])));
        out.push(circle(lx, ly, u * 0.05, radial(lx, ly, u * 0.05, vec![(0.0, Rgba([1.0, 1.0, 1.0, k.min(1.0)])), (0.35, alpha(shade(c, 0.6), 0.8 * k)), (1.0, alpha(c, 0.0))])));
        // Faint rainbow ring around the source.
        let rr = u * 0.3;
        out.push(circle(lx, ly, rr, radial(lx, ly, rr, vec![(0.0, Rgba([0.0; 4])), (0.84, Rgba([0.2, 0.4, 1.0, 0.0])), (0.9, Rgba([0.3, 0.6, 1.0, 0.07 * k])), (0.95, Rgba([1.0, 0.75, 0.3, 0.08 * k])), (1.0, Rgba([1.0, 0.3, 0.3, 0.0]))])));
        // Anamorphic streak: a wide soft blue band and a thin bright core.
        if self.streak > 0.0 {
            let sl = w * self.streak * 0.5;
            out.push(ellipse(lx, ly, sl, u * 0.022, vec![(0.0, alpha(sc, 0.55 * k)), (0.4, alpha(sc, 0.2 * k)), (1.0, alpha(sc, 0.0))]));
            out.push(ellipse(lx, ly, sl * 0.7, u * 0.0045, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.9 * k.min(1.0)])), (0.3, alpha(shade(sc, 0.5), 0.5 * k)), (1.0, alpha(sc, 0.0))]));
        }
        // Ghosts along the axis from the light through the centre.
        let tints = [Rgba([0.4, 1.0, 0.75, 1.0]), Rgba([0.65, 0.5, 1.0, 1.0]), Rgba([1.0, 0.6, 0.3, 1.0]), Rgba([0.35, 0.75, 1.0, 1.0]), c];
        let mut rng = Rng::new(self.seed as u64 * 17 + 5);
        let (vx, vy) = (cx - lx, cy - ly);
        // Ghosts are brightest when the light is off-centre.
        let off = clamp01((vx * vx + vy * vy).sqrt() / (u * 0.6)) * 0.7 + 0.3;
        for i in 0..self.ghosts {
            let f = 0.35 + 1.9 * (i as f32 + rng.f() * 0.6) / self.ghosts.max(1) as f32;
            let (gx, gy) = (lx + vx * f * 1.0, ly + vy * f * 1.0);
            let r = u * rng.range(0.015, 0.11);
            let tint = tints[(rng.f() * tints.len() as f32) as usize % tints.len()];
            let a = rng.range(0.06, 0.16) * k * off * (1.0 - r / (u * 0.2));
            match i % 3 {
                // Ring ghost.
                0 => out.push(circle(gx, gy, r, radial(gx, gy, r, vec![(0.0, alpha(tint, a * 0.2)), (0.75, alpha(tint, a * 0.5)), (0.92, alpha(tint, a * 1.4)), (1.0, alpha(tint, 0.0))]))),
                // Aperture (hexagon) ghost.
                1 => {
                    let mut hex = Path::new();
                    for j in 0..6 {
                        let ang = j as f32 * TAU / 6.0 + 0.3;
                        let q = ((gx + r * ang.cos()) as f64, (gy + r * ang.sin()) as f64);
                        if j == 0 {
                            hex.move_to(q)
                        } else {
                            hex.line_to(q)
                        }
                    }
                    hex.close_path();
                    out.push(fill(hex, radial(gx, gy, r, vec![(0.0, alpha(tint, a * 0.6)), (0.85, alpha(tint, a)), (1.0, alpha(tint, a * 0.4))])));
                }
                // Soft disc.
                _ => out.push(circle(gx, gy, r, radial(gx, gy, r, vec![(0.0, alpha(tint, a * 1.3)), (1.0, alpha(tint, 0.0))]))),
            }
        }
        out.push(Op::PopLayer);
        out
    }

    fn duration(&self) -> Option<f32> {
        if self.path.len() < 2 {
            return None;
        }
        Some(self.times.last().copied().unwrap_or(self.duration))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        let bg = build("gradient", &json!({"colors": ["#0b0f1e", "#1b1430"]})).unwrap();
        for (name, p, t) in [
            ("god-rays", json!({}), 1.0f32),
            ("god-rays-top", json!({"at": [0.5, -0.1], "angle": 90, "spread": 100, "color": "#bfe0ff"}), 3.0),
            ("lens-flare", json!({}), 0.5),
            ("lens-flare-path", json!({"path": [[0.15, 0.25], [0.85, 0.35]], "times": [0, 3]}), 1.5),
        ] {
            let kind = if name.starts_with("god") { "god-rays" } else { "lens-flare" };
            let c = build(kind, &p).unwrap();
            let mut ops = bg.eval(t, 960.0, 540.0, 30.0, &th, a);
            let own = c.eval(t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&own).unwrap();
            assert_eq!(own, c.eval(t, 960.0, 540.0, 30.0, &th, a));
            ops.extend(own);
            preview::snapshot(&format!("rays-{name}"), 960, 540, th.background, ops, a);
        }
        let c = build("lens-flare", &json!({"path": [[0.1, 0.2], [0.9, 0.2]], "duration": 2})).unwrap();
        assert_eq!(c.component.duration(), Some(2.0));
        assert!(build("lens-flare", &json!({"path": [[0, 0]], "times": [0, 1]})).is_err());
    }
}
