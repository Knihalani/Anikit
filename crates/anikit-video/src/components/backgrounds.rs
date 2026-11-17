//! Backgrounds: mesh gradients, aurora, grain, grids, dots, light leaks,
//! bokeh, starfields and plain gradients.
//!
//! Pixel fields (aurora, mesh, gradient) are computed in linear light as
//! floats, upsampled with a cubic filter and quantised once with an ordered
//! dither, so they stay smooth at a 100 % crop with no banding. The aurora is
//! recomputed every frame (it has fine structure that must move smoothly);
//! the mesh changes slowly, so it is computed at keyframes and cross-faded
//! by the renderer (one new image per keyframe, smooth motion every frame).
//! Everything else is vector, with motion evaluated at the exact frame time.
//!
//! Defaults are neutral and follow the theme (`ctx.theme.accent`,
//! `background`, `text`); every look can be pushed elsewhere with params.

use super::bgfx::{self, Lin};
use super::style::*;
use super::{Component, Ctx, Prepare};
use crate::dl::{Blend, Op, Paint, Path, Rgba, IDENTITY};
use serde::Deserialize;
use std::f32::consts::TAU;

/// `true` renders the ground-truth version of every effect, with none of the
/// speed shortcuts below. Off for now; a reference mode can switch it on to
/// measure what each shortcut costs in quality. The shortcuts it disables:
/// - mesh-gradient: field at ⅓ resolution at keyframes, interpolated per
///   frame in linear light (exact: every pixel of every frame);
/// - aurora: field at ¾ × ⅕ resolution (exact: full resolution);
/// - gradient: field at ½ resolution (exact: full resolution);
///
/// Reduced fields are upsampled to the box in linear light and dithered at
/// full resolution (see `field_op`), so the dither matches the exact path's.
/// - stars, starfield, dots: alpha quantised into a few fills (exact: 256 levels);
/// - grain: one baked tile moved to a random offset per update (exact: fresh noise every update).
fn exact() -> bool {
    // qbench reference renders (`components::reference()`).
    super::style::reference()
}

/// Alpha levels for bucketed fills.
fn levels(fast: usize) -> usize {
    if exact() {
        256
    } else {
        fast
    }
}

fn colors_or(v: &[Color], fallback: &[Rgba]) -> Vec<Rgba> {
    if v.is_empty() {
        fallback.to_vec()
    } else {
        v.iter().map(|c| c.0).collect()
    }
}

fn hx3(s: &str) -> Rgba {
    parse_color(s).unwrap()
}

fn colors_key(c: &[Rgba]) -> Vec<u8> {
    c.iter().flat_map(|c| c.0.iter().flat_map(|v| v.to_bits().to_le_bytes())).collect()
}

fn f32s_key(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect()
}

/// A full-box opaque image from a linear field (quantised with dither).
/// Fields computed below the box resolution are upsampled (Catmull-Rom, in
/// linear light) to it before quantising, so the dither is the same per
/// output pixel as a full-resolution field's and the renderer blits it 1:1
/// rather than re-filtering (and re-dithering) a small image.
fn field_op(ctx: &Ctx, key: u64, make: impl FnOnce() -> Lin, size: (usize, usize), opacity: f32) -> Op {
    let full = (ctx.width.ceil().clamp(16.0, 4096.0) as usize, ctx.height.ceil().clamp(16.0, 4096.0) as usize);
    let (w, h) = if size.0 <= full.0 && size.1 <= full.1 { full } else { size };
    let mut make = Some(make);
    let id = ctx.assets.image(key, w as u32, h as u32, &mut || {
        make.take()
            .map(|m| {
                let l = m();
                if (l.w, l.h) == (w, h) {
                    l.to_rgba8()
                } else {
                    l.upsample_rgba8(w, h)
                }
            })
            .unwrap_or_else(|| vec![0; w * h * 4])
    });
    Op::Image { image: id, dst: [0.0, 0.0, w as f32 * ctx.width / full.0 as f32, h as f32 * ctx.height / full.1 as f32], opacity, transform: IDENTITY }
}

/// Image size for a field at `scale` of the box (clamped).
fn image_size(ctx: &Ctx, sx: f32, sy: f32) -> (usize, usize) {
    let (sx, sy) = if exact() { (1.0, 1.0) } else { (sx, sy) };
    (((ctx.width * sx).ceil() as usize).clamp(16, 2048), ((ctx.height * sy).ceil() as usize).clamp(16, 2048))
}

type KeyframeCache = std::sync::Mutex<Vec<(u64, std::sync::Arc<Lin>)>>;

/// Recently made keyframe fields, shared by the frames being evaluated in parallel.
fn keyframe_cache() -> &'static KeyframeCache {
    static C: std::sync::OnceLock<KeyframeCache> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

/// The field for keyframe `key`, made once and kept while it is recent.
fn keyframe(key: u64, make: impl FnOnce() -> Lin) -> std::sync::Arc<Lin> {
    if let Some(l) = keyframe_cache().lock().unwrap().iter().find(|e| e.0 == key).map(|e| e.1.clone()) {
        return l;
    }
    // Made outside the lock (a rare duplicate is harmless).
    let l = std::sync::Arc::new(make());
    let mut c = keyframe_cache().lock().unwrap();
    if !c.iter().any(|e| e.0 == key) {
        if c.len() >= 16 {
            c.remove(0);
        }
        c.push((key, l.clone()));
    }
    l
}

/// Slow fields: computed at `rate` keyframes per second (at the field's own
/// resolution) and interpolated per frame in linear light, then upsampled
/// and dithered at full resolution like a field made for that frame, so
/// motion is continuous and the dither never beats between keyframes.
fn keyframed(ctx: &Ctx, t: f32, rate: f32, key: impl Fn(f32) -> u64, make: impl Fn(f32) -> Lin, size: (usize, usize)) -> Vec<Op> {
    if exact() {
        return vec![field_op(ctx, key(t), || make(t), size, 1.0)];
    }
    let rate = rate.clamp(0.5, 60.0);
    let x = t.max(0.0) * rate;
    let k0 = x.floor();
    let f = x - k0;
    let t0 = k0 / rate;
    // Blend weight quantised to 1/256 (frames inside one step share an image).
    let f = if draft() { 0.0 } else { (f * 256.0).round() / 256.0 };
    let t1 = (k0 + 1.0) / rate;
    let (a, b) = (key(t0), key(t1));
    let fk = fnv(&[b"kf", &a.to_le_bytes(), &b.to_le_bytes(), &f.to_bits().to_le_bytes()]);
    vec![field_op(
        ctx,
        fk,
        || {
            let l0 = keyframe(a, || make(t0));
            if f <= 0.0 {
                return Lin { w: l0.w, h: l0.h, px: l0.px.clone() };
            }
            let l1 = keyframe(b, || make(t1));
            let px = l0.px.iter().zip(l1.px.iter()).map(|(p, q)| p + (q - p) * f).collect();
            Lin { w: l0.w, h: l0.h, px }
        },
        size,
        1.0,
    )]
}

// ------------------------------------------------------------------ gradient

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Gradient {
    /// Two or more colours (interpolated in OKLab: no muddy midpoints).
    pub colors: Vec<Color>,
    /// `"linear"` (default), `"radial"` or `"conic"`.
    pub kind: Option<String>,
    /// Angle in degrees for linear and conic gradients (0 = left→right, 90 = top→bottom).
    pub angle: Option<f32>,
    /// Slowly rotate the angle (degrees per second).
    pub spin: f32,
    /// Centre for radial / conic gradients, as fractions of the box (default `[0.5, 0.5]`).
    pub center: Option<[f32; 2]>,
    /// Radial extent multiplier (1 = the far corner).
    pub radius: Option<f32>,
    /// Ease the stops (0 = linear spacing, 1 = smooth S-curve between stops).
    pub smooth: Option<f32>,
}
impl Prepare for Gradient {}
impl Component for Gradient {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let fallback = if th.dark {
            vec![th.background, mix(th.background, th.surface2, 0.9)]
        } else {
            vec![th.surface, th.background]
        };
        let cols = colors_or(&self.colors, &fallback);
        let cols = if cols.len() == 1 { vec![cols[0], cols[0]] } else { cols };
        let kind = match self.kind.as_deref() {
            Some("radial") => 1u8,
            Some("conic") => 2,
            _ => 0,
        };
        let angle = self.angle.unwrap_or(90.0) + self.spin * step(t, AMBIENT_RATE);
        let (cx, cy) = self.center.map(|c| (c[0], c[1])).unwrap_or((0.5, 0.5));
        let rad = self.radius.unwrap_or(1.0).max(0.01);
        let sm = self.smooth.unwrap_or(0.35).clamp(0.0, 1.0);
        let lut = bgfx::ramp(&cols, 1024);
        let (w, h) = (ctx.width, ctx.height);
        // Gradients are smooth: half resolution plus dither is indistinguishable
        // from full resolution and four times cheaper.
        let size = image_size(ctx, 0.5, 0.5);
        let key = fnv(&[b"grad2", &colors_key(&cols), &[kind], &f32s_key(&[angle, cx, cy, rad, sm, w, h]), &(size.0 as u32).to_le_bytes(), &(size.1 as u32).to_le_bytes()]);
        let make = || {
            let (iw, ih) = size;
            let mut l = Lin::new(iw, ih);
            let (s, c) = angle.to_radians().sin_cos();
            let half = 0.5 * (w * c.abs() + h * s.abs());
            let rmax = {
                let (px, py) = (cx * w, cy * h);
                let dx = px.max(w - px);
                let dy = py.max(h - py);
                (dx * dx + dy * dy).sqrt() * rad
            };
            let n = cols.len() as f32 - 1.0;
            for y in 0..ih {
                let py = (y as f32 + 0.5) / ih as f32 * h;
                for x in 0..iw {
                    let px = (x as f32 + 0.5) / iw as f32 * w;
                    let p = match kind {
                        0 => ((px - w * 0.5) * c + (py - h * 0.5) * s) / (2.0 * half) + 0.5,
                        1 => ((px - cx * w).hypot(py - cy * h) / rmax).min(1.0),
                        _ => ((py - cy * h).atan2(px - cx * w) - angle.to_radians()).rem_euclid(TAU) / TAU,
                    };
                    // Optional ease between neighbouring stops.
                    let q = (p.clamp(0.0, 1.0) * n).min(n - 1e-4);
                    let k = q.floor();
                    let f = q - k;
                    let f = f + (f * f * (3.0 - 2.0 * f) - f) * sm;
                    let p = (k + f) / n;
                    let v = lut[((p * 1023.0) as usize).min(1023)];
                    l.px[(y * iw + x) * 3..][..3].copy_from_slice(&v);
                }
            }
            l
        };
        vec![field_op(ctx, key, make, size, 1.0)]
    }
}

// ------------------------------------------------------------------ mesh

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MeshGradient {
    /// Blob colours (3–6 look best). Defaults to an analogous set around the theme accent.
    pub colors: Vec<Color>,
    /// Base colour behind the blobs (default: the theme background).
    pub base: Option<Color>,
    /// Motion speed multiplier.
    pub speed: f32,
    /// Domain warp amount (0 = round blobs, 1 = swirly, 2 = marbled).
    pub warp: f32,
    pub seed: u32,
    /// Resolution multiplier (1 = default quality).
    pub detail: f32,
    /// Colour strength (1 = the colours as given; lower sits better behind text).
    pub intensity: f32,
    /// Blob size multiplier (bigger = broader, calmer colour fields).
    pub scale: f32,
    /// Silk sheen: glossy folds of light across the colours (0 = matte).
    pub sheen: f32,
    /// Darken the edges (0 = flat).
    pub vignette: f32,
    /// Keyframes per second, at least (interpolated per frame; 0 = automatic from `speed`).
    pub rate: f32,
    /// Kept for old specs; motion is always smooth outside draft mode.
    pub smooth: bool,
}
impl Default for MeshGradient {
    fn default() -> Self {
        MeshGradient { colors: Vec::new(), base: None, speed: 1.0, warp: 0.8, seed: 7, detail: 1.0, intensity: 0.8, scale: 1.0, sheen: 0.85, vignette: 0.25, rate: 0.0, smooth: true }
    }
}
impl Prepare for MeshGradient {}
impl Component for MeshGradient {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let a = th.accent;
        let fallback = if th.dark {
            vec![bgfx::hue_shift(a, -32.0, 0.75, -0.1), bgfx::hue_shift(a, 0.0, 0.3, -0.2), bgfx::hue_shift(a, 36.0, 0.7, -0.04), bgfx::hue_shift(a, 0.0, 0.55, -0.32)]
        } else {
            vec![bgfx::hue_shift(a, -30.0, 0.55, 0.18), bgfx::hue_shift(a, 0.0, 0.2, 0.25), bgfx::hue_shift(a, 35.0, 0.5, 0.2), bgfx::hue_shift(a, 0.0, 0.3, 0.26)]
        };
        let cols = colors_or(&self.colors, &fallback);
        let base = self.base.map(|c| c.0).unwrap_or(th.background);
        let t = if self.speed == 0.0 { 0.0 } else { t };
        let detail = self.detail.clamp(0.25, 4.0) * if draft() { 0.5 } else { 1.0 };
        let size = image_size(ctx, 0.5 * detail, 0.5 * detail);
        // Keyframes are interpolated per frame; at least as many as the motion needs.
        let auto = (12.0 * self.speed.abs()).clamp(6.0, 30.0);
        let rate = if self.rate > 0.0 { self.rate.max(auto) } else { auto };
        let params = f32s_key(&[self.speed, self.warp, self.intensity, self.scale, self.sheen, self.vignette, ctx.width, ctx.height]);
        let ck = [colors_key(&cols), colors_key(&[base])].concat();
        let key = |tt: f32| fnv(&[b"mesh3", &tt.to_bits().to_le_bytes(), &self.seed.to_le_bytes(), &params, &ck, &(size.0 as u32).to_le_bytes(), &(size.1 as u32).to_le_bytes()]);
        keyframed(ctx, t, rate, key, |tt| self.field(tt, ctx, &cols, base, size), size)
    }
}

impl MeshGradient {
    fn field(&self, t: f32, ctx: &Ctx, cols: &[Rgba], base: Rgba, size: (usize, usize)) -> Lin {
        let aspect = ctx.width / ctx.height.max(1.0);
        // Coarse field (smooth content), then a cubic upsample to the image.
        let (cw, ch) = if exact() { size } else { ((ctx.width / 3.0).ceil().max(8.0) as usize, (ctx.height / 3.0).ceil().max(8.0) as usize) };
        let s = t * 0.07 * self.speed;
        let seed = self.seed;
        let n = cols.len();
        let centres: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let k = i as f32 * 17.3;
                let (ax, ay) = if i + 1 == n && n > 3 {
                    (0.5, 0.5)
                } else {
                    let a = i as f32 / (n as f32 - if n > 3 { 1.0 } else { 0.0 }) * TAU + seed as f32;
                    (0.5 + 0.36 * a.cos(), 0.5 + 0.34 * a.sin())
                };
                let x = ax + 0.24 * bgfx::fbm2(k, s, 2, seed);
                let y = ay + 0.24 * bgfx::fbm2(k + 5.1, s + 3.7, 2, seed + 1);
                (x * aspect, y)
            })
            .collect();
        let labs: Vec<[f32; 3]> = cols.iter().map(|c| bgfx::oklab(bgfx::lin3(*c))).collect();
        let lb = bgfx::oklab(bgfx::lin3(base));
        let k = self.intensity.clamp(0.0, 2.0);
        let sc = self.scale.max(0.1);
        let kd = 2.2 / (sc * sc);
        let warp = self.warp;
        let (sheen, vig) = (self.sheen.max(0.0), self.vignette.clamp(0.0, 1.0));
        let mut l = Lin::new(cw, ch);
        for y in 0..ch {
            let v = (y as f32 + 0.5) / ch as f32;
            for x in 0..cw {
                let u = (x as f32 + 0.5) / cw as f32;
                let (mut px, mut py) = (u * aspect, v);
                let wx = bgfx::fbm2(px * 1.4, py * 1.4 + s * 2.0, 3, seed + 11);
                let wy = bgfx::fbm2(px * 1.4 + 9.2, py * 1.4 - s * 2.0, 3, seed + 12);
                px += wx * 0.3 * warp;
                py += wy * 0.3 * warp;
                let mut acc = [0.0f32; 3];
                let mut wsum = 0.0;
                for (c, (cx, cy)) in labs.iter().zip(&centres) {
                    let d2 = (px - cx) * (px - cx) + (py - cy) * (py - cy);
                    let w = 1.0 / (d2 * kd + 0.05).powi(2);
                    wsum += w;
                    for j in 0..3 {
                        acc[j] += c[j] * w;
                    }
                }
                let wb = 0.7;
                let mut lab = [0, 1, 2].map(|j| (acc[j] + lb[j] * wb) / (wsum + wb));
                for j in 0..3 {
                    lab[j] = lb[j] + (lab[j] - lb[j]) * k;
                }
                if sheen > 0.0 {
                    // Satin: a soft height field of folds, lit from the top left.
                    let hf = |x: f32, y: f32| bgfx::fbm2(x * 1.7 + wx * 0.6, y * 1.7 + wy * 0.6 + s * 0.8, 3, seed + 21);
                    let e = 0.02;
                    let h0 = hf(px, py);
                    let (gx, gy) = ((hf(px + e, py) - h0) / e, (hf(px, py + e) - h0) / e);
                    let lit = (-(gx * 0.6 + gy * 0.8) * 0.7).tanh();
                    let spec = lit.max(0.0).powi(6);
                    lab[0] = (lab[0] * (1.0 + sheen * 0.22 * lit) + sheen * 0.2 * spec).max(0.0);
                    let desat = 1.0 - 0.3 * sheen.min(1.0) * spec;
                    lab[1] *= desat;
                    lab[2] *= desat;
                }
                if vig > 0.0 {
                    let d = ((u - 0.5) * (u - 0.5) * 1.6 + (v - 0.5) * (v - 0.5) * 2.2).min(1.0);
                    let f = 1.0 - vig * d * 0.9;
                    lab[0] *= f;
                    lab[1] *= f;
                    lab[2] *= f;
                }
                l.px[(y * cw + x) * 3..][..3].copy_from_slice(&bgfx::oklab_inv(lab));
            }
        }
        l
    }
}

// ------------------------------------------------------------------ aurora

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Aurora {
    /// Curtain colours from the lower edge upwards (2–4). Default: oxygen green → teal → a faint rose top.
    pub colors: Vec<Color>,
    /// Sky colour (top) and horizon colour (bottom).
    pub sky: Option<[Color; 2]>,
    pub speed: f32,
    /// Brightness of the curtains.
    pub intensity: f32,
    pub seed: u32,
    /// Number of curtains (1–5), near to far.
    pub curtains: u32,
    /// Lower edge of the nearest curtain (0 = top, 1 = bottom of the box).
    pub horizon: f32,
    /// Curtain height as a fraction of the box.
    pub height: f32,
    /// Fine vertical ray structure (0 = smooth glow, 1 = strongly rayed).
    pub rays: f32,
    /// Ray density multiplier (higher = finer, more numerous rays).
    pub density: f32,
    /// Soft volumetric glow around the curtains.
    pub glow: f32,
    /// Fold strength: how much the curtains bunch and fold over.
    pub fold: f32,
    /// How much the rays bend and sway with height (flow-field displacement).
    pub sway: f32,
    /// Faint background stars (0 = none, 1 = plenty).
    pub stars: f32,
    /// Dark mountain silhouette along the bottom.
    pub landscape: bool,
    /// Field resolution multiplier (1 = default quality).
    pub detail: f32,
    /// Kept for old specs (draft mode updates this many times per second).
    pub rate: f32,
    /// Kept for old specs; motion is always smooth outside draft mode.
    pub smooth: bool,
    /// Height profiles per curtain (static: built on first use).
    #[serde(skip)]
    luts: std::sync::OnceLock<Vec<[Vec<f32>; 6]>>,
}
impl Default for Aurora {
    fn default() -> Self {
        Aurora {
            colors: Vec::new(),
            sky: None,
            speed: 1.0,
            intensity: 1.0,
            seed: 3,
            curtains: 3,
            horizon: 0.6,
            height: 0.55,
            rays: 0.9,
            density: 1.0,
            glow: 0.7,
            fold: 1.0,
            sway: 1.0,
            stars: 0.5,
            landscape: false,
            detail: 1.0,
            rate: 24.0,
            smooth: true,
            luts: Default::default(),
        }
    }
}
impl Prepare for Aurora {}
impl Component for Aurora {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = if self.speed == 0.0 { 0.0 } else { step(t, self.rate) };
        let cols = colors_or(&self.colors, &[hx3("#5cffa8"), hx3("#25e6b4"), hx3("#1a9fb8"), hx3("#ff5c86")]);
        let sky = self.sky.map(|s| [s[0].0, s[1].0]).unwrap_or([hx3("#01030a"), hx3("#0a1a2c")]);
        let detail = self.detail.clamp(0.25, 2.0) * if draft() { 0.5 } else { 1.0 };
        let size = image_size(ctx, 0.75 * detail, 0.2 * detail);
        let params = f32s_key(&[
            self.speed, self.intensity, self.horizon, self.height, self.rays, self.density, self.glow, self.fold, self.sway, ctx.width, ctx.height,
        ]);
        let key = fnv(&[b"aurora3", &t.to_bits().to_le_bytes(), &self.seed.to_le_bytes(), &self.curtains.to_le_bytes(), &params, &colors_key(&cols), &colors_key(&sky), &(size.0 as u32).to_le_bytes(), &(size.1 as u32).to_le_bytes()]);
        let mut out = vec![field_op(ctx, key, || self.field(t, ctx, &cols, sky, size), size, 1.0)];
        if self.stars > 0.0 {
            out.extend(stars_ops(ctx, t, self.stars, self.seed, self.horizon + 0.25));
        }
        if self.landscape {
            out.push(landscape(ctx, self.seed, sky[0]));
        }
        out
    }
}

/// Faint stars over the sky (crisp vector dots, slow twinkle).
fn stars_ops(ctx: &Ctx, t: f32, amount: f32, seed: u32, max_v: f32) -> Vec<Op> {
    use kurbo::Shape;
    let (w, h) = (ctx.width, ctx.height);
    let n = (amount.clamp(0.0, 2.0) * 0.00012 * w * h) as u32;
    let mut rng = Rng::new(seed as u64 ^ 0x57A2);
    let lvn = levels(8);
    let mut paths: Vec<Path> = (0..lvn).map(|_| Path::new()).collect();
    for _ in 0..n {
        let (x, y, m, ph, f) = (rng.f() * w, rng.f(), rng.f(), rng.f() * TAU, rng.range(0.3, 1.4));
        // Fade out towards the horizon haze.
        let fade = 1.0 - smooth01((y - max_v * 0.6) / (max_v * 0.4).max(0.01));
        let y = y * h;
        let base = 0.12 + 0.55 * m * m * m;
        let a = base * fade * (0.75 + 0.25 * (t * f + ph).sin());
        if a < 0.03 {
            continue;
        }
        let r = 0.45 + 0.75 * m * m;
        let lv = ((a / 0.7) * lvn as f32).min(lvn as f32 - 1.0) as usize;
        paths[lv].extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.05));
    }
    paths
        .into_iter()
        .enumerate()
        .filter(|(_, p)| !p.is_empty())
        .map(|(i, p)| fill(p, solid(Rgba([0.86, 0.92, 1.0, (i as f32 + 0.5) / lvn as f32 * 0.7]))))
        .collect()
}

fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A dark mountain ridge across the bottom of the box.
fn landscape(ctx: &Ctx, seed: u32, sky_top: Rgba) -> Op {
    let (w, h) = (ctx.width, ctx.height);
    let mut p = Path::new();
    p.move_to((0.0, h as f64));
    let steps = (w / 6.0).ceil() as usize;
    for i in 0..=steps {
        let x = i as f32 / steps as f32 * w;
        let u = x / h;
        let ridge = 0.5 * bgfx::fbm2(u * 1.3, 0.5, 4, seed + 77) + 0.5 * (1.0 - (bgfx::vnoise2(u * 3.1, 2.5, seed + 78)).abs()) - 0.25;
        let y = h * (0.9 - 0.08 * ridge);
        p.line_to((x as f64, y as f64));
    }
    p.line_to((w as f64, h as f64));
    p.close_path();
    let dark = mix(sky_top, Rgba([0.0, 0.0, 0.0, 1.0]), 0.6);
    fill(p, vgrad(h * 0.8, h, mix(dark, Rgba([0.02, 0.05, 0.06, 1.0]), 0.3), dark))
}

impl Aurora {
    fn field(&self, t: f32, ctx: &Ctx, cols: &[Rgba], sky: [Rgba; 2], size: (usize, usize)) -> Lin {
        #[cfg(target_arch = "x86_64")]
        if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 checked above. Same arithmetic (no FMA), wider lanes and gathers.
            return unsafe { self.field_avx2(t, ctx, cols, sky, size) };
        }
        self.field_any(t, ctx, cols, sky, size)
    }

    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2")]
    unsafe fn field_avx2(&self, t: f32, ctx: &Ctx, cols: &[Rgba], sky: [Rgba; 2], size: (usize, usize)) -> Lin {
        self.field_any(t, ctx, cols, sky, size)
    }

    #[inline(always)]
    fn field_any(&self, t: f32, ctx: &Ctx, cols: &[Rgba], sky: [Rgba; 2], (fw, fh): (usize, usize)) -> Lin {
        let tt = t * self.speed;
        let aspect = ctx.width / ctx.height.max(1.0);
        let ppu = fw as f32 / aspect; // field px per unit (unit = box height)
        let k_curt = self.curtains.clamp(1, 5) as usize;
        // Colour along height (OKLab ramp) for rays and glow.
        let ramp = bgfx::ramp(cols, 64);
        let ramp_at = |p: f32| -> [f32; 3] {
            let x = p.clamp(0.0, 1.0) * 63.0;
            let i = (x as usize).min(62);
            let f = x - i as f32;
            [0, 1, 2].map(|j| ramp[i][j] + (ramp[i + 1][j] - ramp[i][j]) * f)
        };
        // Height above a curtain's lower edge, in units of its height.
        const HLO: f32 = -0.7;
        const HHI: f32 = 1.6;
        const LN: usize = 2048;
        let hscale = (LN - 1) as f32 / (HHI - HLO);
        let rays = self.rays.clamp(0.0, 1.0);

        struct Curtain<'a> {
            /// Per column, planar: lower edge `v`, 1 / height, ray intensity, glow intensity.
            pc: [Vec<f32>; 4],
            /// Per height, planar: ray colour (3) and glow colour (3).
            pl: &'a [Vec<f32>; 6],
            vmin: f32,
            vmax: f32,
        }
        // 2. Height profiles (with colour) per curtain as lookup tables:
        // they only depend on the parameters, so they are built once.
        let luts = self.luts.get_or_init(|| {
            (0..k_curt)
                .map(|c| {
                    let d = if k_curt > 1 { c as f32 / (k_curt - 1) as f32 } else { 0.0 };
                    let h_c = self.height * (1.0 - 0.45 * d);
                    let edge = 0.007 / h_c.max(0.05);
                    let lut: Vec<[f32; 6]> = (0..LN)
                    .map(|i| {
                        let h = HLO + i as f32 / hscale;
                        // Continuous profile: a soft (logistic) lower edge, a bright
                        // band just above it, then a long fade upwards.
                        let rise = 1.0 / (1.0 + (-(h - 0.5 * edge) / edge).exp());
                        let hp = h.max(0.0);
                        let p = rise * ((-hp * 9.0).exp() * 0.9 + (-hp * 2.0).exp() * 0.55 + 0.15 * (-hp * 0.7).exp()) * (1.0 - bgfx::smoothstep(0.75, 1.5, h));
                        let col = ramp_at(h / 1.05);
                        // A whiter core just above the lower edge.
                        let core = 0.25 * (-((h - 2.0 * edge) / 0.08) * ((h - 2.0 * edge) / 0.08)).exp();
                        let g = if h < 0.0 { 0.89 * (-(h / 0.14) * (h / 0.14)).exp() } else { (-((h - 0.22) / 0.65) * ((h - 0.22) / 0.65)).exp() };
                        let g = g * (1.0 - bgfx::smoothstep(1.2, 1.6, h)) * bgfx::smoothstep(-0.7, -0.5, h);
                        let gc = ramp_at(h / 1.05 * 0.8 + 0.05);
                        let gk = g * 0.35 * self.glow;
                        [p * (col[0] + core), p * (col[1] + core), p * (col[2] + core), gk * gc[0], gk * gc[1], gk * gc[2]]
                    })
                    .collect();
                    std::array::from_fn(|k| lut.iter().map(|c| c[k]).collect())
                })
                .collect::<Vec<[Vec<f32>; 6]>>()
        });
        let mut curtains = Vec::with_capacity(k_curt);
        let mut samples: Vec<[f32; 4]> = Vec::new();
        for c in 0..k_curt {
            let d = if k_curt > 1 { c as f32 / (k_curt - 1) as f32 } else { 0.0 };
            let sc = self.seed.wrapping_mul(7).wrapping_add(c as u32 * 131);
            let cf = c as f32;
            // Near curtains sit higher in the sky, are taller and brighter.
            let base_c = self.horizon + d * 0.16;
            let h_c = self.height * (1.0 - 0.45 * d);
            let k_c = self.intensity * (1.0 - 0.6 * d);
            let fr = 38.0 * self.density.max(0.1) * (1.0 + 0.6 * d);
            let drift = tt * 0.015 * (1.0 + 0.5 * cf);

            // 1. Splat the curtain (sampled along its length) onto columns:
            // folds bunch the samples, which brightens them, as in real aurorae.
            let s_lo = -0.35f32;
            let s_hi = aspect + 0.35;
            let ns = ((s_hi - s_lo) * ppu * 2.0) as usize;
            let ds = (s_hi - s_lo) / ns as f32;
            let wpx = ds * ppu;
            // i, b·i, h·i, w, b·w, h·w
            let mut a = vec![[0.0f32; 6]; fw];
            // Samples first (independent: vectorises), then the scatter onto columns.
            samples.clear();
            samples.extend((0..ns).map(|j| {
                let s = s_lo + (j as f32 + 0.5) * ds;
                let x = s + self.fold * 0.085 * (bgfx::vnoise2(s * 1.7 + cf * 5.3, tt * 0.09, sc) + 0.55 * bgfx::vnoise2(s * 4.1, tt * 0.13, sc + 1));
                let b = base_c + 0.14 * bgfx::vnoise2(s * 0.7 + cf * 2.0, tt * 0.035, sc + 2) + 0.035 * bgfx::vnoise2(s * 2.2, tt * 0.06, sc + 3);
                let e = 0.7 * bgfx::vnoise2(s * 1.1 + cf * 3.0, tt * 0.06, sc + 4) + 0.3 * bgfx::vnoise2(s * 2.9, tt * 0.09, sc + 10);
                let env = bgfx::smoothstep(-0.1 + 0.2 * d, 0.45 + 0.1 * d, e) * (0.65 + 0.35 * bgfx::vnoise2(s * 4.0, tt * 0.12, sc + 8)) + 0.04;
                let rx = s - drift;
                let r1 = 0.5 + 0.5 * bgfx::vnoise2(rx * fr, tt * 0.22, sc + 5);
                let r2 = 0.5 + 0.5 * bgfx::vnoise2(rx * fr * 2.7, tt * 0.35, sc + 6);
                let r3 = 0.5 + 0.5 * bgfx::vnoise2(rx * fr * 6.3, tt * 0.5, sc + 9);
                let ray = r1 * r1 * r1 * 0.6 + r2 * r2 * r2 * r2 * 0.7 + r3 * r3 * r3 * r3 * 0.7;
                let i = k_c * env * ((1.0 - rays) * 0.55 + rays * ray * 1.6);
                let hh = h_c * (0.55 + 0.45 * (0.5 + 0.5 * bgfx::vnoise2(s * 1.3, tt * 0.07, sc + 7))) * (0.7 + 0.5 * r1 * rays + 0.25 * (1.0 - rays));
                [x, b, i, hh]
            }));
            for &[x, b, i, hh] in samples.iter() {
                let col = x * ppu - 0.5;
                let c0 = col.floor();
                let f = col - c0;
                for (ci, wt) in [(c0 as isize, 1.0 - f), (c0 as isize + 1, f)] {
                    if ci < 0 || ci >= fw as isize {
                        continue;
                    }
                    let wv = wt * wpx;
                    let e = &mut a[ci as usize];
                    e[0] += i * wv;
                    e[1] += b * i * wv;
                    e[2] += hh * i * wv;
                    e[3] += wv;
                    e[4] += b * wv;
                    e[5] += hh * wv;
                }
            }
            let mut bcol = vec![base_c; fw];
            let mut hcol = vec![h_c; fw];
            let mut icol = vec![0.0f32; fw];
            for x in 0..fw {
                let e = a[x];
                if e[0] > 1e-5 {
                    bcol[x] = e[1] / e[0];
                    hcol[x] = e[2] / e[0];
                } else if e[3] > 1e-5 {
                    bcol[x] = e[4] / e[3];
                    hcol[x] = e[5] / e[3];
                }
                // Caustics at folds: bright, but soft-clipped.
                icol[x] = 2.2 * (1.0 - (-e[0] / 2.2).exp());
            }
            bgfx::blur1(&mut bcol, (fw / 120).max(2), 3);
            bgfx::blur1(&mut hcol, 2, 2);
            let mut gcol = icol.clone();
            bgfx::blur1(&mut gcol, (fw / 70).max(1), 3);
            let (mut vmin, mut vmax) = (f32::MAX, f32::MIN);
            let cols: Vec<[f32; 4]> = (0..fw)
                .map(|x| {
                    vmin = vmin.min(bcol[x] - HHI * hcol[x]);
                    vmax = vmax.max(bcol[x] - HLO * hcol[x]);
                    [bcol[x], 1.0 / hcol[x].max(1e-3), icol[x], gcol[x]]
                })
                .collect();

            // Planar copies for the per-pixel gathers.
            let pc: [Vec<f32>; 4] = std::array::from_fn(|k| cols.iter().map(|c| c[k]).collect());
            curtains.push(Curtain { pc, pl: &luts[c], vmin, vmax });
        }

        // Flow field for sway: a coarse grid, interpolated once per row.
        let (gw, gh) = (48usize, 16usize);
        let mut grid = vec![0.0f32; (gw + 1) * (gh + 1)];
        for gy in 0..=gh {
            for gx in 0..=gw {
                let (u, v) = (gx as f32 / gw as f32 * aspect, gy as f32 / gh as f32);
                grid[gy * (gw + 1) + gx] = bgfx::fbm2(u * 1.8 + tt * 0.04, v * 2.2 - tt * 0.07, 2, self.seed + 900);
            }
        }
        let sway_px = self.sway * 0.05 * ppu;
        let mut rowsway = vec![0.0f32; fw];

        let s0 = bgfx::lin3(sky[0]);
        let s1 = bgfx::lin3(sky[1]);
        let mut img = Lin::new(fw, fh);
        // Light per column (planar r, g, b).
        let mut acc = vec![0.0f32; fw * 3];
        let xmax = fw as f32 - 1.001;
        for y in 0..fh {
            let v = (y as f32 + 0.5) / fh as f32;
            let gyf = v * gh as f32;
            let gy = (gyf as usize).min(gh - 1);
            let fy = gyf - gy as f32;
            for (x, r) in rowsway.iter_mut().enumerate() {
                let gxf = x as f32 / fw as f32 * gw as f32;
                let gx = (gxf as usize).min(gw - 1);
                let fx = gxf - gx as f32;
                let (i0, i1) = (gy * (gw + 1) + gx, (gy + 1) * (gw + 1) + gx);
                let g0 = grid[i0] + (grid[i0 + 1] - grid[i0]) * fx;
                let g1 = grid[i1] + (grid[i1 + 1] - grid[i1]) * fx;
                *r = (g0 + (g1 - g0) * fy) * sway_px;
            }
            acc.iter_mut().for_each(|a| *a = 0.0);
            let (ar, rest) = acc.split_at_mut(fw);
            let (ag, ab) = rest.split_at_mut(fw);
            for cu in &curtains {
                if v < cu.vmin || v > cu.vmax {
                    continue;
                }
                let (cb, ci, cr, cg) = (&cu.pc[0][..fw], &cu.pc[1][..fw], &cu.pc[2][..fw], &cu.pc[3][..fw]);
                let [l0, l1, l2, l3, l4, l5] = &cu.pl;
                let (l0, l1, l2, l3, l4, l5) = (&l0[..LN], &l1[..LN], &l2[..LN], &l3[..LN], &l4[..LN], &l5[..LN]);
                let sway = &rowsway[..fw];
                // Branch-free (vectorises with gathers): pixels outside the
                // curtain contribute zero weight instead of being skipped.
                for x in 0..fw {
                    let h0 = (cb[x] - v) * ci[x];
                    // Far outside this curtain (sway moves it only a little): no light.
                    let near = h0 > HLO - 0.15 && h0 < HHI + 0.15;
                    let up = bgfx::fmin(bgfx::fmax(h0, 0.0), 1.5);
                    let xs = bgfx::fmin(bgfx::fmax(x as f32 + sway[x] * up * up, 0.0), xmax);
                    let i0 = xs as i32 as usize;
                    let f = xs - i0 as f32;
                    // SAFETY: 0 ≤ xs ≤ fw - 1.001, so i0 + 1 < fw (the length of every column array).
                    let at = |a: &[f32], i: usize| unsafe { *a.get_unchecked(i) };
                    let b = at(cb, i0) + (at(cb, i0 + 1) - at(cb, i0)) * f;
                    let ih = at(ci, i0) + (at(ci, i0 + 1) - at(ci, i0)) * f;
                    let h = (b - v) * ih;
                    let on = near && (HLO..HHI).contains(&h);
                    let i = if on { at(cr, i0) + (at(cr, i0 + 1) - at(cr, i0)) * f } else { 0.0 };
                    let g = if on { at(cg, i0) + (at(cg, i0 + 1) - at(cg, i0)) * f } else { 0.0 };
                    // The table is fine enough (≈½ px per entry) to sample nearest.
                    // SAFETY (reads below): li is clamped to 0..=LN - 1 and every table has LN entries.
                    let li = bgfx::fmin(bgfx::fmax((h - HLO) * hscale + 0.5, 0.0), (LN - 1) as f32) as i32 as usize;
                    ar[x] += i * at(l0, li) + g * at(l3, li);
                    ag[x] += i * at(l1, li) + g * at(l4, li);
                    ab[x] += i * at(l2, li) + g * at(l5, li);
                }
            }
            // Sky, plus the light tone-mapped with a soft shoulder (no clipping).
            let k = v.powf(1.4);
            let sk = [0, 1, 2].map(|j| s0[j] + (s1[j] - s0[j]) * k);
            let row = &mut img.px[y * fw * 3..(y + 1) * fw * 3];
            for (x, o) in row.chunks_exact_mut(3).enumerate() {
                o[0] = sk[0] + bgfx::one_minus_exp_neg(ar[x] * 1.1);
                o[1] = sk[1] + bgfx::one_minus_exp_neg(ag[x] * 1.1);
                o[2] = sk[2] + bgfx::one_minus_exp_neg(ab[x] * 1.1);
            }
        }
        img
    }
}

// ------------------------------------------------------------------ grain

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Grain {
    /// Strength (default 0.016: fine and cheap to encode; 0.04+ is visible film grain, 0.08+ heavy).
    pub amount: f32,
    /// Grain size in pixels.
    pub size: f32,
    /// New grain positions per second (default: every frame; 0 = static).
    pub rate: Option<f32>,
    pub seed: u32,
    /// `"overlay"` (default: luminance-weighted, strongest in mid-tones, like
    /// film), `"soft-light"` (gentler) or `"normal"` (even, also in blacks).
    pub blend: Option<String>,
    /// Softness of each grain (0 = crisp blue noise, 1 = soft film clumps).
    pub softness: f32,
}
impl Default for Grain {
    fn default() -> Self {
        Grain { amount: 0.016, size: 1.0, rate: None, seed: 1, blend: None, softness: 0.3 }
    }
}
impl Prepare for Grain {
    fn prepare(&mut self) -> Result<(), super::Error> {
        match self.blend.as_deref() {
            None | Some("overlay") | Some("soft-light") | Some("normal") => Ok(()),
            Some(b) => Err(super::Error(format!("grain blend {b:?}: use \"overlay\", \"soft-light\" or \"normal\""))),
        }
    }
}

/// One baked 256² tile of fine blue noise (white noise minus its local mean,
/// so no low-frequency blotches), unit variance.
fn blue_tile(seed: u32, soft: f32, salt: u32) -> Vec<f32> {
    const T: usize = 256;
    let mut rng = Rng::new(((seed as u64) << 20) ^ salt as u64 ^ 0x5eed);
    let raw: Vec<f32> = (0..T * T).map(|_| rng.f() + rng.f() - 1.0).collect();
    let at = |v: &[f32], x: usize, y: usize| v[(y % T) * T + (x % T)];
    // Local mean over a 5×5 window (toroidal), removed: a high-pass.
    let mut mean = vec![0.0f32; T * T];
    for y in 0..T {
        for x in 0..T {
            let mut s = 0.0;
            for dy in 0..5 {
                for dx in 0..5 {
                    s += at(&raw, x + T + dx - 2, y + T + dy - 2);
                }
            }
            mean[y * T + x] = s / 25.0;
        }
    }
    let hp: Vec<f32> = raw.iter().zip(&mean).map(|(r, m)| r - m).collect();
    // Softness: a small blur of the high-passed noise (film clumps of ~1.5 px).
    let g: Vec<f32> = (0..T * T)
        .map(|i| {
            let (x, y) = (i % T + T, i / T + T);
            let nb = (at(&hp, x + 1, y) + at(&hp, x - 1, y) + at(&hp, x, y + 1) + at(&hp, x, y - 1)) * 0.25;
            hp[i] * (1.0 - 0.5 * soft) + nb * 0.5 * soft
        })
        .collect();
    let var = g.iter().map(|x| x * x).sum::<f32>() / g.len() as f32;
    let k = 1.0 / var.sqrt().max(1e-6);
    g.into_iter().map(|v| v * k).collect()
}

impl Component for Grain {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        const TILE: u32 = 256;
        let rate = self.rate.unwrap_or(ctx.fps.max(1.0));
        let frame = if rate > 0.0 { (t * rate + 1e-3).floor() as u32 } else { 0 };
        let mode = match self.blend.as_deref() {
            Some("normal") => 0u8,
            Some("soft-light") => 2,
            _ => 1,
        };
        let amt = self.amount.clamp(0.0, 1.0);
        let soft = self.softness.clamp(0.0, 1.0);
        // One baked tile, moved to a fresh random offset every update; the
        // exact path makes fresh noise every update instead.
        // Grain is not switched by reference mode: fresh noise and a moved tile are
        // different random fields, which a per-pixel comparison can't judge.
        let salt = 0u32;
        let key = fnv(&[b"grain4", &salt.to_le_bytes(), &self.seed.to_le_bytes(), &f32s_key(&[amt, soft]), &[mode]]);
        let seed = self.seed;
        let id = ctx.assets.image(key, TILE, TILE, &mut || {
            let g = blue_tile(seed, soft, salt);
            let mut px = vec![0u8; (TILE * TILE * 4) as usize];
            for (p, n) in px.chunks_mut(4).zip(&g) {
                if mode == 0 {
                    // Light specks white, dark specks black, strength in alpha.
                    let c = if *n > 0.0 { 255 } else { 0 };
                    let a = (n.abs() * amt * 0.9 * 255.0 + 0.5).min(255.0) as u8;
                    p.copy_from_slice(&[c, c, c, a]);
                } else {
                    // Mid-grey ± noise, blended as overlay / soft light.
                    let g = ((0.5 + n * amt * if mode == 2 { 0.9 } else { 0.55 }).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    p.copy_from_slice(&[g, g, g, 255]);
                }
            }
            px
        });
        let off = (hash01(frame.wrapping_mul(7919) ^ self.seed) * TILE as f32).floor();
        let offy = (hash01(frame.wrapping_mul(104_729) ^ 99 ^ self.seed) * TILE as f32).floor();
        let s = self.size.max(0.25);
        let paint = Paint::Image { image: id, transform: [1.0 / s, 0.0, 0.0, 1.0 / s, off, offy], opacity: 1.0 };
        let r = rect(0.0, 0.0, ctx.width, ctx.height, 0.0, paint);
        if mode == 0 {
            return vec![r];
        }
        let blend = if mode == 2 { Blend::SoftLight } else { Blend::Overlay };
        vec![Op::PushLayer { opacity: 1.0, blend, filters: Vec::new(), clip: None, cache_key: None }, r, Op::PopLayer]
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
    /// A brighter major line every this many cells (0 = none).
    pub major: u32,
    /// Fade radius as a fraction of the box diagonal (0 = no fade).
    pub fade: f32,
    /// Floor grid receding to a horizon (product-stage / synthwave look).
    pub perspective: bool,
    /// Horizon height (0..1) for perspective mode.
    pub horizon: f32,
    /// Scroll speed in cells per second.
    pub speed: f32,
    /// Draw-on: the grid reveals from the centre over this many seconds.
    pub reveal: f32,
    /// Glow along the horizon (perspective) or around the lines (0 = none).
    pub glow: f32,
    /// Colour of the glow and scan line (default: the line colour).
    pub glow_color: Option<Color>,
    /// A glowing scan line sweeping over the grid every this many seconds (0 = off).
    pub scan: f32,
}
impl Default for Grid {
    fn default() -> Self {
        Grid { cell: 64.0, color: None, line_width: 1.0, major: 0, fade: 0.55, perspective: false, horizon: 0.45, speed: 0.0, reveal: 0.0, glow: 0.0, glow_color: None, scan: 0.0 }
    }
}
impl Prepare for Grid {}
impl Component for Grid {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(alpha(ctx.theme.text, if ctx.theme.dark { 0.10 } else { 0.09 }));
        let gc = self.glow_color.map(|c| c.0).unwrap_or(Rgba([c.0[0], c.0[1], c.0[2], 1.0]));
        let reveal = if self.reveal > 0.0 { ease_out(t / self.reveal) } else { 1.0 };
        let cell = self.cell.max(4.0);
        let lw = self.line_width.max(0.1);
        let mut out = Vec::new();
        if self.perspective {
            self.floor(t, ctx, c, gc, reveal, &mut out);
            return out;
        }
        // Lines on pixel centres stay one crisp pixel wide; while scrolling
        // they move by sub-pixels (smooth motion beats snapped steps).
        let moving = self.speed != 0.0;
        let snap = |v: f32| if moving { v } else { v.round() + if (lw.round() as i32) % 2 == 1 { 0.5 } else { 0.0 } };
        let off = (t * self.speed * cell).rem_euclid(cell);
        let major = self.major as i64;
        let (mut minor_p, mut major_p) = (Path::new(), Path::new());
        // Centre the grid on the box.
        let ox = (w * 0.5).rem_euclid(cell) - off;
        let oy = (h * 0.5).rem_euclid(cell) - off;
        let k0x = ((w * 0.5 - ox) / cell).round() as i64;
        let k0y = ((h * 0.5 - oy) / cell).round() as i64;
        let scroll_k = (t * self.speed).floor() as i64;
        let mut i = -1i64;
        while ox + i as f32 * cell <= w + cell {
            let x = snap(ox + i as f32 * cell);
            let is_major = major > 0 && (i - k0x - scroll_k).rem_euclid(major) == 0;
            let p = if is_major { &mut major_p } else { &mut minor_p };
            p.move_to((x as f64, 0.0));
            p.line_to((x as f64, h as f64));
            i += 1;
        }
        let mut j = -1i64;
        while oy + j as f32 * cell <= h + cell {
            let y = snap(oy + j as f32 * cell);
            let is_major = major > 0 && (j - k0y - scroll_k).rem_euclid(major) == 0;
            let p = if is_major { &mut major_p } else { &mut minor_p };
            p.move_to((0.0, y as f64));
            p.line_to((w as f64, y as f64));
            j += 1;
        }
        let st = |width: f32| crate::dl::Stroke { width, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] };
        if self.glow > 0.0 {
            // Halo: two wide, faint strokes under the hairlines (no blur pass).
            let mut all = minor_p.clone();
            all.extend(major_p.iter());
            let g = self.glow.min(2.0);
            out.push(Op::Stroke { path: all.clone(), paint: fade_paint(alpha(gc, 0.05 * g), w, h, self.fade, reveal), stroke: st(lw * 7.0), transform: IDENTITY });
            out.push(Op::Stroke { path: all, paint: fade_paint(alpha(gc, 0.10 * g), w, h, self.fade, reveal), stroke: st(lw * 3.0), transform: IDENTITY });
        }
        out.push(Op::Stroke { path: minor_p, paint: fade_paint(c, w, h, self.fade, reveal), stroke: st(lw), transform: IDENTITY });
        if !major_p.is_empty() {
            out.push(Op::Stroke { path: major_p, paint: fade_paint(alpha(c, 2.2), w, h, self.fade, reveal), stroke: st(lw), transform: IDENTITY });
        }
        if self.scan > 0.0 {
            // A soft band sweeping top to bottom.
            let p = (t / self.scan).fract();
            let y = -h * 0.15 + p * h * 1.3;
            let bh = h * 0.12;
            out.push(rect(0.0, y - bh, w, 2.0 * bh, 0.0, Paint::Linear { p0: [0.0, y - bh], p1: [0.0, y + bh], stops: vec![(0.0, alpha(gc, 0.0)), (0.5, alpha(gc, 0.10)), (0.52, alpha(gc, 0.35)), (0.54, alpha(gc, 0.06)), (1.0, alpha(gc, 0.0))] }));
        }
        out
    }
}

impl Grid {
    /// Perspective floor: true 1/z spacing, fog towards the horizon, an
    /// optional horizon glow and a scan line travelling towards the viewer.
    fn floor(&self, t: f32, ctx: &Ctx, c: Rgba, gc: Rgba, reveal: f32, out: &mut Vec<Op>) {
        let (w, h) = (ctx.width, ctx.height);
        let hy = h * self.horizon.clamp(0.05, 0.95);
        let cx = w * 0.5;
        let f = h - hy; // screen px per world unit at depth 1 (camera height 1)
        let cs = self.cell.max(4.0) / f; // world cell size
        let lw = self.line_width.max(0.1);
        // Longitudinal lines run almost to the horizon (the fog hides their
        // convergence); cross lines stop where they would be < 1.5 px apart.
        let zfar = 60.0f32;
        let zcross = (f * cs / 1.5).sqrt().max(2.0);
        let y_of = |z: f32| hy + f / z;
        let mut near = Path::new();
        let mut far = Path::new();
        // Longitudinal lines converge on the vanishing point.
        let xmax = (w * 0.5 / f) * zfar + cs;
        let n = (xmax / cs).ceil() as i32;
        let z_split = 3.0f32;
        for i in -n..=n {
            let xw = i as f32 * cs;
            let (x_far, x_mid) = (cx + f * xw / zfar, cx + f * xw / z_split);
            let zb = 0.35f32;
            let x_near = cx + f * xw / zb;
            far.move_to((x_far as f64, y_of(zfar) as f64));
            far.line_to((x_mid as f64, y_of(z_split) as f64));
            near.move_to((x_mid as f64, y_of(z_split) as f64));
            near.line_to((x_near as f64, y_of(zb) as f64));
        }
        // Cross lines at equal world spacing, scrolling towards the viewer.
        let scroll = (t * self.speed).rem_euclid(1.0) * cs;
        let mut k = 0;
        loop {
            let z = 0.6 + k as f32 * cs - scroll;
            k += 1;
            if z > zcross {
                break;
            }
            if z < 0.95 {
                continue;
            }
            let y = y_of(z);
            let p = if z < z_split { &mut near } else { &mut far };
            p.move_to((0.0, y as f64));
            p.line_to((w as f64, y as f64));
        }
        let fog = |a: f32| Paint::Linear {
            p0: [0.0, hy],
            p1: [0.0, h],
            stops: vec![(0.0, alpha(c, 0.0)), (0.04, alpha(c, 0.03 * a)), (0.12, alpha(c, 0.18 * a)), (0.3, alpha(c, 0.5 * a)), (0.6, alpha(c, 0.85 * a)), (1.0, alpha(c, a))],
        };
        let st = |width: f32| crate::dl::Stroke { width, cap: 0, join: 0, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] };
        if self.glow > 0.0 {
            // Horizon glow: a wide soft band and a thin bright line.
            let g = self.glow.min(2.0);
            let bh = h * 0.09;
            out.push(rect(0.0, hy - bh, w, bh * 2.0, 0.0, Paint::Linear { p0: [0.0, hy - bh], p1: [0.0, hy + bh], stops: band_stops(gc, 0.16 * g * reveal) }));
            // The horizon line itself, brightest in the middle.
            out.push(rect(0.0, hy - 0.6, w, 1.2, 0.0, Paint::Linear { p0: [0.0, 0.0], p1: [w, 0.0], stops: band_stops(gc, 0.7 * g.min(1.0) * reveal) }));
        }
        out.push(Op::Stroke { path: far, paint: fog(reveal), stroke: st(lw), transform: IDENTITY });
        out.push(Op::Stroke { path: near, paint: fog(reveal), stroke: st(lw * 1.35), transform: IDENTITY });
        if self.scan > 0.0 {
            // A glowing line travelling from the horizon to the viewer.
            let p = (t / self.scan).fract();
            // Linear in 1/z: the line accelerates as it approaches, as in perspective.
            let zs = 1.0 / (1.0 / zfar + (1.0 / 0.9 - 1.0 / zfar) * p);
            let y = y_of(zs);
            let th = (f * 0.02 / zs).max(1.5);
            let a = smooth01(p * 6.0) * smooth01((1.0 - p) * 4.0) * reveal;
            out.push(rect(0.0, y - th * 3.0, w, th * 6.0, 0.0, Paint::Linear { p0: [0.0, y - th * 3.0], p1: [0.0, y + th * 3.0], stops: band_stops(gc, 0.4 * a) }));
        }
    }
}

/// A soft symmetric band (Gaussian halo plus a bright core) across a gradient's length.
fn band_stops(c: Rgba, a: f32) -> Vec<(f32, Rgba)> {
    let e = (-1.0f32 / 0.06).exp();
    (0..=16)
        .map(|i| {
            let x = i as f32 / 16.0;
            let d = (x - 0.5) * (x - 0.5);
            let g = ((-d / 0.06).exp() - e) / (1.0 - e) * 0.45 + (-d / 0.0025).exp() * 0.55;
            (x, alpha(c, (a * g).min(1.0)))
        })
        .collect()
}

fn fade_paint(c: Rgba, w: f32, h: f32, fade: f32, reveal: f32) -> Paint {
    if fade <= 0.0 && reveal >= 1.0 {
        return solid(c);
    }
    let diag = (w * w + h * h).sqrt() * 0.5;
    let r = if fade > 0.0 { diag * fade * 2.0 } else { diag * 1.5 };
    let r = r * reveal.max(0.001);
    radial(w * 0.5, h * 0.5, r, vec![(0.0, c), (0.3, alpha(c, 0.85)), (0.55, alpha(c, 0.55)), (0.8, alpha(c, 0.18)), (1.0, alpha(c, 0.0))])
}

// ------------------------------------------------------------------ dots

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dots {
    pub spacing: f32,
    pub radius: f32,
    pub color: Option<Color>,
    pub fade: f32,
    /// A slow wave of light travelling out from the centre.
    pub wave: bool,
    /// Colour of the wave (default: the theme accent).
    pub wave_color: Option<Color>,
    /// Wave speed multiplier.
    pub speed: f32,
    /// `"circle"` (default), `"square"` or `"plus"`.
    pub shape: Option<String>,
    /// Drift the whole pattern (pixels per second, `[x, y]`).
    pub drift: Option<[f32; 2]>,
    /// Offset every other row by half a cell (hexagonal packing).
    pub stagger: bool,
}
impl Default for Dots {
    fn default() -> Self {
        Dots { spacing: 28.0, radius: 1.3, color: None, fade: 0.6, wave: false, wave_color: None, speed: 1.0, shape: None, drift: None, stagger: false }
    }
}
impl Prepare for Dots {}
impl Component for Dots {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        use kurbo::Shape;
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(alpha(ctx.theme.text, 0.22));
        let wc = self.wave_color.map(|c| c.0).unwrap_or(ctx.theme.accent);
        let s = self.spacing.max(4.0);
        let shape = match self.shape.as_deref() {
            Some("square") => 1u8,
            Some("plus") => 2,
            _ => 0,
        };
        let [dx, dy] = self.drift.unwrap_or([0.0, 0.0]);
        let (sx, sy) = ((dx * t).rem_euclid(s), (dy * t).rem_euclid(s * if self.stagger { 2.0 } else { 1.0 }));
        let (nx, ny) = ((w / s) as i32 + 3, (h / s) as i32 + 4);
        let (ox, oy) = ((w - (nx - 1) as f32 * s) * 0.5 + sx - s, (h - (ny - 1) as f32 * s) * 0.5 + sy - s);
        let static_grid = dx == 0.0 && dy == 0.0;
        let lvn = levels(12);
        let mut lv: Vec<Path> = (0..=lvn).map(|_| Path::new()).collect();
        let push = |p: &mut Path, x: f32, y: f32, r: f32| match shape {
            1 => p.extend(kurbo::Rect::new((x - r) as f64, (y - r) as f64, (x + r) as f64, (y + r) as f64).path_elements(0.1)),
            2 => {
                let a = r * 0.36;
                p.extend(kurbo::Rect::new((x - r * 1.6) as f64, (y - a) as f64, (x + r * 1.6) as f64, (y + a) as f64).path_elements(0.1));
                p.extend(kurbo::Rect::new((x - a) as f64, (y - r * 1.6) as f64, (x + a) as f64, (y - a) as f64).path_elements(0.1));
                p.extend(kurbo::Rect::new((x - a) as f64, (y + a) as f64, (x + a) as f64, (y + r * 1.6) as f64).path_elements(0.1));
            }
            _ => p.extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.05)),
        };
        for j in 0..ny {
            for i in 0..nx {
                let mut x = ox + i as f32 * s;
                if self.stagger && (j + ((sy / s) as i32)) % 2 == 1 {
                    x += s * 0.5;
                }
                let y = oy + j as f32 * s;
                if x < -s || y < -s || x > w + s || y > h + s {
                    continue;
                }
                // Static grids sit on pixel centres: crisp dots.
                let (x, y) = if static_grid { (x.round() + 0.5 * (shape == 1) as i32 as f32, y.round() + 0.5 * (shape == 1) as i32 as f32) } else { (x, y) };
                let mut r = self.radius;
                let mut k = 0.0;
                if self.wave {
                    let d = ((x - w * 0.5).powi(2) + (y - h * 0.5).powi(2)).sqrt() / s;
                    // A ring with a soft leading edge and a long tail.
                    let ph = (d * 0.18 - t * 0.55 * self.speed).rem_euclid(1.0);
                    k = (1.0 - ph).powi(10) * 0.85 + (-(ph * 22.0)).exp() * 0.0 + (1.0 - (ph - 0.98).abs() * 50.0).max(0.0) * 0.15;
                    k = k.clamp(0.0, 1.0);
                    r *= 1.0 + k * 0.7;
                }
                let li = (k * lvn as f32).round() as usize;
                push(&mut lv[li], x, y, r);
            }
        }
        let mut out = Vec::new();
        for (i, p) in lv.into_iter().enumerate() {
            if p.is_empty() {
                continue;
            }
            let k = i as f32 / lvn as f32;
            let col = if k == 0.0 { c } else { Rgba([lerp(c.0[0], wc.0[0], k), lerp(c.0[1], wc.0[1], k), lerp(c.0[2], wc.0[2], k), lerp(c.0[3], wc.0[3].min(0.95), k)]) };
            out.push(fill(p, fade_paint(col, w, h, self.fade, 1.0)));
        }
        out
    }
}

// ------------------------------------------------------------------ light leak

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightLeak {
    /// Leak colours (default: warm film amber, rose and gold).
    pub colors: Vec<Color>,
    pub intensity: f32,
    pub speed: f32,
    pub seed: u32,
    /// Kept for old specs (draft mode updates this many times per second).
    pub rate: f32,
    /// Anamorphic horizontal streaks through each leak (0 = none).
    pub streak: f32,
    /// Size multiplier of the blooms.
    pub scale: f32,
    /// `"edges"` (default: leaks hug the left and right edges), `"corner"`
    /// (one corner flare) or `"top"`.
    pub position: Option<String>,
    /// `"normal"` (default) or `"screen"` (light adds without clipping over
    /// bright content; costs a full-frame layer).
    pub blend: Option<String>,
}
impl Default for LightLeak {
    fn default() -> Self {
        LightLeak { colors: Vec::new(), intensity: 0.8, speed: 1.0, seed: 11, rate: 0.0, streak: 0.35, scale: 1.0, position: None, blend: None }
    }
}
impl Prepare for LightLeak {}

/// Gaussian-like falloff stops (so blooms have no visible edge or ring).
fn bloom_stops(c: Rgba, a: f32) -> Vec<(f32, Rgba)> {
    let e = (-4.0f32).exp();
    (0..=10)
        .map(|i| {
            let x = i as f32 / 10.0;
            let g = ((-4.0 * x * x).exp() - e) / (1.0 - e);
            (x, alpha(c, a * g))
        })
        .collect()
}

impl Component for LightLeak {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let cols = colors_or(&self.colors, &[hx3("#ff8a3d"), hx3("#ff4d6d"), hx3("#ffd27a")]);
        let (w, h) = (ctx.width, ctx.height);
        let t = step(t, self.rate);
        let s = t * 0.15 * self.speed;
        let seed = self.seed;
        let pos = self.position.as_deref().unwrap_or("edges");
        let mut out = Vec::new();
        let screen = self.blend.as_deref() == Some("screen");
        if screen {
            push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        }
        let big = w.max(h) * self.scale.max(0.05);
        for (i, c) in cols.iter().enumerate() {
            let k = i as f32 * 7.7;
            let n = |o: f32, f: f32| bgfx::fbm2(k + o, s * f, 2, seed);
            let (ax, ay) = match pos {
                "corner" => (-0.05 + 0.12 * i as f32, -0.05 + 0.1 * i as f32),
                "top" => (0.2 + 0.6 * (i as f32 / cols.len().max(2) as f32 - 0.5) + 0.3, -0.12),
                _ => (if i % 2 == 0 { -0.08 } else { 1.08 }, 0.5),
            };
            let cx = w * (ax + 0.28 * n(0.0, 1.0));
            let cy = h * (ay + 0.45 * n(3.0, 0.8));
            let r = big * (0.48 + 0.18 * n(8.0, 1.2));
            let a = (self.intensity * (0.6 + 0.4 * n(1.5, 2.0))).clamp(0.0, 1.0);
            let (x0, y0) = ((cx - r).max(0.0), (cy - r).max(0.0));
            let (x1, y1) = ((cx + r).min(w), (cy + r).min(h));
            if x1 > x0 && y1 > y0 {
                out.push(rect(x0, y0, x1 - x0, y1 - y0, 0.0, radial(cx, cy, r, bloom_stops(*c, a))));
                // A hot inner core, slightly offset, for depth.
                let (hx, hy) = (cx + r * 0.12 * n(4.0, 1.5), cy + r * 0.1 * n(5.0, 1.5));
                let hr = r * 0.38;
                let core = mix(*c, Rgba([1.0, 0.96, 0.88, 1.0]), 0.45);
                out.push(circle(hx, hy, hr, radial(hx, hy, hr, bloom_stops(core, a * 0.5))));
            }
            if self.streak > 0.0 {
                // Anamorphic streak: a very flat ellipse through the bloom.
                let sr = big * 0.7;
                let sy = 0.035 + 0.02 * n(9.0, 1.0);
                let sa = (a * self.streak * 0.8).min(1.0);
                out.push(Op::RoundRect { x: -sr, y: -sr, w: 2.0 * sr, h: 2.0 * sr, radius: 0.0, paint: radial(0.0, 0.0, sr, bloom_stops(mix(*c, Rgba([1.0, 0.95, 0.9, 1.0]), 0.3), sa)), transform: [1.0, 0.0, 0.0, sy, cx, cy] });
            }
        }
        if screen {
            out.push(Op::PopLayer);
        }
        out
    }
}

// ------------------------------------------------------------------ bokeh

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Bokeh {
    pub count: u32,
    /// Light colours (default: warm white, the theme accent and a neighbour of it).
    pub colors: Vec<Color>,
    pub min_size: f32,
    pub max_size: f32,
    pub speed: f32,
    pub seed: u32,
    /// Aperture blades (0 = round discs, 5–8 = polygonal bokeh).
    pub blades: u32,
    /// Depth layers (1–4): far lights are small and crisp, near ones big and soft.
    pub layers: u32,
    /// Brightness of the disc rims (0 = flat discs).
    pub rim: f32,
    /// Chromatic fringe on the disc edges (0 = none).
    pub fringe: f32,
    /// Overall brightness.
    pub intensity: f32,
    /// Drift direction in degrees (default: slowly upwards).
    pub direction: Option<f32>,
    /// `"normal"` (default) or `"screen"` (lights add up where they overlap;
    /// costs a full-frame layer).
    pub blend: Option<String>,
}
impl Default for Bokeh {
    fn default() -> Self {
        Bokeh { count: 30, colors: Vec::new(), min_size: 30.0, max_size: 230.0, speed: 1.0, seed: 5, blades: 0, layers: 3, rim: 0.6, fringe: 0.5, intensity: 1.0, direction: None, blend: None }
    }
}
impl Prepare for Bokeh {}

fn polygon(cx: f32, cy: f32, r: f32, n: u32, rot: f32) -> Path {
    let mut p = Path::new();
    // Slightly curved blades, like a real iris.
    let pts: Vec<(f32, f32)> = (0..n).map(|i| {
        let a = rot + i as f32 / n as f32 * TAU;
        (cx + r * a.cos(), cy + r * a.sin())
    }).collect();
    let bulge = 1.0 + 0.12 / n as f32 * 6.0 * 0.5;
    for i in 0..n as usize {
        let (x0, y0) = pts[i];
        let (x1, y1) = pts[(i + 1) % n as usize];
        let am = rot + (i as f32 + 0.5) / n as f32 * TAU;
        let rm = r * (TAU / (2.0 * n as f32)).cos() * bulge;
        let (mx, my) = (cx + rm * am.cos(), cy + rm * am.sin());
        if i == 0 {
            p.move_to((x0 as f64, y0 as f64));
        }
        p.quad_to(((2.0 * mx - 0.5 * (x0 + x1)) as f64, (2.0 * my - 0.5 * (y0 + y1)) as f64), (x1 as f64, y1 as f64));
    }
    p.close_path();
    p
}

impl Component for Bokeh {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let cols = colors_or(&self.colors, &[hx3("#ffd59a"), bgfx::hue_shift(th.accent, 0.0, 1.0, 0.08), bgfx::hue_shift(th.accent, 45.0, 0.9, 0.1), hx3("#fff1de")]);
        let (w, h) = (ctx.width, ctx.height);
        let mut rng = Rng::new(self.seed as u64);
        let layers = self.layers.clamp(1, 4);
        let dir = self.direction.unwrap_or(-80.0).to_radians();
        let (dcx, dcy) = (dir.cos(), dir.sin());
        struct B {
            depth: f32,
            x: f32,
            y: f32,
            r: f32,
            c: Rgba,
            a: f32,
            rot: f32,
        }
        let mut bs: Vec<B> = Vec::with_capacity(self.count as usize);
        for i in 0..self.count {
            let (x0, y0) = (rng.f(), rng.f());
            let l = (rng.f() * layers as f32).floor().min(layers as f32 - 1.0);
            let depth = if layers > 1 { l / (layers - 1) as f32 } else { 0.5 };
            let jitter = rng.f();
            let size = lerp(self.min_size, self.max_size, (depth * 0.75 + jitter * 0.25).clamp(0.0, 1.0));
            let ph = rng.f() * TAU;
            let sp = rng.range(0.7, 1.3);
            let rot = rng.f() * TAU;
            // Parallax: near lights drift faster.
            let v = (6.0 + 22.0 * depth) * sp * self.speed;
            let m = 2.0 * size;
            let x = (x0 * (w + 2.0 * m) + dcx * v * t + 6.0 * (t * 0.3 * sp + ph).sin()).rem_euclid(w + 2.0 * m) - m;
            let y = (y0 * (h + 2.0 * m) + dcy * v * t).rem_euclid(h + 2.0 * m) - m;
            let c = cols[i as usize % cols.len()];
            // Far lights are small, bright and crisp; near ones big and faint.
            let breathe = 0.85 + 0.15 * (t * 0.5 * sp + ph).sin();
            let a = self.intensity * (0.6 - 0.3 * depth) * breathe * rng.range(0.55, 1.0);
            bs.push(B { depth, x, y, r: size * 0.5, c, a, rot });
        }
        bs.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        let mut out = Vec::new();
        let screen = self.blend.as_deref() == Some("screen");
        if screen {
            push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        }
        let rim = self.rim.max(0.0);
        for b in &bs {
            let soft = 0.03 + 0.10 * b.depth; // edge softness grows with defocus
            let a = b.a.clamp(0.0, 1.0);
            let stops = vec![
                (0.0, alpha(b.c, a * 0.72)),
                (0.6, alpha(b.c, a * 0.78)),
                (1.0 - soft - 0.1, alpha(b.c, a * (0.82 + 0.15 * rim))),
                (1.0 - soft, alpha(b.c, a * (0.9 + 0.45 * rim))),
                (1.0, alpha(b.c, 0.0)),
            ];
            let paint = radial(b.x, b.y, b.r, stops);
            let path = if self.blades >= 3 { polygon(b.x, b.y, b.r, self.blades, b.rot) } else { circle_path(b.x, b.y, b.r) };
            out.push(fill(path, paint));
            if self.fringe > 0.0 && b.r > 6.0 {
                // Lateral chromatic aberration: a cool fringe pushed away from
                // the frame centre, a warm one pulled towards it.
                let (vx, vy) = ((b.x - w * 0.5) / w, (b.y - h * 0.5) / h);
                let off = b.r * 0.035 * self.fringe;
                for (tint, sgn) in [(Rgba([0.35, 0.65, 1.0, 1.0]), 1.0f32), (Rgba([1.0, 0.45, 0.3, 1.0]), -1.0)] {
                    let (fx, fy) = (b.x + vx * off * 2.0 * sgn, b.y + vy * off * 2.0 * sgn);
                    let fr = b.r * 1.01;
                    let fa = a * 0.55 * self.fringe;
                    let paint = radial(fx, fy, fr, vec![(0.0, alpha(tint, 0.0)), (1.0 - soft - 0.06, alpha(tint, 0.0)), (1.0 - soft * 0.6, alpha(tint, fa)), (1.0, alpha(tint, 0.0))]);
                    out.push(fill(circle_path(fx, fy, fr), paint));
                }
            }
        }
        if screen {
            out.push(Op::PopLayer);
        }
        out
    }
}

// ------------------------------------------------------------------ starfield

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Starfield {
    pub count: u32,
    /// One colour for every star (default: natural white with blue and warm tints).
    pub color: Option<Color>,
    /// Fly through the stars (warp speed) instead of twinkling in place.
    pub fly: bool,
    pub speed: f32,
    pub seed: u32,
    /// Twinkle strength (0 = steady).
    pub twinkle: f32,
    /// Soft halos around the brightest stars (0 = none).
    pub glow: f32,
    /// Diffraction spikes on the brightest stars.
    pub spikes: bool,
    /// Streak length when flying (1 = default, 4 = hyperspace).
    pub warp: f32,
    /// Parallax drift in pixels per second for the nearest layer, `[x, y]`.
    pub drift: Option<[f32; 2]>,
    /// Colour variety (0 = all one colour, 1 = natural tints).
    pub tint: f32,
    /// Overall brightness.
    pub brightness: f32,
}
impl Default for Starfield {
    fn default() -> Self {
        Starfield { count: 700, color: None, fly: false, speed: 1.0, seed: 9, twinkle: 0.6, glow: 0.6, spikes: false, warp: 1.0, drift: None, tint: 0.6, brightness: 1.0 }
    }
}
impl Prepare for Starfield {}
impl Component for Starfield {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        use kurbo::Shape;
        let (w, h) = (ctx.width, ctx.height);
        let base = self.color.map(|c| c.0).unwrap_or(Rgba([0.95, 0.96, 1.0, 1.0]));
        let tints = [base, mix(base, Rgba([0.62, 0.76, 1.0, 1.0]), self.tint.clamp(0.0, 1.0)), mix(base, Rgba([1.0, 0.84, 0.66, 1.0]), self.tint.clamp(0.0, 1.0))];
        let mut rng = Rng::new(self.seed as u64);
        let lvn = levels(14);
        let mut buckets: Vec<Path> = (0..lvn * 3).map(|_| Path::new()).collect();
        let mut out = Vec::new();
        let mut halos = Vec::new();
        let bright = self.brightness.max(0.0);
        let [dx, dy] = self.drift.unwrap_or([0.0, 0.0]);
        for _ in 0..self.count {
            let (a, b, z0, ph, tw) = (rng.f(), rng.f(), rng.f(), rng.f() * TAU, rng.range(0.8, 3.2));
            let m = rng.f();
            let ti = { let r = rng.f(); if r < 0.55 { 0 } else if r < 0.8 { 1 } else { 2 } };
            // Magnitude: many faint stars, few bright ones.
            let mag = m.powf(5.0);
            if self.fly {
                let z = (z0 - t * 0.16 * self.speed).rem_euclid(1.0).max(0.012);
                let (sx, sy) = ((a - 0.5) * 2.0, (b - 0.5) * 2.0);
                let proj = |z: f32| (w * 0.5 + sx * w * 0.1 / z, h * 0.5 + sy * w * 0.1 / z);
                let (x, y) = proj(z);
                if x < -40.0 || x > w + 40.0 || y < -40.0 || y > h + 40.0 {
                    continue;
                }
                let near = 1.0 - z;
                // Fade in from the far plane: no popping at wrap-around.
                let fade = smooth01(near * 4.0);
                let al = (fade * (0.25 + 0.75 * near * near) * (0.55 + 0.45 * mag.sqrt()) * bright).min(1.0);
                let len = 0.035 * self.warp * self.speed.abs().max(0.05);
                let (x1, y1) = proj((z + len).min(1.2));
                let width = 0.6 + 2.4 * near * near * near + 1.2 * mag;
                let d = ((x - x1).powi(2) + (y - y1).powi(2)).sqrt();
                let c = tints[ti];
                if d > 2.0 {
                    let mut p = Path::new();
                    p.move_to((x1 as f64, y1 as f64));
                    p.line_to((x as f64, y as f64));
                    out.push(Op::Stroke { path: p, paint: Paint::Linear { p0: [x1, y1], p1: [x, y], stops: vec![(0.0, alpha(c, 0.0)), (0.7, alpha(c, al * 0.5)), (1.0, alpha(c, al))] }, stroke: line(width, [0.0, 1.0]), transform: IDENTITY });
                } else {
                    let lv = ((al * lvn as f32) as usize).min(lvn - 1);
                    buckets[ti * lvn + lv].extend(kurbo::Circle::new((x as f64, y as f64), (width * 0.5) as f64).path_elements(0.05));
                }
            } else {
                // Three depth layers (z0): far = faint, slow; near = bright, faster parallax.
                let depth = z0;
                let x = (a * w + dx * t * (0.15 + 0.85 * depth)).rem_euclid(w);
                let y = (b * h + dy * t * (0.15 + 0.85 * depth)).rem_euclid(h);
                let r = 0.38 + 0.55 * depth + 1.5 * mag;
                let tw_k = self.twinkle.clamp(0.0, 1.0) * (0.35 + 0.65 * (1.0 - depth));
                let k = 1.0 - tw_k * 0.5 * (1.0 + (t * tw + ph).sin() * (t * tw * 0.37 + ph * 1.7).cos());
                let al = ((0.18 + 0.5 * depth * depth + 0.6 * mag) * k * bright).min(1.0);
                let lv = ((al * lvn as f32) as usize).min(lvn - 1);
                buckets[ti * lvn + lv].extend(kurbo::Circle::new((x as f64, y as f64), r as f64).path_elements(0.05));
                if mag > 0.35 && self.glow > 0.0 {
                    halos.push((x, y, r, al, tints[ti]));
                }
            }
        }
        let mut stars = Vec::new();
        for (i, p) in buckets.into_iter().enumerate() {
            if p.is_empty() {
                continue;
            }
            let (ti, lv) = (i / lvn, i % lvn);
            stars.push(fill(p, solid(alpha(tints[ti], (lv as f32 + 0.5) / lvn as f32))));
        }
        for (x, y, r, al, c) in halos {
            let hr = r * (6.0 + 6.0 * self.glow);
            out.push(circle(x, y, hr, radial(x, y, hr, bloom_stops(c, al * 0.22 * self.glow.min(2.0)))));
            if self.spikes {
                let sl = r * 14.0;
                for (ax, ay) in [(1.0f32, 0.0f32), (0.0, 1.0)] {
                    let mut p = Path::new();
                    p.move_to(((x - ax * sl) as f64, (y - ay * sl) as f64));
                    p.line_to(((x + ax * sl) as f64, (y + ay * sl) as f64));
                    let st = vec![(0.0, alpha(c, 0.0)), (0.5, alpha(c, al * 0.6)), (1.0, alpha(c, 0.0))];
                    out.push(Op::Stroke { path: p, paint: Paint::Linear { p0: [x - ax * sl, y - ay * sl], p1: [x + ax * sl, y + ay * sl], stops: st }, stroke: line(0.8, [0.0, 1.0]), transform: IDENTITY });
                }
            }
        }
        // Halos and streaks under the crisp star cores.
        out.extend(stars);
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

    /// Fields move continuously: consecutive frames differ a little, never a lot.
    #[test]
    fn aurora_is_smooth_in_time() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("aurora", &json!({"stars": 0})).unwrap();
        let f = |t: f32| preview::snapshot("bg-aurora-t", 480, 270, th.background, c.eval(t, 480.0, 270.0, 60.0, &th, a), a);
        let (p0, p1) = (f(2.0), f(2.0 + 1.0 / 60.0));
        let diff = p0.iter().zip(&p1).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as u64).sum::<u64>() as f64 / p0.len() as f64;
        assert!(diff > 0.0 && diff < 3.0, "mean abs frame difference {diff}");
    }

    /// Eval cost at 1080p: `BG=aurora cargo test --release -p anikit-video bg_eval_ms -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bg_eval_ms() {
        let th = Theme::default();
        let a = preview::host();
        let only = std::env::var("BG").ok();
        for (name, p) in [
            ("aurora", json!({})),
            ("mesh-gradient", json!({})),
            ("grain", json!({})),
            ("light-leak", json!({})),
            ("bokeh", json!({})),
            ("starfield", json!({})),
            ("starfield", json!({"fly": true})),
            ("grid", json!({"perspective": true, "scan": 3, "glow": 0.6})),
            ("dots", json!({"wave": true})),
            ("gradient", json!({})),
        ] {
            if only.as_deref().is_some_and(|o| o != name) {
                continue;
            }
            let p = std::env::var("BGP").ok().map(|s| serde_json::from_str(&s).unwrap()).unwrap_or(p);
            let c = build(name, &p).unwrap();
            let n = 30;
            let t0 = std::time::Instant::now();
            for i in 0..n {
                std::hint::black_box(c.eval(1.0 + i as f32 / 60.0, 1920.0, 1080.0, 60.0, &th, a));
            }
            println!("{name:14} {p}: {:.2} ms/frame", t0.elapsed().as_secs_f64() * 1000.0 / n as f64);
        }
    }
}
