//! Backgrounds: mesh gradients, aurora, grain, grids, dots, light leaks,
//! bokeh, starfields and plain gradients.
//!
//! Smooth fields (mesh, aurora) are computed at low resolution as images and
//! upsampled by the renderer, which is exact enough for soft colour and costs
//! a few microseconds per frame. Everything else is vector.

use super::style::*;
use super::{Component, Ctx, Prepare};
use crate::dl::{Blend, Op, Paint, Path, Rgba, IDENTITY};
use anikit_core::noise::Noise;
use serde::Deserialize;
use std::sync::{Arc, Mutex};

fn colors_or(v: &[Color], fallback: &[Rgba]) -> Vec<Rgba> {
    if v.is_empty() {
        fallback.to_vec()
    } else {
        v.iter().map(|c| c.0).collect()
    }
}

fn lin(c: f32) -> f32 {
    c * c
}

/// A low-resolution field in (approximately) linear light, 3 floats per pixel.
pub struct Field {
    key: u64,
    w: u32,
    h: u32,
    lin: Vec<f32>,
}

impl Field {
    /// Evaluate `f(u, v)` (normalised coordinates) on a `w×h` grid.
    fn compute(key: u64, w: u32, h: u32, mut f: impl FnMut(f32, f32) -> [f32; 3]) -> Field {
        let mut lin = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            let v = (y as f32 + 0.5) / h as f32;
            for x in 0..w {
                lin.extend_from_slice(&f((x as f32 + 0.5) / w as f32, v));
            }
        }
        Field { key, w, h, lin }
    }
}

/// The last few keyframe fields of one component, shared by render threads.
#[derive(Default)]
pub struct FieldCache(Mutex<Vec<Arc<Field>>>);

impl FieldCache {
    fn get(&self, key: u64, make: impl FnOnce() -> Field) -> Arc<Field> {
        if let Some(f) = self.0.lock().unwrap().iter().find(|f| f.key == key) {
            return f.clone();
        }
        // Compute outside the lock; a rare duplicate computation is harmless.
        let f = Arc::new(make());
        let mut v = self.0.lock().unwrap();
        if v.len() >= 8 {
            v.remove(0);
        }
        v.push(f.clone());
        f
    }
}

/// Slow fields are computed at `rate` keyframes per second and blended per
/// frame into one image: a fraction of the field evaluations, one image draw.
fn keyframed(ctx: &Ctx, t: f32, rate: f32, field: impl Fn(f32) -> Arc<Field>) -> Vec<Op> {
    let rate = rate.clamp(1.0, 120.0);
    let x = t.max(0.0) * rate;
    let k0 = x.floor();
    // Blend weight quantised to 1/64: identical frames share an image.
    let f = ((x - k0) * 64.0).round() / 64.0;
    let a = field(k0 / rate);
    let b = if f > 0.0 { Some(field((k0 + 1.0) / rate)) } else { None };
    let key = fnv(&[b"blend", &a.key.to_le_bytes(), &b.as_ref().map_or(0, |b| b.key).to_le_bytes(), &f.to_bits().to_le_bytes()]);
    let (w, h) = (a.w, a.h);
    let id = ctx.assets.image(key, w, h, &mut || {
        let mut px = vec![255u8; (w * h * 4) as usize];
        for (i, p) in px.chunks_exact_mut(4).enumerate() {
            for k in 0..3 {
                let va = a.lin[i * 3 + k];
                let v = match &b {
                    Some(b) => va + (b.lin[i * 3 + k] - va) * f,
                    None => va,
                };
                // No dithering at field resolution: it would upscale into stripes.
                p[k] = (v.max(0.0).sqrt() * 255.0 + 0.5).min(255.0) as u8;
            }
        }
        px
    });
    vec![Op::Image { image: id, dst: [0.0, 0.0, ctx.width, ctx.height], opacity: 1.0, transform: IDENTITY }]
}

// ------------------------------------------------------------------ gradient

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Gradient {
    /// Two or more colours.
    pub colors: Vec<Color>,
    /// `"linear"` (default) or `"radial"`.
    pub kind: Option<String>,
    /// Angle in degrees for linear gradients (0 = left→right, 90 = top→bottom).
    pub angle: Option<f32>,
    /// Slowly rotate the angle (degrees per second).
    pub spin: f32,
}
impl Prepare for Gradient {}
impl Component for Gradient {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let cols = colors_or(&self.colors, &[ctx.theme.background, shade(ctx.theme.accent, -0.75)]);
        let n = (cols.len() - 1).max(1) as f32;
        let stops: Vec<(f32, Rgba)> = cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect();
        let (w, h) = (ctx.width, ctx.height);
        let paint = if self.kind.as_deref() == Some("radial") {
            radial(w * 0.5, h * 0.5, (w * w + h * h).sqrt() * 0.5, stops)
        } else {
            let a = (self.angle.unwrap_or(90.0) + self.spin * t).to_radians();
            let (s, c) = a.sin_cos();
            let half = 0.5 * (w * c.abs() + h * s.abs());
            Paint::Linear { p0: [w * 0.5 - c * half, h * 0.5 - s * half], p1: [w * 0.5 + c * half, h * 0.5 + s * half], stops }
        };
        vec![rect(0.0, 0.0, w, h, 0.0, paint)]
    }
}

// ------------------------------------------------------------------ mesh

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MeshGradient {
    /// Blob colours (3–6 look best). Defaults to a deep violet/blue/magenta set.
    pub colors: Vec<Color>,
    /// Base colour behind the blobs.
    pub base: Option<Color>,
    /// Motion speed multiplier.
    pub speed: f32,
    /// Domain warp amount (0 = round blobs, 1 = swirly).
    pub warp: f32,
    pub seed: u32,
    /// Field resolution divisor (higher = cheaper, softer).
    pub detail: f32,
    /// Colour strength (1 = the colours as given; lower sits better behind text).
    pub intensity: f32,
    /// Field keyframes per second (blended in between).
    pub rate: f32,
    #[serde(skip)]
    cache: FieldCache,
}
impl Default for MeshGradient {
    fn default() -> Self {
        MeshGradient { colors: Vec::new(), base: None, speed: 1.0, warp: 0.6, seed: 7, detail: 1.0, intensity: 0.75, rate: 12.0, cache: FieldCache::default() }
    }
}
impl Prepare for MeshGradient {}
impl Component for MeshGradient {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        keyframed(ctx, t, self.rate, |tt| self.field_at(tt, ctx))
    }
}

impl MeshGradient {
    fn field_at(&self, t: f32, ctx: &Ctx) -> Arc<Field> {
        let cols = colors_or(&self.colors, &[hx3("#5b2bff"), hx3("#1f6bff"), hx3("#ff3d9a"), hx3("#00c2d1"), hx3("#2a0f6e")]);
        let base = self.base.map(|c| c.0).unwrap_or(ctx.theme.background);
        let (fw, fh) = field_size(ctx, 12.0 * self.detail);
        let key = fnv(&[b"mesh", &t.to_bits().to_le_bytes(), &self.seed.to_le_bytes(), &self.speed.to_bits().to_le_bytes(), &self.warp.to_bits().to_le_bytes(), &self.intensity.to_bits().to_le_bytes(), &fw.to_le_bytes(), &fh.to_le_bytes(), &colors_key(&cols), &colors_key(&[base])]);
        let noise = Noise::new(self.seed);
        let aspect = ctx.width / ctx.height.max(1.0);
        let s = t * 0.07 * self.speed;
        // Blob centres follow slow noise orbits.
        let centres: Vec<(f32, f32)> = (0..cols.len())
            .map(|i| {
                let k = i as f64 * 17.3;
                // Anchors spread over the frame (a ring plus the centre), drifting.
                let n = cols.len().max(2) as f32;
                let (ax, ay) = if i + 1 == cols.len() && cols.len() > 3 {
                    (0.5, 0.5)
                } else {
                    let a = i as f32 / (n - if cols.len() > 3 { 1.0 } else { 0.0 }) * std::f32::consts::TAU + self.seed as f32;
                    (0.5 + 0.36 * a.cos(), 0.5 + 0.34 * a.sin())
                };
                let x = ax + 0.22 * noise.noise2(k, s as f64) as f32;
                let y = ay + 0.22 * noise.noise2(k + 5.1, s as f64 + 3.7) as f32;
                (x * aspect, y)
            })
            .collect();
        let k = self.intensity.clamp(0.0, 2.0);
        let lc: Vec<[f32; 3]> = cols.iter().map(|c| [lin(c.0[0]) * k, lin(c.0[1]) * k, lin(c.0[2]) * k]).collect();
        let lb = [lin(base.0[0]), lin(base.0[1]), lin(base.0[2])];
        let warp = self.warp;
        self.cache.get(key, || Field::compute(key, fw, fh, |u, v| {
            let (mut x, mut y) = (u * aspect, v);
            let wx = noise.noise2((x * 1.6) as f64, (y * 1.6 + s * 2.0) as f64) as f32;
            let wy = noise.noise2((x * 1.6 + 9.2) as f64, (y * 1.6 - s * 2.0) as f64) as f32;
            x += wx * 0.25 * warp;
            y += wy * 0.25 * warp;
            let mut acc = [0.0f32; 3];
            let mut wsum = 0.0;
            for (c, (cx, cy)) in lc.iter().zip(&centres) {
                let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
                let w = 1.0 / (d2 * 7.0 + 0.06).powi(3);
                wsum += w;
                for k in 0..3 {
                    acc[k] += c[k] * w;
                }
            }
            // The base colour joins as a weak constant-weight blob.
            let wb = 2.5;
            [0, 1, 2].map(|k| (acc[k] + lb[k] * wb) / (wsum + wb))
        }))
    }
}

fn hx3(s: &str) -> Rgba {
    parse_color(s).unwrap()
}

fn colors_key(c: &[Rgba]) -> Vec<u8> {
    c.iter().flat_map(|c| c.0.iter().flat_map(|v| v.to_bits().to_le_bytes())).collect()
}

fn field_size(ctx: &Ctx, div: f32) -> (u32, u32) {
    let div = div.max(2.0);
    (((ctx.width / div).ceil() as u32).clamp(8, 512), ((ctx.height / div).ceil() as u32).clamp(8, 512))
}

// ------------------------------------------------------------------ aurora

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Aurora {
    pub colors: Vec<Color>,
    /// Sky colour (top) and horizon colour (bottom).
    pub sky: Option<[Color; 2]>,
    pub speed: f32,
    /// Brightness of the ribbons.
    pub intensity: f32,
    pub seed: u32,
    /// Field keyframes per second (blended in between).
    pub rate: f32,
    #[serde(skip)]
    cache: FieldCache,
}
impl Default for Aurora {
    fn default() -> Self {
        Aurora { colors: Vec::new(), sky: None, speed: 1.0, intensity: 1.0, seed: 3, rate: 15.0, cache: FieldCache::default() }
    }
}
impl Prepare for Aurora {}
impl Component for Aurora {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        keyframed(ctx, t, self.rate, |tt| self.field_at(tt, ctx))
    }
}

impl Aurora {
    fn field_at(&self, t: f32, ctx: &Ctx) -> Arc<Field> {
        let cols = colors_or(&self.colors, &[hx3("#2cffb0"), hx3("#21c7ff"), hx3("#8a5cff")]);
        let sky = self.sky.map(|s| [s[0].0, s[1].0]).unwrap_or([hx3("#02030a"), hx3("#081026")]);
        let (fw, fh) = field_size(ctx, 10.0);
        let key = fnv(&[b"aurora", &t.to_bits().to_le_bytes(), &self.seed.to_le_bytes(), &self.speed.to_bits().to_le_bytes(), &self.intensity.to_bits().to_le_bytes(), &fw.to_le_bytes(), &fh.to_le_bytes(), &colors_key(&cols), &colors_key(&sky)]);
        let noise = Noise::new(self.seed);
        let s = (t * 0.12 * self.speed) as f64;
        let lc: Vec<[f32; 3]> = cols.iter().map(|c| [lin(c.0[0]), lin(c.0[1]), lin(c.0[2])]).collect();
        let ls = sky.map(|c| [lin(c.0[0]), lin(c.0[1]), lin(c.0[2])]);
        let k = self.intensity;
        let aspect = (ctx.width / ctx.height.max(1.0)) as f64;
        self.cache.get(key, || Field::compute(key, fw, fh, |u, v| {
            let mut c = [0, 1, 2].map(|i| ls[0][i] + (ls[1][i] - ls[0][i]) * v);
            for (i, col) in lc.iter().enumerate() {
                let fi = i as f64 * 3.3;
                let x = u as f64 * aspect;
                // Ribbon centre line and thickness wander with noise.
                let centre = 0.48 + 0.09 * i as f32 + 0.14 * noise.noise2(x * 0.9 + fi, s + fi) as f32;
                let thick = 0.06 + 0.04 * noise.noise2(x * 1.7 - fi, s * 1.3) as f32;
                let d = v - centre;
                // Sharp lower edge, long upward rays.
                let band = if d > 0.0 { (-(d / (thick * 1.3)).powi(2)).exp() } else { (-(d / (thick * 2.6)).powi(2)).exp() };
                let rays = 0.55 + 0.45 * noise.noise2(x * 14.0 + fi, s * 3.0) as f32;
                let a = band * rays * k * (0.5 + 0.5 * noise.noise2(x * 0.6 - fi, s * 0.7 + 2.0) as f32).max(0.0) * 1.4;
                for j in 0..3 {
                    c[j] += col[j] * a;
                }
            }
            c
        }))
    }
}

// ------------------------------------------------------------------ grain

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Grain {
    /// Overlay strength (0.03–0.1 is subtle film grain).
    pub amount: f32,
    /// Grain size in pixels.
    pub size: f32,
    /// New grain pattern this many times per second (0 = static).
    pub rate: f32,
    pub seed: u32,
}
impl Default for Grain {
    fn default() -> Self {
        Grain { amount: 0.06, size: 1.0, rate: 24.0, seed: 1 }
    }
}
impl Prepare for Grain {}
impl Component for Grain {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        const TILE: u32 = 256;
        const VARIANTS: u32 = 6;
        let frame = if self.rate > 0.0 { (t * self.rate).floor() as u32 } else { 0 };
        let v = (self.seed.wrapping_add(frame)) % VARIANTS;
        let key = fnv(&[b"grain", &v.to_le_bytes(), &self.seed.to_le_bytes()]);
        let seed = self.seed;
        let id = ctx.assets.image(key, TILE, TILE, &mut || {
            let mut rng = Rng::new(((seed as u64) << 8) ^ v as u64 ^ 0x5eed);
            let mut px = vec![0u8; (TILE * TILE * 4) as usize];
            for p in px.chunks_mut(4) {
                // Roughly Gaussian (sum of 3 uniforms), mid-grey centred.
                let g = (rng.f() + rng.f() + rng.f()) / 3.0;
                let b = (g * 255.0) as u8;
                p.copy_from_slice(&[b, b, b, 255]);
            }
            px
        });
        let off = (hash01(frame.wrapping_mul(7919)) * TILE as f32).floor();
        let offy = (hash01(frame.wrapping_mul(104_729) ^ 99) * TILE as f32).floor();
        let s = self.size.max(0.25);
        let paint = Paint::Image { image: id, transform: [1.0 / s, 0.0, 0.0, 1.0 / s, off, offy], opacity: 1.0 };
        let mut out = Vec::new();
        push_layer(&mut out, self.amount.clamp(0.0, 1.0) * 2.0, Blend::Overlay, Vec::new());
        out.push(rect(0.0, 0.0, ctx.width, ctx.height, 0.0, paint));
        out.push(Op::PopLayer);
        out
    }
}

// ------------------------------------------------------------------ grid

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Grid {
    /// Cell size in pixels.
    pub cell: f32,
    pub color: Option<Color>,
    pub line_width: f32,
    /// Fade radius as a fraction of the box diagonal (0 = no fade).
    pub fade: f32,
    /// Floor grid receding to a horizon (synthwave / product-stage look).
    pub perspective: bool,
    /// Horizon height (0..1) for perspective mode.
    pub horizon: f32,
    /// Scroll speed in cells per second.
    pub speed: f32,
    /// Draw-on: the grid reveals from the centre over this many seconds.
    pub reveal: f32,
}
impl Default for Grid {
    fn default() -> Self {
        Grid { cell: 64.0, color: None, line_width: 1.0, fade: 0.55, perspective: false, horizon: 0.45, speed: 0.0, reveal: 0.0 }
    }
}
impl Prepare for Grid {}
impl Component for Grid {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(alpha(ctx.theme.text, if ctx.theme.dark { 0.09 } else { 0.08 }));
        let reveal = if self.reveal > 0.0 { ease_out(t / self.reveal) } else { 1.0 };
        let mut p = Path::new();
        let cell = self.cell.max(4.0);
        if self.perspective {
            let hy = h * self.horizon;
            let cx = w * 0.5;
            let n = (w / cell * 2.0) as i32;
            for i in -n..=n {
                let x = cx + i as f32 * cell * 1.5;
                p.move_to((cx as f64 + (x - cx) as f64 * 0.02, hy as f64));
                p.line_to((x as f64 + (x - cx) as f64 * 1.2, h as f64));
            }
            // Horizontal lines spaced by 1/z.
            let scroll = (t * self.speed).fract();
            for k in 0..40 {
                let z = 1.0 + (k as f32 - scroll) * 0.6;
                if z <= 0.2 {
                    continue;
                }
                let y = hy + (h - hy) * (1.0 / z).min(1.2) * 0.9;
                if y > h {
                    continue;
                }
                p.move_to((0.0, y as f64));
                p.line_to((w as f64, y as f64));
            }
            let paint = Paint::Linear { p0: [0.0, hy], p1: [0.0, h], stops: vec![(0.0, alpha(c, 0.0)), (0.5, alpha(c, 0.7 * reveal)), (1.0, alpha(c, reveal))] };
            return vec![stroke_op(p, paint, self.line_width, [0.0, 1.0])];
        }
        let off = (t * self.speed * cell) % cell;
        let mut x = -off;
        while x <= w + 0.5 {
            let xx = x.round() + 0.5;
            p.move_to((xx as f64, 0.0));
            p.line_to((xx as f64, h as f64));
            x += cell;
        }
        let mut y = -off;
        while y <= h + 0.5 {
            let yy = y.round() + 0.5;
            p.move_to((0.0, yy as f64));
            p.line_to((w as f64, yy as f64));
            y += cell;
        }
        let paint = fade_paint(c, w, h, self.fade, reveal);
        vec![Op::Stroke { path: p, paint, stroke: crate::dl::Stroke { width: self.line_width, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY }]
    }
}

fn fade_paint(c: Rgba, w: f32, h: f32, fade: f32, reveal: f32) -> Paint {
    if fade <= 0.0 && reveal >= 1.0 {
        return solid(c);
    }
    let diag = (w * w + h * h).sqrt() * 0.5;
    let r = if fade > 0.0 { diag * fade * 2.0 } else { diag * 1.5 };
    let r = r * reveal.max(0.001);
    radial(w * 0.5, h * 0.5, r, vec![(0.0, c), (0.55, alpha(c, 0.6)), (1.0, alpha(c, 0.0))])
}

// ------------------------------------------------------------------ dots

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dots {
    pub spacing: f32,
    pub radius: f32,
    pub color: Option<Color>,
    pub fade: f32,
    /// A slow wave of brightness travelling across the dots.
    pub wave: bool,
}
impl Default for Dots {
    fn default() -> Self {
        Dots { spacing: 28.0, radius: 1.3, color: None, fade: 0.6, wave: false }
    }
}
impl Prepare for Dots {}
impl Component for Dots {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(alpha(ctx.theme.text, 0.22));
        let s = self.spacing.max(4.0);
        let (nx, ny) = ((w / s) as i32 + 1, (h / s) as i32 + 1);
        let (ox, oy) = ((w - (nx - 1) as f32 * s) * 0.5, (h - (ny - 1) as f32 * s) * 0.5);
        let mut base = Path::new();
        let mut hot = Path::new();
        for j in 0..ny {
            for i in 0..nx {
                let (x, y) = (ox + i as f32 * s, oy + j as f32 * s);
                let mut r = self.radius;
                let mut lit = false;
                if self.wave {
                    let d = ((x - w * 0.5).powi(2) + (y - h * 0.5).powi(2)).sqrt() / s;
                    let k = (d * 0.35 - t * 2.2).sin().max(0.0).powi(6);
                    r *= 1.0 + k * 0.8;
                    lit = k > 0.3;
                }
                let circ = kurbo::Circle::new((x as f64, y as f64), r as f64);
                use kurbo::Shape;
                if lit {
                    hot.extend(circ.path_elements(0.1));
                } else {
                    base.extend(circ.path_elements(0.1));
                }
            }
        }
        let mut out = vec![fill(base, fade_paint(c, w, h, self.fade, 1.0))];
        if !hot.is_empty() {
            out.push(fill(hot, fade_paint(alpha(ctx.theme.accent, 0.9), w, h, self.fade, 1.0)));
        }
        out
    }
}

// ------------------------------------------------------------------ light leak

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightLeak {
    pub colors: Vec<Color>,
    pub intensity: f32,
    pub speed: f32,
    pub seed: u32,
}
impl Default for LightLeak {
    fn default() -> Self {
        LightLeak { colors: Vec::new(), intensity: 0.8, speed: 1.0, seed: 11 }
    }
}
impl Prepare for LightLeak {}
impl Component for LightLeak {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let cols = colors_or(&self.colors, &[hx3("#ff8a3d"), hx3("#ff3d6e"), hx3("#ffd27a")]);
        let n = Noise::new(self.seed);
        let (w, h) = (ctx.width, ctx.height);
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        let s = (t * 0.15 * self.speed) as f64;
        for (i, c) in cols.iter().enumerate() {
            let k = i as f64 * 7.7;
            // Leaks hug the edges, sweeping slowly.
            let edge = if i % 2 == 0 { -0.1 } else { 1.1 };
            let cx = w * (edge + 0.25 * n.noise2(k, s) as f32);
            let cy = h * (0.5 + 0.5 * n.noise2(k + 3.0, s * 0.8) as f32);
            let r = w.max(h) * (0.45 + 0.15 * n.noise2(k + 8.0, s * 1.2) as f32);
            let a = (self.intensity * (0.55 + 0.45 * n.noise2(k + 1.5, s * 2.0) as f32)).clamp(0.0, 1.0);
            out.push(rect(0.0, 0.0, w, h, 0.0, radial(cx, cy, r, vec![(0.0, alpha(*c, a)), (0.4, alpha(*c, a * 0.45)), (1.0, alpha(*c, 0.0))])));
        }
        out.push(Op::PopLayer);
        out
    }
}

// ------------------------------------------------------------------ bokeh

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bokeh {
    pub count: u32,
    pub colors: Vec<Color>,
    pub min_size: f32,
    pub max_size: f32,
    pub speed: f32,
    pub seed: u32,
}
impl Default for Bokeh {
    fn default() -> Self {
        Bokeh { count: 28, colors: Vec::new(), min_size: 30.0, max_size: 140.0, speed: 1.0, seed: 5 }
    }
}
impl Prepare for Bokeh {}
impl Component for Bokeh {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let cols = colors_or(&self.colors, &[ctx.theme.accent, ctx.theme.pal(1), ctx.theme.pal(2), hx3("#ffd9a0")]);
        let (w, h) = (ctx.width, ctx.height);
        let mut rng = Rng::new(self.seed as u64);
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        for i in 0..self.count {
            let (x0, y0) = (rng.f(), rng.f());
            let size = rng.range(self.min_size, self.max_size);
            let ph = rng.f() * std::f32::consts::TAU;
            let vx = rng.range(-8.0, 8.0) * self.speed;
            let vy = rng.range(-14.0, -3.0) * self.speed;
            let x = (x0 * (w + 2.0 * size) + vx * t).rem_euclid(w + 2.0 * size) - size;
            let y = (y0 * (h + 2.0 * size) + vy * t).rem_euclid(h + 2.0 * size) - size;
            let c = cols[i as usize % cols.len()];
            let a = 0.10 + 0.14 * (0.5 + 0.5 * (t * 0.6 + ph).sin()) * (1.0 - size / (self.max_size * 1.4));
            // Disc with a slightly brighter rim, like a real aperture.
            out.push(circle(x, y, size * 0.5, radial(x, y, size * 0.5, vec![(0.0, alpha(c, a * 0.7)), (0.82, alpha(c, a)), (0.93, alpha(c, a * 1.3)), (1.0, alpha(c, 0.0))])));
        }
        out.push(Op::PopLayer);
        out
    }
}

// ------------------------------------------------------------------ starfield

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Starfield {
    pub count: u32,
    pub color: Option<Color>,
    /// Fly through the stars (warp speed) instead of twinkling in place.
    pub fly: bool,
    pub speed: f32,
    pub seed: u32,
}
impl Default for Starfield {
    fn default() -> Self {
        Starfield { count: 600, color: None, fly: false, speed: 1.0, seed: 9 }
    }
}
impl Prepare for Starfield {}
impl Component for Starfield {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(Rgba([0.92, 0.94, 1.0, 1.0]));
        let mut rng = Rng::new(self.seed as u64);
        // Three brightness buckets → three fills (cheap).
        let mut paths = [Path::new(), Path::new(), Path::new()];
        let mut streaks = Path::new();
        use kurbo::Shape;
        for _ in 0..self.count {
            let (a, b, z0, ph, tw) = (rng.f(), rng.f(), rng.f(), rng.f() * std::f32::consts::TAU, rng.range(0.6, 2.5));
            if self.fly {
                let z = (z0 - t * 0.18 * self.speed).rem_euclid(1.0).max(0.02);
                let (dx, dy) = ((a - 0.5) * 2.0, (b - 0.5) * 2.0);
                let f = 0.12 / z;
                let (x, y) = (w * 0.5 + dx * w * f, h * 0.5 + dy * w * f);
                let z1 = (z + 0.02 * self.speed).min(1.0);
                let f1 = 0.12 / z1;
                let (x1, y1) = (w * 0.5 + dx * w * f1, h * 0.5 + dy * w * f1);
                if x < -20.0 || x > w + 20.0 || y < -20.0 || y > h + 20.0 {
                    continue;
                }
                streaks.move_to((x1 as f64, y1 as f64));
                streaks.line_to((x as f64, y as f64));
                let bucket = ((1.0 - z) * 3.0).min(2.0) as usize;
                paths[bucket].extend(kurbo::Circle::new((x as f64, y as f64), (0.6 + 1.6 * (1.0 - z)) as f64).path_elements(0.1));
            } else {
                let (x, y) = (a * w, b * h);
                let r = 0.4 + 1.3 * z0 * z0;
                let k = 0.5 + 0.5 * (t * tw + ph).sin();
                let bucket = ((z0 * 0.6 + k * 0.4) * 3.0).min(2.0) as usize;
                paths[bucket].extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.1));
            }
        }
        let mut out = Vec::new();
        if !streaks.is_empty() {
            out.push(stroke_op(streaks, solid(alpha(c, 0.35)), 1.2, [0.0, 1.0]));
        }
        for (i, p) in paths.into_iter().enumerate() {
            out.push(fill(p, solid(alpha(c, [0.35, 0.65, 1.0][i]))));
        }
        out
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
        for (name, p) in [
            ("mesh-gradient", json!({})),
            ("aurora", json!({})),
            ("grid", json!({"reveal": 1.0})),
            ("dots", json!({"wave": true})),
            ("light-leak", json!({})),
            ("bokeh", json!({})),
            ("starfield", json!({"fly": true})),
            ("gradient", json!({"colors": ["#0b1020", "#2a1060", "#c03a8a"], "angle": 30})),
        ] {
            let c = build(name, &p).unwrap();
            let mut ops = c.eval(1.0, 960.0, 540.0, 30.0, &th, a);
            if name == "aurora" || name == "mesh-gradient" {
                ops.extend(build("grain", &json!({})).unwrap().eval(1.0, 960.0, 540.0, 30.0, &th, a));
            }
            preview::validate(&ops).unwrap();
            let px = preview::snapshot(&format!("bg-{name}"), 960, 540, th.background, ops, a);
            assert!(px.chunks(4).any(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30), "{name} drew nothing");
        }
    }
}
