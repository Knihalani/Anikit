//! `particle-text`: thousands of glowing particles fly in, form a word, hold,
//! optionally re-form into the next word, then blow away.
//!
//! Target points come from the glyph outlines (bundled fonts, variation
//! aware), sampled once in `prepare` on a jittered grid so the dots cover the
//! letters evenly. Points are stored relative to the text's bounds, so the
//! same samples serve any box size. Motion is closed-form in `t`.

use super::fxkit::{self, hot, kelvin};
use super::fxpix;
use super::style::*;
use super::text::{Face, TextStyle};
use super::{Component, Ctx, Error, Prepare};
use crate::dl::{Blend, Filter, Op, Path, Rgba, IDENTITY};
use anikit_core::noise::Noise;
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
/// `path.winding(p)` for many points: the segments with their control-point
/// y-range, so segments a point's horizontal ray can't cross are skipped.
/// kurbo's winding is the sum over segments, and a segment whose y-range
/// (padded for rounding) excludes `p.y` contributes exactly 0, so the result
/// is the same as `path.winding(p)`; it just skips the per-segment extrema
/// split for most segments.
struct Winding {
    segs: Vec<(f64, f64, kurbo::BezPath)>,
}

impl Winding {
    fn new(path: &Path) -> Winding {
        use kurbo::ParamCurve;
        let segs = path
            .segments()
            .map(|seg| {
                let ys: Vec<f64> = match seg {
                    kurbo::PathSeg::Line(l) => vec![l.p0.y, l.p1.y],
                    kurbo::PathSeg::Quad(q) => vec![q.p0.y, q.p1.y, q.p2.y],
                    kurbo::PathSeg::Cubic(c) => vec![c.p0.y, c.p1.y, c.p2.y, c.p3.y],
                };
                let (lo, hi) = ys.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &y| (a.min(y), b.max(y)));
                // Sub-segment endpoints are evaluated points of the curve: inside the
                // control hull up to rounding, which this margin covers many times over.
                let pad = 1e-7 * (1.0 + lo.abs().max(hi.abs()));
                // One segment as its own path: its winding is that segment's term.
                let mut one = kurbo::BezPath::new();
                one.move_to(seg.start());
                one.push(seg.as_path_el());
                (lo - pad, hi + pad, one)
            })
            .collect();
        Winding { segs }
    }

    fn winding(&self, p: kurbo::Point) -> i32 {
        self.segs.iter().filter(|(lo, hi, _)| p.y >= *lo && p.y <= *hi).map(|(_, _, seg)| seg.winding(p)).sum()
    }
}

fn sample_inside(path: &Path, n: usize, seed: u64) -> Vec<[f32; 2]> {
    let wind = Winding::new(path);
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
                if wind.winding(kurbo::Point::new(jx, jy)) != 0 {
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
    /// Depth while flying (-1 far .. 1 near); 0 once formed.
    z: f32,
    /// Exit randoms.
    e: [f32; 3],
}

const FLOWS: &[&str] = &["curl", "direct", "spiral"];

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
    /// Gradient across the text (left → right). Default: the theme's text colour.
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
    /// Soft bloom around the formed text.
    pub glow: bool,
    /// Bloom strength (0..2).
    pub bloom: f32,
    /// How particles travel: `curl` (swirling noise flow), `direct` (gentle
    /// arcs) or `spiral` (orbit into place).
    pub flow: String,
    /// Strength of the flow's swirl (0..3).
    pub turbulence: f32,
    /// Depth while flying: near particles grow and blur, far ones shrink (0 = flat).
    pub depth: f32,
    /// Share of particles that glint while the text holds (0..1).
    pub sparkle: f32,
    pub seed: u32,
    /// `screen` (default: glowing light on dark) or `normal` (solid dots, for
    /// dark particles on a light background).
    pub blend: String,
    /// How much the exit sweeps left to right, 0..1 (1 = default wave, 0 =
    /// every particle leaves at once, a burst).
    pub stagger: f32,
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
            count: 6000,
            size: 1.9,
            colors: Vec::new(),
            font_size: None,
            weight: 800,
            face: Face::Display,
            form: 1.8,
            hold: 1.4,
            exit: "none".into(),
            exit_time: 1.6,
            delay: 0.0,
            glow: true,
            bloom: 0.7,
            flow: "curl".into(),
            turbulence: 1.0,
            depth: 1.0,
            sparkle: 0.5,
            seed: 1,
            blend: "screen".into(),
            stagger: 1.0,
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
        if !["screen", "normal"].contains(&self.blend.as_str()) {
            return Err(Error(format!("blend must be screen or normal, not {:?}", self.blend)));
        }
        fxkit::choice("flow", &self.flow, FLOWS)?;
        self.stagger = self.stagger.clamp(0.0, 1.0);
        self.bloom = self.bloom.clamp(0.0, 2.0);
        self.turbulence = self.turbulence.clamp(0.0, 3.0);
        self.depth = self.depth.clamp(0.0, 2.0);
        self.sparkle = self.sparkle.clamp(0.0, 1.0);
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
                let zr = rng.f();
                Particle {
                    from: [0.5 + a.cos() * r * 0.9, 0.5 + a.sin() * r * 0.75],
                    delay: rng.f(),
                    bow: (rng.f() - 0.5) * 0.7,
                    size: 0.6 + 0.7 * rng.f() * rng.f() + 0.25 * rng.f(),
                    phase: rng.f() * TAU,
                    z: (zr * 2.0 - 1.0) * (0.4 + 0.6 * zr),
                    e: [rng.f(), rng.f(), rng.f()],
                }
            })
            .collect();
        Ok(())
    }
}

/// Where one particle is at one time.
struct Place {
    x: f32,
    y: f32,
    a: f32,
    r: f32,
    /// Depth (0 = in the text plane).
    z: f32,
    /// 0 flying .. 1 settled in the text.
    settle: f32,
}

impl ParticleText {
    fn exit_start(&self) -> f32 {
        self.delay + self.words.len() as f32 * (self.form + self.hold)
    }

    /// Scale from reference outline units to box pixels (one size for all
    /// words, the widest decides, so re-forms don't jump).
    fn scale(&self, w: f32, h: f32) -> f32 {
        match self.font_size {
            Some(fs) => fs / REF,
            None => {
                let maxw = self.plan.iter().map(|p| p.w).fold(1.0f32, f32::max);
                (w * 0.8 / maxw).min(h * 0.30 / (REF * 0.72))
            }
        }
    }

    /// Particle `i` at time `t` (`None` when not yet visible).
    fn place(&self, i: usize, t: f32, w: f32, h: f32, flow: &Noise) -> Option<Place> {
        let lt = t - self.delay;
        if lt < 0.0 {
            return None;
        }
        let k = h / 1080.0;
        let p = &self.parts[i];
        let cyc = self.form + self.hold;
        let nw = self.plan.len();
        let wi = ((lt / cyc).floor() as usize).min(nw - 1);
        let st = lt - wi as f32 * cyc;
        let (cx, cy) = (w * 0.5, h * 0.5);
        let s = self.scale(w, h);
        let half_w = self.plan.iter().map(|p| p.w).fold(1.0f32, f32::max) * s * 0.5;
        let tw = &self.plan[wi].pts;
        let target = [cx + tw[i][0] * s, cy + tw[i][1] * s];
        let src = match wi {
            0 => [p.from[0] * w, p.from[1] * h],
            _ => {
                let f = &self.plan[wi - 1].pts;
                [cx + f[i][0] * s, cy + f[i][1] * s]
            }
        };
        let span = 0.45;
        let xf = clamp01((target[0] - (cx - half_w)) / (2.0 * half_w).max(1.0));
        let d = (0.55 * p.delay + 0.45 * xf) * span * self.form;
        let q = clamp01((st - d) / (self.form * (1.0 - span)));
        let e = if wi == 0 { ease_out(q) } else { ease_in_out(q) };
        let (dx, dy) = (target[0] - src[0], target[1] - src[1]);
        let left = 1.0 - e;
        let mut x;
        let mut y;
        match self.flow.as_str() {
            "spiral" => {
                // Orbit into place: the offset from the target turns as it shrinks.
                let ang = left * left * (2.4 + 1.6 * p.bow) * if i.is_multiple_of(2) { 1.0 } else { -1.0 };
                let (sn, cs) = ang.sin_cos();
                let (ox, oy) = (-dx * left, -dy * left);
                x = target[0] + ox * cs - oy * sn;
                y = target[1] + ox * sn + oy * cs;
            }
            _ => {
                let bow = (q * PI).sin() * p.bow;
                x = src[0] + dx * e - dy * bow;
                y = src[1] + dy * e + dx * bow;
            }
        }
        if self.flow == "curl" && left > 0.0 && self.turbulence > 0.0 {
            // Swirl through a divergence-free noise field, fading as particles land.
            // Sampled along the particle's own path (from its target, as the
            // flight progresses), so neighbours move together in ribbons.
            let amp = 0.2 * h * self.turbulence * left.powf(1.3) * (0.7 + 0.6 * p.e[0]);
            let sc = 1.0 / (0.45 * h);
            let (fx, fy) = fxkit::curl(flow, target[0] * sc + left * 0.9, target[1] * sc - left * 0.6, t * 0.08 + wi as f32 * 3.0);
            x += fx * amp;
            y += fy * amp;
        }
        let mut a = if wi == 0 { clamp01(q * 2.5) } else { 1.0 };
        let mut r = self.size * k * p.size * if wi == 0 { 0.5 + 0.5 * e } else { 1.0 };
        let mut z = p.z * self.depth * left.powf(1.2) * if wi == 0 { 1.0 } else { 0.5 };
        let settle = clamp01((q - 0.85) / 0.15);
        // Settled: a slow breathing shimmer.
        let tt = t * 1.4 + p.phase;
        x += tt.sin() * 0.8 * k * settle;
        y += (tt * 1.3).cos() * 0.8 * k * settle;
        let exit_t = t - self.exit_start();
        if self.exit != "none" && exit_t > 0.0 {
            let xs = clamp01((x - (cx - half_w)) / (2.0 * half_w).max(1.0));
            let qd = (0.5 * xs + 0.3 * p.e[0]) * self.exit_time * 0.55 * self.stagger;
            let qq = clamp01((exit_t - qd) / (self.exit_time * 0.6));
            if qq > 0.0 {
                let q2 = qq * qq;
                match self.exit.as_str() {
                    "blow" => {
                        x += q2 * (w * 0.35 + p.e[1] * w * 0.45);
                        y += -q2 * h * (0.05 + 0.25 * p.e[2]);
                        if self.turbulence > 0.0 {
                            let sc = 1.0 / (0.3 * h);
                            let (fx, fy) = fxkit::curl(flow, x * sc, y * sc, t * 0.3 + 7.0);
                            x += fx * 0.12 * h * qq * self.turbulence;
                            y += fy * 0.12 * h * qq * self.turbulence;
                        }
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
                z += (p.e[2] - 0.3) * qq * self.depth;
            }
        }
        // Depth: near grows, far shrinks and dims.
        r *= (1.0 + 0.9 * z).max(0.35);
        a *= 1.0 - 0.45 * z.abs().min(1.0);
        Some(Place { x, y, a, r, z, settle })
    }
}

/// Colour buckets across the text and brightness levels.
const NB: usize = 8;
const NA: usize = 4;

/// Particles splatted directly into one image (the speed shortcut for the
/// thousands of dot fills): the glow (union of the bright dots' halos,
/// blurred with the renderer's own blur), the out-of-focus gaussian discs,
/// then the dots, bucket by bucket (union within a bucket, as one fill
/// draws them; source-over between buckets), all in premultiplied sRGB.
#[derive(Default)]
struct Splat {
    /// `(bucket, x, y, r)`.
    dots: Vec<(usize, f32, f32, f32)>,
    /// Halo discs `(x, y, r)` of the bright dots.
    glow: Vec<(f32, f32, f32)>,
    /// Glow colour (premultiplied) and blur sigma, when the glow shows.
    glow_paint: Option<([f32; 4], f32)>,
    /// Blur sigma of the near (out-of-focus) dots' layer.
    near_blur: f32,
    dot_col: [[f32; 4]; NB * NA],
    /// Scratch, reused per thread: pixels (premultiplied) that dots or
    /// discs touched and when (`stamp == frame`), the touched list,
    /// coverage (kept all zero between uses), glow coverage, dot order.
    img: Vec<[f32; 4]>,
    stamp: Vec<u32>,
    frame: u32,
    touched: Vec<u32>,
    cov: Vec<f32>,
    gl: Vec<u8>,
    /// The near dots' layer (premultiplied, then RGBA8 for the blur).
    near: Vec<[f32; 4]>,
    near8: Vec<u8>,
    order: Vec<u32>,
    blur: Option<Box<crate::render::blur::BlurScratch>>,
}

thread_local! {
    static SPLAT: std::cell::Cell<Splat> = std::cell::Cell::new(Splat::default());
}

/// Coverage of the pixel centred at `(px, py)` by a disc of radius `r` at
/// `(cx, cy)`: a one-pixel ramp across the edge, or for sub-pixel discs an
/// equal-area square (exact total area, no lost specks).
#[inline(always)]
fn disc_cov(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> f32 {
    use fxpix::{mn, mx};
    if r >= 1.0 {
        // Exact pixel-area coverage of the tangent half-plane: the CDF of a
        // unit square projected on the edge normal (a trapezoid).
        let (dx, dy) = ((px - cx).abs(), (py - cy).abs());
        let d = (dx * dx + dy * dy).sqrt();
        let u = r - d;
        let inv = 1.0 / mx(d, 1e-6);
        let (a, b) = (mx(dx, dy) * inv, mn(dx, dy) * inv);
        let (s1, s2) = (0.5 * (a - b), 0.5 * (a + b));
        if u <= -s2 {
            0.0
        } else if u >= s2 {
            1.0
        } else if u < -s1 {
            (u + s2) * (u + s2) / mx(2.0 * a * b, 1e-9)
        } else if u > s1 {
            1.0 - (s2 - u) * (s2 - u) / mx(2.0 * a * b, 1e-9)
        } else {
            0.5 + u / mx(a, 1e-6)
        }
    } else {
        let hs = 0.886_226_9 * r; // half side: (r·√π) / 2
        let o = |p: f32, c: f32| mx(mn(p + 0.5, c + hs) - mx(p - 0.5, c - hs), 0.0);
        o(px, cx) * o(py, cy)
    }
}

/// Radius of the disc with the area `fxkit::dot` draws (an octagon below 2.5 px).
#[inline(always)]
fn dot_radius(r: f32) -> f32 {
    if r.max(0.05) < 2.5 {
        r.max(0.05) * 0.949_1
    } else {
        r
    }
}

impl Splat {
    fn take() -> Splat {
        let mut s = SPLAT.with(|c| c.take());
        s.dots.clear();
        s.glow.clear();
        s.glow_paint = None;
        s
    }

    fn give_back(self) {
        SPLAT.with(|c| c.set(self));
    }

    fn image(&mut self, ctx: &Ctx) -> Option<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let gm = self.glow_paint.map_or(0.0, |(_, s)| (s * 3.0).ceil() + 2.0);
        let mut bb = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut grow = |x: f32, y: f32, e: f32| bb = [bb[0].min(x - e), bb[1].min(y - e), bb[2].max(x + e), bb[3].max(y + e)];
        for &(_, x, y, r) in &self.dots {
            grow(x, y, r + 1.0);
        }
        let nm = (self.near_blur * 3.0).ceil() + 2.0;
        for &(b, x, y, r) in &self.dots {
            if b >= NB * NA {
                grow(x, y, r + 1.0 + nm);
            }
        }
        if self.glow_paint.is_some() {
            for &(x, y, r) in &self.glow {
                grow(x, y, r + 1.0 + gm);
            }
        }
        let x0 = bb[0].max(0.0).floor();
        let y0 = bb[1].max(0.0).floor();
        let x1 = bb[2].min(w).ceil();
        let y1 = bb[3].min(h).ceil();
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        let (iw, ih) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let mut hs = fxpix::Hash::new();
        hs.bytes(b"particle-text-splat");
        hs.f32s(&[x0, y0, x1, y1]);
        for &(b, x, y, r) in &self.dots {
            hs.u32(b as u32);
            hs.f32s(&[x, y, r]);
        }
        hs.f32s(&[self.near_blur]);
        if let Some((c, s)) = self.glow_paint {
            hs.f32s(&c);
            hs.f32s(&[s]);
            for &(x, y, r) in &self.glow {
                hs.f32s(&[x, y, r]);
            }
        }
        for c in self.dot_col.iter() {
            hs.f32s(c);
        }
        let id = ctx.assets.image_premul(hs.finish(), iw as u32, ih as u32, &mut || self.pixels(x0, y0, iw, ih));
        Some(Op::Image { image: id, dst: [x0, y0, iw as f32, ih as f32], opacity: 1.0, transform: IDENTITY })
    }

    /// Pixel bounds `[i0, i1) × [j0, j1)` of a disc in a `iw × ih` image at `(x0, y0)`.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn span(x: f32, y: f32, e: f32, x0: f32, y0: f32, iw: usize, ih: usize) -> (usize, usize, usize, usize) {
        let i0 = (x - e - x0).floor().max(0.0) as usize;
        let j0 = (y - e - y0).floor().max(0.0) as usize;
        let i1 = ((x + e - x0).ceil().max(0.0) as usize).min(iw);
        let j1 = ((y + e - y0).ceil().max(0.0) as usize).min(ih);
        (i0, i1, j0, j1)
    }

    /// One bucket of dots: coverage summed (clamped, like one non-zero
    /// fill of all its discs), then `put` once per covered pixel.
    #[allow(clippy::too_many_arguments)]
    fn bucket(dots: &[(usize, f32, f32, f32)], run: &[u32], cov: &mut [f32], iw: usize, ih: usize, x0: f32, y0: f32, c: [f32; 4], put: &mut dyn FnMut(usize, [f32; 4])) {
        if run.is_empty() {
            return;
        }
        for &k in run {
            let (_, x, y, r) = dots[k as usize];
            let rr = dot_radius(r);
            let (i0, i1, j0, j1) = Self::span(x, y, rr + 1.0, x0, y0, iw, ih);
            for j in j0..j1 {
                let py = y0 + j as f32 + 0.5;
                for i in i0..i1 {
                    cov[j * iw + i] += disc_cov(x0 + i as f32 + 0.5, py, x, y, rr);
                }
            }
        }
        for &k in run {
            let (_, x, y, r) = dots[k as usize];
            let rr = dot_radius(r);
            let (i0, i1, j0, j1) = Self::span(x, y, rr + 1.0, x0, y0, iw, ih);
            for j in j0..j1 {
                for i in i0..i1 {
                    let q = cov[j * iw + i];
                    if q > 0.0 {
                        let a = q.min(1.0);
                        cov[j * iw + i] = 0.0;
                        put(j * iw + i, [c[0] * a, c[1] * a, c[2] * a, c[3] * a]);
                    }
                }
            }
        }
    }

    fn pixels(&mut self, x0: f32, y0: f32, iw: usize, ih: usize) -> Vec<u8> {
        let n = iw * ih;
        // Scratch sizes only grow; `cov` stays zero, `img` is valid where stamped.
        if self.cov.len() < n {
            self.cov.resize(n, 0.0);
            self.img.resize(n, [0.0; 4]);
            self.stamp.resize(n, 0);
        }
        self.frame = self.frame.wrapping_add(1);
        if self.frame == 0 {
            self.stamp.iter_mut().for_each(|s| *s = 0);
            self.frame = 1;
        }
        self.touched.clear();
        let (img, cov, stamp, touched, frame) = (&mut self.img[..n], &mut self.cov[..n], &mut self.stamp[..n], &mut self.touched, self.frame);
        // 1. Glow: union of the halo discs, blurred, in the glow colour.
        let glow = self.glow_paint;
        let gl = &mut self.gl;
        gl.clear();
        if let Some((_, sigma)) = glow {
            for &(x, y, r) in &self.glow {
                let rr = dot_radius(r);
                let (i0, i1, j0, j1) = Self::span(x, y, rr + 1.0, x0, y0, iw, ih);
                for j in j0..j1 {
                    let py = y0 + j as f32 + 0.5;
                    for i in i0..i1 {
                        cov[j * iw + i] += disc_cov(x0 + i as f32 + 0.5, py, x, y, rr);
                    }
                }
            }
            gl.extend(cov.iter().map(|c| fxpix::byte(fxpix::mn(*c, 1.0) * 255.0)));
            cov.iter_mut().for_each(|c| *c = 0.0);
            let bs = self.blur.get_or_insert_with(Default::default);
            crate::render::blur::blur::<1>(gl, iw, ih, sigma, bs);
        }
        let gl = &self.gl;
        // A pixel's colour before discs and dots: the glow, or nothing.
        let base = |p: usize| match glow {
            Some((gc, _)) => {
                let k = gl[p] as f32 * (1.0 / 255.0);
                [gc[0] * k, gc[1] * k, gc[2] * k, gc[3] * k]
            }
            None => [0.0; 4],
        };
        // Composite `c` over pixel `p`, starting from the glow the first time.
        let mut put = |p: usize, c: [f32; 4]| {
            if stamp[p] != frame {
                stamp[p] = frame;
                touched.push(p as u32);
                img[p] = base(p);
            }
            img[p] = fxpix::over(c, img[p]);
        };
        // 3. Dots, bucket by bucket: coverage summed (clamped, like one
        // non-zero fill of all of a bucket's discs), then composited once.
        const NBK: usize = NB * NA * 2;
        let order = &mut self.order;
        order.clear();
        order.resize(self.dots.len(), 0);
        let mut first = [0u32; NBK + 1];
        for d in &self.dots {
            first[d.0 + 1] += 1;
        }
        for b in 0..NBK {
            first[b + 1] += first[b];
        }
        let mut fill = first;
        for (k, d) in self.dots.iter().enumerate() {
            order[fill[d.0] as usize] = k as u32;
            fill[d.0] += 1;
        }
        // Near dots first (they sit behind): composited into their own
        // layer over their bounds, blurred like the renderer blurs a layer,
        // then laid over the glow; then the sharp dots.
        let (nb0, nb1) = (first[NB * NA] as usize, first[NBK] as usize);
        if nb1 > nb0 {
            let nm = (self.near_blur * 3.0).ceil() + 2.0;
            let mut bb = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for &k in &order[nb0..nb1] {
                let (_, x, y, r) = self.dots[k as usize];
                let e = r + 1.0 + nm;
                bb = [bb[0].min(x - e), bb[1].min(y - e), bb[2].max(x + e), bb[3].max(y + e)];
            }
            let (bx0, by0) = (((bb[0] - x0).floor().max(0.0) as usize).min(iw), ((bb[1] - y0).floor().max(0.0) as usize).min(ih));
            let (bx1, by1) = (((bb[2] - x0).ceil().max(0.0) as usize).min(iw), ((bb[3] - y0).ceil().max(0.0) as usize).min(ih));
            let (bw, bh) = (bx1.saturating_sub(bx0), by1.saturating_sub(by0));
            if bw > 0 && bh > 0 {
                let near = &mut self.near;
                near.clear();
                near.resize(bw * bh, [0.0; 4]);
                for b in NB * NA..NBK {
                    let run = &order[first[b] as usize..first[b + 1] as usize];
                    let c = self.dot_col[b % (NB * NA)];
                    Self::bucket(&self.dots, run, cov, iw, ih, x0, y0, c, &mut |p, v| {
                        let (i, j) = (p % iw, p / iw);
                        if i >= bx0 && i < bx1 && j >= by0 && j < by1 {
                            let q = &mut near[(j - by0) * bw + (i - bx0)];
                            *q = fxpix::over(v, *q);
                        }
                    });
                }
                let n8 = &mut self.near8;
                n8.clear();
                n8.extend(near.iter().flat_map(|c| fxpix::pack(*c)));
                let bs = self.blur.get_or_insert_with(Default::default);
                crate::render::blur::blur::<4>(n8, bw, bh, self.near_blur, bs);
                for j in 0..bh {
                    for i in 0..bw {
                        let q = &n8[(j * bw + i) * 4..][..4];
                        if q[3] > 0 || q[0] > 0 || q[1] > 0 || q[2] > 0 {
                            put((by0 + j) * iw + bx0 + i, [0, 1, 2, 3].map(|k| q[k] as f32 * (1.0 / 255.0)));
                        }
                    }
                }
            }
        }
        for b in 0..NB * NA {
            let run = &order[first[b] as usize..first[b + 1] as usize];
            let c = self.dot_col[b];
            Self::bucket(&self.dots, run, cov, iw, ih, x0, y0, c, &mut put);
        }
        // 4. Premultiplied RGBA8: the glow through a table, then the touched pixels.
        let mut px = vec![0u8; n * 4];
        if let Some((gc, _)) = glow {
            let tab: Vec<[u8; 4]> = (0..256).map(|g| fxpix::pack(gc.map(|v| v * g as f32 / 255.0))).collect();
            for (o, &g) in px.chunks_exact_mut(4).zip(gl.iter()) {
                o.copy_from_slice(&tab[g as usize]);
            }
        }
        for &p in self.touched.iter() {
            let o = p as usize * 4;
            px[o..o + 4].copy_from_slice(&fxpix::pack(self.img[p as usize]));
        }
        px
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
        let exit_t = t - self.exit_start();
        let exiting = self.exit != "none" && exit_t > 0.0;
        if exiting && exit_t > self.exit_time * 1.6 {
            return Vec::new();
        }
        let normal = self.blend == "normal";
        let cols: Vec<Rgba> = fxkit::colors_or(&self.colors, || {
            let base = ctx.theme.text;
            vec![mix(base, kelvin(5200.0), if ctx.theme.dark { 0.25 } else { 0.0 })]
        });
        let flow = Noise::new(self.seed ^ 0xf10);
        let s = self.scale(w, h);
        let half_w = self.plan.iter().map(|p| p.w).fold(1.0f32, f32::max) * s * 0.5;
        let cx = w * 0.5;
        let shutter = 0.5 / ctx.fps.max(1.0);
        // Buckets: depth band (0 sharp, 1 near/blurred) × colour × brightness.
        // Shortcut: unless exact, dots, glow and out-of-focus discs are
        // splatted straight into one image (see [`Splat`]).
        let direct = !fxkit::exact();
        let mut sp = if direct { Splat::take() } else { Splat::default() };
        let mut dots: Vec<Path> = vec![Path::new(); 2 * NB * NA];
        let mut streaks: Vec<Path> = vec![Path::new(); 2 * NB];
        let mut glow = Path::new();
        let mut sparks = Vec::new();
        let tw_t = step(t, 30.0);
        for i in 0..self.parts.len() {
            let Some(pl) = self.place(i, t, w, h, &flow) else { continue };
            let p = &self.parts[i];
            let twinkle = 0.8 + 0.2 * (tw_t * 2.3 + p.phase * 3.0).sin();
            let a = pl.a * twinkle;
            let m = 12.0 * k;
            if a < 0.03 || pl.x < -m || pl.x > w + m || pl.y < -m || pl.y > h + m {
                continue;
            }
            let band = (pl.z > 0.4) as usize;
            let ci = (clamp01((pl.x - (cx - half_w)) / (2.0 * half_w).max(1.0)) * (NB - 1) as f32).round() as usize;
            let ci = if cols.len() == 1 { 0 } else { ci };
            // Motion: fast-leaving particles become short streaks.
            if exiting {
                if let Some(pp) = self.place(i, t - shutter, w, h, &flow) {
                    let mv = ((pl.x - pp.x).powi(2) + (pl.y - pp.y).powi(2)).sqrt();
                    if mv > pl.r * 2.5 {
                        fxkit::trail_path(&mut streaks[band * NB + ci], &[(pp.x, pp.y), (pl.x, pl.y)], pl.r * 0.5, pl.r * 1.8);
                        continue;
                    }
                }
            }
            let ai = ((a * NA as f32) as usize).min(NA - 1);
            if direct && band == 1 {
                sp.dots.push(((band * NB + ci) * NA + ai, pl.x, pl.y, pl.r.max(0.3)));
            } else {
                fxkit::dot(&mut dots[(band * NB + ci) * NA + ai], pl.x, pl.y, pl.r.max(0.3));
            }
            if self.glow && band == 0 && ai >= NA - 2 {
                if direct {
                    sp.glow.push((pl.x, pl.y, pl.r * 1.8));
                } else {
                    fxkit::dot(&mut glow, pl.x, pl.y, pl.r * 1.8);
                }
            }
            // Glints: a few settled particles flare briefly with a cross of light.
            if pl.settle > 0.99 && !exiting && self.sparkle > 0.0 {
                let rate = 0.35 + 0.3 * p.e[2];
                let ph = (t * rate + p.e[0] * 7.0).fract();
                if p.e[1] < self.sparkle * 0.06 && ph < 0.12 {
                    let f = (ph / 0.12 * PI).sin();
                    sparks.push((pl.x, pl.y, pl.r, f, ci));
                }
            }
        }
        let grad = |ci: usize| -> Rgba {
            if cols.len() == 1 {
                cols[0]
            } else {
                fxkit::ramp(&cols, ci as f32 / (NB - 1) as f32)
            }
        };
        let lvl_col = |ci: usize, ai: usize| -> Rgba {
            let c = grad(ci);
            let c = if !normal && ai == NA - 1 { hot(c, 0.25) } else { c };
            alpha(c, [0.35, 0.6, 0.82, 1.0][ai])
        };
        let mut out = Vec::new();
        push_layer(&mut out, 1.0, if normal { Blend::Normal } else { Blend::Add }, Vec::new());
        // Bloom: the bright dots again, blurred (only while the text is formed;
        // blurring a frame-wide scatter costs far more and reads as haze).
        let cyc = self.form + self.hold;
        let st = lt % cyc;
        let first = lt < cyc;
        let bloom = if exiting {
            1.0 - clamp01(exit_t / (self.exit_time * 0.4))
        } else if first {
            clamp01((st - self.form * 0.5) / (self.form * 0.5))
        } else {
            1.0
        } * self.bloom;
        if direct {
            let glow_on = self.glow && !normal && bloom > 0.02 && !sp.glow.is_empty();
            sp.glow_paint = glow_on.then(|| (fxpix::premul(alpha(grad(NB / 2), 0.55 * bloom)), 7.0 * k));
            sp.near_blur = 3.5 * k;
            for ci in 0..NB {
                for ai in 0..NA {
                    sp.dot_col[ci * NA + ai] = fxpix::premul(lvl_col(ci, ai));
                }
            }
            if let Some(op) = sp.image(ctx) {
                out.push(op);
            }
            sp.give_back();
            // Near motion streaks (exits), blurred like the near dots.
            if streaks[NB..2 * NB].iter().any(|p| !p.is_empty()) {
                let pushed = fxkit::push_blur(&mut out, 3.5 * k, Blend::Normal);
                for ci in 0..NB {
                    fxkit::fill_solid(&mut out, std::mem::take(&mut streaks[NB + ci]), alpha(grad(ci), 0.45));
                }
                fxkit::pop_if(&mut out, pushed);
            }
            // Sharp dots and streaks stay vector fills (their edges are what the eye checks).
            for ci in 0..NB {
                fxkit::fill_solid(&mut out, std::mem::take(&mut streaks[ci]), alpha(grad(ci), 0.45));
                for ai in 0..NA {
                    fxkit::fill_solid(&mut out, std::mem::take(&mut dots[ci * NA + ai]), lvl_col(ci, ai));
                }
            }
        } else {
            if self.glow && !normal && bloom > 0.02 && !glow.is_empty() {
                push_layer(&mut out, 1.0, Blend::Normal, vec![Filter::Blur(7.0 * k)]);
                out.push(fill(glow, solid(alpha(grad(NB / 2), 0.55 * bloom))));
                out.push(Op::PopLayer);
            }
            for band in [1usize, 0] {
                let pushed = if band == 1 { fxkit::push_blur(&mut out, 3.5 * k, Blend::Normal) } else { false };
                for ci in 0..NB {
                    fxkit::fill_solid(&mut out, std::mem::take(&mut streaks[band * NB + ci]), alpha(grad(ci), 0.45));
                    for ai in 0..NA {
                        fxkit::fill_solid(&mut out, std::mem::take(&mut dots[(band * NB + ci) * NA + ai]), lvl_col(ci, ai));
                    }
                }
                fxkit::pop_if(&mut out, pushed);
            }
        }
        for (x, y, r, f, ci) in sparks {
            let c = hot(grad(ci), 0.6);
            let l = r * (7.0 + 5.0 * f);
            out.push(fxkit::light(x, y, r * 6.0, fxkit::gauss(c, 0.7 * f)));
            out.push(fxkit::needle(x, y, 0.0, l, r * 0.9, c, f));
            out.push(fxkit::needle(x, y, PI * 0.5, l * 0.8, r * 0.9, c, f));
        }
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

    /// The segment index gives exactly kurbo's winding number, including on
    /// and next to the outline (points on segment end y values, extrema).
    #[test]
    fn winding_index_matches_kurbo() {
        use super::super::text::{Face, TextStyle};
        use super::{text_outline, Rgba, Rng, Winding, REF};
        use kurbo::{ParamCurve, Shape};
        for (word, face) in [("PULSE", Face::Display), ("Ship it, gq@&%", Face::Sans), ("Hello", Face::Serif)] {
            let st = TextStyle::new(face, REF, 700, Rgba([1.0; 4]));
            let path = text_outline(word, &st);
            let wind = Winding::new(&path);
            let b = path.bounding_box();
            let mut rng = Rng::new(9);
            let mut checked = 0;
            for i in 0..40_000 {
                let p = if i % 4 == 0 {
                    // Exactly on a segment end point's y (and x), the fiddly case.
                    let seg = path.segments().nth(i / 4 % path.segments().count()).unwrap();
                    let e = if i % 8 == 0 { seg.start() } else { seg.end() };
                    kurbo::Point::new(e.x + (rng.f() as f64 - 0.5) * 3.0, e.y)
                } else {
                    kurbo::Point::new(b.x0 + rng.f() as f64 * b.width(), b.y0 + rng.f() as f64 * b.height())
                };
                assert_eq!(wind.winding(p), path.winding(p), "{word} at {p:?}");
                checked += 1;
            }
            assert_eq!(checked, 40_000);
        }
    }

    #[test]
    fn forms_reforms_and_leaves() {
        let th = Theme::default();
        let a = preview::host();
        let c = build("particle-text", &json!({"words": ["Hello", "World"], "exit": "blow", "form": 1.6})).unwrap();
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
        // Solid dots for light backgrounds, and a burst exit (no sweep).
        let c = build("particle-text", &json!({"text": "Go", "blend": "normal", "stagger": 0, "exit": "explode", "form": 0.1, "hold": 0})).unwrap();
        let ops = c.eval(0.15, 960.0, 540.0, 30.0, &th, a);
        preview::validate(&ops).unwrap();
        assert!(!ops.is_empty());
        assert!(build("particle-text", &json!({"text": "a", "blend": "add"})).is_err());
    }
}
