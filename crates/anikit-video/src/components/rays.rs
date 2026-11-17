//! Light: `god-rays` (volumetric shafts from a point, with dust in the
//! light) and `lens-flare` (hot core, diffraction starburst, streak, halo
//! and chromatic ghosts along the optical axis through the frame centre).
//!
//! God rays are a fan of hundreds of hair-thin slices, each lit by a noise
//! density over angle (and over distance, for haze), so shafts of every
//! width emerge, drift and breathe without any pixel work. Slices share
//! edges inside one additive layer, so there is no overdraw and no seams.
//! The flare is a few dozen gradient-filled vector ops in one additive layer.

use super::fxkit::{self, bloom, chroma_ring, gauss, hot, kelvin, light, needle, polygon};
use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, FillRule, Op, Paint, Path, Rgba, IDENTITY};
use anikit_core::noise::Noise;
use serde::Deserialize;
use std::f32::consts::{PI, TAU};

// ------------------------------------------------------------------ god rays

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GodRays {
    /// Light source as fractions of the box (may be outside 0..1).
    pub at: [f32; 2],
    /// Light colour (default: soft daylight, a touch towards the theme accent).
    pub color: Option<Color>,
    /// Colour far from the source (default: `color`); a second colour gives
    /// the light a temperature gradient (warm core, cool haze).
    pub far_color: Option<Color>,
    /// How many distinct shafts across the fan (roughly).
    pub count: u32,
    /// Direction of the fan in degrees (90 = straight down).
    pub angle: f32,
    /// Fan width in degrees (360 = all around).
    pub spread: f32,
    /// Beam length as a fraction of the box diagonal.
    pub length: f32,
    /// Overall brightness.
    pub intensity: f32,
    /// How distinct the shafts are, 0 (an even glow) .. 1 (hard beams with dark gaps).
    pub contrast: f32,
    /// Ambient bloom around the source and haze along the shafts, 0..1.
    pub haze: f32,
    /// Rotation of the fan in degrees per second.
    pub spin: f32,
    /// How fast the shafts shift and breathe (1 = calm).
    pub speed: f32,
    /// Dust motes floating in the beams (0 = none).
    pub dust: u32,
    /// Depth of field on the dust: near motes become soft bokeh discs (0 = all sharp).
    pub depth: f32,
    pub seed: u32,
}

impl Default for GodRays {
    fn default() -> Self {
        GodRays {
            at: [0.32, -0.08],
            color: None,
            far_color: None,
            count: 26,
            angle: 62.0,
            spread: 70.0,
            length: 1.15,
            intensity: 1.0,
            contrast: 0.7,
            haze: 0.5,
            spin: 1.0,
            speed: 1.0,
            dust: 260,
            depth: 1.0,
            seed: 4,
        }
    }
}

impl Prepare for GodRays {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.count > 400 {
            return Err(Error("count must be at most 400".into()));
        }
        if self.dust > 3000 {
            return Err(Error("dust must be at most 3000".into()));
        }
        self.spread = self.spread.clamp(1.0, 360.0);
        self.contrast = self.contrast.clamp(0.0, 1.0);
        self.haze = self.haze.clamp(0.0, 1.0);
        self.depth = self.depth.clamp(0.0, 2.0);
        Ok(())
    }
}

impl GodRays {
    /// Shaft density 0..1 at fan coordinate `u` (-0.5..0.5) and time `t`.
    fn density(&self, n: &Noise, u: f32, t: f32) -> f32 {
        let f = self.count.max(1) as f64;
        let (u, z) = (u as f64, (t * self.speed * 0.12) as f64);
        // Broad shafts, fine shafts and hair-thin striations.
        let v = 0.55 * n.noise2(u * f * 0.5, z) + 0.3 * n.noise2(u * f * 1.3 + 17.0, z * 1.6) + 0.15 * n.noise2(u * f * 3.7 + 41.0, z * 2.3);
        let v = (0.5 + 0.62 * v as f32).clamp(0.0, 1.0);
        let c = self.contrast;
        let shaped = fxkit::smooth((v - 0.5 * c + 0.05) / (1.0 - 0.8 * c).max(0.15));
        let d = lerp(v, shaped, c).powf(1.0 + 1.6 * c);
        // Broad bright and dark regions across the fan.
        let env = 0.3 + 0.7 * fxkit::smooth(0.5 + 0.9 * n.noise2(u * f * 0.11 + 5.0, z * 0.6) as f32);
        // Soft fan edges.
        let edge = if self.spread >= 359.0 { 1.0 } else { fxkit::smooth((0.5 - u.abs() as f32) / 0.16) };
        d * env * edge
    }

    fn fan_angle(&self, t: f32) -> f32 {
        (self.angle + self.spin * t + 2.0 * (t * 0.13 * self.speed).sin()).to_radians()
    }
}

impl Component for GodRays {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let c = self.color.map(|c| c.0).unwrap_or_else(|| fxkit::light_tint(ctx.theme, 4600.0, 0.08));
        let cf = self.far_color.map(|c| c.0).unwrap_or(c);
        let (sx, sy) = (self.at[0] * w, self.at[1] * h);
        let diag = (w * w + h * h).sqrt();
        let len = self.length * diag;
        let n = Noise::new(self.seed);
        let nh = Noise::new(self.seed.wrapping_add(77));
        let fan = self.fan_angle(t);
        let spread = self.spread.to_radians();
        let ink = self.intensity;
        let mut out = Vec::new();
        fxkit::push_light(&mut out, 1.0, true);

        // Haze: a broad bloom around the source, wider than the beams.
        if self.haze > 0.0 {
            let br = diag * (0.35 + 0.3 * self.haze);
            out.push(light(sx, sy, br, bloom(c, 1.3 * ink * self.haze, 0.16)));
        }

        // Slices: each at most ~5 px wide at its far end.
        let slice = (5.0 * k / len).atan().max(1e-4);
        let ns = ((spread / slice).ceil() as usize).clamp(48, 1400);
        let da = spread / ns as f32;
        let a0 = fan - spread * 0.5;
        let warm = hot(c, 0.35);
        for i in 0..ns {
            let u = (i as f32 + 0.5) / ns as f32 - 0.5;
            let d = self.density(&n, u, t);
            if d < 0.004 {
                continue;
            }
            // Shaft length and haze patches vary with noise too.
            let z = (t * self.speed * 0.08) as f64;
            let ud = (u * self.count.max(1) as f32) as f64;
            let lk = 0.62 + 0.38 * (0.5 + 0.5 * nh.noise2(ud * 0.35, z) as f32);
            let h1 = 0.7 + 0.6 * (0.5 + 0.5 * nh.noise2(ud * 0.9 + 9.0, z * 1.3 + 3.0) as f32);
            let h2 = 0.6 + 0.8 * (0.5 + 0.5 * nh.noise2(ud * 0.7 + 23.0, z * 1.1 + 7.0) as f32);
            let l = len * lk;
            let (aa, ab) = (a0 + i as f32 * da, a0 + (i + 1) as f32 * da);
            let am = (aa + ab) * 0.5;
            let mut p = Path::new();
            p.move_to((sx as f64, sy as f64));
            // Overshoot the far edge a little so the chord stays outside the fade.
            let lf = l / (da * 0.5).cos();
            p.line_to(((sx + aa.cos() * lf) as f64, (sy + aa.sin() * lf) as f64));
            p.line_to(((sx + ab.cos() * lf) as f64, (sy + ab.sin() * lf) as f64));
            p.close_path();
            let a = 0.75 * ink * d;
            let stops = vec![
                (0.0, alpha(warm, a)),
                (0.05, alpha(mix(warm, c, 0.5), a * 0.92)),
                (0.18, alpha(c, a * 0.62 * h1)),
                (0.4, alpha(mix(c, cf, 0.5), a * 0.32 * h2)),
                (0.7, alpha(cf, a * 0.11 * h1)),
                (1.0, alpha(cf, 0.0)),
            ];
            let (dx, dy) = (am.cos(), am.sin());
            out.push(Op::Fill { path: p, paint: Paint::Linear { p0: [sx, sy], p1: [sx + dx * l, sy + dy * l], stops }, rule: FillRule::NonZero, transform: IDENTITY });
        }

        // Hot source.
        out.push(light(sx, sy, diag * 0.12, bloom(hot(c, 0.6), 0.9 * ink, 0.08)));
        out.push(Op::PopLayer);

        if self.dust > 0 {
            self.dust_ops(&mut out, t, ctx, &n, c, (sx, sy), fan, spread, len);
        }
        out
    }
}

impl GodRays {
    /// Dust motes: sharp specks far away, soft bokeh discs up close, each
    /// as bright as the shaft it floats in.
    fn dust_ops(&self, out: &mut Vec<Op>, t: f32, ctx: &Ctx, n: &Noise, c: Rgba, s: (f32, f32), fan: f32, spread: f32, len: f32) {
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let drift = Noise::new(self.seed ^ 0x5eed);
        let mut rng = Rng::new(self.seed as u64 ^ 0xd05e);
        const LV: usize = 6;
        let mut sharp: Vec<Path> = (0..LV).map(|_| Path::new()).collect();
        let mut soft = Vec::new();
        let mut glints = Vec::new();
        for i in 0..self.dust {
            let (a, b, z, ph) = (rng.f(), rng.f(), rng.f(), rng.f() * TAU);
            let z = z * z; // most motes are far
            let sp = 0.35 + 0.65 * z; // parallax: near motes move faster
            let ts = (t * 0.06 * self.speed) as f64;
            let fi = i as f64 * 1.731;
            let x = (a * (w + 80.0) + t * 7.0 * k * sp + drift.noise2(fi, ts) as f32 * 90.0 * k * sp).rem_euclid(w + 80.0) - 40.0;
            let y = (b * (h + 80.0) + t * 3.0 * k * sp + drift.noise2(fi + 50.0, ts) as f32 * 70.0 * k * sp).rem_euclid(h + 80.0) - 40.0;
            let (dx, dy) = (x - s.0, y - s.1);
            let d = (dx * dx + dy * dy).sqrt();
            let mut da = (dy.atan2(dx) - fan).rem_euclid(TAU);
            if da > PI {
                da -= TAU;
            }
            let u = da / spread;
            if u.abs() > 0.5 || d > len {
                continue;
            }
            let lit = (0.25 + self.density(n, u, t)) * (1.0 - d / len).powf(1.1) * self.intensity * 1.4;
            let tw = 0.55 + 0.45 * (t * (0.7 + 1.3 * z) * self.speed + ph).sin();
            let lit = lit * tw;
            if lit < 0.03 {
                continue;
            }
            let near = z * self.depth;
            if near > 0.55 {
                // Out of focus: a soft disc with a faint rim, larger and dimmer.
                let r = (5.0 + 16.0 * (near - 0.55)) * k;
                soft.push((x, y, r, (lit * 0.55 / (1.0 + near)).min(0.5)));
            } else {
                let r = (0.9 + 2.2 * z) * k;
                let lv = ((lit * LV as f32) as usize).min(LV - 1);
                fxkit::dot(&mut sharp[lv], x, y, r);
                if lit > 0.7 && z > 0.25 {
                    glints.push((x, y, r, lit));
                }
            }
        }
        fxkit::push_light(out, 1.0, true);
        let mc = hot(c, 0.4);
        for (x, y, r, a) in soft {
            out.push(light(x, y, r, vec![(0.0, alpha(mc, a * 0.55)), (0.7, alpha(mc, a * 0.62)), (0.88, alpha(mc, a * 0.8)), (1.0, alpha(mc, 0.0))]));
        }
        for (x, y, r, a) in glints {
            out.push(light(x, y, r * 6.0, gauss(mc, 0.35 * a)));
        }
        for (lv, p) in sharp.into_iter().enumerate() {
            fxkit::fill_solid(out, p, alpha(mc, (lv as f32 + 0.6) / LV as f32));
        }
        out.push(Op::PopLayer);
    }
}

// ------------------------------------------------------------------ lens flare

const LENSES: &[&str] = &["spherical", "anamorphic", "vintage"];

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
    /// Lens character: `spherical` (starburst, polygon ghosts), `anamorphic`
    /// (long streak, oval ghosts) or `vintage` (warm, soft, big halos).
    pub lens: String,
    /// Main tint of the core and halo (default: daylight, per lens).
    pub color: Option<Color>,
    /// Colour of the streak (default: per lens).
    pub streak_color: Option<Color>,
    /// Streak length as a fraction of the box width (0 = none; default per lens).
    pub streak: Option<f32>,
    pub intensity: f32,
    /// Number of ghosts along the axis (0 = none).
    pub ghosts: u32,
    /// Diffraction spikes (aperture blades; 0 = none, default per lens).
    pub spikes: Option<u32>,
    /// Aperture blades: the ghosts' polygon shape (0 = round).
    pub blades: u32,
    /// Chromatic dispersion of ghosts and rings, 0..2.
    pub chroma: f32,
    /// Size of the halo ring around the light (fraction of the box height, 0 = none).
    pub halo: f32,
    /// Starburst rotation in degrees.
    pub rotation: f32,
    pub seed: u32,
}

impl Default for LensFlare {
    fn default() -> Self {
        LensFlare {
            at: [0.3, 0.3],
            path: Vec::new(),
            times: Vec::new(),
            duration: 4.0,
            lens: "spherical".into(),
            color: None,
            streak_color: None,
            streak: None,
            intensity: 1.0,
            ghosts: 8,
            spikes: None,
            blades: 6,
            chroma: 1.0,
            halo: 0.3,
            rotation: 12.0,
            seed: 2,
        }
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
        if self.spikes.unwrap_or(0) > 32 || self.blades > 16 {
            return Err(Error("spikes and blades must be at most 32 and 16".into()));
        }
        fxkit::choice("lens", &self.lens, LENSES)?;
        self.chroma = self.chroma.clamp(0.0, 2.0);
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

/// Ghost shapes.
#[derive(Clone, Copy)]
enum Ghost {
    /// Flat aperture polygon with a brighter rim.
    Iris,
    /// Soft disc.
    Disc,
    /// Thin ring.
    Ring,
}

impl Component for LensFlare {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let p = self.pos(t);
        let (lx, ly) = (p[0] * w, p[1] * h);
        let (cx, cy) = (w * 0.5, h * 0.5);
        let lens = self.lens.as_str();
        let (ana, vintage) = (lens == "anamorphic", lens == "vintage");
        let c = self.color.map(|c| c.0).unwrap_or_else(|| if vintage { kelvin(3300.0) } else { kelvin(5600.0) });
        let sc = self.streak_color.map(|c| c.0).unwrap_or_else(|| if ana { Rgba([0.35, 0.62, 1.0, 1.0]) } else { mix(c, kelvin(9000.0), 0.5) });
        let streak = self.streak.unwrap_or(if ana { 1.0 } else if vintage { 0.0 } else { 0.45 });
        let spikes = self.spikes.unwrap_or(if ana { 0 } else if vintage { 4 } else { self.blades.max(2) });
        let u = h;
        // Fades out as the light leaves the frame; ghosts strengthen as the
        // light moves off-axis, the core as it nears the centre.
        let outside = ((lx.clamp(0.0, w) - lx).abs().max((ly.clamp(0.0, h) - ly).abs())) / (u * 0.25);
        let vis = 1.0 - fxkit::smooth(outside);
        if vis <= 0.001 {
            return Vec::new();
        }
        let (vx, vy) = (cx - lx, cy - ly);
        let off = clamp01((vx * vx + vy * vy).sqrt() / (0.5 * (w * w + h * h).sqrt()));
        // Gentle flicker, like real light through glass (smooth at full rate).
        let fl = 0.95 + 0.05 * ((t * 5.3).sin() * 0.6 + (t * 8.9 + 1.0).sin() * 0.4);
        let k = self.intensity * fl * vis;
        let kc = k * (1.0 - 0.25 * off);
        let mut out = Vec::new();
        fxkit::push_light(&mut out, 1.0, true);

        // Veiling glare: lifts the whole frame a touch around the light.
        out.push(light(lx, ly, u * 1.1, bloom(c, 0.16 * kc, 0.06)));
        // Bloom and hot core.
        out.push(light(lx, ly, u * 0.5, bloom(c, 1.0 * kc, 0.06)));
        out.push(light(lx, ly, u * 0.08, gauss(hot(c, 0.7), (1.2 * kc).min(1.0))));
        out.push(light(lx, ly, u * 0.018, gauss(Rgba([1.0; 4]), kc.min(1.0))));

        // Starburst: a few long blade spikes with chromatic edges, plus fine hair.
        if spikes > 0 {
            let n = if spikes % 2 == 0 { spikes / 2 } else { spikes } as usize;
            let rot = self.rotation.to_radians();
            let sl = u * if vintage { 0.22 } else { 0.48 };
            let sw = u * 0.006;
            let mut rng = Rng::new(self.seed as u64 * 5 + 1);
            for i in 0..n {
                let a = rot + i as f32 * PI / n as f32;
                let lk = 0.85 + 0.3 * rng.f();
                let full = if spikes % 2 == 1 { 1.0 } else { 1.0 };
                let _ = full;
                out.push(needle(lx, ly, a, sl * lk * 1.08, sw * 3.2, Rgba([1.0, 0.45, 0.3, 1.0]), 0.1 * kc * self.chroma.min(1.0)));
                out.push(needle(lx, ly, a, sl * lk * 0.92, sw * 2.6, Rgba([0.3, 0.55, 1.0, 1.0]), 0.1 * kc * self.chroma.min(1.0)));
                out.push(needle(lx, ly, a, sl * lk, sw * 1.6, hot(c, 0.5), 0.55 * kc));
                out.push(needle(lx, ly, a, sl * lk * 0.45, sw * 0.7, Rgba([1.0; 4]), 0.6 * kc));
            }
            // Fine diffraction hair: many faint thin needles of random length.
            let hair = if vintage { 0 } else { 28 };
            for _ in 0..hair {
                let a = rng.f() * PI;
                out.push(needle(lx, ly, a, sl * rng.range(0.25, 0.75), sw * 0.6, hot(c, 0.4), rng.range(0.05, 0.16) * kc));
            }
        }

        // Halo ring around the light, dispersed red outside, blue inside.
        if self.halo > 0.0 {
            let hr = u * self.halo * if vintage { 1.4 } else { 1.0 };
            let hw = u * if vintage { 0.03 } else { 0.012 };
            let a = 0.07 * k * (0.5 + 0.5 * off) * if vintage { 1.5 } else { 1.0 };
            chroma_ring(&mut out, lx, ly, hr, hw, u * 0.006 * self.chroma, a);
        }

        // Anamorphic streak: a soft wide band, a tighter band and a hot thread.
        if streak > 0.0 {
            let sl = w * streak * 0.5;
            let thick = if ana { 1.0 } else { 0.6 };
            out.push(fxkit::ellipse(lx, ly, sl * 1.1, u * 0.03 * thick, 0.0, bloom(sc, 0.32 * kc, 0.12)));
            out.push(fxkit::ellipse(lx, ly, sl * 0.8, u * 0.008 * thick, 0.0, bloom(hot(sc, 0.3), 0.55 * kc, 0.1)));
            out.push(needle(lx, ly, 0.0, sl * 0.75, u * 0.0035, hot(sc, 0.8), 0.9 * kc.min(1.0)));
        }

        // Ghosts along the axis from the light through the centre and beyond,
        // each split into R, G and B copies at slightly different positions.
        let mut rng = Rng::new(self.seed as u64 * 17 + 5);
        let blades = self.blades as usize;
        for i in 0..self.ghosts {
            // Position along the axis: 0 = the light, 1 = the centre, 2 = mirrored.
            let s = 0.25 + 1.95 * (i as f32 + 0.15 + rng.f() * 0.7) / self.ghosts.max(1) as f32;
            let r = u * rng.range(0.012, 0.09) * if vintage { 1.5 } else { 1.0 } * (0.6 + 0.6 * (s - 1.0).abs());
            let kind = match (i as usize + self.seed as usize) % 5 {
                0 | 3 => Ghost::Iris,
                1 => Ghost::Ring,
                _ => Ghost::Disc,
            };
            let tint = Rgba([rng.range(0.6, 1.0), rng.range(0.7, 1.0), rng.range(0.6, 1.0), 1.0]);
            let tint = mix(tint, c, 0.35);
            let a = rng.range(0.1, 0.26) * k * (0.35 + 1.3 * off) * (1.0 - r / (u * 0.25)).max(0.2);
            let rot = rng.f() * TAU;
            let disp = 0.035 * self.chroma;
            for (j, ch) in [Rgba([1.0, 0.25, 0.2, 1.0]), Rgba([0.25, 1.0, 0.35, 1.0]), Rgba([0.25, 0.4, 1.0, 1.0])].iter().enumerate() {
                let sj = s * (1.0 + (j as f32 - 1.0) * disp);
                let (gx, gy) = (lx + vx * sj, ly + vy * sj);
                let col = Rgba([ch.0[0] * tint.0[0], ch.0[1] * tint.0[1], ch.0[2] * tint.0[2], 1.0]);
                let col = if self.chroma <= 0.0 { tint } else { col };
                let ga = a * if self.chroma <= 0.0 { 0.45 } else { 1.0 };
                let rx = r * if ana { 1.6 } else { 1.0 };
                let ry = r * if ana { 0.75 } else { 1.0 };
                match kind {
                    Ghost::Iris if blades >= 3 && !ana => {
                        let mut pp = Path::new();
                        polygon(&mut pp, 0.0, 0.0, 1.0, blades, rot);
                        out.push(Op::Fill {
                            path: pp,
                            paint: radial(0.0, 0.0, 1.0, vec![(0.0, alpha(col, ga * 0.55)), (0.7, alpha(col, ga * 0.7)), (0.92, alpha(col, ga)), (1.0, alpha(col, ga * 0.6))]),
                            rule: FillRule::NonZero,
                            transform: [rx, 0.0, 0.0, ry, gx, gy],
                        });
                    }
                    Ghost::Iris | Ghost::Disc => out.push(fxkit::ellipse(gx, gy, rx, ry, 0.0, vec![(0.0, alpha(col, ga * 0.7)), (0.6, alpha(col, ga * 0.75)), (0.85, alpha(col, ga * 0.9)), (0.97, alpha(col, ga * 0.4)), (1.0, alpha(col, 0.0))])),
                    Ghost::Ring => {
                        let rr = r * 1.6;
                        out.push(fxkit::ellipse(gx, gy, rr * rx / r, rr * ry / r, 0.0, vec![(0.0, alpha(col, 0.0)), (0.78, alpha(col, ga * 0.1)), (0.92, alpha(col, ga * 0.9)), (1.0, alpha(col, 0.0))]));
                    }
                }
            }
        }
        // A broad, faint secondary glow at the mirrored position.
        let (mx, my) = (lx + vx * 2.0, ly + vy * 2.0);
        out.push(light(mx, my, u * 0.22, bloom(mix(c, sc, 0.5), 0.05 * k * off, 0.25)));
        out.push(Op::PopLayer);
        let _ = Blend::Add;
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
            ("god-rays-top", json!({"at": [0.5, -0.1], "angle": 90, "spread": 100, "color": "#bfe0ff", "contrast": 0.9, "count": 12}), 3.0),
            ("lens-flare", json!({}), 0.5),
            ("lens-flare-ana", json!({"lens": "anamorphic"}), 0.5),
            ("lens-flare-path", json!({"path": [[0.15, 0.25], [0.85, 0.35]], "times": [0, 3], "lens": "vintage"}), 1.5),
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
        assert!(build("lens-flare", &json!({"lens": "anamorphc"})).err().unwrap().0.contains("anamorphic"));
        // Off-frame light draws nothing.
        let c = build("lens-flare", &json!({"at": [2.0, 0.5]})).unwrap();
        assert!(c.eval(0.0, 960.0, 540.0, 30.0, &th, a).is_empty());
    }
}
