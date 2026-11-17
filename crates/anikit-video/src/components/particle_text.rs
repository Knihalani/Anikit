//! `particle-text`: thousands of glowing particles fly in, form a word, hold,
//! optionally re-form into the next word, then blow away.
//!
//! Target points come from the glyph outlines (bundled fonts, variation
//! aware), sampled once in `prepare` on a jittered grid so the dots cover the
//! letters evenly. Points are stored relative to the text's bounds, so the
//! same samples serve any box size. Motion is closed-form in `t`.

use super::style::*;
use super::text::{Face, TextStyle};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, Op, Path, Rgba};
use crate::text::FontStore;
use kurbo::Shape;
use serde::Deserialize;
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

/// Bundled fonts used only to outline text (never drawn directly).
fn fonts() -> &'static FontStore {
    static F: OnceLock<FontStore> = OnceLock::new();
    F.get_or_init(FontStore::new)
}

struct Pen<'a> {
    p: &'a mut Path,
    m: [f64; 6],
}
impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> (f64, f64) {
        let (x, y) = (x as f64, -y as f64);
        let m = &self.m;
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }
}
impl skrifa::outline::OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.p.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.p.line_to(p);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (c, p) = (self.pt(cx, cy), self.pt(x, y));
        self.p.quad_to(c, p);
    }
    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        let (a, b, p) = (self.pt(ax, ay), self.pt(bx, by), self.pt(x, y));
        self.p.curve_to(a, b, p);
    }
    fn close(&mut self) {
        self.p.close_path();
    }
}

/// Outline of one line of text (baseline at y = 0), as a fillable path.
pub(crate) fn text_outline(s: &str, st: &TextStyle) -> Path {
    use skrifa::MetadataProvider;
    let fs = fonts();
    let (ops, _) = super::host::draw_text(fs, s, st, 0.0, 0.0);
    let mut path = Path::new();
    for op in ops {
        let Op::Glyphs { font, size, glyphs, transform: t, .. } = op else { continue };
        let face = fs.face(font);
        let Ok(f) = skrifa::FontRef::from_index(face.data.data.as_ref(), face.data.index) else { continue };
        let coords: Vec<skrifa::instance::NormalizedCoord> = face.coords.iter().map(|c| skrifa::instance::NormalizedCoord::from_bits(*c)).collect();
        let outlines = f.outline_glyphs();
        for g in glyphs {
            let Some(o) = outlines.get(skrifa::GlyphId::new(g.id as u32)) else { continue };
            let m = [t[0] as f64, t[1] as f64, t[2] as f64, t[3] as f64, (t[0] * g.x + t[2] * g.y + t[4]) as f64, (t[1] * g.x + t[3] * g.y + t[5]) as f64];
            let loc = skrifa::instance::LocationRef::new(&coords);
            let _ = o.draw((skrifa::instance::Size::new(size), loc), &mut Pen { p: &mut path, m });
        }
    }
    path
}

/// About `n` points evenly covering the inside of `path` (jittered grid),
/// in random order. Returns exactly `n` points (duplicating if short).
fn sample_inside(path: &Path, n: usize, seed: u64) -> Vec<[f32; 2]> {
    let b = path.bounding_box();
    let area = path.area().abs().max(1.0);
    let mut rng = Rng::new(seed);
    let mut s = (area / n as f64).sqrt();
    let mut pts = Vec::new();
    for _ in 0..3 {
        pts.clear();
        let mut y = b.y0 + s * 0.5;
        let mut row = 0;
        while y < b.y1 {
            // Offset alternate rows: a hexagonal-ish lattice reads more organic.
            let mut x = b.x0 + s * if row % 2 == 0 { 0.25 } else { 0.75 };
            while x < b.x1 {
                let jx = x + (rng.f() as f64 - 0.5) * s * 0.5;
                let jy = y + (rng.f() as f64 - 0.5) * s * 0.5;
                if path.winding(kurbo::Point::new(jx, jy)) != 0 {
                    pts.push([jx as f32, jy as f32]);
                }
                x += s;
            }
            y += s;
            row += 1;
        }
        if pts.len() >= n && pts.len() < n + n / 8 + 4 {
            break;
        }
        s *= (pts.len().max(1) as f64 / n as f64).sqrt() * if pts.len() < n { 0.97 } else { 1.0 };
    }
    // Shuffle (Fisher–Yates), then trim or pad to exactly `n`.
    for i in (1..pts.len()).rev() {
        let j = (rng.f() * (i + 1) as f32) as usize;
        pts.swap(i, j.min(i));
    }
    if pts.is_empty() {
        pts.push([b.center().x as f32, b.center().y as f32]);
    }
    let have = pts.len();
    while pts.len() < n {
        let p = pts[(rng.f() * have as f32) as usize % have];
        pts.push([p[0] + (rng.f() - 0.5) * s as f32 * 0.5, p[1] + (rng.f() - 0.5) * s as f32 * 0.5]);
    }
    pts.truncate(n);
    pts
}

/// A circle as four cubics (cheaper than a general arc conversion).
#[inline]
fn dot(p: &mut Path, x: f32, y: f32, r: f32) {
    let (x, y, r) = (x as f64, y as f64, r as f64);
    if r < 3.0 {
        // Small dots: an octagon is indistinguishable and far cheaper to
        // rasterise than four cubics (thousands of dots per frame).
        const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
        const K: [(f64, f64); 8] = [(1.0, 0.0), (D, D), (0.0, 1.0), (-D, D), (-1.0, 0.0), (-D, -D), (0.0, -1.0), (D, -D)];
        p.move_to((x + r, y));
        for (cx, cy) in &K[1..] {
            p.line_to((x + cx * r, y + cy * r));
        }
        p.close_path();
        return;
    }
    let c = r * 0.552_284_75;
    p.move_to((x + r, y));
    p.curve_to((x + r, y + c), (x + c, y + r), (x, y + r));
    p.curve_to((x - c, y + r), (x - r, y + c), (x - r, y));
    p.curve_to((x - r, y - c), (x - c, y - r), (x, y - r));
    p.curve_to((x + c, y - r), (x + r, y - c), (x + r, y));
    p.close_path();
}

/// One word's samples, relative to its bounds' centre, plus its size.
struct Word {
    pts: Vec<[f32; 2]>,
    w: f32,
}

struct Particle {
    /// Scatter start (fractions of the box, may be outside 0..1).
    from: [f32; 2],
    /// Stagger 0..1.
    delay: f32,
    /// Sideways bow of the flight path (fraction of distance).
    bow: f32,
    size: f32,
    phase: f32,
    /// Exit randoms.
    e: [f32; 3],
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ParticleText {
    /// The word or phrase (shorthand for `words: [text]`).
    pub text: String,
    /// Several words: particles re-form from one into the next.
    pub words: Vec<String>,
    /// Number of particles.
    pub count: u32,
    /// Particle radius in px at 1080p.
    pub size: f32,
    /// Gradient across the text (left → right). Defaults to the theme accent set.
    pub colors: Vec<Color>,
    /// Font size in px; default fits the box (~80% wide, ≤ 30% tall).
    pub font_size: Option<f32>,
    pub weight: u16,
    pub face: Face,
    /// Seconds to fly in / re-form.
    pub form: f32,
    /// Seconds each word holds.
    pub hold: f32,
    /// How the last word leaves: `blow` (wind), `explode`, `fade` or `none` (stays).
    pub exit: String,
    /// Seconds the exit takes.
    pub exit_time: f32,
    pub delay: f32,
    pub glow: bool,
    pub seed: u32,
    #[serde(skip)]
    plan: Vec<Word>,
    #[serde(skip)]
    parts: Vec<Particle>,
}

impl Default for ParticleText {
    fn default() -> Self {
        ParticleText {
            text: String::new(),
            words: Vec::new(),
            count: 4000,
            size: 2.4,
            colors: Vec::new(),
            font_size: None,
            weight: 800,
            face: Face::Display,
            form: 1.6,
            hold: 1.4,
            exit: "none".into(),
            exit_time: 1.6,
            delay: 0.0,
            glow: true,
            seed: 1,
            plan: Vec::new(),
            parts: Vec::new(),
        }
    }
}

/// Reference font size the outlines are sampled at.
const REF: f32 = 200.0;

impl Prepare for ParticleText {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.words.is_empty() {
            self.words.push(if self.text.is_empty() { "Hello".into() } else { self.text.clone() });
        } else if !self.text.is_empty() {
            return Err(Error("give `text` or `words`, not both".into()));
        }
        if !["blow", "explode", "fade", "none"].contains(&self.exit.as_str()) {
            return Err(Error(format!("exit must be blow, explode, fade or none, not {:?}", self.exit)));
        }
        if self.count == 0 || self.count > 50_000 {
            return Err(Error("count must be 1..50000".into()));
        }
        self.form = self.form.max(0.05);
        self.hold = self.hold.max(0.0);
        self.exit_time = self.exit_time.max(0.05);
        let n = self.count as usize;
        let st = TextStyle::new(self.face, REF, self.weight, Rgba([1.0; 4]));
        for (i, w) in self.words.iter().enumerate() {
            let path = text_outline(w.trim(), &st);
            let b = path.bounding_box();
            let (cx, cy) = (b.center().x as f32, b.center().y as f32);
            let mut pts = if path.is_empty() { vec![[0.0, 0.0]; n] } else { sample_inside(&path, n, self.seed as u64 * 131 + i as u64) };
            for p in &mut pts {
                p[0] -= cx;
                p[1] -= cy;
            }
            self.plan.push(Word { pts, w: b.width().max(1.0) as f32 });
        }
        let mut rng = Rng::new(self.seed as u64 ^ 0xface);
        self.parts = (0..n)
            .map(|_| {
                // Scatter: a wide ring around the frame, some from far outside.
                let a = rng.f() * TAU;
                let r = 0.35 + 0.75 * rng.f().sqrt();
                Particle {
                    from: [0.5 + a.cos() * r * 0.9, 0.5 + a.sin() * r * 0.75],
                    delay: rng.f(),
                    bow: (rng.f() - 0.5) * 0.7,
                    size: 0.55 + 0.75 * rng.f() * rng.f() + 0.2 * rng.f(),
                    phase: rng.f() * TAU,
                    e: [rng.f(), rng.f(), rng.f()],
                }
            })
            .collect();
        Ok(())
    }
}

impl ParticleText {
    fn exit_start(&self) -> f32 {
        self.delay + self.words.len() as f32 * (self.form + self.hold)
    }

    /// Scale from reference outline units to box pixels for word `i`.
    fn scale(&self, i: usize, w: f32, h: f32) -> f32 {
        let wd = &self.plan[i];
        match self.font_size {
            Some(fs) => fs / REF,
            None => {
                // One size for all words (the widest decides), so re-forms don't jump.
                let maxw = self.plan.iter().map(|p| p.w).fold(1.0f32, f32::max);
                let _ = wd;
                (w * 0.8 / maxw).min(h * 0.30 / (REF * 0.72))
            }
        }
    }
}

impl Component for ParticleText {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let k = h / 1080.0;
        let lt = t - self.delay;
        if lt < 0.0 || self.plan.is_empty() {
            return Vec::new();
        }
        let cols: Vec<Rgba> = if self.colors.is_empty() {
            vec![shade(ctx.theme.accent, 0.25), ctx.theme.pal(1), shade(ctx.theme.pal(2), 0.1)]
        } else {
            self.colors.iter().map(|c| c.0).collect()
        };
        let cyc = self.form + self.hold;
        let nw = self.plan.len();
        // Which transition we are in: word `wi` is forming (from `wi - 1` or scatter).
        let wi = ((lt / cyc).floor() as usize).min(nw - 1);
        let st = lt - wi as f32 * cyc;
        let exit_t = t - self.exit_start();
        let exiting = self.exit != "none" && exit_t > 0.0;
        if exiting && exit_t > self.exit_time * 1.6 {
            return Vec::new();
        }
        let (cx, cy) = (w * 0.5, h * 0.5);
        let s_to = self.scale(wi, w, h);
        let s_from = if wi > 0 { self.scale(wi - 1, w, h) } else { s_to };
        let half_w = self.plan.iter().map(|p| p.w).fold(1.0f32, f32::max) * s_to * 0.5;
        let tw = &self.plan[wi].pts;
        let fw = if wi > 0 { Some(&self.plan[wi - 1].pts) } else { None };

        const NB: usize = 10;
        const NA: usize = 3;
        let mut core: Vec<Vec<[f32; 3]>> = (0..NB * NA).map(|_| Vec::with_capacity(self.parts.len() / 8)).collect();
        let rbase = self.size * k;
        let span = 0.45; // fraction of `form` used for stagger
        for (i, p) in self.parts.iter().enumerate() {
            let target = [cx + tw[i][0] * s_to, cy + tw[i][1] * s_to];
            let src = match fw {
                Some(f) => [cx + f[i][0] * s_from, cy + f[i][1] * s_from],
                None => [p.from[0] * w, p.from[1] * h],
            };
            // Stagger sweeps left → right, with noise.
            let xf = clamp01((target[0] - (cx - half_w)) / (2.0 * half_w).max(1.0));
            let d = (0.55 * p.delay + 0.45 * xf) * span * self.form;
            let q = clamp01((st - d) / (self.form * (1.0 - span)));
            let e = if wi == 0 { ease_out(q) } else { ease_in_out(q) };
            let (dx, dy) = (target[0] - src[0], target[1] - src[1]);
            let bow = (q * PI).sin() * p.bow;
            let mut x = src[0] + dx * e - dy * bow;
            let mut y = src[1] + dy * e + dx * bow;
            let mut a = if wi == 0 { clamp01(q * 3.0) } else { 1.0 };
            let mut r = rbase * p.size * if wi == 0 { 0.4 + 0.6 * e } else { 1.0 };
            // Settled: a slow shimmer.
            let settle = clamp01((q - 0.85) / 0.15);
            let tt = t * 1.7 + p.phase;
            x += tt.sin() * 0.9 * k * settle;
            y += (tt * 1.3).cos() * 0.9 * k * settle;
            // Twinkle steps 10×/s so held text repeats frames (cheap downstream).
            let tw_t = (t * 10.0).floor() / 10.0;
            let twinkle = 0.75 + 0.25 * (tw_t * 3.1 + p.phase * 3.0).sin();
            if exiting {
                let xs = clamp01((x - (cx - half_w)) / (2.0 * half_w).max(1.0));
                let qd = (0.5 * xs + 0.3 * p.e[0]) * self.exit_time * 0.55;
                let qq = clamp01((exit_t - qd) / (self.exit_time * 0.6));
                if qq > 0.0 {
                    let q2 = qq * qq;
                    match self.exit.as_str() {
                        "blow" => {
                            x += q2 * (w * 0.35 + p.e[1] * w * 0.45) + (qq * 9.0 + p.phase).sin() * 30.0 * k * qq;
                            y += -q2 * h * (0.05 + 0.25 * p.e[2]) + (qq * 7.0 + p.phase).cos() * 24.0 * k * qq;
                        }
                        "explode" => {
                            let (ox, oy) = (x - cx, y - cy);
                            let l = (ox * ox + oy * oy).sqrt().max(1.0);
                            let sp = (0.4 + 0.8 * p.e[1]) * w * 0.5 * (1.0 - (1.0 - qq).powi(3));
                            x += ox / l * sp + (p.e[2] - 0.5) * 60.0 * k * qq;
                            y += oy / l * sp + q2 * 120.0 * k;
                        }
                        _ => {
                            y -= q2 * 50.0 * k * (0.5 + p.e[1]);
                            x += (qq * 6.0 + p.phase).sin() * 10.0 * k * qq;
                        }
                    }
                    a *= 1.0 - qq;
                    r *= 1.0 - 0.5 * qq;
                }
            }
            let a = a * twinkle;
            if a < 0.04 || x < -10.0 || x > w + 10.0 || y < -10.0 || y > h + 10.0 {
                continue;
            }
            let ci = (clamp01((x - (cx - half_w)) / (2.0 * half_w).max(1.0)) * (NB - 1) as f32).round() as usize;
            let ai = ((a * NA as f32) as usize).min(NA - 1);
            core[ci * NA + ai].push([x, y, r.max(0.3)]);
        }
        let grad = |f: f32| -> Rgba {
            let f = clamp01(f) * (cols.len() - 1) as f32;
            let i = (f as usize).min(cols.len().saturating_sub(2));
            if cols.len() == 1 {
                return cols[0];
            }
            mix(cols[i], cols[i + 1], f - i as f32)
        };
        let mut dots = Vec::new();
        let mut halos = Vec::new();
        for (b, d) in core.into_iter().enumerate() {
            if d.is_empty() {
                continue;
            }
            let mut p = Path::from_vec(Vec::with_capacity(d.len() * 6));
            let bright = b % NA == NA - 1;
            let mut hp = Path::new();
            for [x, y, r] in d {
                dot(&mut p, x, y, r);
                if bright && self.glow {
                    dot(&mut hp, x, y, r * 2.6);
                }
            }
            if !hp.is_empty() {
                halos.push((b / NA, hp));
            }
            let (ci, ai) = (b / NA, b % NA);
            let c = shade(grad(ci as f32 / (NB - 1) as f32), if ai == NA - 1 { 0.12 } else { 0.0 });
            dots.push((ai == NA - 1, fill(p, solid(alpha(c, [0.45, 0.8, 1.0][ai])))));
        }
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, Blend::Screen, Vec::new());
        // Bloom while the text is formed (blurring a frame-wide scatter costs
        // far more and reads as haze anyway).
        let bloom = if exiting {
            1.0 - clamp01(exit_t / (self.exit_time * 0.4))
        } else if wi == 0 {
            clamp01((st - self.form * 0.55) / (self.form * 0.45))
        } else {
            1.0
        };
        if self.glow && bloom > 0.02 {
            // Bloom without a blur pass: bright dots again, larger and faint, added.
            push_layer(&mut out, bloom, Blend::Add, Vec::new());
            for (ci, hp) in halos {
                out.push(fill(hp, solid(alpha(grad(ci as f32 / (NB - 1) as f32), 0.16))));
            }
            out.push(Op::PopLayer);
        }
        out.extend(dots.into_iter().map(|d| d.1));
        out.push(Op::PopLayer);
        out
    }

    fn duration(&self) -> Option<f32> {
        Some(self.exit_start() + if self.exit == "none" { 0.0 } else { self.exit_time * 1.6 })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{build, preview, Theme};
    use serde_json::json;

    #[test]
    fn forms_reforms_and_leaves() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("particle-text", &json!({"words": ["Hello", "World"], "exit": "blow"})).unwrap();
        let end = c.component.duration().unwrap();
        assert!((end - (2.0 * 3.0 + 1.6 * 1.6)).abs() < 1e-3, "{end}");
        for (tag, t) in [("start", 0.4f32), ("hello", 2.5), ("between", 3.8), ("world", 5.6), ("exit", 6.5)] {
            let ops = c.eval(t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&ops).unwrap();
            assert!(!ops.is_empty(), "{tag}");
            assert_eq!(ops, c.eval(t, 960.0, 540.0, 30.0, &th, a));
            preview::snapshot(&format!("particle-text-{tag}"), 960, 540, th.background, ops, a);
        }
        assert!(c.eval(end + 0.1, 960.0, 540.0, 30.0, &th, a).is_empty());
        let c = build("particle-text", &json!({"text": "Ship it", "exit": "explode", "colors": ["#ffb547", "#ff5d73"]})).unwrap();
        let ops = c.eval(2.0, 1920.0, 1080.0, 30.0, &th, a);
        preview::snapshot("particle-text-1080", 1920, 1080, th.background, ops, a);
        assert!(build("particle-text", &json!({"text": "a", "exit": "boom"})).is_err());
    }
}
