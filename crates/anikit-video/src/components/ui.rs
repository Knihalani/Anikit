//! UI kit: surfaces (cards, buttons, toggles, badges, avatars, progress,
//! toasts), pointers and overlays (cursor, tooltip, callout, arrow) and
//! framing (lower third, title card).
//!
//! Shared timing keys: `delay` (entrance start, s) and, where it makes sense,
//! `out` (exit start, s; omitted = stays).

use super::style::*;
use super::text::{draw, wrap, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Prepare};
use crate::dl::{Blend, Op, Path, Rgba, IDENTITY};
use serde::Deserialize;

/// Entrance/exit envelope: 0 → 1 with a spring in, ease out.
fn presence(t: f32, delay: f32, out: Option<f32>, out_dur: f32) -> (f32, f32) {
    let enter = spring(t - delay, 1.0);
    let exit = out.map(|o| ease_in(seg(t, o, out_dur))).unwrap_or(0.0);
    (enter, exit)
}

/// Rise-and-fade entrance about a pivot.
fn reveal(mut ops: Vec<Op>, p: f32, exit: f32, px: f32, py: f32, rise: f32) -> Vec<Op> {
    let s = lerp(0.94, 1.0, p.min(1.05)) * lerp(1.0, 0.97, exit);
    let dy = (1.0 - p) * rise - exit * rise * 0.5;
    let m = scale_about(s, px, py);
    let m = crate::dl::mul(&translate(0.0, dy), &m);
    transform_ops(&mut ops, &m);
    with_opacity(ops, clamp01(p * 1.6) * (1.0 - exit))
}

fn sans(size: f32, weight: u16, c: Rgba) -> TextStyle {
    TextStyle::new(Face::Sans, size, weight, c)
}

/// Icon glyphs drawn as vectors so they work without fonts.
fn icon(out: &mut Vec<Op>, kind: &str, cx: f32, cy: f32, s: f32, c: Rgba) {
    let mut p = Path::new();
    let pt = |x: f32, y: f32| ((cx + x * s) as f64, (cy + y * s) as f64);
    match kind {
        "success" | "check" => {
            p.move_to(pt(-0.42, 0.02));
            p.line_to(pt(-0.12, 0.32));
            p.line_to(pt(0.45, -0.3));
        }
        "error" | "x" => {
            p.move_to(pt(-0.3, -0.3));
            p.line_to(pt(0.3, 0.3));
            p.move_to(pt(0.3, -0.3));
            p.line_to(pt(-0.3, 0.3));
        }
        "warning" | "info" => {
            let (a, b) = if kind == "info" { (-0.05, 0.38) } else { (-0.38, 0.12) };
            p.move_to(pt(0.0, a));
            p.line_to(pt(0.0, b));
            out.push(circle(cx, cy + if kind == "info" { -0.36 } else { 0.38 } * s, 0.09 * s, solid(c)));
        }
        "plus" => {
            p.move_to(pt(-0.38, 0.0));
            p.line_to(pt(0.38, 0.0));
            p.move_to(pt(0.0, -0.38));
            p.line_to(pt(0.0, 0.38));
        }
        "arrow" => {
            p.move_to(pt(-0.4, 0.0));
            p.line_to(pt(0.4, 0.0));
            p.move_to(pt(0.08, -0.32));
            p.line_to(pt(0.4, 0.0));
            p.line_to(pt(0.08, 0.32));
        }
        "bolt" => {
            p.move_to(pt(0.1, -0.5));
            p.line_to(pt(-0.3, 0.08));
            p.line_to(pt(0.02, 0.08));
            p.line_to(pt(-0.1, 0.5));
            p.line_to(pt(0.3, -0.08));
            p.line_to(pt(-0.02, -0.08));
            p.close_path();
            out.push(fill(p, solid(c)));
            return;
        }
        _ => {
            // "spark": four-point star.
            p.move_to(pt(0.0, -0.5));
            p.quad_to(pt(0.06, -0.06), pt(0.5, 0.0));
            p.quad_to(pt(0.06, 0.06), pt(0.0, 0.5));
            p.quad_to(pt(-0.06, 0.06), pt(-0.5, 0.0));
            p.quad_to(pt(-0.06, -0.06), pt(0.0, -0.5));
            out.push(fill(p, solid(c)));
            return;
        }
    }
    out.push(stroke_op(p, solid(c), (0.13 * s).max(1.5), [0.0, 1.0]));
}

/// Accent tile with an icon (cards, toasts).
fn icon_tile(out: &mut Vec<Op>, kind: &str, x: f32, y: f32, s: f32, c: Rgba) {
    out.push(rect(x, y, s, s, s * 0.28, Paint_lin(x, y, s, shade(c, 0.15), shade(c, -0.2))));
    out.push(rect(x, y, s, s * 0.5, s * 0.28, vgrad(y, y + s * 0.5, Rgba([1.0, 1.0, 1.0, 0.18]), Rgba([1.0, 1.0, 1.0, 0.0]))));
    icon(out, kind, x + s * 0.5, y + s * 0.5, s * 0.5, Rgba([1.0, 1.0, 1.0, 1.0]));
}

#[allow(non_snake_case)]
fn Paint_lin(x: f32, y: f32, s: f32, a: Rgba, b: Rgba) -> crate::dl::Paint {
    crate::dl::Paint::Linear { p0: [x, y], p1: [x + s, y + s], stops: vec![(0.0, a), (1.0, b)] }
}

// ------------------------------------------------------------------ card

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Card {
    pub title: String,
    pub body: String,
    /// Icon in an accent tile: "spark", "bolt", "check", "plus", "arrow", "info", "warning", "x"; empty for none.
    pub icon: Option<String>,
    pub accent: Option<Color>,
    /// Glowing accent border.
    pub glow: bool,
    pub delay: f32,
    pub out: Option<f32>,
    /// Title size (default scales with the card); the body is 0.62× it.
    pub font_size: Option<f32>,
}
impl Prepare for Card {}
impl Component for Card {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width.min(2000.0), ctx.height.min(2000.0));
        let (p, exit) = presence(t, self.delay, self.out, 0.35);
        if p <= 0.0 {
            return Vec::new();
        }
        let acc = self.accent.map(|c| c.0).unwrap_or(th.accent);
        let mut o = Vec::new();
        if self.glow {
            glow(&mut o, 0.0, 0.0, w, h, th.radius, 28.0, alpha(acc, 0.35));
        }
        surface(&mut o, th, 0.0, 0.0, w, h, th.radius, 1.0);
        if self.glow {
            o.push(Op::Stroke { path: rrect_path(0.5, 0.5, w - 1.0, h - 1.0, th.radius), paint: crate::dl::Paint::Linear { p0: [0.0, 0.0], p1: [w, h], stops: vec![(0.0, alpha(acc, 0.9)), (0.5, alpha(acc, 0.1)), (1.0, alpha(acc, 0.6))] }, stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        }
        let pad = (w.min(h) * 0.09).clamp(18.0, 56.0);
        let mut y = pad;
        if let Some(k) = self.icon.as_deref().filter(|k| !k.is_empty()) {
            let s = (pad * 1.5).min(48.0);
            icon_tile(&mut o, k, pad, y, s, acc);
            y += s + pad * 0.7;
        }
        let ts = sans(self.font_size.unwrap_or_else(|| (w * 0.065).min(h * 0.16).clamp(18.0, 60.0)), 600, th.text).tracking(-0.01);
        for l in wrap(ctx.assets, &self.title, &ts, w - 2.0 * pad) {
            y += ts.size * 1.0;
            draw(&mut o, ctx.assets, &l, &ts, pad, y, 0.0);
            y += ts.size * 0.22;
        }
        let bs = sans((ts.size * 0.62).max(14.0), 400, th.muted);
        y += bs.size * 0.5;
        for l in wrap(ctx.assets, &self.body, &bs, w - 2.0 * pad) {
            y += bs.size * 1.45;
            if y > h - pad * 0.5 {
                break;
            }
            draw(&mut o, ctx.assets, &l, &bs, pad, y, 0.0);
        }
        reveal(o, p, exit, w * 0.5, h, 28.0)
    }
}

// ------------------------------------------------------------------ button

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Button {
    pub label: String,
    /// "primary", "secondary", "ghost", "gradient".
    pub variant: String,
    /// Font size; the button sizes itself around the label.
    pub size: f32,
    pub color: Option<Color>,
    /// Times (s) at which it is pressed.
    pub press: Vec<f32>,
    /// Trailing arrow icon.
    pub arrow: bool,
    pub delay: f32,
    pub out: Option<f32>,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
}
impl Default for Button {
    fn default() -> Self {
        Button { label: "Button".into(), variant: "primary".into(), size: 26.0, color: None, press: Vec::new(), arrow: false, delay: 0.0, out: None, align: 0.5 }
    }
}
impl Prepare for Button {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if !["primary", "secondary", "ghost", "gradient"].contains(&self.variant.as_str()) {
            return Err(super::Error(format!("button variant must be primary, secondary, ghost or gradient, not {:?}", self.variant)));
        }
        Ok(())
    }
}
impl Component for Button {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (p, exit) = presence(t, self.delay, self.out, 0.3);
        if p <= 0.0 {
            return Vec::new();
        }
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let fs = self.size;
        let fg = match self.variant.as_str() {
            "primary" | "gradient" => Rgba([1.0, 1.0, 1.0, 1.0]),
            _ => th.text,
        };
        let st = sans(fs, 600, fg);
        let tw = ctx.assets.measure(&self.label, &st);
        let (px, py) = (fs * 1.2, fs * 0.72);
        let aw = if self.arrow { fs * 1.2 } else { 0.0 };
        let (w, h) = (tw + 2.0 * px + aw, fs + 2.0 * py);
        let r = h * 0.5;
        let mut o = Vec::new();
        match self.variant.as_str() {
            "primary" | "gradient" => {
                shadow(&mut o, 0.0, 0.0, w, h, r, 8.0, 18.0, 0.35);
                let paint = if self.variant == "gradient" {
                    crate::dl::Paint::Linear { p0: [0.0, 0.0], p1: [w, h], stops: vec![(0.0, acc), (1.0, th.pal(2))] }
                } else {
                    vgrad(0.0, h, shade(acc, 0.12), shade(acc, -0.08))
                };
                o.push(rect(0.0, 0.0, w, h, r, paint));
                o.push(Op::Stroke { path: rrect_path(0.5, 0.5, w - 1.0, h - 1.0, r), paint: vgrad(0.0, h, Rgba([1.0, 1.0, 1.0, 0.35]), Rgba([1.0, 1.0, 1.0, 0.0])), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
            }
            "secondary" => {
                o.push(rect(0.0, 0.0, w, h, r, solid(th.surface2)));
                border(&mut o, 0.0, 0.0, w, h, r, th.border);
            }
            _ => {}
        }
        draw(&mut o, ctx.assets, &self.label, &st, px, py + fs * 0.78, 0.0);
        if self.arrow {
            icon(&mut o, "arrow", px + tw + aw * 0.6, h * 0.5, fs * 0.8, fg);
        }
        // Press: quick squash and a light flash.
        let mut s = 1.0;
        for &pt in &self.press {
            let d = t - pt;
            if (0.0..0.4).contains(&d) {
                s = 1.0 - 0.06 * (d / 0.4 * std::f32::consts::PI).sin();
                o.push(rect(0.0, 0.0, w, h, r, solid(Rgba([1.0, 1.0, 1.0, 0.18 * (1.0 - d / 0.4)]))));
            }
        }
        transform_ops(&mut o, &scale_about(s, w * 0.5, h * 0.5));
        transform_ops(&mut o, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        let (cx, cy) = ((ctx.width - w) * self.align + w * 0.5, (ctx.height - h).max(0.0) * 0.5 + h * 0.5);
        reveal(o, p, exit, cx, cy, 14.0)
    }
}

// ------------------------------------------------------------------ toggle

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Toggle {
    /// Time it flips (s).
    pub at: f32,
    /// Initial state.
    pub on: bool,
    /// Height in pixels.
    pub size: f32,
    pub color: Option<Color>,
    pub label: Option<String>,
}
impl Default for Toggle {
    fn default() -> Self {
        Toggle { at: 0.5, on: false, size: 44.0, color: None, label: None }
    }
}
impl Prepare for Toggle {}
impl Component for Toggle {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let h = self.size;
        let w = h * 1.65;
        let k = spring(t - self.at, 1.2);
        let k = if self.on { 1.0 - k } else { k };
        let acc = self.color.map(|c| c.0).unwrap_or(th.positive);
        let off = if th.dark { Rgba([1.0, 1.0, 1.0, 0.16]) } else { Rgba([0.0, 0.0, 0.0, 0.12]) };
        let mut o = vec![rect(0.0, 0.0, w, h, h * 0.5, solid(mix(off, acc, clamp01(k))))];
        let pad = h * 0.09;
        let d = h - 2.0 * pad;
        // The knob stretches while moving.
        let moving = (k * (1.0 - k) * 4.0).clamp(0.0, 1.0);
        let kw = d * (1.0 + 0.25 * moving);
        let kx = pad + (w - 2.0 * pad - kw) * k;
        shadow(&mut o, kx, pad, kw, d, d * 0.5, 2.0, 4.0, 0.3);
        o.push(rect(kx, pad, kw, d, d * 0.5, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &sans(h * 0.5, 500, th.text), w + h * 0.45, h * 0.68, 0.0);
        }
        o
    }
}

// ------------------------------------------------------------------ badge

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Badge {
    pub label: String,
    pub color: Option<Color>,
    pub size: f32,
    /// Leading pulsing dot ("live" badges).
    pub dot: bool,
    /// Solid fill instead of a tint.
    pub solid: bool,
    pub delay: f32,
    pub out: Option<f32>,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
}
impl Default for Badge {
    fn default() -> Self {
        Badge { label: "New".into(), color: None, size: 20.0, dot: false, solid: false, delay: 0.0, out: None, align: 0.5 }
    }
}
impl Prepare for Badge {}
impl Component for Badge {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (p, exit) = presence(t, self.delay, self.out, 0.25);
        if p <= 0.0 {
            return Vec::new();
        }
        let c = self.color.map(|c| c.0).unwrap_or(th.accent);
        let fs = self.size;
        let st = sans(fs, 600, if self.solid { Rgba([1.0, 1.0, 1.0, 1.0]) } else { shade(c, if th.dark { 0.35 } else { -0.2 }) }).tracking(0.01);
        let tw = ctx.assets.measure(&self.label, &st);
        let dw = if self.dot { fs * 0.9 } else { 0.0 };
        let (px, h) = (fs * 0.7, fs * 1.75);
        let w = tw + 2.0 * px + dw;
        let mut o = vec![rect(0.0, 0.0, w, h, h * 0.5, solid(if self.solid { c } else { alpha(c, 0.16) }))];
        if !self.solid {
            border(&mut o, 0.0, 0.0, w, h, h * 0.5, alpha(c, 0.35));
        }
        if self.dot {
            let (cx, cy) = (px + fs * 0.25, h * 0.5);
            let ph = (t * 1.2).fract();
            o.push(circle(cx, cy, fs * 0.25 + ph * fs * 0.5, solid(alpha(c, 0.5 * (1.0 - ph)))));
            o.push(circle(cx, cy, fs * 0.25, solid(if self.solid { Rgba([1.0, 1.0, 1.0, 1.0]) } else { c })));
        }
        draw(&mut o, ctx.assets, &self.label, &st, px + dw, h * 0.5 + fs * 0.36, 0.0);
        transform_ops(&mut o, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        let (cx, cy) = ((ctx.width - w) * self.align + w * 0.5, (ctx.height - h).max(0.0) * 0.5 + h * 0.5);
        reveal(o, p, exit, cx, cy, 8.0)
    }
}

// ------------------------------------------------------------------ avatars

fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

fn draw_avatar(o: &mut Vec<Op>, ctx: &Ctx, name: &str, src: Option<&str>, x: f32, y: f32, s: f32, ring: Rgba) {
    let th = ctx.theme;
    let h = fnv(&[name.as_bytes()]) as usize;
    let (c1, c2) = (th.pal(h), th.pal(h / 7 + 1));
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.5, s * 0.5);
    o.push(circle(cx, cy, r + s * 0.045, solid(ring)));
    if let Some((id, iw, ih)) = src.and_then(|s| ctx.assets.media(s, 0.0)) {
        // Cover-fit the image in the circle.
        let k = s / iw.min(ih) as f32;
        let (dw, dh) = (iw as f32 * k, ih as f32 * k);
        o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((circle_path(cx, cy, r), IDENTITY)), cache_key: None });
        o.push(Op::Image { image: id, dst: [cx - dw * 0.5, cy - dh * 0.5, dw, dh], opacity: 1.0, transform: IDENTITY });
        o.push(Op::PopLayer);
    } else {
        o.push(circle(cx, cy, r, crate::dl::Paint::Linear { p0: [x, y], p1: [x + s, y + s], stops: vec![(0.0, shade(c1, 0.1)), (1.0, shade(c2, -0.15))] }));
        draw(o, ctx.assets, &initials(name), &sans(s * 0.38, 600, Rgba([1.0, 1.0, 1.0, 1.0])), cx, cy + s * 0.135, 0.5);
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Avatar {
    pub name: String,
    pub src: Option<String>,
    pub size: f32,
    /// "online", "away", "busy" or none.
    pub status: Option<String>,
    pub delay: f32,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
}
impl Default for Avatar {
    fn default() -> Self {
        Avatar { name: "Ada Lovelace".into(), src: None, size: 96.0, status: None, delay: 0.0, align: 0.5 }
    }
}
impl Prepare for Avatar {}
impl Component for Avatar {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let p = spring(t - self.delay, 1.1);
        if p <= 0.0 {
            return Vec::new();
        }
        let s = self.size;
        let mut o = Vec::new();
        draw_avatar(&mut o, ctx, &self.name, self.src.as_deref(), 0.0, 0.0, s, ctx.theme.background);
        if let Some(st) = &self.status {
            let c = match st.as_str() {
                "away" => parse_color("#ffb547").unwrap(),
                "busy" => ctx.theme.negative,
                _ => ctx.theme.positive,
            };
            let (cx, cy) = (s * 0.85, s * 0.85);
            o.push(circle(cx, cy, s * 0.13, solid(ctx.theme.background)));
            o.push(circle(cx, cy, s * 0.09, solid(c)));
        }
        transform_ops(&mut o, &scale_about(p, s * 0.5, s * 0.5));
        transform_ops(&mut o, &translate((ctx.width - s) * self.align, (ctx.height - s).max(0.0) * 0.5));
        with_opacity(o, clamp01(p * 2.0))
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AvatarStack {
    pub names: Vec<String>,
    pub srcs: Vec<String>,
    pub size: f32,
    /// Overlap fraction 0..0.6.
    pub overlap: f32,
    /// Show "+N" after this many.
    pub max: usize,
    pub delay: f32,
    pub stagger: f32,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
}
impl Default for AvatarStack {
    fn default() -> Self {
        AvatarStack { names: Vec::new(), srcs: Vec::new(), size: 72.0, overlap: 0.3, max: 5, delay: 0.0, stagger: 0.08, align: 0.5 }
    }
}
impl Prepare for AvatarStack {}
impl Component for AvatarStack {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let s = self.size;
        let step = s * (1.0 - self.overlap.clamp(0.0, 0.6));
        let n = self.names.len().min(self.max.max(1));
        let mut o = Vec::new();
        let total = n + usize::from(self.names.len() > n);
        for i in 0..total {
            let p = spring(t - self.delay - i as f32 * self.stagger, 1.2);
            if p <= 0.0 {
                continue;
            }
            let x = i as f32 * step;
            let mut a = Vec::new();
            if i < n {
                draw_avatar(&mut a, ctx, &self.names[i], self.srcs.get(i).map(|s| s.as_str()), x, 0.0, s, ctx.theme.background);
            } else {
                a.push(circle(x + s * 0.5, s * 0.5, s * 0.545, solid(ctx.theme.background)));
                a.push(circle(x + s * 0.5, s * 0.5, s * 0.5, solid(ctx.theme.surface2)));
                draw(&mut a, ctx.assets, &format!("+{}", self.names.len() - n), &sans(s * 0.34, 600, ctx.theme.text), x + s * 0.5, s * 0.62, 0.5);
            }
            transform_ops(&mut a, &scale_about(p, x + s * 0.5, s * 0.5));
            o.extend(with_opacity(a, clamp01(p * 2.0)));
        }
        let tw = total.saturating_sub(1) as f32 * step + s;
        transform_ops(&mut o, &translate((ctx.width - tw) * self.align, (ctx.height - s).max(0.0) * 0.5));
        o
    }
}

// ------------------------------------------------------------------ progress

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProgressBar {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub delay: f32,
    /// Bar thickness.
    pub thickness: f32,
    pub color: Option<Color>,
    pub label: Option<String>,
    /// Show the percentage on the right.
    pub percent: bool,
}
impl Default for ProgressBar {
    fn default() -> Self {
        ProgressBar { from: 0.0, to: 1.0, duration: 1.6, delay: 0.0, thickness: 12.0, color: None, label: None, percent: true }
    }
}
impl Prepare for ProgressBar {}
impl Component for ProgressBar {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let w = ctx.width.min(4000.0);
        let v = lerp(self.from, self.to, ease_in_out(seg(t, self.delay, self.duration))).clamp(0.0, 1.0);
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let h = self.thickness;
        let mut o = Vec::new();
        let ty = if self.label.is_some() || self.percent { h.max(14.0) * 2.2 } else { 0.0 };
        let st = sans(h.max(14.0) * 1.15, 500, th.text);
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &st, 0.0, ty - st.size * 0.6, 0.0);
        }
        if self.percent {
            draw(&mut o, ctx.assets, &format!("{}%", (v * 100.0).round()), &st.clone().color(th.muted).tabular(), w, ty - st.size * 0.6, 1.0);
        }
        o.push(rect(0.0, ty, w, h, h * 0.5, solid(if th.dark { Rgba([1.0, 1.0, 1.0, 0.08]) } else { Rgba([0.0, 0.0, 0.0, 0.07]) })));
        let fw = (w * v).max(if v > 0.0 { h } else { 0.0 });
        if fw > 0.0 {
            o.push(rect(0.0, ty, fw, h, h * 0.5, crate::dl::Paint::Linear { p0: [0.0, 0.0], p1: [fw, 0.0], stops: vec![(0.0, shade(acc, -0.1)), (1.0, shade(acc, 0.25))] }));
            // Glint at the leading edge.
            push_layer(&mut o, 0.8, Blend::Screen, vec![crate::dl::Filter::Blur(h)]);
            o.push(circle(fw - h * 0.5, ty + h * 0.5, h * 1.1, solid(alpha(shade(acc, 0.4), 0.8))));
            o.push(Op::PopLayer);
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration)
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProgressRing {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub delay: f32,
    pub size: f32,
    pub thickness: f32,
    pub color: Option<Color>,
    /// Centre text: percentage by default; set `label` for a caption below it.
    pub label: Option<String>,
    /// Show the percentage in the centre (false: an empty ring, e.g. behind a countdown).
    pub percent: bool,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
}
impl Default for ProgressRing {
    fn default() -> Self {
        ProgressRing { from: 0.0, to: 1.0, duration: 1.6, delay: 0.0, size: 240.0, thickness: 18.0, color: None, label: None, percent: true, align: 0.5 }
    }
}
impl Prepare for ProgressRing {}
impl Component for ProgressRing {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let v = lerp(self.from, self.to, ease_in_out(seg(t, self.delay, self.duration))).clamp(0.0, 1.0);
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let s = self.size;
        let r = (s - self.thickness) * 0.5;
        let (cx, cy) = (s * 0.5, s * 0.5);
        let ring = arc_path(cx, cy, r, 0.0, 1.0);
        let mut o = vec![stroke_op(ring.clone(), solid(alpha(th.text, 0.08)), self.thickness, [0.0, 1.0])];
        if v > 0.0 {
            let paint = crate::dl::Paint::Linear { p0: [0.0, 0.0], p1: [s, s], stops: vec![(0.0, shade(acc, 0.3)), (1.0, th.pal(1))] };
            o.push(stroke_op(ring, paint, self.thickness, [0.0, v]));
        }
        let st = TextStyle::new(Face::Display, s * 0.22, 600, th.text).tabular().tracking(-0.02);
        let dy = if self.label.is_some() { -s * 0.02 } else { s * 0.08 };
        if self.percent {
            draw(&mut o, ctx.assets, &format!("{}%", (v * 100.0).round()), &st, cx, cy + dy, 0.5);
        }
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &sans(s * 0.08, 500, th.muted), cx, cy + s * 0.13, 0.5);
        }
        transform_ops(&mut o, &translate((ctx.width - s) * self.align, (ctx.height - s).max(0.0) * 0.5));
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration)
    }
}

/// Clockwise arc from 12 o'clock, fractions `a..b` of a full turn.
pub fn arc_path(cx: f32, cy: f32, r: f32, a: f32, b: f32) -> Path {
    use kurbo::Shape;
    let start = (a * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2) as f64;
    let sweep = ((b - a) * std::f32::consts::TAU) as f64;
    let arc = kurbo::Arc::new((cx as f64, cy as f64), (r as f64, r as f64), start, sweep, 0.0);
    let mut p = Path::new();
    p.extend(arc.path_elements(0.05));
    p
}

// ------------------------------------------------------------------ toast

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Toast {
    pub title: String,
    pub body: String,
    /// "success", "info", "warning", "error".
    pub kind: String,
    pub delay: f32,
    /// How long it stays before leaving (s); 0 = stays.
    pub hold: f32,
    /// Slide in from "top", "bottom" or "right".
    pub from: String,
    pub width: f32,
}
impl Default for Toast {
    fn default() -> Self {
        Toast { title: String::new(), body: String::new(), kind: "success".into(), delay: 0.0, hold: 0.0, from: "top".into(), width: 520.0 }
    }
}
impl Prepare for Toast {}
impl Component for Toast {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let out = (self.hold > 0.0).then_some(self.delay + self.hold);
        let p = spring(t - self.delay, 0.9);
        let exit = out.map(|o| ease_in(seg(t, o, 0.35))).unwrap_or(0.0);
        if p <= 0.0 || exit >= 1.0 {
            return Vec::new();
        }
        let w = self.width;
        // Metrics scale with the width (designed at 400 px).
        let u = (w / 400.0).max(0.5);
        let pad = 18.0 * u;
        let ts = sans(18.0 * u, 600, th.text);
        let bs = sans(15.0 * u, 400, th.muted);
        let lines = wrap(ctx.assets, &self.body, &bs, w - pad * 2.0 - 46.0 * u);
        let h = pad * 2.0 + 22.0 * u + if self.body.is_empty() { 0.0 } else { lines.len() as f32 * 21.0 * u + 2.0 * u };
        let mut o = Vec::new();
        surface(&mut o, th, 0.0, 0.0, w, h, 16.0 * u, 1.0);
        let c = match self.kind.as_str() {
            "error" => th.negative,
            "warning" => parse_color("#ffb547").unwrap(),
            "info" => th.pal(5),
            _ => th.positive,
        };
        o.push(circle(pad + 16.0 * u, pad + 15.0 * u, 16.0 * u, solid(alpha(c, 0.18))));
        icon(&mut o, &self.kind, pad + 16.0 * u, pad + 15.0 * u, 17.0 * u, c);
        draw(&mut o, ctx.assets, &self.title, &ts, pad + 46.0 * u, pad + 20.0 * u, 0.0);
        for (i, l) in lines.iter().enumerate() {
            draw(&mut o, ctx.assets, l, &bs, pad + 46.0 * u, pad + (44.0 + i as f32 * 21.0) * u, 0.0);
        }
        let k = 1.0 - p + exit;
        let (dx, dy) = match self.from.as_str() {
            "bottom" => (0.0, k * (h + 40.0)),
            "right" => (k * (w + 40.0), 0.0),
            _ => (0.0, -k * (h + 40.0)),
        };
        transform_ops(&mut o, &translate(dx, dy));
        with_opacity(o, clamp01(p * 2.0) * (1.0 - exit))
    }
    fn duration(&self) -> Option<f32> {
        (self.hold > 0.0).then_some(self.delay + self.hold + 0.35)
    }
}

// ------------------------------------------------------------------ cursor

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cursor {
    /// Waypoints `[x, y]` in box coordinates.
    pub path: Vec<[f32; 2]>,
    /// Arrival time of each waypoint; default spreads them 0.8 s apart.
    pub times: Vec<f32>,
    /// Click times (s): squash and a ripple.
    pub clicks: Vec<f32>,
    pub size: f32,
    /// "arrow" (default) or "hand".
    pub style: String,
    pub ripple: Option<Color>,
}
impl Default for Cursor {
    fn default() -> Self {
        Cursor { path: vec![[0.0, 0.0]], times: Vec::new(), clicks: Vec::new(), size: 36.0, style: "arrow".into(), ripple: None }
    }
}
impl Prepare for Cursor {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if self.path.is_empty() {
            return Err(super::Error("cursor needs at least one point in `path`".into()));
        }
        if self.times.is_empty() {
            self.times = (0..self.path.len()).map(|i| i as f32 * 0.8).collect();
        }
        if self.times.len() != self.path.len() {
            return Err(super::Error(format!("cursor: {} times for {} points", self.times.len(), self.path.len())));
        }
        Ok(())
    }
}
impl Cursor {
    fn pos(&self, t: f32) -> (f32, f32) {
        let n = self.path.len();
        if t <= self.times[0] || n == 1 {
            return (self.path[0][0], self.path[0][1]);
        }
        for i in 1..n {
            if t < self.times[i] {
                let k = ease_in_out((t - self.times[i - 1]) / (self.times[i] - self.times[i - 1]).max(1e-3));
                let (a, b) = (self.path[i - 1], self.path[i]);
                // Slight arc, like a real hand.
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let bow = (k * (1.0 - k)) * 0.18;
                return (a[0] + dx * k - dy * bow, a[1] + dy * k + dx * bow);
            }
        }
        let l = self.path[n - 1];
        (l[0], l[1])
    }
}
impl Component for Cursor {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (x, y) = self.pos(t);
        let mut o = Vec::new();
        let rc = self.ripple.map(|c| c.0).unwrap_or(ctx.theme.accent);
        let mut squash = 1.0;
        for &c in &self.clicks {
            let d = t - c;
            if (0.0..0.6).contains(&d) {
                let k = ease_out(d / 0.6);
                let (rx, ry) = self.pos(c);
                o.push(circle(rx, ry, 6.0 + k * self.size * 1.6, solid(alpha(rc, 0.35 * (1.0 - k)))));
                o.push(Op::Stroke { path: circle_path(rx, ry, 6.0 + k * self.size * 1.6), paint: solid(alpha(rc, 0.8 * (1.0 - k))), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
            }
            if (0.0..0.25).contains(&d) {
                squash = 1.0 - 0.15 * (d / 0.25 * std::f32::consts::PI).sin();
            }
        }
        let s = self.size / 24.0;
        let mut p = Path::new();
        let pt = |px: f32, py: f32| ((x + px * s) as f64, (y + py * s) as f64);
        if self.style == "hand" {
            // Simplified pointing hand.
            p.move_to(pt(6.0, 1.0));
            p.line_to(pt(6.0, 11.0));
            p.line_to(pt(3.0, 9.5));
            p.line_to(pt(1.5, 11.5));
            p.line_to(pt(7.0, 20.0));
            p.line_to(pt(17.0, 20.0));
            p.line_to(pt(18.5, 12.0));
            p.line_to(pt(9.5, 10.0));
            p.line_to(pt(9.5, 1.0));
            p.quad_to(pt(7.75, -0.6), pt(6.0, 1.0));
        } else {
            p.move_to(pt(0.0, 0.0));
            p.line_to(pt(0.0, 17.0));
            p.line_to(pt(4.2, 13.2));
            p.line_to(pt(7.0, 19.6));
            p.line_to(pt(9.8, 18.4));
            p.line_to(pt(7.1, 12.2));
            p.line_to(pt(12.6, 12.2));
        }
        p.close_path();
        let mut c = Vec::new();
        push_layer(&mut c, 1.0, Blend::Normal, vec![crate::dl::Filter::Blur(3.0 * s)]);
        let mut sh = vec![fill(p.clone(), solid(Rgba([0.0, 0.0, 0.0, 0.45])))];
        transform_ops(&mut sh, &translate(1.0 * s, 2.0 * s));
        c.extend(sh);
        c.push(Op::PopLayer);
        c.push(Op::Stroke { path: p.clone(), paint: solid(Rgba([0.0, 0.0, 0.0, 1.0])), stroke: Stroke2(1.6 * s), transform: IDENTITY });
        c.push(fill(p, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
        transform_ops(&mut c, &scale_about(squash, x, y));
        o.extend(c);
        o
    }
    fn duration(&self) -> Option<f32> {
        let a = self.times.last().copied().unwrap_or(0.0);
        let b = self.clicks.iter().fold(0.0f32, |m, c| m.max(*c + 0.6));
        Some(a.max(b))
    }
}

#[allow(non_snake_case)]
fn Stroke2(w: f32) -> crate::dl::Stroke {
    crate::dl::Stroke { width: w, cap: 1, join: 1, miter_limit: 4.0, dash: vec![], dash_offset: 0.0, trim: [0.0, 1.0] }
}

// ------------------------------------------------------------------ tooltip

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tooltip {
    pub text: String,
    /// Anchor point (the arrow tip), box coordinates.
    pub at: [f32; 2],
    /// "top" (bubble above the anchor) or "bottom".
    pub side: String,
    pub size: f32,
    pub delay: f32,
    pub out: Option<f32>,
}
impl Default for Tooltip {
    fn default() -> Self {
        Tooltip { text: "Tooltip".into(), at: [160.0, 80.0], side: "top".into(), size: 22.0, delay: 0.0, out: None }
    }
}
impl Prepare for Tooltip {}
impl Component for Tooltip {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (p, exit) = presence(t, self.delay, self.out, 0.2);
        if p <= 0.0 {
            return Vec::new();
        }
        let bg = if th.dark { Rgba([0.96, 0.96, 0.98, 1.0]) } else { parse_color("#16171d").unwrap() };
        let fg = if th.dark { parse_color("#111217").unwrap() } else { Rgba([1.0, 1.0, 1.0, 1.0]) };
        let st = sans(self.size, 500, fg);
        let tw = ctx.assets.measure(&self.text, &st);
        let (px, py) = (self.size * 0.75, self.size * 0.55);
        let (w, h) = (tw + 2.0 * px, self.size + 2.0 * py);
        let [ax, ay] = self.at;
        let up = self.side != "bottom";
        let a = 7.0;
        let y0 = if up { ay - a - h } else { ay + a };
        let x0 = ax - w * 0.5;
        let mut o = Vec::new();
        shadow(&mut o, x0, y0, w, h, 8.0, 6.0, 14.0, 0.35);
        o.push(rect(x0, y0, w, h, 8.0, solid(bg)));
        let mut tri = Path::new();
        let by = if up { y0 + h - 0.5 } else { y0 + 0.5 };
        tri.move_to(((ax - a) as f64, by as f64));
        tri.line_to((ax as f64, ay as f64));
        tri.line_to(((ax + a) as f64, by as f64));
        tri.close_path();
        o.push(fill(tri, solid(bg)));
        draw(&mut o, ctx.assets, &self.text, &st, x0 + px, y0 + py + self.size * 0.78, 0.0);
        transform_ops(&mut o, &scale_about(lerp(0.85, 1.0, p.min(1.05)), ax, ay));
        with_opacity(o, clamp01(p * 2.0) * (1.0 - exit))
    }
}

// ------------------------------------------------------------------ callout & arrow

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Callout {
    /// The point being annotated.
    pub from: [f32; 2],
    /// Where the label sits.
    pub to: [f32; 2],
    pub text: String,
    pub color: Option<Color>,
    pub size: f32,
    pub delay: f32,
    pub out: Option<f32>,
}
impl Default for Callout {
    fn default() -> Self {
        Callout { from: [100.0, 100.0], to: [300.0, 60.0], text: String::new(), color: None, size: 26.0, delay: 0.0, out: None }
    }
}
impl Prepare for Callout {}
impl Component for Callout {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let c = self.color.map(|c| c.0).unwrap_or(th.accent);
        let t = t - self.delay;
        if t <= 0.0 {
            return Vec::new();
        }
        let exit = self.out.map(|o| ease_in(seg(t + self.delay, o, 0.3))).unwrap_or(0.0);
        let [fx, fy] = self.from;
        let [tx, ty] = self.to;
        let mut o = Vec::new();
        let dot = spring(t, 1.2);
        let ph = (t * 0.9).fract();
        o.push(circle(fx, fy, 6.0 + ph * 18.0, solid(alpha(c, 0.4 * (1.0 - ph)))));
        o.push(circle(fx, fy, 6.0 * dot, solid(c)));
        o.push(circle(fx, fy, 2.5 * dot, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
        // Elbow leader: diagonal then horizontal.
        let mut p = Path::new();
        let ex = tx - (tx - fx).signum() * 8.0;
        let elbow = (fx + (ty - fy).abs() * (tx - fx).signum() * 0.6, ty);
        p.move_to((fx as f64, fy as f64));
        p.line_to((elbow.0 as f64, elbow.1 as f64));
        p.line_to((ex as f64, ty as f64));
        let k = ease_in_out(seg(t, 0.15, 0.5));
        o.push(stroke_op(p, solid(alpha(c, 0.9)), 1.5, [0.0, k]));
        let lp = ease_out(seg(t, 0.5, 0.45));
        if lp > 0.0 {
            let st = sans(self.size, 600, th.text);
            let tw = ctx.assets.measure(&self.text, &st);
            let right = tx >= fx;
            let (pad, h) = (self.size * 0.6, self.size * 1.9);
            let x0 = if right { tx } else { tx - tw - 2.0 * pad };
            let mut l = Vec::new();
            l.push(rect(x0, ty - h * 0.5, tw + 2.0 * pad, h, h * 0.5, solid(alpha(th.surface2, 0.92))));
            border(&mut l, x0, ty - h * 0.5, tw + 2.0 * pad, h, h * 0.5, alpha(c, 0.5));
            draw(&mut l, ctx.assets, &self.text, &st, x0 + pad, ty + self.size * 0.36, 0.0);
            transform_ops(&mut l, &translate(if right { -12.0 } else { 12.0 } * (1.0 - lp), 0.0));
            o.extend(with_opacity(l, lp));
        }
        with_opacity(o, 1.0 - exit)
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 0.95)
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Arrow {
    pub from: [f32; 2],
    pub to: [f32; 2],
    /// Curvature (-1..1); 0 is straight.
    pub bend: f32,
    pub color: Option<Color>,
    pub width: f32,
    pub delay: f32,
    pub duration: f32,
    /// Dashed line.
    pub dashed: bool,
}
impl Default for Arrow {
    fn default() -> Self {
        Arrow { from: [100.0, 300.0], to: [400.0, 120.0], bend: 0.35, color: None, width: 3.0, delay: 0.0, duration: 0.7, dashed: false }
    }
}
impl Prepare for Arrow {}
impl Component for Arrow {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let k = ease_in_out(seg(t, self.delay, self.duration));
        if k <= 0.0 {
            return Vec::new();
        }
        let c = self.color.map(|c| c.0).unwrap_or(ctx.theme.text);
        let [ax, ay] = self.from;
        let [bx, by] = self.to;
        let (dx, dy) = (bx - ax, by - ay);
        let (mx, my) = ((ax + bx) * 0.5 - dy * self.bend * 0.5, (ay + by) * 0.5 + dx * self.bend * 0.5);
        let mut p = Path::new();
        p.move_to((ax as f64, ay as f64));
        p.quad_to((mx as f64, my as f64), (bx as f64, by as f64));
        let mut st = line(self.width, [0.0, k]);
        if self.dashed {
            st.dash = vec![self.width * 3.0, self.width * 2.5];
        }
        let mut o = vec![Op::Stroke { path: p, paint: solid(c), stroke: st, transform: IDENTITY }];
        // Head along the tangent at the current tip.
        let tip = |u: f32| {
            let v = 1.0 - u;
            (v * v * ax + 2.0 * v * u * mx + u * u * bx, v * v * ay + 2.0 * v * u * my + u * u * by)
        };
        let u = k; // close enough to arclength for gentle curves
        let (hx, hy) = tip(u);
        let (qx, qy) = tip((u - 0.02).max(0.0));
        let ang = (hy - qy).atan2(hx - qx);
        let hl = self.width * 4.0;
        let mut h = Path::new();
        for s in [-1.0f32, 1.0] {
            let a = ang + std::f32::consts::PI + s * 0.5;
            h.move_to(((hx + a.cos() * hl) as f64, (hy + a.sin() * hl) as f64));
            h.line_to((hx as f64, hy as f64));
        }
        o.push(stroke_op(h, solid(c), self.width, [0.0, 1.0]));
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.duration)
    }
}

// ------------------------------------------------------------------ lower third

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LowerThird {
    pub title: String,
    pub subtitle: String,
    pub accent: Option<Color>,
    pub size: f32,
    pub delay: f32,
    /// Exit time (s); omitted = stays.
    pub out: Option<f32>,
    /// "left" (default) or "center".
    pub align: String,
    /// Margin from the bottom-left of the box.
    pub margin: f32,
}
impl Default for LowerThird {
    fn default() -> Self {
        LowerThird { title: String::new(), subtitle: String::new(), accent: None, size: 44.0, delay: 0.0, out: None, align: "left".into(), margin: 72.0 }
    }
}
impl Prepare for LowerThird {}
impl Component for LowerThird {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let acc = self.accent.map(|c| c.0).unwrap_or(th.accent);
        let t = t - self.delay;
        if t <= 0.0 {
            return Vec::new();
        }
        let exit = self.out.map(|o| ease_in_out(seg(t + self.delay, o, 0.45))).unwrap_or(0.0);
        if exit >= 1.0 {
            return Vec::new();
        }
        let ts = sans(self.size, 700, th.text).tracking(-0.015);
        let ss = sans(self.size * 0.55, 500, th.muted);
        let tw = ctx.assets.measure(&self.title, &ts).max(ctx.assets.measure(&self.subtitle, &ss));
        let pad = self.size * 0.6;
        let (w, h) = (tw + pad * 2.0 + 10.0, self.size * 2.6);
        let x0 = if self.align == "center" { (ctx.width - w) * 0.5 } else { self.margin };
        let y0 = ctx.height - self.margin - h;
        let bar = ease_out(seg(t, 0.0, 0.45));
        let panel = ease_in_out(seg(t, 0.15, 0.55)) * (1.0 - exit);
        let mut o = Vec::new();
        // Accent bar grows vertically, then the panel wipes out of it.
        let bh = h * bar * (1.0 - exit);
        o.push(rect(x0, y0 + (h - bh) * 0.5, 5.0, bh, 2.5, vgrad(y0, y0 + h, shade(acc, 0.2), acc)));
        if panel > 0.0 {
            let pw = w * panel;
            push_clip(&mut o, x0 + 9.0, y0, pw, h, 0.0);
            o.push(rect(x0 + 9.0, y0, w, h, 10.0, solid(alpha(th.surface, 0.86))));
            border(&mut o, x0 + 9.0, y0, w, h, 10.0, th.border);
            let slide = (1.0 - ease_out(seg(t, 0.3, 0.6))) * -30.0;
            draw(&mut o, ctx.assets, &self.title, &ts, x0 + 9.0 + pad + slide, y0 + h * 0.48, 0.0);
            let s2 = (1.0 - ease_out(seg(t, 0.4, 0.6))) * -30.0;
            draw(&mut o, ctx.assets, &self.subtitle, &ss, x0 + 9.0 + pad + s2, y0 + h * 0.48 + self.size * 0.95, 0.0);
            o.push(Op::PopLayer);
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 1.0)
    }
}

// ------------------------------------------------------------------ title card

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TitleCard {
    pub title: String,
    pub eyebrow: String,
    pub subtitle: String,
    /// Title size; default scales with the box.
    pub size: Option<f32>,
    /// Gradient title fill (two colours).
    pub gradient: Option<[Color; 2]>,
    /// "word" (default), "line" or "fade".
    pub reveal: String,
    pub stagger: f32,
    pub delay: f32,
    pub out: Option<f32>,
    /// 0 left, 0.5 centre (default), 1 right.
    pub align: f32,
}
impl Default for TitleCard {
    fn default() -> Self {
        TitleCard { title: String::new(), eyebrow: String::new(), subtitle: String::new(), size: None, gradient: None, reveal: "word".into(), stagger: 0.07, delay: 0.0, out: None, align: 0.5 }
    }
}
impl Prepare for TitleCard {}
impl Component for TitleCard {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let size = self.size.unwrap_or((w * 0.062).clamp(28.0, 132.0));
        let exit = self.out.map(|o| ease_in(seg(t, o, 0.4))).unwrap_or(0.0);
        let ts = TextStyle::new(Face::Display, size, 700, th.text).tracking(-0.035);
        let lines = wrap(ctx.assets, &self.title, &ts, w * 0.86);
        let es = sans(size * 0.2, 600, th.accent).tracking(0.14);
        let ss = sans(size * 0.3, 400, th.muted);
        let sub = wrap(ctx.assets, &self.subtitle, &ss, w * 0.7);
        let lh = size * 1.06;
        let block = (if self.eyebrow.is_empty() { 0.0 } else { es.size * 2.6 }) + lines.len() as f32 * lh + if sub.is_empty() { 0.0 } else { size * 0.35 + sub.len() as f32 * ss.size * 1.45 };
        let mut y = (h - block) * 0.5;
        let ax = w * (0.07 + 0.86 * self.align);
        let mut o = Vec::new();
        let mut k = 0usize;
        let t0 = t - self.delay;
        let piece = |o: &mut Vec<Op>, ops: Vec<Op>, start: f32, rise: f32| {
            let p = ease_out(seg(t0, start, 0.7));
            if p <= 0.0 {
                return;
            }
            let mut ops = ops;
            transform_ops(&mut ops, &translate(0.0, (1.0 - p) * rise));
            o.extend(with_opacity(ops, p * (1.0 - exit)));
        };
        if !self.eyebrow.is_empty() {
            let mut e = Vec::new();
            draw(&mut e, ctx.assets, &self.eyebrow.to_uppercase(), &es, ax, y + es.size, self.align);
            piece(&mut o, e, 0.0, 10.0);
            y += es.size * 2.6;
            k += 1;
        }
        let gradient = self.gradient.map(|[a, b]| (a.0, b.0));
        for l in &lines {
            y += size * 0.92;
            let lw = ctx.assets.measure(l, &ts);
            let mut x = ax - lw * self.align;
            let words: Vec<&str> = if self.reveal == "word" { l.split(' ').collect() } else { vec![l.as_str()] };
            for wd in words {
                let mut e = Vec::new();
                let wst = match gradient {
                    Some((a, b)) => ts.clone().gradient(a, b, [ax - lw * self.align, 0.0], [ax - lw * self.align + lw, 0.0]),
                    None => ts.clone(),
                };
                let adv = draw(&mut e, ctx.assets, wd, &wst, x, y, 0.0);
                let start = if self.reveal == "fade" { 0.0 } else { k as f32 * self.stagger };
                piece(&mut o, e, start, size * 0.35);
                x += adv + ctx.assets.measure(" ", &ts);
                k += 1;
            }
            y += lh - size * 0.92;
        }
        y += size * 0.35;
        for l in &sub {
            y += ss.size * 1.45;
            let mut e = Vec::new();
            draw(&mut e, ctx.assets, l, &ss, ax, y - ss.size * 0.35, self.align);
            piece(&mut o, e, k as f32 * self.stagger + 0.15, 12.0);
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        let words = self.title.split_whitespace().count() + 2;
        Some(self.delay + words as f32 * self.stagger + 0.85)
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
        let items = [
            ("card", json!({"x": 40, "y": 40, "width": 360, "height": 220, "title": "Instant previews", "body": "Render any frame in under 100 ms, without a browser.", "icon": "bolt", "glow": true})),
            ("button", json!({"x": 440, "y": 40, "width": 200, "height": 60, "align": 0, "label": "Get started", "arrow": true})),
            ("button", json!({"x": 440, "y": 110, "width": 200, "height": 60, "align": 0, "label": "Docs", "variant": "secondary"})),
            ("toggle", json!({"x": 640, "y": 60, "at": 0.2, "label": "Dark mode"})),
            ("badge", json!({"x": 440, "y": 185, "width": 90, "height": 36, "align": 0, "label": "Live", "dot": true, "color": "#3ddc97"})),
            ("badge", json!({"x": 540, "y": 185, "width": 90, "height": 36, "align": 0, "label": "v2.0", "solid": true})),
            ("avatar", json!({"x": 640, "y": 120, "name": "Grace Hopper", "status": "online"})),
            ("avatar-stack", json!({"x": 740, "y": 130, "names": ["Ada L", "Grace H", "Alan T", "Linus T", "Ken T", "Dennis R"], "max": 4})),
            ("progress-bar", json!({"x": 40, "y": 300, "width": 360, "to": 0.72, "label": "Rendering"})),
            ("progress-ring", json!({"x": 440, "y": 260, "to": 0.72, "size": 150})),
            ("toast", json!({"x": 620, "y": 260, "title": "Deployed to production", "body": "anikit.dev is live in 3 regions"})),
            ("tooltip", json!({"x": 40, "y": 380, "text": "Copy to clipboard", "at": [120, 60]})),
            ("callout", json!({"x": 40, "y": 380, "from": [260, 120], "to": [400, 60], "text": "New API"})),
            ("arrow", json!({"x": 40, "y": 380, "from": [460, 140], "to": [620, 40]})),
            ("cursor", json!({"x": 0, "y": 0, "path": [[200, 200], [520, 82]], "clicks": [1.1]})),
            ("lower-third", json!({"title": "Ada Lovelace", "subtitle": "Founder, Analytical Engines"})),
        ];
        let mut ops = Vec::new();
        for (k, p) in &items {
            let c = build(k, p).unwrap();
            ops.extend(c.eval(1.2, 960.0, 540.0, 30.0, &th, a));
        }
        preview::validate(&ops).unwrap();
        preview::snapshot("ui-kit", 960, 540, th.background, ops, a);
        let c = build("title-card", &json!({"eyebrow": "Introducing", "title": "Video for agents, rendered natively", "subtitle": "One JSON spec in. A finished film out.", "gradient": ["#ffffff", "#9c8cff"]})).unwrap();
        for (i, t) in [0.2f32, 0.5, 2.0].iter().enumerate() {
            let ops = c.eval(*t, 960.0, 540.0, 30.0, &th, a);
            preview::validate(&ops).unwrap();
            preview::snapshot(&format!("ui-title-{i}"), 960, 540, th.background, ops, a);
        }
    }
}
