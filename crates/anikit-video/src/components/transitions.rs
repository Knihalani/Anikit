//! Scene transitions.
//!
//! [`apply`] composites two scenes' ops (`from`, `to`) at transition
//! progress `p` (0..1) — this is what the evaluator calls for
//! `{"transition": {"type": "wipe", ...}}` between scenes. The `wipe`,
//! `iris` and `glitch` *components* are overlays: a colour panel that sweeps
//! in and out, an iris that closes and opens, glitch bars over the cut.

use super::style::*;
use super::{transform_ops, with_opacity, Component, Ctx, Prepare};
use crate::dl::{Blend, Filter, Op, Path, Rgba, IDENTITY};
use serde::Deserialize;

/// Transition parameters (spec: `{"type": "wipe", "direction": "left", ...}`).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Transition {
    /// fade, wipe, slide, push, zoom, iris, glitch, blur, whip, light-leak.
    #[serde(rename = "type")]
    pub kind: String,
    pub duration: f32,
    /// left, right, up, down (wipe/slide/push/whip); for wipe also any angle in degrees via `angle`.
    pub direction: String,
    pub angle: Option<f32>,
    /// Iris centre as a fraction of the frame.
    pub center: [f32; 2],
    /// Edge highlight colour for wipes and iris (none = no edge).
    pub edge: Option<Color>,
    pub seed: u32,
}
impl Default for Transition {
    fn default() -> Self {
        Transition { kind: "fade".into(), duration: 0.6, direction: "left".into(), angle: None, center: [0.5, 0.5], edge: None, seed: 1 }
    }
}

/// Transition kinds understood by [`apply`].
pub const KINDS: &[&str] = &["fade", "wipe", "slide", "push", "zoom", "iris", "glitch", "blur", "whip", "light-leak"];

impl Transition {
    pub fn validate(&self) -> Result<(), String> {
        if !KINDS.contains(&self.kind.as_str()) {
            return Err(match closest(&self.kind, KINDS) {
                Some(s) => format!("unknown transition `{}`; did you mean `{s}`?", self.kind),
                None => format!("unknown transition `{}`; use one of {}", self.kind, KINDS.join(", ")),
            });
        }
        if !["left", "right", "up", "down"].contains(&self.direction.as_str()) {
            return Err(format!("transition direction must be left, right, up or down, not {:?}", self.direction));
        }
        Ok(())
    }

    fn dir(&self) -> (f32, f32) {
        match self.direction.as_str() {
            "right" => (1.0, 0.0),
            "up" => (0.0, -1.0),
            "down" => (0.0, 1.0),
            _ => (-1.0, 0.0),
        }
    }
}

fn layer(clip: Option<Path>, opacity: f32, blend: Blend, filters: Vec<Filter>, ops: Vec<Op>) -> Vec<Op> {
    if ops.is_empty() || opacity <= 0.001 {
        return Vec::new();
    }
    let mut o = Vec::with_capacity(ops.len() + 2);
    o.push(Op::PushLayer { opacity: opacity.min(1.0), blend, filters, clip: clip.map(|c| (c, IDENTITY)), cache_key: None });
    o.extend(ops);
    o.push(Op::PopLayer);
    o
}

/// Half-plane clip polygon: the part of the frame "behind" a line at
/// position `k` (0..1) sweeping along `angle` (degrees, 0 = towards +x).
fn wipe_path(w: f32, h: f32, angle: f32, k: f32) -> Path {
    let (s, c) = angle.to_radians().sin_cos();
    // Project corners onto the sweep direction to find the extent.
    let proj = |x: f32, y: f32| x * c + y * s;
    let ds = [proj(0.0, 0.0), proj(w, 0.0), proj(0.0, h), proj(w, h)];
    let (lo, hi) = ds.iter().fold((f32::MAX, f32::MIN), |(a, b), d| (a.min(*d), b.max(*d)));
    let d = lo + (hi - lo) * k;
    // Big quad on the swept side of the line x·c + y·s = d.
    let big = (w + h) * 2.0;
    let (px, py) = (c * d, s * d); // a point on the line
    let (tx, ty) = (-s, c); // along the line
    let mut p = Path::new();
    p.move_to(((px + tx * big) as f64, (py + ty * big) as f64));
    p.line_to(((px - tx * big) as f64, (py - ty * big) as f64));
    p.line_to(((px - tx * big - c * big) as f64, (py - ty * big - s * big) as f64));
    p.line_to(((px + tx * big - c * big) as f64, (py + ty * big - s * big) as f64));
    p.close_path();
    p
}

fn edge_line(w: f32, h: f32, angle: f32, k: f32, c: Rgba) -> Vec<Op> {
    let (s, co) = angle.to_radians().sin_cos();
    let proj = |x: f32, y: f32| x * co + y * s;
    let ds = [proj(0.0, 0.0), proj(w, 0.0), proj(0.0, h), proj(w, h)];
    let (lo, hi) = ds.iter().fold((f32::MAX, f32::MIN), |(a, b), d| (a.min(*d), b.max(*d)));
    let d = lo + (hi - lo) * k;
    let big = (w + h) * 2.0;
    let (px, py) = (co * d, s * d);
    let mut p = Path::new();
    p.move_to(((px - s * big) as f64, (py + co * big) as f64));
    p.line_to(((px + s * big) as f64, (py - co * big) as f64));
    let mut o = Vec::new();
    push_layer(&mut o, 1.0, Blend::Screen, vec![Filter::Blur(12.0)]);
    o.push(stroke_op(p.clone(), solid(alpha(c, 0.9)), 10.0, [0.0, 1.0]));
    o.push(Op::PopLayer);
    o.push(stroke_op(p, solid(c), 2.0, [0.0, 1.0]));
    o
}

/// Composite `from` → `to` at progress `p` (0..1, already eased or not —
/// `apply` eases internally) on a `w × h` frame.
pub fn apply(tr: &Transition, p: f32, w: f32, h: f32, from: Vec<Op>, to: Vec<Op>) -> Vec<Op> {
    let p = clamp01(p);
    if p <= 0.0 {
        return from;
    }
    if p >= 1.0 {
        return to;
    }
    let e = ease_in_out(p);
    let (dx, dy) = tr.dir();
    let mut o = Vec::new();
    match tr.kind.as_str() {
        "wipe" => {
            let angle = tr.angle.unwrap_or(match tr.direction.as_str() {
                "right" => 0.0,
                "down" => 90.0,
                "up" => 270.0,
                _ => 180.0,
            });
            o.extend(from);
            o.extend(layer(Some(wipe_path(w, h, angle, e)), 1.0, Blend::Normal, vec![], to));
            if let Some(c) = tr.edge {
                o.extend(edge_line(w, h, angle, e, c.0));
            }
        }
        "slide" => {
            // Incoming slides over the outgoing scene, which recedes slightly.
            let mut a = from;
            transform_ops(&mut a, &scale_about(1.0 - 0.06 * e, w * 0.5, h * 0.5));
            o.extend(with_opacity(a, 1.0 - 0.5 * e));
            let mut b = to;
            transform_ops(&mut b, &translate(-dx * w * (1.0 - e), -dy * h * (1.0 - e)));
            o.extend(b);
        }
        "push" => {
            let mut a = from;
            transform_ops(&mut a, &translate(dx * w * e, dy * h * e));
            let mut b = to;
            transform_ops(&mut b, &translate(-dx * w * (1.0 - e), -dy * h * (1.0 - e)));
            o.extend(a);
            o.extend(b);
        }
        "zoom" => {
            let mut a = from;
            transform_ops(&mut a, &scale_about(1.0 + 0.25 * e, w * 0.5, h * 0.5));
            let mut b = to;
            transform_ops(&mut b, &scale_about(0.85 + 0.15 * e, w * 0.5, h * 0.5));
            o.extend(layer(None, 1.0 - e, Blend::Normal, vec![Filter::Blur(12.0 * e)], a));
            o.extend(layer(None, e, Blend::Normal, vec![Filter::Blur(12.0 * (1.0 - e))], b));
        }
        "iris" => {
            let (cx, cy) = (tr.center[0] * w, tr.center[1] * h);
            let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)].iter().map(|(x, y)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()).fold(0.0, f32::max);
            let r = far * e;
            o.extend(from);
            o.extend(layer(Some(circle_path(cx, cy, r)), 1.0, Blend::Normal, vec![], to));
            if let Some(c) = tr.edge {
                o.push(Op::Stroke { path: circle_path(cx, cy, r), paint: solid(c.0), stroke: line(3.0, [0.0, 1.0]), transform: IDENTITY });
            }
        }
        "blur" => {
            let b = 30.0 * (1.0 - (2.0 * p - 1.0).abs());
            o.extend(layer(None, 1.0, Blend::Normal, vec![Filter::Blur(b)], from));
            o.extend(layer(None, e, Blend::Normal, vec![Filter::Blur(b)], to));
        }
        "whip" => {
            // Fast push with heavy motion blur; both scenes stay edge to edge.
            let b = 40.0 * (1.0 - (2.0 * p - 1.0).abs()).powf(0.5);
            let k = ease_in_out(ease_in_out(p));
            let mut a = from;
            transform_ops(&mut a, &translate(dx * w * k, dy * h * k));
            let mut c = to;
            transform_ops(&mut c, &translate(-dx * w * (1.0 - k), -dy * h * (1.0 - k)));
            a.extend(c);
            o.extend(layer(None, 1.0, Blend::Normal, vec![Filter::Blur(b)], a));
        }
        "glitch" => {
            let src = if p < 0.5 { from } else { to };
            o.extend(glitch_slices(src, p, w, h, tr.seed));
        }
        "light-leak" => {
            o.extend(from);
            o.extend(layer(None, e, Blend::Normal, vec![], to));
            let k = (1.0 - (2.0 * p - 1.0).abs()).powf(0.7);
            o.extend(leak(w, h, p, k));
        }
        _ => {
            o.extend(from);
            o.extend(layer(None, e, Blend::Normal, vec![], to));
        }
    }
    o
}

/// Warm screen-blended flare sweeping across, peaking at `k`.
fn leak(w: f32, h: f32, p: f32, k: f32) -> Vec<Op> {
    let mut o = Vec::new();
    push_layer(&mut o, k, Blend::Screen, Vec::new());
    let cx = w * (-0.2 + 1.4 * p);
    for (c, r, dy) in [("#ff8a3d", 0.9, 0.3), ("#ffd27a", 0.55, 0.55), ("#ff3d6e", 0.7, 0.8)] {
        let col = parse_color(c).unwrap();
        o.push(rect(0.0, 0.0, w, h, 0.0, radial(cx, h * dy, w * r, vec![(0.0, alpha(col, 0.95)), (0.5, alpha(col, 0.4)), (1.0, alpha(col, 0.0))])));
    }
    o.push(rect(0.0, 0.0, w, h, 0.0, solid(Rgba([1.0, 0.95, 0.9, 0.35 * k]))));
    o.push(Op::PopLayer);
    o
}

/// The scene cut into horizontal slices shifted sideways, with an RGB split.
fn glitch_slices(src: Vec<Op>, p: f32, w: f32, h: f32, seed: u32) -> Vec<Op> {
    let amt = 1.0 - (2.0 * p - 1.0).abs();
    let frame = (p * 24.0).floor() as u32;
    let mut rng = Rng::new(((seed as u64) << 20) ^ frame as u64);
    let mut o = Vec::new();
    let mut y = 0.0;
    while y < h {
        let sh = h * rng.range(0.02, 0.12);
        let dx = if rng.f() < 0.6 * amt { rng.range(-1.0, 1.0) * w * 0.08 * amt } else { 0.0 };
        let clip = rrect_path(0.0, y, w, sh + 0.5, 0.0);
        let mut s = src.clone();
        transform_ops(&mut s, &translate(dx, 0.0));
        o.extend(layer(Some(clip), 1.0, Blend::Normal, vec![], s));
        y += sh;
    }
    // RGB split: red and cyan ghosts offset in opposite directions.
    let off = 10.0 * amt;
    if off > 0.5 {
        for (sign, m) in [(1.0f32, [1.0, 0.0, 0.0]), (-1.0, [0.0, 1.0, 1.0])] {
            let mut s = src.clone();
            transform_ops(&mut s, &translate(sign * off, 0.0));
            let cm = [m[0], 0.0, 0.0, 0.0, 0.0, 0.0, m[1], 0.0, 0.0, 0.0, 0.0, 0.0, m[2], 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0];
            o.extend(layer(None, 0.6 * amt, Blend::Screen, vec![Filter::ColorMatrix(cm)], s));
        }
    }
    o
}

// ------------------------------------------------------------------ overlay components

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Wipe {
    /// When the panel starts covering (s).
    pub at: f32,
    /// Seconds to cover; uncovering takes the same.
    pub duration: f32,
    /// Seconds fully covered (cut your scenes in this window).
    pub hold: f32,
    /// Panel colours: several make staggered layered panels.
    pub colors: Vec<Color>,
    pub direction: String,
    /// Slant of the panel edge in degrees.
    pub slant: f32,
}
impl Default for Wipe {
    fn default() -> Self {
        Wipe { at: 0.0, duration: 0.45, hold: 0.1, colors: Vec::new(), direction: "right".into(), slant: 12.0 }
    }
}
impl Prepare for Wipe {}
impl Component for Wipe {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let cols: Vec<Rgba> = if self.colors.is_empty() { vec![ctx.theme.accent, ctx.theme.pal(2), ctx.theme.surface] } else { self.colors.iter().map(|c| c.0).collect() };
        let angle = match self.direction.as_str() {
            "left" => 180.0,
            "down" => 90.0,
            "up" => 270.0,
            _ => 0.0,
        } + self.slant;
        let mut o = Vec::new();
        let n = cols.len();
        for (i, c) in cols.iter().enumerate() {
            let lag = i as f32 * 0.07;
            let cover = ease_in_out(seg(t, self.at + lag, self.duration));
            let uncover = ease_in_out(seg(t, self.at + self.duration + self.hold + (n - 1 - i) as f32 * 0.07, self.duration));
            if cover <= 0.0 || uncover >= 1.0 {
                continue;
            }
            // Region between the trailing (uncover) and leading (cover) edges.
            let lead = wipe_path(w, h, angle, cover * 1.15);
            o.extend(layer(Some(lead), 1.0, Blend::Normal, vec![], {
                let mut inner = Vec::new();
                if uncover > 0.0 {
                    let trail = wipe_path(w, h, angle + 180.0, 1.0 - uncover * 1.15 + 0.0);
                    inner.extend(layer(Some(trail), 1.0, Blend::Normal, vec![], vec![rect(-w, -h, w * 3.0, h * 3.0, 0.0, solid(*c))]));
                } else {
                    inner.push(rect(-w, -h, w * 3.0, h * 3.0, 0.0, solid(*c)));
                }
                inner
            }));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.at + 2.0 * self.duration + self.hold + 0.07 * self.colors.len().max(3) as f32)
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Iris {
    pub at: f32,
    pub duration: f32,
    pub hold: f32,
    pub color: Option<Color>,
    pub center: [f32; 2],
}
impl Default for Iris {
    fn default() -> Self {
        Iris { at: 0.0, duration: 0.5, hold: 0.15, color: None, center: [0.5, 0.5] }
    }
}
impl Prepare for Iris {}
impl Component for Iris {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (w, h) = (ctx.width, ctx.height);
        let c = self.color.map(|c| c.0).unwrap_or(Rgba([0.0, 0.0, 0.0, 1.0]));
        let close = ease_in_out(seg(t, self.at, self.duration));
        let open = ease_in_out(seg(t, self.at + self.duration + self.hold, self.duration));
        let k = close - open; // 0 open, 1 closed
        if k <= 0.0 {
            return Vec::new();
        }
        let (cx, cy) = (self.center[0] * w, self.center[1] * h);
        let far = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)].iter().map(|(x, y)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt()).fold(0.0, f32::max);
        let r = far * (1.0 - k);
        let mut p = rrect_path(-2.0, -2.0, w + 4.0, h + 4.0, 0.0);
        p.extend(circle_path(cx, cy, r));
        vec![Op::Fill { path: p, paint: solid(c), rule: crate::dl::FillRule::EvenOdd, transform: IDENTITY }]
    }
    fn duration(&self) -> Option<f32> {
        Some(self.at + 2.0 * self.duration + self.hold)
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Glitch {
    pub at: f32,
    pub duration: f32,
    pub intensity: f32,
    pub colors: Vec<Color>,
    pub seed: u32,
}
impl Default for Glitch {
    fn default() -> Self {
        Glitch { at: 0.0, duration: 0.5, intensity: 1.0, colors: Vec::new(), seed: 3 }
    }
}
impl Prepare for Glitch {}
impl Component for Glitch {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let p = seg(t, self.at, self.duration);
        if p <= 0.0 || p >= 1.0 {
            return Vec::new();
        }
        let (w, h) = (ctx.width, ctx.height);
        let amt = (1.0 - (2.0 * p - 1.0).abs()) * self.intensity;
        let cols: Vec<Rgba> = if self.colors.is_empty() { vec![parse_color("#ff2e63").unwrap(), parse_color("#08d9d6").unwrap(), Rgba([1.0, 1.0, 1.0, 1.0])] } else { self.colors.iter().map(|c| c.0).collect() };
        let frame = (t * 30.0).floor() as u64;
        let mut rng = Rng::new(((self.seed as u64) << 24) ^ frame);
        let mut o = Vec::new();
        let n = (24.0 * amt) as usize;
        push_layer(&mut o, 1.0, Blend::Screen, Vec::new());
        for i in 0..n {
            let y = rng.f() * h;
            let bh = h * rng.range(0.003, 0.03);
            let x = rng.range(-0.2, 0.8) * w;
            let bw = w * rng.range(0.1, 0.6);
            o.push(rect(x, y, bw, bh, 0.0, solid(alpha(cols[i % cols.len()], rng.range(0.3, 0.9) * amt.min(1.0)))));
        }
        o.push(Op::PopLayer);
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.at + self.duration)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{preview, Theme};
    use super::*;

    fn scene(c: &str, label: f32) -> Vec<Op> {
        let col = parse_color(c).unwrap();
        vec![rect(0.0, 0.0, 480.0, 270.0, 0.0, solid(col)), circle(240.0 + label, 135.0, 60.0, solid(Rgba([1.0, 1.0, 1.0, 0.9])))]
    }

    #[test]
    fn every_transition_at_every_phase() {
        let a = preview::host();
        let th = Theme::default();
        let mut sheet = Vec::new();
        for (i, k) in KINDS.iter().enumerate() {
            let tr = Transition { kind: k.to_string(), edge: Some(Color(th.accent)), ..Default::default() };
            tr.validate().unwrap();
            for p in [0.0, 0.3, 0.5, 0.7, 1.0] {
                let ops = apply(&tr, p, 480.0, 270.0, scene("#2a1f6e", -60.0), scene("#0f5c66", 60.0));
                preview::validate(&ops).unwrap_or_else(|e| panic!("{k} at {p}: {e}"));
            }
            assert_eq!(apply(&tr, 0.0, 480.0, 270.0, scene("#000000", 0.0), vec![]), scene("#000000", 0.0));
            let mut ops = apply(&tr, 0.4, 480.0, 270.0, scene("#2a1f6e", -60.0), scene("#0f5c66", 60.0));
            let m = [0.4, 0.0, 0.0, 0.4, (i % 5) as f32 * 192.0, (i / 5) as f32 * 108.0];
            // Clip each cell so slides do not leak into neighbours.
            transform_ops(&mut ops, &m);
            sheet.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(m[4], m[5], 192.0, 108.0, 0.0), IDENTITY)), cache_key: None });
            sheet.extend(ops);
            sheet.push(Op::PopLayer);
        }
        preview::validate(&sheet).unwrap();
        preview::snapshot("transitions", 960, 216, th.background, sheet, a);
        let bad = Transition { kind: "wip".into(), ..Default::default() };
        assert!(bad.validate().unwrap_err().contains("did you mean `wipe`"));
    }
}
