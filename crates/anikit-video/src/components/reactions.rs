//! Live-stream style reactions: hearts, thumbs up, fire, stars, sparkles,
//! "100" and laughing faces float up from an origin with a wobble, a scale
//! pop and a fade.
//!
//! Emoji are drawn as vector shapes (colour emoji fonts are often missing on
//! render machines), built once in `prepare` in a unit box and placed per
//! particle with one affine. Motion is closed-form and seeded.

use super::style::*;
use super::text::{Face, TextStyle};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{self, Affine, Op, Paint, Path, Rgba, IDENTITY};
use serde::Deserialize;

const KINDS: [&str; 7] = ["heart", "like", "fire", "star", "sparkles", "100", "laugh"];

fn canonical(k: &str) -> Option<usize> {
    let k = match k {
        "hearts" | "love" => "heart",
        "thumbs" | "thumb" | "thumbsup" | "thumbs-up" | "thumbs_up" | "likes" => "like",
        "flame" => "fire",
        "stars" => "star",
        "sparkle" => "sparkles",
        "hundred" => "100",
        "lol" | "haha" | "joy" => "laugh",
        k => k,
    };
    KINDS.iter().position(|&n| n == k)
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Reactions {
    /// Which reactions, picked at random per particle.
    pub kinds: Vec<String>,
    /// Total reactions over `duration` (ignored when `rate` is set).
    pub count: u32,
    /// Reactions per second as an endless stream.
    pub rate: Option<f32>,
    /// Emission window for `count` (s).
    pub duration: f32,
    /// Where they start, as fractions of the box.
    pub origin: [f32; 2],
    /// Emoji size (px).
    pub size: f32,
    /// Horizontal drift/wobble range (px).
    pub spread: f32,
    /// How far up they travel, as a fraction of the origin's height.
    pub rise: f32,
    /// Seconds each reaction lives.
    pub life: f32,
    pub delay: f32,
    pub seed: u32,
    #[serde(skip)]
    shapes: Vec<Vec<Op>>,
    #[serde(skip)]
    which: Vec<usize>,
}

impl Default for Reactions {
    fn default() -> Self {
        Reactions {
            kinds: vec!["heart".into(), "like".into(), "fire".into(), "star".into()],
            count: 24,
            rate: None,
            duration: 3.0,
            origin: [0.82, 0.94],
            size: 72.0,
            spread: 150.0,
            rise: 0.8,
            life: 2.8,
            delay: 0.0,
            seed: 1,
            shapes: Vec::new(),
            which: Vec::new(),
        }
    }
}

impl Prepare for Reactions {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.kinds.is_empty() {
            return Err(Error(format!("`kinds` needs at least one of {}", KINDS.join(", "))));
        }
        for k in &self.kinds {
            let kk = k.to_ascii_lowercase();
            match canonical(&kk) {
                Some(i) => self.which.push(i),
                None => {
                    let hint = closest(&kk, &KINDS).map(|c| format!("; did you mean `{c}`?")).unwrap_or_default();
                    return Err(Error(format!("unknown reaction {k:?}{hint} (known: {})", KINDS.join(", "))));
                }
            }
        }
        if let Some(r) = self.rate {
            if !(r > 0.0 && r <= 200.0) {
                return Err(Error("`rate` must be between 0 and 200 per second".into()));
            }
        }
        if !(self.life > 0.0 && self.size > 0.0 && self.duration >= 0.0) {
            return Err(Error("`life` and `size` must be > 0, `duration` ≥ 0".into()));
        }
        self.shapes = (0..KINDS.len()).map(shape).collect();
        Ok(())
    }
}

// ------------------------------------------------------------------ shapes (unit box, centre 0,0)

fn hx(s: &str) -> Rgba {
    parse_color(s).unwrap()
}

fn lin(y0: f32, y1: f32, a: &str, b: &str) -> Paint {
    Paint::Linear { p0: [-0.2, y0], p1: [0.2, y1], stops: vec![(0.0, hx(a)), (1.0, hx(b))] }
}

fn gloss(cx: f32, cy: f32, rx: f32, ry: f32, a: f32) -> Op {
    let path = kurbo::Shape::to_path(&kurbo::Ellipse::new((cx as f64, cy as f64), (rx as f64, ry as f64), -0.5), 0.01);
    fill(path, radial(cx, cy, rx.max(ry), vec![(0.0, Rgba([1.0, 1.0, 1.0, a])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))]))
}

fn shadow_of(path: &Path) -> Op {
    Op::Fill { path: path.clone(), paint: solid(Rgba([0.0, 0.0, 0.0, 0.22])), rule: dl::FillRule::NonZero, transform: [1.04, 0.0, 0.0, 1.04, 0.0, 0.05] }
}

fn heart_path() -> Path {
    let mut h = Path::new();
    h.move_to((0.0, 0.42));
    h.curve_to((-0.05, 0.36), (-0.48, 0.1), (-0.48, -0.14));
    h.curve_to((-0.48, -0.33), (-0.35, -0.43), (-0.22, -0.43));
    h.curve_to((-0.12, -0.43), (-0.04, -0.37), (0.0, -0.27));
    h.curve_to((0.04, -0.37), (0.12, -0.43), (0.22, -0.43));
    h.curve_to((0.35, -0.43), (0.48, -0.33), (0.48, -0.14));
    h.curve_to((0.48, 0.1), (0.05, 0.36), (0.0, 0.42));
    h.close_path();
    h
}

fn star_path(ro: f64, ri: f64, n: usize) -> Path {
    let mut s = Path::new();
    for i in 0..n * 2 {
        let a = std::f64::consts::PI * i as f64 / n as f64 - std::f64::consts::FRAC_PI_2;
        let r = if i % 2 == 0 { ro } else { ri };
        let pt = (r * a.cos(), r * a.sin() + 0.03);
        if i == 0 {
            s.move_to(pt);
        } else {
            s.line_to(pt);
        }
    }
    s.close_path();
    s
}

fn sparkle_path(cx: f64, cy: f64, r: f64) -> Path {
    let k = r * 0.12;
    let mut s = Path::new();
    s.move_to((cx, cy - r));
    s.quad_to((cx + k, cy - k), (cx + r, cy));
    s.quad_to((cx + k, cy + k), (cx, cy + r));
    s.quad_to((cx - k, cy + k), (cx - r, cy));
    s.quad_to((cx - k, cy - k), (cx, cy - r));
    s.close_path();
    s
}

fn flame_path(s: f64, dy: f64) -> Path {
    let pt = |x: f64, y: f64| (x * s, y * s + dy);
    let mut f = Path::new();
    f.move_to(pt(0.04, -0.5));
    f.curve_to(pt(0.12, -0.28), pt(0.4, -0.16), pt(0.4, 0.12));
    f.curve_to(pt(0.4, 0.34), pt(0.22, 0.48), pt(0.0, 0.48));
    f.curve_to(pt(-0.22, 0.48), pt(-0.4, 0.34), pt(-0.4, 0.12));
    f.curve_to(pt(-0.4, -0.04), pt(-0.3, -0.14), pt(-0.24, -0.27));
    f.curve_to(pt(-0.18, -0.12), pt(-0.12, -0.08), pt(-0.06, -0.08));
    f.curve_to(pt(-0.1, -0.24), pt(-0.05, -0.4), pt(0.04, -0.5));
    f.close_path();
    f
}

fn disc() -> Path {
    circle_path(0.0, 0.0, 0.48)
}

fn stroke(path: Path, c: Rgba, w: f32) -> Op {
    Op::Stroke { path, paint: solid(c), stroke: line(w, [0.0, 1.0]), transform: IDENTITY }
}

/// The ops for reaction `k` in a unit box centred on the origin.
fn shape(k: usize) -> Vec<Op> {
    let mut o = Vec::new();
    match KINDS[k] {
        "heart" => {
            let h = heart_path();
            o.push(shadow_of(&h));
            o.push(fill(h, Paint::Radial { c: [-0.18, -0.25], r: 0.75, stops: vec![(0.0, hx("#ff8fab")), (0.45, hx("#ff3b6b")), (1.0, hx("#d90f45"))] }));
            o.push(gloss(-0.22, -0.22, 0.13, 0.07, 0.75));
        }
        "star" => {
            let s = star_path(0.5, 0.22, 5);
            o.push(shadow_of(&s));
            let paint = lin(-0.5, 0.5, "#fff089", "#ffaa00");
            o.push(Op::Stroke { path: s.clone(), paint: paint.clone(), stroke: line(0.08, [0.0, 1.0]), transform: IDENTITY });
            o.push(fill(s, paint));
            o.push(gloss(-0.1, -0.12, 0.1, 0.05, 0.7));
        }
        "fire" => {
            let f = flame_path(1.0, 0.0);
            o.push(shadow_of(&f));
            o.push(fill(f, Paint::Linear { p0: [0.0, -0.5], p1: [0.0, 0.48], stops: vec![(0.0, hx("#ff3d1f")), (0.55, hx("#ff7a1a")), (1.0, hx("#ffb02e"))] }));
            o.push(fill(flame_path(0.58, 0.19), Paint::Linear { p0: [0.0, -0.1], p1: [0.0, 0.47], stops: vec![(0.0, hx("#ffd34d")), (1.0, hx("#fff4c4"))] }));
        }
        "like" => {
            let d = disc();
            o.push(shadow_of(&d));
            o.push(fill(d, lin(-0.48, 0.48, "#5aa2ff", "#1556d8")));
            let w = Rgba([1.0, 1.0, 1.0, 1.0]);
            o.push(rect(-0.27, -0.02, 0.11, 0.28, 0.03, solid(w)));
            o.push(rect(-0.13, -0.04, 0.37, 0.3, 0.07, solid(w)));
            let mut th = Path::new();
            th.move_to((-0.13, 0.02));
            th.line_to((-0.03, -0.12));
            th.curve_to((0.01, -0.18), (-0.01, -0.27), (0.04, -0.3));
            th.curve_to((0.1, -0.33), (0.15, -0.25), (0.12, -0.15));
            th.line_to((0.09, -0.03));
            th.line_to((-0.13, -0.03));
            th.close_path();
            o.push(fill(th, solid(w)));
            for y in [0.05, 0.12, 0.19] {
                let mut l = Path::new();
                l.move_to((0.1, y));
                l.line_to((0.24, y));
                o.push(stroke(l, hx("#2a6ef0"), 0.018));
            }
            o.push(gloss(-0.2, -0.26, 0.16, 0.07, 0.35));
        }
        "sparkles" => {
            let big = sparkle_path(-0.08, 0.06, 0.4);
            o.push(shadow_of(&big));
            let paint = lin(-0.4, 0.5, "#fff7b8", "#ffb800");
            o.push(fill(big, paint.clone()));
            o.push(fill(sparkle_path(0.3, -0.3, 0.17), paint.clone()));
            o.push(fill(sparkle_path(0.3, 0.32, 0.11), paint));
        }
        "laugh" => {
            let d = disc();
            o.push(shadow_of(&d));
            o.push(fill(d, Paint::Radial { c: [-0.12, -0.18], r: 0.7, stops: vec![(0.0, hx("#ffe98a")), (0.7, hx("#ffc93a")), (1.0, hx("#f5a316"))] }));
            let brown = hx("#5b3416");
            for sx in [-1.0, 1.0] {
                let mut e = Path::new();
                e.move_to((sx * 0.25, -0.08));
                e.quad_to((sx * 0.17, -0.22), (sx * 0.09, -0.08));
                o.push(stroke(e, brown, 0.055));
            }
            let mut m = Path::new();
            m.move_to((-0.27, 0.04));
            m.line_to((0.27, 0.04));
            m.curve_to((0.25, 0.34), (-0.25, 0.34), (-0.27, 0.04));
            m.close_path();
            o.push(fill(m, solid(brown)));
            o.push(fill(kurbo::Shape::to_path(&kurbo::Ellipse::new((0.0, 0.2), (0.12, 0.06), 0.0), 0.01), solid(hx("#ff6b81"))));
            o.push(rect(-0.22, 0.04, 0.44, 0.06, 0.02, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
            // Tears of joy.
            for sx in [-1.0f64, 1.0] {
                let mut t = Path::new();
                t.move_to((sx * 0.32, -0.04));
                t.curve_to((sx * 0.46, 0.08), (sx * 0.48, 0.2), (sx * 0.38, 0.22));
                t.curve_to((sx * 0.28, 0.22), (sx * 0.27, 0.1), (sx * 0.32, -0.04));
                t.close_path();
                o.push(fill(t, lin(-0.04, 0.22, "#9be3ff", "#3aa8f0")));
            }
            o.push(gloss(-0.2, -0.3, 0.14, 0.06, 0.45));
        }
        _ => {} // "100" is text, drawn in eval.
    }
    o
}

// ------------------------------------------------------------------ motion

#[inline]
fn h(i: u32, k: u32, seed: u32) -> f32 {
    hash01(i.wrapping_mul(0x9e37_79b9) ^ k.wrapping_mul(0x85eb_ca6b) ^ seed.wrapping_mul(0xc2b2_ae35))
}

/// Ease out with overshoot (the "pop").
fn pop(x: f32) -> f32 {
    let x = clamp01(x) - 1.0;
    1.0 + 3.2 * x * x * x + 2.2 * x * x
}

impl Component for Reactions {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let at = t - self.delay;
        if at < 0.0 {
            return Vec::new();
        }
        let (w, hh) = (ctx.width, ctx.height);
        let (ox, oy) = (self.origin[0] * w, self.origin[1] * hh);
        let max_life = self.life * 1.15;
        // Particle index range alive now.
        let (lo, hi, birth): (u32, u32, Box<dyn Fn(u32) -> f32>) = match self.rate {
            Some(r) => {
                let lo = ((at - max_life) * r).floor().max(0.0) as u32;
                let hi = (at * r).floor() as u32 + 1;
                (lo, hi, Box::new(move |i| (i as f32 + 0.6 * h(i, 2, self.seed)) / r))
            }
            None => {
                let n = self.count;
                let d = self.duration;
                (0, n, Box::new(move |i| d * ((i as f32 + h(i, 2, self.seed)) / n.max(1) as f32).powf(1.15)))
            }
        };
        let mut o = Vec::new();
        for i in lo..hi {
            let b = birth(i);
            let life = self.life * (0.85 + 0.3 * h(i, 3, self.seed));
            let age = at - b;
            if age < 0.0 || age >= life {
                continue;
            }
            let a = age / life;
            let kind = self.which[((h(i, 1, self.seed) * self.which.len() as f32) as usize).min(self.which.len() - 1)];
            let size = self.size * (0.75 + 0.5 * h(i, 4, self.seed));
            let rise = oy * self.rise * (0.8 + 0.3 * h(i, 5, self.seed));
            let drift = (h(i, 6, self.seed) - 0.5) * self.spread;
            let wob = self.spread * 0.18 * (0.5 + h(i, 7, self.seed));
            let freq = 2.2 + 1.6 * h(i, 8, self.seed);
            let ph = h(i, 9, self.seed) * std::f32::consts::TAU;
            let x = ox + (h(i, 10, self.seed) - 0.5) * self.spread * 0.25 + drift * a + wob * (age * freq + ph).sin() * clamp01(a * 4.0);
            let y = oy - rise * (0.65 * a + 0.35 * ease_out(a));
            let s = size * pop(age / 0.38) * (1.0 - 0.3 * seg(a, 0.65, 0.35));
            let op = 1.0 - ease_in(seg(a, 0.55, 0.45));
            let rot = 0.22 * (age * freq * 0.7 + ph).sin();
            if s <= 0.5 || op <= 0.01 {
                continue;
            }
            let mut m: Affine = dl::mul(&translate(x, y), &rotate_about(rot, 0.0, 0.0));
            m = dl::mul(&m, &[s, 0.0, 0.0, s, 0.0, 0.0]);
            let mut ops = if KINDS[kind] == "100" {
                hundred(ctx)
            } else {
                self.shapes[kind].clone()
            };
            if KINDS[kind] == "fire" {
                // Flicker.
                let f = 1.0 + 0.06 * (age * 23.0 + ph).sin();
                m = dl::mul(&m, &[f, 0.0, 0.0, 2.0 - f, 0.0, 0.0]);
            }
            super::transform_ops(&mut ops, &m);
            o.extend(super::with_opacity(ops, op));
        }
        o
    }

    fn duration(&self) -> Option<f32> {
        match self.rate {
            Some(_) => None,
            None => Some(self.delay + self.duration + self.life * 1.15),
        }
    }
}

/// "100" with two underlines, slanted, in the unit box.
fn hundred(ctx: &Ctx) -> Vec<Op> {
    let red = hx("#ff2d55");
    let st = TextStyle::new(Face::Display, 100.0, 800, red).tracking(-0.04);
    let tw = ctx.assets.measure("100", &st);
    let (mut ops, _) = ctx.assets.text("100", &st, -tw * 0.5, 30.0);
    let lw = tw * 0.95;
    ops.push(rect(-lw * 0.5, 44.0, lw, 9.0, 4.5, solid(red)));
    ops.push(rect(-lw * 0.42, 60.0, lw * 0.84, 9.0, 4.5, solid(red)));
    // Unit box: 100 px → 0.95, slanted like italics.
    let k = 0.95 / tw.max(1.0);
    super::transform_ops(&mut ops, &[k, 0.0, -0.2 * k, k, 0.0, -0.05]);
    ops
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    #[test]
    fn gallery() {
        let th = Theme::default();
        let a = preview::host();
        // Every kind in a row, at rest.
        let mut ops = Vec::new();
        for (i, k) in super::KINDS.iter().enumerate() {
            let c = build("reactions", &json!({"kinds": [k], "count": 1, "duration": 0, "life": 100, "rise": 0, "spread": 0, "size": 110, "origin": [0.5, 0.5], "x": 20 + i * 130, "y": 20, "width": 120, "height": 120})).unwrap();
            ops.extend(c.eval(1.0, 960.0, 540.0, 30.0, &th, a));
        }
        let c = build("reactions", &json!({"kinds": ["heart", "like", "fire", "laugh", "100", "sparkles"], "rate": 8, "y": 160, "height": 380})).unwrap();
        for t in [0.0, 1.0, 2.5, 30.0] {
            preview::validate(&c.eval(t, 960.0, 540.0, 30.0, &th, a)).unwrap();
        }
        ops.extend(c.eval(3.0, 960.0, 540.0, 30.0, &th, a));
        preview::validate(&ops).unwrap();
        preview::snapshot("reactions", 960, 540, th.background, ops, a);
    }

    #[test]
    fn count_mode_ends_and_errors() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("reactions", &json!({})).unwrap();
        let end = c.component.duration().unwrap();
        assert!(!c.eval(1.5, 800.0, 600.0, 30.0, &th, a).is_empty());
        assert!(c.eval(end + 0.1, 800.0, 600.0, 30.0, &th, a).is_empty());
        let e = build("reactions", &json!({"kinds": ["hart"]})).err().unwrap();
        assert!(e.0.contains("did you mean `heart`"), "{e}");
        assert!(build("reactions", &json!({"kinds": ["thumbs", "lol", "hundred"]})).is_ok());
    }
}
