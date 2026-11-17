//! Illustrated environments: procedural landscapes, plants, clouds and water
//! drawn in one consistent "hand".
//!
//! * `landscape`: layered parallax ranges (mountains, hills, dunes, mesas,
//!   forests) under a sky with a sun or moon, atmospheric perspective, mist.
//! * `plants`: trees, bushes, grass and flowers along a ground line, swaying.
//! * `clouds`: designed, palette-aware clouds that drift.
//! * `water`: an illustrated water surface with bands, glints and reflections.
//!
//! Every kind shares the [`Hand`] props (`style`, `palette`, `grain`, `light`,
//! `seed`, `boil`, `onTwos`/`stepFps`), so pieces from different components
//! sit in one world. Shapes are painted into a small canvas (colour, coverage
//! and a shading value), then *finished* by the style: flat cel shading,
//! risograph ink plates with halftone and misregistration, painterly brush
//! texture with broken edges, ink washes with hatching and outlines, or torn
//! paper with lifted shadows. Texture is computed in the object's own
//! coordinates, so grain and halftone travel with each layer. Finished
//! layers are cached images; a frame is a handful of blits.

use super::bgfx::{fbm2, hash2, lin3, oklab, oklab_inv, smoothstep, to_srgb, vnoise2};
use super::style::{Color, Rng};
use super::{step_time, Component, Ctx, Error, Prepare};
use crate::dl::{Affine, ImageId, Op, Rgba};
use serde::Deserialize;
use std::sync::{Arc, Mutex};

type C3 = [f32; 3];

// ------------------------------------------------------------------ styles

/// How shapes are finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Crisp vector shapes, cel shading, gradient plus grain.
    Flat,
    /// Two or three spot inks, halftone, misregistration, ink grain.
    Riso,
    /// Soft broken edges, brush-textured colour, canvas tooth.
    Painterly,
    /// Watercolour washes, brush outlines and hatched shadows.
    Ink,
    /// Torn paper with a fibrous rim and a soft lifted shadow.
    Papercut,
}

pub const STYLES: &[&str] = &["flat", "risograph", "painterly", "ink", "papercut"];

pub fn parse_style(s: &str) -> Result<Style, Error> {
    Ok(match s {
        "" | "flat" | "vector" => Style::Flat,
        "risograph" | "riso" => Style::Riso,
        "painterly" | "paint" | "gouache" => Style::Painterly,
        "ink" | "pen" => Style::Ink,
        "papercut" | "paper" | "cutpaper" => Style::Papercut,
        _ => {
            let hint = super::style::closest(s, STYLES).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(Error(format!("unknown style `{s}` (one of {}){hint}", STYLES.join(", "))));
        }
    })
}

// ----------------------------------------------------------------- palette

/// `palette`: a name, or a list of colours `[skyTop, skyHorizon, far, near, sun?, foliage?, accent?]`.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum PalSpec {
    Name(String),
    List(Vec<Color>),
}
impl Default for PalSpec {
    fn default() -> Self {
        PalSpec::Name(String::new())
    }
}

pub const PALETTES: &[&str] = &["dawn", "day", "golden", "dusk", "night", "forest", "desert", "riso"];

/// A resolved environment palette (sRGB, 0..1).
#[derive(Clone, Debug)]
pub struct Palette {
    pub sky_top: C3,
    pub sky_low: C3,
    pub sun: C3,
    pub haze: C3,
    pub far: C3,
    pub near: C3,
    /// Foliage dark, mid, light.
    pub fol: [C3; 3],
    pub accent: C3,
    pub water: C3,
    pub paper: C3,
    pub ink: C3,
    /// Cloud lit and shadow.
    pub cloud: [C3; 2],
    /// Multiply tint for shadows (cool or complementary, never plain grey).
    pub shade: C3,
    /// Risograph spot inks.
    pub inks: [C3; 3],
    pub night: bool,
}

fn hx(s: &str) -> C3 {
    let c = Color::hex(s).0 .0;
    [c[0], c[1], c[2]]
}

/// Mix in OKLab (perceptual).
pub fn mixok(a: C3, b: C3, t: f32) -> C3 {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let (la, lb) = (oklab(lin3(Rgba([a[0], a[1], a[2], 1.0]))), oklab(lin3(Rgba([b[0], b[1], b[2], 1.0]))));
    let l = oklab_inv([0, 1, 2].map(|i| la[i] + (lb[i] - la[i]) * t));
    l.map(|v| to_srgb(v).clamp(0.0, 1.0))
}

fn lerp3(a: C3, b: C3, t: f32) -> C3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

const RISO_INKS: [&str; 3] = ["#ff48b0", "#0078bf", "#ffe800"];

fn named_palette(name: &str) -> Option<Palette> {
    // [sky_top, sky_low, sun, haze, far, near, fol0, fol1, fol2, accent, water, paper, ink, cloud0, cloud1, shade]
    let t: [&str; 16] = match name {
        "dawn" => ["#7e9cc9", "#f7d3be", "#fff1d6", "#e9c9cf", "#a792b8", "#2f2d4e", "#24324a", "#3e5a6e", "#7f9e9a", "#f28c8c", "#6f8fb8", "#f5efe4", "#1e1e2c", "#fff0e6", "#c3a9c4", "#9c8fc4"],
        "day" => ["#4f8fd0", "#cfe6ef", "#fff6da", "#bfd8dc", "#8fb3b8", "#23473a", "#17352b", "#2f6b4a", "#7daf63", "#f2c14e", "#3c7fb0", "#f4f0e6", "#142026", "#ffffff", "#a9c3d6", "#7f9cc8"],
        "golden" => ["#e7a15a", "#fbe3b0", "#fff4d0", "#f1c68e", "#d9915f", "#4a2328", "#3a2226", "#7a4a2e", "#c98b3e", "#e8553a", "#c77b48", "#f6ecd9", "#2a1715", "#fff0d0", "#d98c66", "#a77fa8"],
        "dusk" => ["#26306e", "#f2a08c", "#ffe2a6", "#e59aa6", "#b9779e", "#1d1940", "#1b2440", "#34466e", "#6e86b8", "#ff7a59", "#2f3e86", "#f3ecdd", "#17142b", "#ffd2bf", "#b97a9e", "#7d6cb4"],
        "night" => ["#0a0f2a", "#2c3a6b", "#f4f1de", "#3b4878", "#2c3765", "#070a1a", "#060a18", "#14203d", "#2e4470", "#f7d774", "#1a2550", "#e9e6da", "#05060f", "#5a6b9c", "#222d55", "#5866a8"],
        "forest" => ["#8fa9a3", "#e6e2cf", "#fff4d8", "#c9cfbf", "#8fa597", "#14261f", "#0f1f19", "#2c4a36", "#6f8e5a", "#e9b44c", "#5e7f7a", "#efebdd", "#10160f", "#f4f1e4", "#aab5a8", "#6f86a0"],
        "desert" => ["#f08a4b", "#fbd6a0", "#fff3d1", "#f6c08c", "#eba06a", "#8c3b2a", "#5a3a20", "#8c6a2e", "#c9a15a", "#2e6e7e", "#2e7e8c", "#f6e9d2", "#2b1610", "#fff0dc", "#e3a27a", "#b0789a"],
        "riso" => ["#0078bf", "#ffb3c7", "#ffe800", "#f7c6d4", "#e98bb8", "#1d2b6b", "#1d2b6b", "#3b5fa8", "#7fb0d9", "#ff48b0", "#0078bf", "#f4eee1", "#141a3c", "#fff4f0", "#e7a0c4", "#8a7fc8"],
        _ => return None,
    };
    let c = t.map(hx);
    Some(Palette {
        sky_top: c[0],
        sky_low: c[1],
        sun: c[2],
        haze: c[3],
        far: c[4],
        near: c[5],
        fol: [c[6], c[7], c[8]],
        accent: c[9],
        water: c[10],
        paper: c[11],
        ink: c[12],
        cloud: [c[13], c[14]],
        shade: c[15],
        inks: RISO_INKS.map(hx),
        night: name == "night",
    })
}

impl Palette {
    pub fn resolve(spec: &PalSpec, time: &str) -> Result<Palette, Error> {
        let default = match time {
            "dawn" => "dawn",
            "day" | "noon" => "day",
            "golden" | "sunset" => "golden",
            "night" => "night",
            _ => "dusk",
        };
        match spec {
            PalSpec::Name(n) if n.is_empty() => Ok(named_palette(default).unwrap()),
            PalSpec::Name(n) => named_palette(n).ok_or_else(|| {
                let hint = super::style::closest(n, PALETTES).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                Error(format!("unknown palette `{n}` (one of {}, or a list of colours){hint}", PALETTES.join(", ")))
            }),
            PalSpec::List(cols) => {
                if cols.len() < 4 {
                    return Err(Error("palette list needs at least 4 colours: [skyTop, skyHorizon, far, near, sun?, foliage?, accent?]".into()));
                }
                let c: Vec<C3> = cols.iter().map(|c| [c.0 .0[0], c.0 .0[1], c.0 .0[2]]).collect();
                let mut p = named_palette(default).unwrap();
                let (st, sl, far, near) = (c[0], c[1], c[2], c[3]);
                p.sky_top = st;
                p.sky_low = sl;
                p.far = far;
                p.near = near;
                p.haze = mixok(sl, far, 0.35);
                p.sun = c.get(4).copied().unwrap_or_else(|| mixok(sl, [1.0, 0.97, 0.88], 0.7));
                let f = c.get(5).copied().unwrap_or_else(|| mixok(near, far, 0.3));
                p.fol = [mixok(f, near, 0.6), f, mixok(f, p.sun, 0.35)];
                p.accent = c.get(6).copied().unwrap_or(p.sun);
                p.water = mixok(st, near, 0.3);
                p.cloud = [mixok(sl, [1.0, 1.0, 1.0], 0.6), mixok(sl, far, 0.6)];
                p.shade = mixok(st, [1.0, 1.0, 1.0], 0.35);
                p.ink = mixok(near, [0.0, 0.0, 0.0], 0.5);
                Ok(p)
            }
        }
    }
}

// -------------------------------------------------------------------- hand

/// The shared look every environment kind draws with: style, palette,
/// texture amount, light direction and seed. `illo` and other illustrated
/// kinds can build one from the same props (`style`, `palette`, `grain`, `light`).
#[derive(Clone, Debug)]
pub struct Hand {
    pub style: Style,
    pub pal: Palette,
    /// Texture amount 0..1 (grain, brush, fibre).
    pub grain: f32,
    /// Unit vector pointing *towards* the light, y down.
    pub light: [f32; 2],
    pub seed: u32,
    /// Pixels per 1080p pixel (texture sizes scale with the frame).
    pub scale: f32,
}

impl Hand {
    /// `light` is the direction the light comes from, in degrees: 0 from straight
    /// above, negative from the left, positive from the right.
    pub fn new(style: Style, pal: Palette, grain: f32, light_deg: f32, seed: u32, scale: f32) -> Hand {
        let a = light_deg.to_radians();
        Hand { style, pal, grain: grain.clamp(0.0, 2.0), light: [a.sin(), -a.cos()], seed, scale: scale.max(0.25) }
    }
    fn key(&self, h: &mut super::fxpix::Hash) {
        h.u32(self.style as u32);
        let p = &self.pal;
        for c in [p.sky_top, p.sky_low, p.sun, p.haze, p.far, p.near, p.fol[0], p.fol[1], p.fol[2], p.accent, p.water, p.paper, p.ink, p.cloud[0], p.cloud[1], p.shade] {
            h.f32s(&c);
        }
        h.f32s(&[self.grain, self.light[0], self.light[1], self.scale]);
        h.u32(self.seed);
    }
    /// Shade a colour: `s > 0` multiplies towards the palette's shadow tint, `s < 0` lifts towards the light colour.
    #[inline]
    pub fn shade(&self, c: C3, s: f32) -> C3 {
        if s >= 0.0 {
            let t = self.pal.shade;
            let k = s.min(1.2);
            // Multiply by the tint, plus a little extra value drop so hues shift and values separate.
            [0, 1, 2].map(|i| c[i] * (1.0 - k * (1.0 - t[i])) * (1.0 - 0.18 * k))
        } else {
            let k = (-s).min(1.0) * 0.6;
            let l = self.pal.sun;
            [0, 1, 2].map(|i| c[i] + (l[i].max(c[i]) - c[i]) * k)
        }
    }
}


// ------------------------------------------------------------------ canvas

/// A paint buffer: premultiplied colour, coverage, a shading value and an ink-line channel.
pub struct Canvas {
    pub w: usize,
    pub h: usize,
    pc: Vec<f32>,
    pub a: Vec<f32>,
    s: Vec<f32>,
    ln: Vec<f32>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        let n = w * h;
        Canvas { w, h, pc: vec![0.0; n * 3], a: vec![0.0; n], s: vec![0.0; n], ln: vec![0.0; n] }
    }
    /// Paint `col` with shading `s` over pixel `i` at coverage `cov`.
    #[inline]
    pub fn put(&mut self, i: usize, cov: f32, col: C3, s: f32) {
        if cov <= 0.0 {
            return;
        }
        let k = 1.0 - cov;
        let p = &mut self.pc[i * 3..i * 3 + 3];
        p[0] = col[0] * cov + p[0] * k;
        p[1] = col[1] * cov + p[1] * k;
        p[2] = col[2] * cov + p[2] * k;
        self.a[i] = cov + self.a[i] * k;
        self.s[i] = s * cov + self.s[i] * k;
        self.ln[i] *= k;
    }
    /// Mark an explicit ink line (drawn by the ink style; a darker crease in others).
    #[inline]
    pub fn line(&mut self, i: usize, cov: f32) {
        if cov > self.ln[i] {
            self.ln[i] = cov;
        }
    }
    /// Fill a disc (anti-aliased) with a shading function of the local unit offset.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, col: C3, shade: &dyn Fn(f32, f32) -> f32) {
        let (x0, x1) = (((cx - r - 1.0).floor().max(0.0)) as usize, ((cx + r + 1.0).ceil().max(0.0) as usize).min(self.w));
        let (y0, y1) = (((cy - r - 1.0).floor().max(0.0)) as usize, ((cy + r + 1.0).ceil().max(0.0) as usize).min(self.h));
        let inv = 1.0 / r.max(0.5);
        for y in y0..y1 {
            let dy = y as f32 + 0.5 - cy;
            for x in x0..x1 {
                let dx = x as f32 + 0.5 - cx;
                let d = (dx * dx + dy * dy).sqrt();
                let cov = (r - d + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    self.put(y * self.w + x, cov, col, shade(dx * inv, dy * inv));
                }
            }
        }
    }
    /// A tapered capsule from `a` (radius `ra`) to `b` (radius `rb`).
    pub fn limb(&mut self, a: [f32; 2], b: [f32; 2], ra: f32, rb: f32, col: C3, s_lit: f32, s_dark: f32, light: [f32; 2]) {
        let r = ra.max(rb);
        let (x0, x1) = (((a[0].min(b[0]) - r - 1.0).floor().max(0.0)) as usize, ((a[0].max(b[0]) + r + 1.0).ceil().max(0.0) as usize).min(self.w));
        let (y0, y1) = (((a[1].min(b[1]) - r - 1.0).floor().max(0.0)) as usize, ((a[1].max(b[1]) + r + 1.0).ceil().max(0.0) as usize).min(self.h));
        let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = (vx * vx + vy * vy).max(1e-4);
        let len = l2.sqrt();
        let (nx, ny) = (-vy / len, vx / len);
        let side = if nx * light[0] + ny * light[1] > 0.0 { 1.0 } else { -1.0 };
        for y in y0..y1 {
            let py = y as f32 + 0.5;
            for x in x0..x1 {
                let px = x as f32 + 0.5;
                let u = (((px - a[0]) * vx + (py - a[1]) * vy) / l2).clamp(0.0, 1.0);
                let (qx, qy) = (a[0] + vx * u - px, a[1] + vy * u - py);
                let d = (qx * qx + qy * qy).sqrt();
                let rr = ra + (rb - ra) * u;
                let cov = (rr - d + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    // Signed offset across the limb: the light side is lit.
                    let o = -(qx * nx + qy * ny) * side / rr.max(0.5);
                    let s = if o > 0.15 { s_lit } else { s_dark };
                    self.put(y * self.w + x, cov, col, s);
                }
            }
        }
    }
}

/// Box blur of a plane (`passes` × radius `r`), in place.
fn blur_plane(p: &mut [f32], w: usize, h: usize, r: usize, passes: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0.0f32; w.max(h)];
    let norm = 1.0 / (2 * r + 1) as f32;
    for _ in 0..passes {
        for y in 0..h {
            let row = &mut p[y * w..(y + 1) * w];
            let mut acc = row[0] * r as f32;
            for x in 0..=r.min(w - 1) {
                acc += row[x];
            }
            acc += row[w - 1] * (r + 1).saturating_sub(w) as f32;
            for x in 0..w {
                tmp[x] = acc * norm;
                let add = row[(x + r + 1).min(w - 1)];
                let sub = row[x.saturating_sub(r)];
                acc += add - sub;
            }
            row.copy_from_slice(&tmp[..w]);
        }
        for x in 0..w {
            let mut acc = p[x] * r as f32;
            for y in 0..=r.min(h - 1) {
                acc += p[y * w + x];
            }
            acc += p[(h - 1) * w + x] * (r + 1).saturating_sub(h) as f32;
            for y in 0..h {
                tmp[y] = acc * norm;
                let add = p[(y + r + 1).min(h - 1) * w + x];
                let sub = p[y.saturating_sub(r) * w + x];
                acc += add - sub;
            }
            for y in 0..h {
                p[y * w + x] = tmp[y];
            }
        }
    }
}

#[inline]
fn h01(x: i32, y: i32, seed: u32) -> f32 {
    hash2(x, y, seed) * 0.5 + 0.5
}

/// How a canvas is finished.
#[derive(Clone, Copy)]
pub struct Finish {
    /// Texture origin: the canvas's position in its own world (object-locked texture).
    pub ox: f32,
    pub oy: f32,
    /// Break up and tear edges (off for full-frame skies).
    pub edges: bool,
    /// Lifted paper shadow (papercut) / soft halo, when the canvas has a margin for it.
    pub shadow: bool,
    /// Edge re-draw variant (boil).
    pub variant: u32,
    /// Quantise shading to cel steps (flat, riso, papercut).
    pub hard: bool,
}

/// Turn a painted canvas into premultiplied RGBA8 pixels in `hand`'s style.
pub fn finish(hand: &Hand, cv: &Canvas, f: Finish) -> Vec<u8> {
    let (w, h) = (cv.w, cv.h);
    let n = w * h;
    let sc = hand.scale;
    let g = hand.grain;
    let seed = hand.seed.wrapping_mul(747_796_405).wrapping_add(f.variant.wrapping_mul(2_891_336_453));
    let st = hand.style;
    // 1. Edges: blur the coverage a little and threshold it against noise.
    let mut a = cv.a.clone();
    if f.edges {
        let (r, amp, cell) = match st {
            Style::Flat => (1.0, 0.22, 5.0),
            Style::Riso => (1.0, 0.3, 4.0),
            Style::Painterly => (3.0, 0.75, 7.0),
            Style::Ink => (1.0, 0.3, 4.0),
            Style::Papercut => (2.0, 0.55, 3.0),
        };
        let rr = (r * sc).round().max(1.0) as usize;
        blur_plane(&mut a, w, h, rr, 2);
        let cs = cell * sc;
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let v = a[i];
                if v <= 0.0 || v >= 1.0 {
                    a[i] = if v >= 1.0 { 1.0 } else { 0.0 };
                    continue;
                }
                let (px, py) = (x as f32 + f.ox, y as f32 + f.oy);
                let mut nz = vnoise2(px / cs, py / cs, seed ^ 0x51) * 0.6 + vnoise2(px / (cs * 3.7), py / (cs * 3.7), seed ^ 0x77) * 0.4;
                if st == Style::Painterly {
                    // Dry-brush: streaky along x.
                    nz = nz * 0.6 + vnoise2(px / (cs * 4.0), py / (cs * 0.35), seed ^ 0x99) * 0.4;
                }
                let t = v + nz * amp * 0.5;
                let aa = 0.5 / (rr as f32 * 1.2);
                a[i] = smoothstep(0.5 - aa, 0.5 + aa, t);
            }
        }
    }
    // Soft edge band for rims/outlines.
    let need_band = f.edges && matches!(st, Style::Ink | Style::Papercut | Style::Painterly);
    let band = if need_band {
        let mut b = a.clone();
        let r = match st {
            Style::Ink => 2.0,
            Style::Papercut => 2.5,
            _ => 4.0,
        };
        blur_plane(&mut b, w, h, (r * sc).round().max(1.0) as usize, 2);
        b
    } else {
        Vec::new()
    };
    // 2. Colour.
    let mut out = vec![0.0f32; n * 4];
    let pal = &hand.pal;
    let ink = pal.ink;
    let paper = pal.paper;
    let hatch_sp = 6.5 * sc;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let al = a[i];
            if al <= 0.0 {
                continue;
            }
            let ca = cv.a[i].max(1e-4);
            let mut c = [cv.pc[i * 3] / ca, cv.pc[i * 3 + 1] / ca, cv.pc[i * 3 + 2] / ca];
            let mut s = cv.s[i] / ca;
            let (px, py) = (x as f32 + f.ox, y as f32 + f.oy);
            if f.hard {
                let aa = 0.05;
                s = smoothstep(0.28 - aa, 0.28 + aa, s) * 0.55 + smoothstep(0.75 - aa, 0.75 + aa, s) * 0.3 - smoothstep(-0.3 + aa, -0.3 - aa, s) * 0.45;
            }
            match st {
                Style::Flat | Style::Riso => {
                    c = hand.shade(c, s);
                    if cv.ln[i] > 0.0 {
                        c = hand.shade(c, cv.ln[i] * 0.5);
                    }
                }
                Style::Painterly => {
                    // Brush strokes: anisotropic value and hue variation, wet blooms.
                    // Domain-warped strokes so marks curve like a loaded brush.
                    let wx = vnoise2(px / (90.0 * sc), py / (90.0 * sc), seed ^ 0x11) * 30.0 * sc;
                    let wy = vnoise2(px / (90.0 * sc), py / (90.0 * sc), seed ^ 0x12) * 12.0 * sc;
                    let (qx, qy) = (px + wx, py + wy);
                    let streak = vnoise2(qx / (55.0 * sc), qy / (9.0 * sc), seed ^ 0x13);
                    let dab = vnoise2(qx / (16.0 * sc), qy / (11.0 * sc), seed ^ 0x17);
                    let bloom = fbm2(px / (180.0 * sc), py / (140.0 * sc), 3, seed ^ 0x19);
                    let k = if f.edges { 1.0 } else { 0.45 };
                    let s2 = s + (streak * 0.13 + dab * 0.07) * (0.4 + g) * k;
                    c = hand.shade(c, s2);
                    let v = 1.0 + bloom * 0.07 * (0.5 + g);
                    let warm = bloom * 0.025;
                    c = [c[0] * v + warm, c[1] * v, c[2] * v - warm];
                    if cv.ln[i] > 0.0 {
                        c = hand.shade(c, cv.ln[i] * 0.35);
                    }
                    // Paint is thinner at the rim: a darker pooled edge.
                    if !band.is_empty() {
                        let e = smoothstep(0.98, 0.7, band[i]) * smoothstep(0.2, 0.5, band[i]);
                        c = hand.shade(c, e * 0.08);
                    }
                }
                Style::Ink => {
                    // A transparent wash of the colour over paper; pooled at edges.
                    let blotch = fbm2(px / (110.0 * sc), py / (80.0 * sc), 3, seed ^ 0x23);
                    let wash = 0.66 + blotch * if f.edges { 0.09 } else { 0.04 };
                    let base = hand.shade(c, s * 0.45);
                    c = lerp3(paper, base, wash.clamp(0.0, 1.0));
                    if !band.is_empty() {
                        let pool = smoothstep(0.98, 0.7, band[i]);
                        c = lerp3(c, base, pool * 0.35);
                    }
                    // Hatching follows the shading: denser and crossed in deep shadow.
                    let mut hatch = 0.0f32;
                    if s > 0.15 {
                        let wob = vnoise2(px / (40.0 * sc), py / (40.0 * sc), seed ^ 0x29) * 0.35;
                        let u = (px * 0.7071 + py * 0.7071) / hatch_sp + wob;
                        let d = (u - u.floor() - 0.5).abs();
                        let wdt = ((s - 0.15) * 0.55).min(0.32);
                        hatch = smoothstep(wdt + 0.08, wdt - 0.04, d);
                        if s > 0.55 {
                            let v = (px * 0.7071 - py * 0.7071) / (hatch_sp * 1.1) - wob;
                            let d2 = (v - v.floor() - 0.5).abs();
                            let w2 = ((s - 0.55) * 0.5).min(0.28);
                            hatch = hatch.max(smoothstep(w2 + 0.08, w2 - 0.04, d2));
                        }
                    }
                    // Outline: a band inside the edge, its width breathing like a brush.
                    let mut line = cv.ln[i];
                    if !band.is_empty() {
                        let wv = 0.72 + vnoise2(px / (60.0 * sc), py / (60.0 * sc), seed ^ 0x31) * 0.16;
                        line = line.max(smoothstep(wv, wv - 0.14, band[i]));
                    }
                    let inkc = mixok(ink, c, 0.12);
                    c = lerp3(c, inkc, (hatch * 0.75).max(line));
                }
                Style::Papercut => {
                    c = hand.shade(c, s);
                    // Paper fibre and tooth.
                    let fib = vnoise2(px / (3.0 * sc), py / (14.0 * sc), seed ^ 0x41) * 0.5 + vnoise2(px / (1.5 * sc), py / (1.5 * sc), seed ^ 0x43) * 0.5;
                    let v = 1.0 + fib * 0.035 * (0.5 + g);
                    c = c.map(|q| q * v);
                    if cv.ln[i] > 0.0 {
                        c = hand.shade(c, cv.ln[i] * 0.4);
                    }
                    // The torn rim shows the paper's white core.
                    if !band.is_empty() {
                        let wv = 0.68 + vnoise2(px / (5.0 * sc), py / (5.0 * sc), seed ^ 0x45) * 0.18;
                        let rim = smoothstep(wv, wv - 0.12, band[i]) * al;
                        let core = lerp3(c, [0.99, 0.97, 0.93], 0.85);
                        c = lerp3(c, core, rim);
                    }
                }
            }
            // Grain locked to the object: fine speckle plus a soft mottle.
            if g > 0.0 && st != Style::Riso {
                let fine = h01(px as i32, py as i32, seed ^ 0x61) - 0.5;
                let mott = vnoise2(px / (5.0 * sc), py / (5.0 * sc), seed ^ 0x63);
                let k = g * (0.032 * fine + 0.022 * mott);
                c = c.map(|q| q + k);
            }
            let o = &mut out[i * 4..i * 4 + 4];
            o[0] = c[0];
            o[1] = c[1];
            o[2] = c[2];
            o[3] = al;
        }
    }
    // 3. Risograph: split into ink plates, misregister, halftone, print on paper.
    if st == Style::Riso {
        riso_print(hand, &mut out, w, h, f, seed);
    }
    // 4. Lifted shadow / halo.
    if f.shadow && matches!(st, Style::Papercut) {
        let mut sh: Vec<f32> = (0..n).map(|i| out[i * 4 + 3]).collect();
        let rr = (7.0 * sc).round().max(1.0) as usize;
        blur_plane(&mut sh, w, h, rr, 3);
        let (dx, dy) = ((-hand.light[0] * 3.0 * sc).round() as isize, (-6.0 * sc).round() as isize);
        let sc_col = hand.shade(pal.near, 1.0);
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let (sx, sy) = (x as isize - dx, y as isize - dy);
                let v = if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h { sh[sy as usize * w + sx as usize] } else { 0.0 };
                let sa = v * 0.42;
                let o = &mut out[i * 4..i * 4 + 4];
                let fa = o[3];
                // layer over shadow
                let na = fa + sa * (1.0 - fa);
                if na > 0.0 {
                    for k in 0..3 {
                        o[k] = (o[k] * fa + sc_col[k] * sa * (1.0 - fa)) / na;
                    }
                }
                o[3] = na;
            }
        }
    }
    // 5. Quantise with a blue-ish dither (premultiplied bytes).
    let mut px = vec![0u8; n * 4];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let o = &out[i * 4..i * 4 + 4];
            let al = o[3].clamp(0.0, 1.0);
            if al <= 0.0 {
                continue;
            }
            let d = h01(x as i32, y as i32, 0x0d17) - 0.5;
            let ab = (al * 255.0 + d).round().clamp(0.0, 255.0);
            for k in 0..3 {
                px[i * 4 + k] = (o[k].clamp(0.0, 1.0) * al * 255.0 + d).round().clamp(0.0, ab) as u8;
            }
            px[i * 4 + 3] = ab as u8;
        }
    }
    px
}

/// Risograph printing: decompose each pixel's colour into ink densities, then
/// print each plate as an offset, angled halftone with patchy ink.
fn riso_print(hand: &Hand, out: &mut [f32], w: usize, h: usize, f: Finish, seed: u32) {
    let pal = &hand.pal;
    let n = w * h;
    let mut cov = vec![0.0f32; n * 3];
    let mut al = vec![0.0f32; n];
    let mut memo: std::collections::HashMap<u32, C3> = std::collections::HashMap::new();
    for i in 0..n {
        al[i] = out[i * 4 + 3];
        if al[i] <= 0.0 {
            continue;
        }
        let q = |v: f32| (v.clamp(0.0, 1.0) * 63.0).round() as u32;
        let key = q(out[i * 4]) << 12 | q(out[i * 4 + 1]) << 6 | q(out[i * 4 + 2]);
        let c = *memo.entry(key).or_insert_with(|| {
            let t = [(key >> 12) as f32 / 63.0, ((key >> 6) & 63) as f32 / 63.0, (key & 63) as f32 / 63.0];
            ink_split(pal, t)
        });
        let c = if f.hard {
            // Riso artists print a few tints, not continuous tone.
            c.map(|v| {
                let k = (v * 4.0).round() / 4.0;
                k + (v - k) * 0.25
            })
        } else {
            c
        };
        cov[i * 3..i * 3 + 3].copy_from_slice(&c);
    }
    let sc = hand.scale;
    let mut rng = Rng::new(seed as u64 ^ 0x5157);
    let offs: [[isize; 2]; 3] = [0, 1, 2].map(|k| {
        if k == 1 {
            [0, 0]
        } else {
            [(rng.range(-3.0, 3.0) * sc).round() as isize, (rng.range(-2.5, 2.5) * sc).round() as isize]
        }
    });
    let angles = [15.0f32, 75.0, 0.0].map(|a: f32| a.to_radians().sin_cos());
    let cell = 5.0 * sc;
    let tau = std::f32::consts::TAU;
    let g = hand.grain;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let (px, py) = (x as f32 + f.ox, y as f32 + f.oy);
            let mut col = pal.paper;
            let mut amax = 0.0f32;
            for k in 0..3 {
                let (sx, sy) = (x as isize - offs[k][0], y as isize - offs[k][1]);
                if sx < 0 || sy < 0 || sx as usize >= w || sy as usize >= h {
                    continue;
                }
                let j = sy as usize * w + sx as usize;
                let a = al[j];
                if a <= 0.0 {
                    continue;
                }
                amax = amax.max(a);
                let mut c = cov[j * 3 + k];
                // Patchy ink: drum starvation blotches plus speckle.
                let blot = vnoise2(px / (40.0 * sc), py / (40.0 * sc), seed ^ (0x700 + k as u32));
                let speck = h01(px as i32, py as i32, seed ^ (0x710 + k as u32));
                c *= 1.0 - g * (0.1 + 0.1 * blot) - g * 0.18 * (speck > 0.93) as u8 as f32;
                if c <= 0.004 {
                    continue;
                }
                let (sn, cs) = angles[k];
                let u = (px * cs + py * sn) / cell;
                let v = (-px * sn + py * cs) / cell;
                let spot = 0.5 + 0.25 * ((u * tau).cos() + (v * tau).cos());
                let ht = if c > 0.97 { 1.0 } else { smoothstep(-0.07, 0.07, c + spot - 1.0) };
                let ink = pal.inks[k];
                let t = ht * a;
                for q in 0..3 {
                    col[q] *= 1.0 - t * (1.0 - ink[q]);
                }
            }
            let o = &mut out[i * 4..i * 4 + 4];
            // Soft coverage (mist, glows) prints as a dot screen of paper.
            let fa = if amax >= 0.98 {
                1.0
            } else {
                let u = (px + py) * 0.7071 / cell;
                let v = (py - px) * 0.7071 / cell;
                let spot = 0.5 + 0.25 * ((u * tau).cos() + (v * tau).cos());
                smoothstep(-0.07, 0.07, amax + spot - 1.0) * amax.min(1.0).sqrt().max(amax)
            };
            o[0] = col[0];
            o[1] = col[1];
            o[2] = col[2];
            o[3] = fa.min(1.0);
        }
    }
}

/// Ink coverages that print `target` on the paper: halftone dots of independent
/// plates average to `paper × Π(1 − c·(1 − ink))` per channel; solved by
/// damped Gauss-Newton in log space.
fn ink_split(pal: &Palette, target: C3) -> C3 {
    let paper = pal.paper.map(|v| v.max(0.05));
    let goal: C3 = [0, 1, 2].map(|ch| (target[ch].max(0.02) / paper[ch]).min(1.0).ln());
    let ab: [C3; 3] = pal.inks.map(|ink| [0, 1, 2].map(|ch| (1.0 - ink[ch] / paper[ch]).clamp(0.0, 0.97)));
    let mut c = [0.3f32; 3];
    for _ in 0..14 {
        let mut f = [0.0f32; 3];
        let mut j = [[0.0f32; 3]; 3];
        for ch in 0..3 {
            let mut v = 0.0;
            for k in 0..3 {
                let d = 1.0 - c[k] * ab[k][ch];
                v += d.ln();
                j[ch][k] = -ab[k][ch] / d;
            }
            f[ch] = v - goal[ch];
        }
        let mut jtj = [[0.0f32; 3]; 3];
        let mut jtf = [0.0f32; 3];
        for a in 0..3 {
            for b in 0..3 {
                jtj[a][b] = (0..3).map(|ch| j[ch][a] * j[ch][b]).sum::<f32>() + if a == b { 0.01 } else { 0.0 };
            }
            jtf[a] = (0..3).map(|ch| j[ch][a] * f[ch]).sum();
        }
        let inv = inv3(jtj);
        for a in 0..3 {
            let dc: f32 = (0..3).map(|b| inv[a][b] * jtf[b]).sum();
            c[a] = (c[a] - dc).clamp(0.0, 1.0);
        }
    }
    c
}

fn inv3(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let d = if det.abs() < 1e-8 { 1e-8 } else { det };
    let id = 1.0 / d;
    [
        [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * id, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * id, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * id],
        [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * id, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * id, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * id],
        [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * id, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * id, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * id],
    ]
}

// ------------------------------------------------------------ shared props

fn default_grain() -> f32 {
    0.5
}

/// Stepped time for animated parts (sway, glints, boil); camera moves stay smooth.
fn anim_time(t: f32, on_twos: bool, step_fps: Option<f32>) -> f32 {
    step_time(t, on_twos, step_fps)
}

fn boil_variant(t: f32, boil: f32) -> u32 {
    if boil > 0.0 {
        ((t * boil).floor().max(0.0) as u32) % 3
    } else {
        0
    }
}

/// Image op at an integer position (fast blit) or through `m` (resampled).
fn image_op(id: ImageId, x: f32, y: f32, w: f32, h: f32, m: Option<Affine>) -> Op {
    match m {
        None => Op::Image { image: id, dst: [x.round(), y.round(), w, h], opacity: 1.0, transform: crate::dl::IDENTITY },
        Some(m) => Op::Image { image: id, dst: [x, y, w, h], opacity: 1.0, transform: m },
    }
}

// ----------------------------------------------------------------- terrain

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Terr {
    Mountains,
    Hills,
    Dunes,
    Mesas,
    Forest,
    Woods,
}
const TERRAINS: &[&str] = &["mountains", "hills", "dunes", "mesas", "forest", "woods", "mixed"];

fn parse_terr(s: &str) -> Result<Terr, Error> {
    Ok(match s {
        "mountains" | "mountain" | "peaks" => Terr::Mountains,
        "hills" | "hill" => Terr::Hills,
        "dunes" | "dune" => Terr::Dunes,
        "mesas" | "mesa" | "buttes" => Terr::Mesas,
        "forest" | "pines" | "trees" => Terr::Forest,
        "woods" | "canopy" => Terr::Woods,
        _ => {
            let hint = super::style::closest(s, TERRAINS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(Error(format!("unknown terrain `{s}` (one of {}){hint}", TERRAINS.join(", "))));
        }
    })
}

/// Periodic smooth 1D value noise in [-1, 1]: `x` in cells, period `n` cells.
fn pnoise(x: f32, n: u32, seed: u32) -> f32 {
    let n = n.max(1) as i32;
    let i = x.floor();
    let f = x - i;
    let i = (i as i32).rem_euclid(n);
    let j = (i + 1) % n;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash2(i, 0, seed);
    let b = hash2(j, 0, seed);
    a + (b - a) * u
}

/// Periodic fbm over `[0, period)` px with base cell count `cells`.
fn pfbm(x: f32, period: f32, cells: u32, oct: u32, seed: u32) -> f32 {
    let (mut s, mut a, mut nrm, mut c) = (0.0, 1.0, 0.0, cells.max(1));
    for o in 0..oct {
        s += a * pnoise(x / period * c as f32, c, seed.wrapping_add(o * 131));
        nrm += a;
        a *= 0.5;
        c *= 2;
    }
    s / nrm
}

#[derive(Clone, Debug)]
struct Peak {
    x: f32,
    h: f32,
    sl: f32,
    sr: f32,
    round: f32,
    skew: f32,
    /// Ground line under a tree (forests).
    foot: f32,
}

/// One range: its silhouette and how its surface is lit.
#[derive(Clone, Debug)]
struct Geo {
    terr: Terr,
    d: f32,
    base: f32,
    amp: f32,
    p: f32,
    seed: u32,
    sc: f32,
    peaks: Vec<Peak>,
    /// Peaks front to back (who overlaps whom).
    order: Vec<usize>,
}

impl Geo {
    fn new(terr: Terr, d: f32, base: f32, amp: f32, p: f32, w: f32, seed: u32, sc: f32) -> Geo {
        let mut rng = Rng::new(seed as u64 * 7919 + 17);
        let mut peaks = Vec::new();
        match terr {
            Terr::Mountains => {
                let n = ((p / (w * (0.13 + 0.09 * d))).round() as usize).max(3);
                for k in 0..n {
                    let x = (k as f32 + rng.range(0.15, 0.85)) * p / n as f32;
                    let big = rng.f();
                    peaks.push(Peak {
                        x,
                        h: amp * (0.35 + 0.65 * big * big.sqrt()).max(rng.range(0.3, 0.6)),
                        sl: rng.range(0.42, 1.0),
                        sr: rng.range(0.42, 1.0),
                        round: rng.range(3.0, 9.0) * sc,
                        skew: rng.range(-0.35, 0.35),
                        foot: 0.0,
                    });
                }
            }
            Terr::Dunes => {
                let n = ((p / (w * (0.28 + 0.12 * d))).round() as usize).max(2);
                for k in 0..n {
                    let x = (k as f32 + rng.range(0.2, 0.8)) * p / n as f32;
                    let (wind, lee) = (rng.range(0.14, 0.26), rng.range(0.42, 0.6));
                    peaks.push(Peak { x, h: amp * rng.range(0.45, 1.0), sl: wind, sr: lee, round: rng.range(4.0, 14.0) * sc, skew: rng.range(0.25, 0.7), foot: 0.0 });
                }
            }
            Terr::Forest | Terr::Woods => {
                let sp = if terr == Terr::Forest { (7.0 + 16.0 * d) * sc } else { (26.0 + 50.0 * d) * sc };
                let n = ((p / sp).round() as usize).max(4);
                for k in 0..n {
                    let x = (k as f32 + rng.range(-0.45, 0.45)) * p / n as f32;
                    let grove = 0.5 + 0.5 * pfbm(x, p, (p / (260.0 * sc)).max(2.0) as u32, 2, seed ^ 0xabc);
                    let hh = amp * (0.3 + 0.7 * grove) * rng.range(0.6, 1.0);
                    if grove < 0.18 && rng.f() < 0.7 {
                        continue;
                    }
                    let wd = if terr == Terr::Forest { hh * rng.range(0.2, 0.3) } else { sp * rng.range(0.6, 1.0) };
                    peaks.push(Peak { x, h: hh, sl: wd, sr: wd, round: rng.range(3.0, 6.0), skew: rng.range(-0.1, 0.1), foot: 0.0 });
                }
                // Draw tall ones last so they sit in front.
                peaks.sort_by(|a, b| a.h.partial_cmp(&b.h).unwrap());
            }
            _ => {}
        }
        let mut order: Vec<usize> = (0..peaks.len()).collect();
        let pri: Vec<f32> = peaks.iter().map(|pk| pk.h + rng.range(0.0, 0.35) * amp).collect();
        order.sort_by(|a, b| pri[*b].partial_cmp(&pri[*a]).unwrap());
        let mut g = Geo { terr, d, base, amp, p, seed, sc, peaks, order };
        if matches!(terr, Terr::Forest | Terr::Woods) {
            let cells = (p / (700.0 * sc)).max(2.0) as u32;
            let feet: Vec<f32> = g.peaks.iter().map(|pk| g.base - amp * 0.3 * g.hill(pk.x, cells, 2)).collect();
            for (pk, f) in g.peaks.iter_mut().zip(feet) {
                pk.foot = f;
            }
        }
        g
    }

    #[inline]
    fn wrapd(&self, dx: f32) -> f32 {
        let p = self.p;
        let mut d = dx % p;
        if d > p * 0.5 {
            d -= p;
        } else if d < -p * 0.5 {
            d += p;
        }
        d
    }

    /// Height (px above the base) of peak `k`'s tent at `x`.
    #[inline]
    fn tent(&self, k: usize, x: f32) -> f32 {
        let pk = &self.peaks[k];
        let dx = self.wrapd(x - pk.x);
        let s = if dx < 0.0 { pk.sl } else { pk.sr };
        let r = pk.round;
        pk.h - s * ((dx * dx + r * r).sqrt() - r)
    }

    /// Ridge roughness (px) at `x` for a ridge `v` px above the base.
    fn jag(&self, x: f32, v: f32) -> f32 {
        let (amp, sc) = (self.amp, self.sc);
        if self.terr == Terr::Mountains {
            let k = (v / amp).clamp(0.2, 1.0);
            amp * 0.045 * k * pfbm(x, self.p, (self.p / (38.0 * sc)) as u32, 3, self.seed ^ 0x3333)
        } else {
            amp * 0.012 * pfbm(x, self.p, (self.p / (60.0 * sc)) as u32, 2, self.seed ^ 0x3333)
        }
    }

    /// Height above the base line (px) for the low rolling ground under forests.
    fn hill(&self, x: f32, cells: u32, oct: u32) -> f32 {
        0.5 + 0.5 * pfbm(x, self.p, cells, oct, self.seed ^ 0x1111)
    }

    /// Ridge y (px) and the index of the shape that owns this column.
    fn ridge(&self, x: f32) -> (f32, usize) {
        let (amp, sc) = (self.amp, self.sc);
        match self.terr {
            Terr::Mountains | Terr::Dunes => {
                let mut best = f32::MIN;
                let mut bi = 0;
                for k in 0..self.peaks.len() {
                    let v = self.tent(k, x);
                    if v > best {
                        best = v;
                        bi = k;
                    }
                }
                let foot = amp * (0.08 + 0.06 * pfbm(x, self.p, 6, 2, self.seed ^ 0x2222));
                (self.base - best.max(foot) - self.jag(x, best), bi)
            }
            Terr::Hills => {
                let cells = (self.p / (self.amp * 7.0 + 300.0 * sc)).max(2.0) as u32;
                let v = 0.5 + 0.5 * pfbm(x, self.p, cells, 3, self.seed);
                (self.base - amp * v, 0)
            }
            Terr::Mesas => {
                let cells = (self.p / (520.0 * sc)).max(2.0) as u32;
                let q = 0.5 + 0.5 * pfbm(x, self.p, cells, 2, self.seed);
                let plateau = smoothstep(0.5, 0.515, q);
                let tier = smoothstep(0.66, 0.672, q);
                let ground = 0.12 + 0.06 * pfbm(x, self.p, cells * 3, 2, self.seed ^ 0x55);
                let edge = 0.012 * pfbm(x, self.p, (self.p / (14.0 * sc)) as u32, 2, self.seed ^ 0x66);
                let v = ground + plateau * 0.55 + tier * 0.28 + edge * (plateau + 0.3);
                (self.base - amp * v, 0)
            }
            Terr::Forest | Terr::Woods => {
                let g = self.base - amp * 0.3 * self.hill(x, (self.p / (700.0 * sc)).max(2.0) as u32, 2);
                let mut best = g;
                let mut bi = usize::MAX;
                for k in 0..self.peaks.len() {
                    if let Some(y) = self.tree_top(k, x) {
                        if y < best {
                            best = y;
                            bi = k;
                        }
                    }
                }
                (best, bi)
            }
        }
    }

    /// Top of tree `k`'s silhouette at column `x`, if the tree covers it.
    #[inline]
    fn tree_top(&self, k: usize, x: f32) -> Option<f32> {
        let pk = &self.peaks[k];
        let dx = self.wrapd(x - pk.x).abs();
        if dx >= pk.sl {
            return None;
        }
        let top = pk.foot - pk.h;
        Some(if self.terr == Terr::Forest {
            // Conifer: tiers of jagged boughs.
            let u = dx / pk.sl;
            let saw = (u * 5.0 + pk.skew * 3.0).fract();
            top + pk.h * u * (0.9 + 0.18 * saw) * 0.95
        } else {
            let r = pk.sl;
            top + r - (r * r - dx * dx).max(0.0).sqrt()
        })
    }

    /// Shading at a pixel below the ridge: (s, explicit line).
    fn shade_at(&self, x: f32, y: f32, r: f32, owner: usize, slope: f32, light: [f32; 2]) -> (f32, f32) {
        let depth = y - r;
        let sc = self.sc;
        let lx = if light[0] >= 0.0 { 1.0 } else { -1.0 };
        // Surface normal at the ridge (y down): (slope, -1).
        let nl = (slope * light[0] - light[1]) / (1.0 + slope * slope).sqrt();
        let rim = if depth < 2.2 * sc && nl > 0.35 { -0.6 } else { 0.0 };
        match self.terr {
            Terr::Mountains | Terr::Dunes => {
                let pk = &self.peaks[owner.min(self.peaks.len().saturating_sub(1))];
                let top = self.base - pk.h;
                let dy = (y - top).max(0.0);
                let curve = if self.terr == Terr::Dunes { dy * dy / (self.amp * 1.5) * pk.skew.signum() } else { 0.0 };
                let wob = vnoise2(y / (30.0 * sc), pk.x * 0.01, self.seed ^ 0x77) * 7.0 * sc;
                let split = pk.x + dy * pk.skew + curve + wob;
                let dx = self.wrapd(x - split);
                let side = dx * lx; // > 0: towards the light
                let lit = smoothstep(-0.8, 0.8, side);
                let mut s = 0.62 * (1.0 - lit) - 0.12 * lit;
                if self.terr == Terr::Mountains {
                    // Rock ribs and gullies running down the faces.
                    // Streaks follow the fall line, away from the crease.
                    let u = dx - dy * 0.45 * dx.signum();
                    let rib = vnoise2(u / (9.0 * sc), dy / (150.0 * sc), self.seed ^ 0x88);
                    let fade = smoothstep(self.amp * 1.1, self.amp * 0.1, dy) * smoothstep(0.0, 20.0 * sc, dy);
                    s += fade * if lit > 0.5 { smoothstep(0.42, 0.52, rib) * 0.4 } else { -smoothstep(0.5, 0.6, rib) * 0.38 };
                } else {
                    // Wind ripples on the lit face.
                    let rp = ((y + 0.35 * x) / (9.0 * sc) + vnoise2(x / (80.0 * sc), y / (40.0 * sc), self.seed) * 1.5).sin();
                    s += lit * smoothstep(0.85, 0.97, rp) * 0.3;
                }
                let crease = smoothstep(1.4 * sc, 0.3 * sc, dx.abs()) * smoothstep(self.amp * 0.9, 0.0, dy);
                ((s + rim).min(s), crease)
            }
            Terr::Hills => {
                let fall = (-depth / (self.amp * 0.55 + 1.0)).exp();
                let s = (0.25 - nl * 0.85) * fall + 0.12 * (1.0 - fall);
                (s.min(s + rim), 0.0)
            }
            Terr::Mesas => {
                let steep = smoothstep(1.2, 3.0, slope.abs());
                let face = if slope * lx < 0.0 { -0.25 } else { 0.6 };
                let strata = ((y / (11.0 * sc) + vnoise2(x / (200.0 * sc), y / (50.0 * sc), self.seed) * 0.8).fract() < 0.22) as u8 as f32;
                let cap = smoothstep(4.0 * sc, 1.0 * sc, depth);
                let gully = smoothstep(0.55, 0.7, vnoise2(x / (12.0 * sc), y / (140.0 * sc), self.seed ^ 0x99));
                let s = 0.18 + steep * (face - 0.18) + strata * 0.16 - cap * 0.5 + gully * 0.3;
                (s, strata * 0.4)
            }
            Terr::Forest | Terr::Woods => {
                let mut s = 0.3;
                let mut depth = depth;
                if owner != usize::MAX {
                    let pk = &self.peaks[owner];
                    depth = y - (pk.foot - pk.h);
                    let dx = self.wrapd(x - pk.x);
                    let (lit, sh) = if self.terr == Terr::Forest { (-0.15, 0.5) } else { (-0.2, 0.45) };
                    let side = dx * lx;
                    s = if self.terr == Terr::Woods {
                        // Round crowns: light from the light's side and top.
                        let u = dx / pk.sl;
                        let v = (depth / pk.sl).min(1.0);
                        let nl = u * light[0] * lx.abs() + (1.0 - v) * -light[1] - 0.3;
                        if nl > 0.15 {
                            lit
                        } else {
                            sh
                        }
                    } else if side > 0.0 {
                        lit
                    } else {
                        sh
                    };
                }
                if owner != usize::MAX {
                    s += smoothstep(0.0, self.peaks[owner].h * 1.1, depth) * 0.3;
                }
                (s + rim * 0.5, 0.0)
            }
        }
    }
}

// --------------------------------------------------------------- landscape

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Terrains {
    One(String),
    Many(Vec<String>),
}
impl Default for Terrains {
    fn default() -> Self {
        Terrains::One("mixed".into())
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Landscape {
    /// `flat`, `risograph`, `painterly`, `ink` or `papercut`.
    pub style: String,
    /// Palette name (`dawn day golden dusk night forest desert riso`) or colours.
    pub palette: PalSpec,
    /// Texture amount 0..1.
    pub grain: f32,
    /// Light direction in degrees (0 = from above, negative = from the left); default: from the sun.
    pub light: Option<f32>,
    pub seed: u32,
    /// Edge re-draws per second (0 = still edges).
    pub boil: f32,
    /// Step animated parts at 12 fps.
    pub on_twos: bool,
    /// Step animated parts at this rate (overrides `onTwos`).
    pub step_fps: Option<f32>,
    /// `dawn`, `day`, `golden`, `dusk` or `night`.
    pub time: String,
    /// Number of ranges, far to near.
    pub layers: usize,
    /// Terrain for every layer, or a list cycled far → near (`mountains hills dunes mesas forest woods`, or `mixed`).
    pub terrain: Terrains,
    /// Horizon height as a fraction of the box (0 top .. 1 bottom).
    pub horizon: f32,
    /// How far down the frame the nearest range reaches (0 = all at the horizon).
    pub depth: f32,
    /// Height multiplier for every range.
    pub relief: f32,
    /// Mist 0..1: pooled haze at the foot of each range plus drifting bands.
    pub mist: f32,
    /// Camera pan speed at the nearest range (px/s); farther ranges move less.
    pub parallax: f32,
    /// Multiplane push-in per second (fraction, e.g. 0.01); nearer ranges grow faster.
    pub push: f32,
    /// Sun (or moon) centre as fractions of the box.
    pub sun: Option<[f32; 2]>,
    /// Sun size multiplier (0 hides it).
    pub sun_size: f32,
    /// Draw the sky (off to put ranges over your own background).
    pub sky: bool,
    #[serde(skip)]
    plan: Mutex<Option<Arc<Plan>>>,
}

impl Default for Landscape {
    fn default() -> Self {
        Landscape {
            style: "flat".into(),
            palette: PalSpec::default(),
            grain: default_grain(),
            light: None,
            seed: 1,
            boil: 0.0,
            on_twos: false,
            step_fps: None,
            time: "dusk".into(),
            layers: 5,
            terrain: Terrains::default(),
            horizon: 0.62,
            depth: 0.75,
            relief: 1.0,
            mist: 0.4,
            parallax: 18.0,
            push: 0.0,
            sun: None,
            sun_size: 1.0,
            sky: true,
            plan: Mutex::new(None),
        }
    }
}

const TIMES: &[&str] = &["dawn", "day", "golden", "dusk", "night"];

impl Prepare for Landscape {
    fn prepare(&mut self) -> Result<(), Error> {
        parse_style(&self.style)?;
        if !TIMES.contains(&self.time.as_str()) {
            let hint = super::style::closest(&self.time, TIMES).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
            return Err(Error(format!("unknown time `{}` (one of {}){hint}", self.time, TIMES.join(", "))));
        }
        Palette::resolve(&self.palette, &self.time)?;
        if self.layers == 0 || self.layers > 12 {
            return Err(Error("`layers` must be 1..12".into()));
        }
        match &self.terrain {
            Terrains::One(s) if s == "mixed" => {}
            Terrains::One(s) => {
                parse_terr(s)?;
            }
            Terrains::Many(v) => {
                if v.is_empty() {
                    return Err(Error("`terrain` list is empty".into()));
                }
                for s in v {
                    parse_terr(s)?;
                }
            }
        }
        self.horizon = self.horizon.clamp(0.1, 0.95);
        self.depth = self.depth.clamp(0.0, 1.0);
        self.mist = self.mist.clamp(0.0, 1.0);
        Ok(())
    }
}

struct LayerPlan {
    geo: Geo,
    col: C3,
    /// Image rows [top, bottom) in box pixels.
    top: usize,
    bottom: usize,
    speed: f32,
    key: u64,
}

struct MistPlan {
    y: f32,
    h: usize,
    speed: f32,
    alpha: f32,
    key: u64,
    seed: u32,
}

struct Plan {
    w: usize,
    h: usize,
    p: usize,
    hand: Hand,
    sun: [f32; 2],
    sun_r: f32,
    layers: Vec<LayerPlan>,
    mists: Vec<MistPlan>,
    sky_key: u64,
    sun_key: u64,
    mist_amt: f32,
}

impl Landscape {
    fn sun_pos(&self) -> [f32; 2] {
        if let Some(s) = self.sun {
            return s;
        }
        let hz = self.horizon;
        match self.time.as_str() {
            "dawn" => [0.27, hz - 0.07],
            "day" => [0.7, hz * 0.3],
            "golden" => [0.7, hz - 0.2],
            "night" => [0.72, hz * 0.32],
            _ => [0.56, hz - 0.17],
        }
    }

    fn terrains(&self) -> Vec<Terr> {
        let n = self.layers;
        match &self.terrain {
            Terrains::One(s) if s == "mixed" => (0..n)
                .map(|i| {
                    let d = if n > 1 { i as f32 / (n - 1) as f32 } else { 1.0 };
                    if d < 0.45 {
                        Terr::Mountains
                    } else if i == n - 2 {
                        Terr::Forest
                    } else {
                        Terr::Hills
                    }
                })
                .collect(),
            Terrains::One(s) => vec![parse_terr(s).unwrap(); n],
            Terrains::Many(v) => (0..n).map(|i| parse_terr(&v[i % v.len()]).unwrap()).collect(),
        }
    }

    fn plan(&self, w: usize, h: usize) -> Arc<Plan> {
        if let Ok(g) = self.plan.lock() {
            if let Some(p) = g.as_ref().filter(|p| p.w == w && p.h == h) {
                return p.clone();
            }
        }
        let p = Arc::new(self.make_plan(w, h));
        if let Ok(mut g) = self.plan.lock() {
            *g = Some(p.clone());
        }
        p
    }

    fn make_plan(&self, w: usize, h: usize) -> Plan {
        let style = parse_style(&self.style).unwrap();
        let pal = Palette::resolve(&self.palette, &self.time).unwrap();
        let sc = h as f32 / 1080.0;
        let sun = self.sun_pos();
        let light = self.light.unwrap_or_else(|| {
            let a = ((sun[0] - 0.5) * 140.0).clamp(-70.0, 70.0);
            if a.abs() < 35.0 { 35.0f32.copysign(if a == 0.0 { 1.0 } else { a }) } else { a }
        });
        let hand = Hand::new(style, pal.clone(), self.grain, light, self.seed, sc);
        let p = ((w as f32 * 1.6).round() as usize).max(w + 2);
        let hz = h as f32 * self.horizon;
        let n = self.layers;
        let terrs = self.terrains();
        let mut base_key = super::fxpix::Hash::new();
        hand.key(&mut base_key);
        base_key.f32s(&[self.horizon, self.depth, self.relief, self.mist, w as f32, h as f32]);
        base_key.u32(n as u32);
        for t in &terrs {
            base_key.u32(*t as u32);
        }
        let mut layers = Vec::with_capacity(n);
        for (i, &terr) in terrs.iter().enumerate() {
            let d = if n > 1 { i as f32 / (n - 1) as f32 } else { 1.0 };
            let base = hz + (h as f32 - hz) * self.depth * d.powf(1.3) * 0.9;
            let amp_f = match terr {
                Terr::Mountains => 0.30 * (1.0 - 0.45 * d),
                Terr::Hills => 0.11,
                Terr::Dunes => 0.12 * (0.7 + 0.3 * d),
                Terr::Mesas => 0.2 * (0.8 + 0.2 * d),
                Terr::Forest => 0.10 * (0.6 + 0.4 * d),
                Terr::Woods => 0.09 * (0.6 + 0.4 * d),
            };
            let amp = h as f32 * amp_f * self.relief;
            let seed = self.seed.wrapping_mul(1013).wrapping_add(i as u32 * 7919);
            let geo = Geo::new(terr, d, base, amp, p as f32, w as f32, seed, sc);
            // Atmospheric perspective: far ranges take the haze's value and lose chroma.
            let atm = (1.0 - d).powf(1.25);
            let mut col = mixok(pal.near, pal.far, atm.min(1.0) * 0.92);
            col = mixok(col, pal.haze, atm * 0.42);
            layers.push(LayerPlan { geo, col, top: 0, bottom: h, speed: self.parallax * (0.05 + 0.95 * d.powf(1.6)), key: 0 });
        }
        // Rows each layer needs: from its highest ridge to just below the next layer's lowest ridge.
        let ridges: Vec<(f32, f32)> = layers
            .iter()
            .map(|l| {
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                let mut x = 0.0;
                while x < p as f32 {
                    let (r, _) = l.geo.ridge(x);
                    lo = lo.min(r);
                    hi = hi.max(r);
                    x += 2.0;
                }
                (lo, hi)
            })
            .collect();
        let margin = (24.0 * sc) as usize;
        for i in 0..n {
            let top = (ridges[i].0 as isize - margin as isize).max(0) as usize;
            let bottom = if i + 1 < n { ((ridges[i + 1].1 as usize) + margin * 2).min(h) } else { h };
            let l = &mut layers[i];
            l.top = top.min(h.saturating_sub(1));
            l.bottom = bottom.max(l.top + 1);
            let mut k = super::fxpix::Hash::new();
            k.u32(base_key.finish() as u32);
            k.u32((base_key.finish() >> 32) as u32);
            k.u32(i as u32);
            l.key = k.finish();
        }
        let mut mists = Vec::new();
        if self.mist > 0.0 && n > 1 {
            for i in 0..n - 1 {
                let l = &layers[i];
                let hb = ((l.geo.amp * 0.45 + 50.0 * sc) as usize).max(8);
                let y = l.geo.base - hb as f32 * 0.55;
                let d = l.geo.d;
                let mut k = super::fxpix::Hash::new();
                k.u32(base_key.finish() as u32);
                k.u32(0x3157 + i as u32);
                mists.push(MistPlan { y, h: hb, speed: l.speed * 1.4 + 6.0 * sc, alpha: (self.mist * (0.95 - 0.35 * d)).min(0.85), key: k.finish(), seed: self.seed ^ (i as u32 * 31 + 5) });
            }
        }
        let sun_r = h as f32 * 0.055 * self.sun_size;
        let sky_key = {
            let mut k = super::fxpix::Hash::new();
            k.u32(base_key.finish() as u32);
            k.f32s(&[sun[0], sun[1], sun_r, 0x5c1 as f32]);
            k.finish()
        };
        let sun_key = {
            let mut k = super::fxpix::Hash::new();
            k.u32(sky_key as u32);
            k.u32(0x5a);
            k.finish()
        };
        Plan { w, h, p, hand, sun, sun_r, layers, mists, sky_key, sun_key, mist_amt: self.mist }
    }
}

/// Paint one range into a `p × (bottom - top)` canvas.
fn paint_layer(pl: &Plan, l: &LayerPlan) -> Canvas {
    let (p, top, bottom) = (pl.p, l.top, l.bottom);
    let rows = bottom - top;
    let mut cv = Canvas::new(p, rows);
    let hand = &pl.hand;
    let g = &l.geo;
    let ridge: Vec<(f32, usize)> = (0..p).map(|x| g.ridge(x as f32 + 0.5)).collect();
    let light = hand.light;
    let k_shade = 0.4 + 0.6 * g.d;
    let fade = g.amp * 0.9 + 60.0 * hand.scale;
    let mist = pl.mist_amt;
    let faceted = matches!(g.terr, Terr::Mountains | Terr::Dunes);
    let treed = matches!(g.terr, Terr::Forest | Terr::Woods);
    let mut cands: Vec<(usize, f32)> = Vec::new();
    let mut tops: Vec<f32> = vec![0.0; g.order.len()];
    let mut rng = Rng::new(g.seed as u64 ^ 0x7055);
    for x in 0..p {
        let (r, col_owner) = ridge[x];
        let slope = (ridge[(x + 1) % p].0 - ridge[(x + p - 1) % p].0) * 0.5;
        if treed {
            cands.clear();
            let xf = x as f32 + 0.5;
            for k in 0..g.peaks.len() {
                if let Some(t) = g.tree_top(k, xf) {
                    cands.push((k, t));
                }
            }
        }
        if faceted {
            let xf = x as f32 + 0.5;
            for (j, &k) in g.order.iter().enumerate() {
                let v = g.tent(k, xf);
                tops[j] = g.base - v - g.jag(xf, v);
            }
        }
        let y0 = (r.floor() as isize - top as isize).max(0) as usize;
        for yy in y0..rows {
            let y = (yy + top) as f32 + 0.5;
            let cov = (y - r + 0.5).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let (mut owner, mut edge) = (col_owner, 0.0f32);
            if faceted {
                for (j, &k) in g.order.iter().enumerate() {
                    if y >= tops[j] - 0.5 {
                        owner = k;
                        // A crease where a nearer peak overlaps a farther one.
                        if k != col_owner {
                            edge = smoothstep(1.6 * hand.scale, 0.2, y - tops[j]);
                        }
                        break;
                    }
                }
            }
            if treed {
                owner = usize::MAX;
                // Front-most (tallest) tree containing the pixel.
                for &(k, t) in cands.iter().rev() {
                    if y >= t - 0.5 && y <= g.peaks[k].foot + 2.0 {
                        owner = k;
                        break;
                    }
                }
            }
            let (s, ln) = g.shade_at(x as f32 + 0.5, y, r, owner, slope, light);
            let ln = ln.max(edge);
            // Haze pools at the foot of each range.
            let pool = smoothstep(0.0, fade, y - r) * mist * (0.9 - 0.45 * g.d);
            let c = if pool > 0.002 { lerp3(l.col, hand.pal.haze, pool) } else { l.col };
            let i = yy * p + x;
            cv.put(i, cov, c, s * k_shade * (1.0 - pool));
            if ln > 0.0 {
                cv.line(i, ln * k_shade);
            }
        }
    }
    if g.terr == Terr::Hills && g.d > 0.3 {
        // Hedgerows following the hill contours, broken into copses.
        let fol = hand.pal.fol;
        let rows_n = 3;
        let sc = hand.scale;
        for j in 0..rows_n {
            let off = g.amp * (0.22 + 0.42 * j as f32) + 8.0 * sc;
            let rr = (3.5 + 2.5 * j as f32) * sc * (0.4 + g.d);
            let mut x = rng.f() * 20.0;
            while x < p as f32 {
                let gap = pfbm(x, p as f32, (p as f32 / (300.0 * sc)) as u32, 2, g.seed ^ (0x4e0 + j as u32));
                let xi = (x as usize).min(p - 1);
                let r0 = ridge[xi].0;
                let wav = pfbm(x, p as f32, (p as f32 / (500.0 * sc)) as u32, 2, g.seed ^ (0x4f0 + j as u32)) * g.amp * 0.18;
                let y = r0 + off + wav;
                if gap > -0.1 && y > top as f32 && (y as usize) < top + rows {
                    let pool = smoothstep(0.0, fade, y - r0) * mist * (0.9 - 0.45 * g.d);
                    let base = lerp3(mixok(l.col, fol[1], 0.3), hand.pal.haze, pool);
                    let r = rr * rng.range(0.75, 1.3) * (1.0 + gap.max(0.0));
                    let lx = light[0];
                    cv.disc(x, y - r * 0.4 - top as f32, r, base, &move |u, v| if u * lx - v * 0.6 > 0.2 { -0.25 } else { 0.4 });
                }
                x += rr * rng.range(0.9, 1.5);
            }
        }
    }
    cv
}

fn paint_sky(pl: &Plan) -> Canvas {
    let (w, h) = (pl.w, pl.h);
    let pal = &pl.hand.pal;
    let mut cv = Canvas::new(w, h);
    let hz = pl.layers.first().map(|l| l.geo.base).unwrap_or(h as f32 * 0.6).max(1.0);
    let (sx, sy) = (pl.sun[0] * w as f32, pl.sun[1] * h as f32);
    let r = pl.sun_r.max(1.0);
    let glow_c = if pal.night { mixok(pal.sky_low, pal.sun, 0.25) } else { pal.sun };
    let steps = 256;
    let ramp: Vec<C3> = (0..steps).map(|i| mixok(pal.sky_top, pal.sky_low, (i as f32 / (steps - 1) as f32).powf(1.6))).collect();
    let mut rng = Rng::new(pl.hand.seed as u64 + 99);
    for y in 0..h {
        let t = (y as f32 / hz).clamp(0.0, 1.0);
        let base = ramp[((t * (steps - 1) as f32) as usize).min(steps - 1)];
        for x in 0..w {
            let (dx, dy) = (x as f32 - sx, (y as f32 - sy) * 1.25);
            let d = (dx * dx + dy * dy).sqrt();
            // Wide horizon glow plus a tighter halo.
            let g1 = (-d / (r * 9.0)).exp() * 0.55;
            let g2 = (-d / (r * 2.2)).exp() * 0.5;
            let hzg = (-(hz - y as f32).abs() / (hz * 0.35)).exp() * (-(dx.abs()) / (w as f32 * 0.6)).exp() * 0.3;
            let k = (g1 + g2 + hzg).min(1.0) * if pal.night { 0.55 } else { 1.0 };
            let c = lerp3(base, glow_c, k);
            cv.put(y * w + x, 1.0, c, 0.0);
        }
    }
    if pal.night {
        // Stars: a few bright, many faint.
        let n = (w * h) / 2600;
        for _ in 0..n {
            let (x, y) = (rng.f() * w as f32, rng.f() * hz * 0.92);
            let b = rng.f().powi(3);
            let rr = (0.6 + b * 1.6) * pl.hand.scale;
            let col = lerp3(pal.sky_low, [1.0, 0.97, 0.9], 0.4 + 0.6 * b);
            cv.disc(x, y, rr, col, &|_, _| 0.0);
        }
    }
    cv
}

fn paint_sun(pl: &Plan, m: usize) -> Canvas {
    let r = pl.sun_r;
    let size = (r * 2.0) as usize + 2 * m;
    let mut cv = Canvas::new(size, size);
    let c = size as f32 * 0.5;
    let pal = &pl.hand.pal;
    let light = pl.hand.light;
    if pal.night {
        let dark = mixok(pal.sun, pal.sky_low, 0.5);
        cv.disc(c, c, r, pal.sun, &|u, v| {
            let crater = ((u * 3.1 + 0.4).sin() * (v * 2.7 - 0.2).cos()).max(0.0);
            if u * 0.8 + v * 0.3 > 0.55 { 0.35 } else { crater * 0.25 }
        });
        let _ = dark;
    } else {
        cv.disc(c, c, r, pal.sun, &|u, v| if u * light[0] + v * light[1] < -0.55 { 0.12 } else { 0.0 });
    }
    cv
}

fn paint_mist(pl: &Plan, m: &MistPlan) -> Canvas {
    let (p, hb) = (pl.p, m.h);
    let mut cv = Canvas::new(p, hb);
    let sc = pl.hand.scale;
    let col = lerp3(pl.hand.pal.haze, [1.0, 1.0, 1.0], 0.2);
    let cells = (p as f32 / (380.0 * sc)).max(2.0) as u32;
    let soft = pl.hand.style == Style::Painterly;
    let strip = pl.hand.style == Style::Papercut;
    for x in 0..p {
        let xf = x as f32;
        let wisp = 0.5 + 0.5 * pfbm(xf, p as f32, cells, 3, m.seed);
        // A designed fog shape: a wavy top edge, fading downwards.
        let top = hb as f32 * (0.18 + 0.32 * (1.0 - wisp));
        let bot = hb as f32 * (0.8 + 0.15 * pfbm(xf, p as f32, cells * 2, 2, m.seed ^ 7));
        for y in 0..hb {
            let yf = y as f32 + 0.5;
            let edge = if soft { smoothstep(top - 10.0 * sc, top + 14.0 * sc, yf) } else { (yf - top + 0.5).clamp(0.0, 1.0) };
            let a = if strip {
                edge * (bot - yf + 0.5).clamp(0.0, 1.0)
            } else {
                edge * (1.0 - smoothstep(top, hb as f32, yf)).powf(0.8)
            };
            let a = a * m.alpha;
            if a > 0.004 {
                cv.put(y * p + x, a, col, 0.0);
            }
        }
    }
    cv
}

impl Component for Landscape {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width.round().clamp(16.0, 8192.0) as usize, ctx.height.round().clamp(16.0, 8192.0) as usize);
        let pl = self.plan(w, h);
        let ta = anim_time(t, self.on_twos, self.step_fps);
        let variant = boil_variant(ta, self.boil);
        let hand = &pl.hand;
        let hard = matches!(hand.style, Style::Flat | Style::Riso | Style::Papercut);
        let mut ops = Vec::with_capacity(4 + pl.layers.len() * 3);
        let cx = w as f32 * 0.5;
        let cy = pl.layers.first().map(|l| l.geo.base).unwrap_or(h as f32 * 0.6);
        // Camera stays smooth (not stepped).
        let push_m = |d: f32| -> Option<Affine> {
            if self.push == 0.0 {
                return None;
            }
            let s = 1.0 + self.push * t * (0.25 + 0.75 * d);
            Some([s, 0.0, 0.0, s, cx * (1.0 - s), cy * (1.0 - s)])
        };
        if self.sky {
            let id = ctx.assets.image_premul(pl.sky_key, w as u32, h as u32, &mut || {
                finish(hand, &paint_sky(&pl), Finish { ox: 0.0, oy: 0.0, edges: false, shadow: false, variant: 0, hard: false })
            });
            ops.push(Op::Image { image: id, dst: [0.0, 0.0, w as f32, h as f32], opacity: 1.0, transform: crate::dl::IDENTITY });
            if pl.sun_r > 0.5 {
                let m = (16.0 * hand.scale) as usize;
                let size = (pl.sun_r * 2.0) as usize + 2 * m;
                let key = pl.sun_key ^ variant as u64;
                let id = ctx.assets.image_premul(key, size as u32, size as u32, &mut || {
                    finish(hand, &paint_sun(&pl, m), Finish { ox: 0.0, oy: 0.0, edges: true, shadow: true, variant, hard: false })
                });
                let (sx, sy) = (pl.sun[0] * w as f32 - size as f32 * 0.5, pl.sun[1] * h as f32 - size as f32 * 0.5);
                ops.push(image_op(id, sx, sy, size as f32, size as f32, push_m(0.0)));
            }
        }
        let p = pl.p as f32;
        for (i, l) in pl.layers.iter().enumerate() {
            let rows = l.bottom - l.top;
            let key = l.key ^ (variant as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let id = ctx.assets.image_premul(key, pl.p as u32, rows as u32, &mut || {
                let cv = paint_layer(&pl, l);
                finish(hand, &cv, Finish { ox: 0.0, oy: l.top as f32, edges: true, shadow: true, variant, hard })
            });
            let off = -((l.speed * t).rem_euclid(p));
            let m = push_m(l.geo.d);
            ops.push(image_op(id, off, l.top as f32, p, rows as f32, m));
            if off + p < w as f32 + 1.0 {
                ops.push(image_op(id, off + p, l.top as f32, p, rows as f32, m));
            }
            if let Some(ms) = pl.mists.get(i) {
                let key = ms.key ^ (variant as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
                let id = ctx.assets.image_premul(key, pl.p as u32, ms.h as u32, &mut || {
                    let cv = paint_mist(&pl, ms);
                    finish(hand, &cv, Finish { ox: 0.0, oy: ms.y, edges: hand.style == Style::Papercut, shadow: hand.style == Style::Papercut, variant, hard: false })
                });
                let off = -((ms.speed * t).rem_euclid(p));
                ops.push(image_op(id, off, ms.y, p, ms.h as f32, m));
                if off + p < w as f32 + 1.0 {
                    ops.push(image_op(id, off + p, ms.y, p, ms.h as f32, m));
                }
            }
        }
        ops
    }
}
