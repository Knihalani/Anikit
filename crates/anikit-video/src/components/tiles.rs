//! Tile grids: `tile-reveal` (a grid of tiles flips, scales or fades in with
//! a wave to reveal an image or a gradient, and can reverse) and
//! `kinetic-grid` (rounded squares or dots whose size, rotation and colour
//! ripple with a wave: a motion-graphics backdrop).

use super::style::*;
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Rgba};
use serde::Deserialize;
use std::f32::consts::{PI, TAU};

fn check(name: &str, v: &str, options: &[&str]) -> Result<(), Error> {
    if options.contains(&v) {
        return Ok(());
    }
    let hint = closest(v, options).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
    Err(Error(format!("{name} must be one of {}, not {v:?}{hint}", options.join(", "))))
}

/// Order 0..1 in which tile `(i, j)` of a `cols × rows` grid starts.
fn order(wave: &str, i: usize, j: usize, cols: usize, rows: usize, seed: u32) -> f32 {
    let u = if cols > 1 { i as f32 / (cols - 1) as f32 } else { 0.5 };
    let v = if rows > 1 { j as f32 / (rows - 1) as f32 } else { 0.5 };
    match wave {
        "radial" => {
            let (dx, dy) = ((u - 0.5) * cols as f32, (v - 0.5) * rows as f32);
            let m = ((cols * cols + rows * rows) as f32).sqrt() * 0.5;
            ((dx * dx + dy * dy).sqrt() / m).min(1.0)
        }
        "random" => hash01((j * cols + i) as u32 ^ seed.wrapping_mul(0x9e37_79b9)),
        "left" => u,
        "right" => 1.0 - u,
        "top" => v,
        "bottom" => 1.0 - v,
        _ => (u + v) * 0.5,
    }
}

/// Ease out with a little overshoot.
fn back(x: f32) -> f32 {
    let x = clamp01(x) - 1.0;
    1.0 + 2.7 * x * x * x + 1.7 * x * x
}

/// Gradient palette, or the theme's default.
fn colors(cols: &[Color], ctx: &Ctx) -> Vec<Rgba> {
    if cols.is_empty() {
        vec![ctx.theme.accent, ctx.theme.pal(2), ctx.theme.pal(1)]
    } else {
        cols.iter().map(|c| c.0).collect()
    }
}

fn stops(cols: &[Rgba]) -> Vec<(f32, Rgba)> {
    let n = (cols.len() - 1).max(1) as f32;
    cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect()
}

// ------------------------------------------------------------------ tile-reveal

const WAVES: [&str; 7] = ["diagonal", "radial", "random", "left", "right", "top", "bottom"];

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TileReveal {
    /// Image to reveal (cover-fit); without it a gradient of `colors`.
    pub src: Option<String>,
    pub colors: Vec<Color>,
    /// Columns; rows follow from the box aspect unless given.
    pub cols: u32,
    pub rows: Option<u32>,
    /// `diagonal`, `radial`, `random`, `left`, `right`, `top` or `bottom`.
    pub wave: String,
    /// `flip`, `scale` or `fade`.
    pub effect: String,
    /// Gap between tiles and their corner radius (px).
    pub gap: f32,
    pub radius: f32,
    /// Seconds for the wave to cross the grid, and for one tile to turn.
    pub duration: f32,
    pub tile: f32,
    pub delay: f32,
    /// When the tiles leave again (same wave), if at all.
    pub out: Option<f32>,
    /// Start revealed and hide instead.
    pub reverse: bool,
    /// Colour of the tiles' back faces (flip).
    pub back: Option<Color>,
    pub seed: u32,
}

impl Default for TileReveal {
    fn default() -> Self {
        TileReveal {
            src: None,
            colors: Vec::new(),
            cols: 10,
            rows: None,
            wave: "diagonal".into(),
            effect: "flip".into(),
            gap: 6.0,
            radius: 10.0,
            duration: 1.1,
            tile: 0.7,
            delay: 0.0,
            out: None,
            reverse: false,
            back: None,
            seed: 1,
        }
    }
}

impl Prepare for TileReveal {
    fn prepare(&mut self) -> Result<(), Error> {
        check("wave", &self.wave, &WAVES)?;
        check("effect", &self.effect, &["flip", "scale", "fade"])?;
        if self.cols == 0 || self.cols > 200 || self.rows.is_some_and(|r| r == 0 || r > 200) {
            return Err(Error("`cols`/`rows` must be 1..200".into()));
        }
        if !(self.duration >= 0.0 && self.tile > 0.0) {
            return Err(Error("`duration` must be ≥ 0 and `tile` > 0".into()));
        }
        Ok(())
    }
}

impl Component for TileReveal {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let cols = self.cols as usize;
        let rows = self.rows.map(|r| r as usize).unwrap_or_else(|| ((h / (w / cols as f32)).round() as usize).max(1));
        let (cw, ch) = (w / cols as f32, h / rows as f32);
        let g = self.gap.max(0.0);
        let r = self.radius.min(cw.min(ch) * 0.5);
        // The face paint, in box coordinates (each tile's transform carries it).
        let face = match self.src.as_deref().and_then(|s| ctx.assets.media(s, t)) {
            Some((image, iw, ih)) => {
                let k = (w / iw as f32).max(h / ih as f32);
                let (ox, oy) = ((w - iw as f32 * k) * 0.5, (h - ih as f32 * k) * 0.5);
                Paint::Image { image, transform: [1.0 / k, 0.0, 0.0, 1.0 / k, -ox / k, -oy / k], opacity: 1.0 }
            }
            None => Paint::Linear { p0: [0.0, 0.0], p1: [w, h], stops: stops(&colors(&self.colors, ctx)) },
        };
        let back_c = self.back.map(|c| c.0).unwrap_or(mix(ctx.theme.surface2, ctx.theme.accent, 0.3));
        let mut o = Vec::with_capacity(cols * rows * 2);
        for j in 0..rows {
            for i in 0..cols {
                let k = order(&self.wave, i, j, cols, rows, self.seed);
                let mut p = seg(t, self.delay + k * self.duration, self.tile);
                if let Some(out) = self.out {
                    p *= 1.0 - seg(t, out + k * self.duration, self.tile);
                }
                if self.reverse {
                    p = 1.0 - p;
                }
                if p <= 0.0 {
                    continue;
                }
                let (x, y) = (i as f32 * cw + g * 0.5, j as f32 * ch + g * 0.5);
                let (tw, th) = (cw - g, ch - g);
                let (cx, cy) = (x + tw * 0.5, y + th * 0.5);
                let tile = |paint: Paint, m: crate::dl::Affine| Op::RoundRect { x, y, w: tw, h: th, radius: r, paint, transform: m };
                let sheen = hash01((j * cols + i) as u32 + 77) * 0.07;
                match self.effect.as_str() {
                    "flip" => {
                        // Turn about the vertical axis from the back face (θ = π) to the front (θ = 0).
                        let th_ = PI * (1.0 - ease_in_out(p));
                        let sx = th_.cos().abs().max(0.02);
                        let sy = 1.0 + 0.08 * th_.sin();
                        let m = [sx, 0.0, 0.0, sy, cx - sx * cx, cy - sy * cy];
                        let fade_in = clamp01(p * 6.0);
                        if th_ > PI * 0.5 {
                            o.push(tile(solid(alpha(back_c, fade_in)), m));
                            o.push(tile(solid(Rgba([1.0, 1.0, 1.0, 0.05 * fade_in])), m));
                        } else {
                            o.push(tile(face.clone(), m));
                            o.push(tile(solid(Rgba([1.0, 1.0, 1.0, sheen])), m));
                        }
                        // Darken while edge-on.
                        let d = (1.0 - sx) * 0.55;
                        if d > 0.01 {
                            o.push(tile(solid(Rgba([0.0, 0.0, 0.0, d])), m));
                        }
                    }
                    "scale" => {
                        let s = back(p);
                        let a = rotate_about((1.0 - ease_out(p)) * -0.35, cx, cy);
                        let m = crate::dl::mul(&a, &scale_about(s, cx, cy));
                        let fa = clamp01(p * 3.0);
                        o.push(tile(face.clone(), m));
                        o.push(tile(solid(Rgba([1.0, 1.0, 1.0, sheen + 0.25 * (1.0 - fa)])), m));
                        if fa < 1.0 {
                            // Fade by covering with the background (keeps the face paint exact).
                            o.push(tile(solid(alpha(ctx.theme.background, 1.0 - fa)), m));
                        }
                    }
                    _ => {
                        let e = ease_out(p);
                        let m = crate::dl::mul(&translate(0.0, (1.0 - e) * ch * 0.25), &scale_about(lerp(0.9, 1.0, e), cx, cy));
                        o.push(tile(face.clone(), m));
                        o.push(tile(solid(Rgba([1.0, 1.0, 1.0, sheen])), m));
                        if e < 1.0 {
                            o.push(tile(solid(alpha(ctx.theme.background, 1.0 - e)), m));
                        }
                    }
                }
            }
        }
        o
    }

    fn duration(&self) -> Option<f32> {
        let one = self.duration + self.tile;
        Some(self.out.map(|o| o + one).unwrap_or(self.delay + one))
    }
}

// ------------------------------------------------------------------ kinetic-grid

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KineticGrid {
    /// Columns (rows follow from the box aspect).
    pub cols: u32,
    /// `rounded`, `square`, `dot` or `diamond`.
    pub shape: String,
    /// `radial`, `diagonal`, `linear` or `interference`.
    pub wave: String,
    /// Colour cycle; default accent → pink → cyan.
    pub colors: Vec<Color>,
    /// Wave speed multiplier.
    pub speed: f32,
    /// Size of a cell's shape at the crest and trough, as fractions of the cell.
    pub size: f32,
    pub min: f32,
    /// Rotation at the crest in degrees (default 90 for squares, 0 for dots).
    pub rotate: Option<f32>,
    /// Wavelength in cells.
    pub wavelength: f32,
    pub delay: f32,
    /// Seconds per step (e.g. `0.5` = one beat at 120 BPM): the wave snaps
    /// forward over the first 0.15 s of each step and holds, so frames between
    /// steps are identical (rhythmic, and cheap to encode). `0` = continuous.
    pub step: f32,
}

impl Default for KineticGrid {
    fn default() -> Self {
        KineticGrid { cols: 18, shape: "rounded".into(), wave: "radial".into(), colors: Vec::new(), speed: 1.0, size: 0.8, min: 0.16, rotate: None, wavelength: 7.0, delay: 0.0, step: 0.0 }
    }
}

impl Prepare for KineticGrid {
    fn prepare(&mut self) -> Result<(), Error> {
        check("shape", &self.shape, &["rounded", "square", "dot", "diamond"])?;
        check("wave", &self.wave, &["radial", "diagonal", "linear", "interference"])?;
        if self.cols == 0 || self.cols > 200 {
            return Err(Error("`cols` must be 1..200".into()));
        }
        if !(self.step >= 0.0 && self.step.is_finite()) {
            return Err(Error("`step` must be ≥ 0 seconds".into()));
        }
        if !(self.wavelength > 0.0 && self.speed.is_finite()) {
            return Err(Error("`wavelength` must be > 0 and `speed` finite".into()));
        }
        Ok(())
    }
}

impl Component for KineticGrid {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let t = step(t, AMBIENT_RATE);
        let at = t - self.delay;
        if at < 0.0 {
            return Vec::new();
        }
        let (w, h) = (ctx.width, ctx.height);
        let cols = self.cols as usize;
        let cell = w / cols as f32;
        let rows = (h / cell).ceil() as usize + 1;
        let oy = (h - rows as f32 * cell) * 0.5;
        let mut cyc = colors(&self.colors, ctx);
        cyc.push(cyc[0]);
        let rot = self.rotate.unwrap_or(if self.shape == "dot" { 0.0 } else { 90.0 }).to_radians();
        let k = TAU / self.wavelength;
        // Wave clock: continuous, or snapped forward once per `step`.
        let wt = if self.step > 0.0 {
            let n = (at / self.step).floor();
            let f = ((at - n * self.step) / self.step.min(0.15)).min(1.0);
            (n + 1.0 - (1.0 - f).powi(3)) * self.step
        } else {
            at
        };
        let ph = wt * self.speed * TAU * 0.45;
        let (gx, gy) = (cols as f32 * 0.5, rows as f32 * 0.5);
        let mut o = Vec::with_capacity(cols * rows);
        for j in 0..rows {
            for i in 0..cols {
                let (x, y) = (i as f32 + 0.5, j as f32 + 0.5);
                let (dx, dy) = (x - gx, y - gy);
                let d = (dx * dx + dy * dy).sqrt();
                let s = match self.wave.as_str() {
                    "diagonal" => 0.5 + 0.5 * ((x + y) * k * 0.7 - ph).sin(),
                    "linear" => 0.5 + 0.5 * (x * k - ph).sin(),
                    "interference" => {
                        let d1 = ((x - gx * 0.45).powi(2) + dy * dy).sqrt();
                        let d2 = ((x - gx * 1.55).powi(2) + dy * dy).sqrt();
                        0.5 + 0.25 * ((d1 * k - ph).sin() + (d2 * k - ph * 1.1).sin())
                    }
                    _ => 0.5 + 0.5 * (d * k - ph).sin(),
                };
                // Sharpen so shapes linger at the extremes, like a motion-graphics loop.
                let s = s * s * (3.0 - 2.0 * s);
                // Entrance: cells pop in from the centre outwards.
                let pop = back(seg(at, d / (gx.max(gy) * 1.4) * 0.8, 0.5));
                if pop <= 0.001 {
                    continue;
                }
                let sz = cell * lerp(self.min, self.size, s) * pop;
                let (px, py) = (i as f32 * cell + cell * 0.5, oy + j as f32 * cell + cell * 0.5);
                let c = {
                    let f = (d * 0.055 + s * 0.35 - wt * 0.06 * self.speed).rem_euclid(1.0) * (cyc.len() - 1) as f32;
                    let n = (f as usize).min(cyc.len() - 2);
                    mix(cyc[n], cyc[n + 1], f - n as f32)
                };
                let c = alpha(shade(c, 0.15 * s - 0.1), lerp(0.28, 1.0, s));
                let (radius, ang) = match self.shape.as_str() {
                    "dot" => (sz * 0.5, 0.0),
                    "square" => (sz * 0.08, s * rot),
                    "diamond" => (sz * 0.12, PI * 0.25 + s * rot),
                    _ => (sz * lerp(0.5, 0.22, s), s * rot),
                };
                let m = if ang != 0.0 { rotate_about(ang, px, py) } else { crate::dl::IDENTITY };
                o.push(Op::RoundRect { x: px - sz * 0.5, y: py - sz * 0.5, w: sz, h: sz, radius, paint: solid(c), transform: m });
            }
        }
        o
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
        a.add_media("tiles-photo.png", 1200, 800, super::super::ken_burns::tests::photo(1200, 800));
        let cases = [
            ("tile-reveal-flip", "tile-reveal", json!({"src": "tiles-photo.png"}), 1.0),
            ("tile-reveal-scale", "tile-reveal", json!({"effect": "scale", "wave": "radial"}), 1.0),
            ("tile-reveal-fade", "tile-reveal", json!({"effect": "fade", "wave": "random", "src": "tiles-photo.png", "out": 2.0}), 2.6),
            ("kinetic-grid", "kinetic-grid", json!({}), 2.0),
            ("kinetic-grid-dots", "kinetic-grid", json!({"shape": "dot", "wave": "interference"}), 3.0),
        ];
        for (name, kind, p, t) in cases {
            let c = build(kind, &p).unwrap();
            for tt in [0.0, t * 0.5, t, 10.0] {
                preview::validate(&c.eval(tt, 960.0, 540.0, 30.0, &th, a)).unwrap();
            }
            preview::snapshot(name, 960, 540, th.background, c.eval(t, 960.0, 540.0, 30.0, &th, a), a);
        }
    }

    #[test]
    fn reveal_and_reverse() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("tile-reveal", &json!({"cols": 4, "out": 3})).unwrap();
        assert!(c.eval(0.0, 400.0, 200.0, 30.0, &th, a).is_empty());
        assert!(!c.eval(2.5, 400.0, 200.0, 30.0, &th, a).is_empty());
        assert!(c.eval(10.0, 400.0, 200.0, 30.0, &th, a).is_empty());
        let r = build("tile-reveal", &json!({"cols": 4, "reverse": true})).unwrap();
        assert!(!r.eval(0.0, 400.0, 200.0, 30.0, &th, a).is_empty());
        assert!(r.eval(10.0, 400.0, 200.0, 30.0, &th, a).is_empty());
        assert!(build("tile-reveal", &json!({"wave": "diagnal"})).err().unwrap().0.contains("did you mean `diagonal`"));
        assert!(build("kinetic-grid", &json!({"shape": "dots"})).err().unwrap().0.contains("did you mean `dot`"));
        // `step`: frames between steps are identical once the entrance is over.
        let c = build("kinetic-grid", &json!({"step": 0.5})).unwrap();
        let (th, a) = (Theme::default(), preview::host());
        assert_eq!(c.eval(3.2, 960.0, 540.0, 30.0, &th, a), c.eval(3.45, 960.0, 540.0, 30.0, &th, a));
        assert_ne!(c.eval(3.45, 960.0, 540.0, 30.0, &th, a), c.eval(3.55, 960.0, 540.0, 30.0, &th, a));
        assert!(build("kinetic-grid", &json!({"step": -1})).is_err());
    }
}
