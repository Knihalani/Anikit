//! UI kit: surfaces (cards, buttons, toggles, badges, avatars, progress,
//! toasts), pointers and overlays (cursor, tooltip, callout, arrow) and
//! framing (lower third, title card).
//!
//! Shared timing keys: `delay` (entrance start, s) and, where it makes sense,
//! `out` (exit start, s; omitted = stays).
//!
//! Shared look keys (see [`super::polish`]): `surface` (`solid`, `glass`,
//! `outline`, `flat`, `paper`), `motion` (`smooth`, `snappy`, `soft`,
//! `bouncy`), `radius` (px; default from the theme) and `face` (`sans`,
//! `display`, `serif`, `mono`). Colours default to the theme.

use super::polish::{self, ease_in_out_quint, ease_out_expo, face_style, finish, headline, masked_rise, spring_z, ui, Motion, Surface};
use super::style::*;
use super::text::{draw, wrap, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Prepare};
use crate::dl::{Blend, Op, Paint, Path, Rgba, IDENTITY};
use serde::Deserialize;

/// Entrance/exit envelope: 0 → 1 in the motion's character, ease out.
fn presence(m: Motion, t: f32, delay: f32, out: Option<f32>, out_dur: f32) -> (f32, f32) {
    let enter = m.enter(t - delay);
    let exit = out.map(|o| ease_in(seg(t, o, out_dur))).unwrap_or(0.0);
    (enter, exit)
}

/// Rise-and-fade entrance about a pivot.
fn reveal(mut ops: Vec<Op>, p: f32, exit: f32, px: f32, py: f32, rise: f32) -> Vec<Op> {
    let s = lerp(0.95, 1.0, p) * lerp(1.0, 0.97, exit);
    let dy = (1.0 - p) * rise - exit * rise * 0.5;
    let m = scale_about(s, px, py);
    let m = crate::dl::mul(&translate(0.0, dy), &m);
    transform_ops(&mut ops, &m);
    with_opacity(ops, clamp01(p * 1.6) * (1.0 - exit))
}

/// Fade + small rise for a child element (staggered content).
fn sub_rise(ops: Vec<Op>, p: f32, dy: f32) -> Vec<Op> {
    if p <= 0.0 {
        return Vec::new();
    }
    let mut ops = ops;
    transform_ops(&mut ops, &translate(0.0, (1.0 - p) * dy));
    with_opacity(ops, clamp01(p * 1.3))
}

fn white() -> Rgba {
    Rgba([1.0, 1.0, 1.0, 1.0])
}

/// Icon glyphs drawn as vectors so they work without fonts.
fn icon(out: &mut Vec<Op>, kind: &str, cx: f32, cy: f32, s: f32, c: Rgba) {
    let mut p = Path::new();
    let pt = |x: f32, y: f32| ((cx + x * s) as f64, (cy + y * s) as f64);
    match kind {
        "success" | "check" => {
            p.move_to(pt(-0.38, 0.02));
            p.line_to(pt(-0.12, 0.28));
            p.line_to(pt(0.4, -0.26));
        }
        "error" | "x" => {
            p.move_to(pt(-0.27, -0.27));
            p.line_to(pt(0.27, 0.27));
            p.move_to(pt(0.27, -0.27));
            p.line_to(pt(-0.27, 0.27));
        }
        "warning" | "info" => {
            let (a, b) = if kind == "info" { (-0.05, 0.36) } else { (-0.36, 0.1) };
            p.move_to(pt(0.0, a));
            p.line_to(pt(0.0, b));
            out.push(circle(cx, cy + if kind == "info" { -0.33 } else { 0.36 } * s, 0.085 * s, solid(c)));
        }
        "plus" => {
            p.move_to(pt(-0.34, 0.0));
            p.line_to(pt(0.34, 0.0));
            p.move_to(pt(0.0, -0.34));
            p.line_to(pt(0.0, 0.34));
        }
        "arrow" => {
            p.move_to(pt(-0.36, 0.0));
            p.line_to(pt(0.36, 0.0));
            p.move_to(pt(0.06, -0.3));
            p.line_to(pt(0.36, 0.0));
            p.line_to(pt(0.06, 0.3));
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
    out.push(stroke_op(p, solid(c), (0.12 * s).max(1.5), [0.0, 1.0]));
}

/// Accent tile with an icon (cards).
fn icon_tile(out: &mut Vec<Op>, kind: &str, x: f32, y: f32, s: f32, c: Rgba, surface: Surface) {
    let r = s * 0.28;
    match surface {
        Surface::Outline => {
            out.push(Op::Stroke { path: rrect_path(x + 0.75, y + 0.75, s - 1.5, s - 1.5, r), paint: solid(c), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
            icon(out, kind, x + s * 0.5, y + s * 0.5, s * 0.5, c);
        }
        Surface::Flat | Surface::Paper => {
            out.push(rect(x, y, s, s, r, solid(c)));
            icon(out, kind, x + s * 0.5, y + s * 0.5, s * 0.5, on_color(&[c]));
        }
        _ => {
            let (a, b) = (shade(c, 0.18), shade(c, -0.22));
            polish::tinted_shadow(out, x, y, s, s, r, s * 0.12, s * 0.25, alpha(c, 0.35));
            out.push(rect(x, y, s, s, r, Paint::Linear { p0: [x, y], p1: [x + s, y + s], stops: vec![(0.0, a), (1.0, b)] }));
            polish::bevel(out, x, y, s, s, r, 1.6);
            icon(out, kind, x + s * 0.5, y + s * 0.5, s * 0.5, on_color(&[a, b]));
        }
    }
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
    /// Glowing accent rim (off by default).
    pub glow: bool,
    pub delay: f32,
    pub out: Option<f32>,
    /// Title size (default scales with the card); the body is 0.6× it.
    pub font_size: Option<f32>,
    /// Card fill (default: theme surface).
    pub color: Option<Color>,
    /// Surface finish: solid, glass, outline, flat, paper.
    pub surface: Surface,
    pub motion: Motion,
    pub radius: Option<f32>,
    /// Title face: sans (default), display, serif, mono.
    pub face: Option<Face>,
    /// Small label above the title.
    pub eyebrow: String,
}
impl Prepare for Card {}
impl Component for Card {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width.min(2000.0), ctx.height.min(2000.0));
        let m = self.motion;
        let (p, exit) = presence(m, t, self.delay, self.out, 0.35);
        if p <= 0.0 {
            return Vec::new();
        }
        let acc = self.accent.map(|c| c.0).unwrap_or(th.accent);
        let r = self.radius.unwrap_or(th.radius);
        let fillc = self.color.map(|c| c.0);
        let mut o = Vec::new();
        if self.glow {
            glow(&mut o, 0.0, 0.0, w, h, r, 34.0, alpha(acc, 0.3));
        }
        finish(&mut o, th, self.surface, 0.0, 0.0, w, h, r, 1.2, fillc);
        if self.glow {
            // Rim light that slowly travels round the card.
            let a = (t - self.delay) * 0.6;
            let (c, s) = (a.cos() * w * 0.6, a.sin() * h * 0.6);
            o.push(Op::Stroke { path: rrect_path(0.75, 0.75, w - 1.5, h - 1.5, r), paint: Paint::Linear { p0: [w * 0.5 - c, h * 0.5 - s], p1: [w * 0.5 + c, h * 0.5 + s], stops: vec![(0.0, alpha(acc, 0.95)), (0.45, alpha(acc, 0.08)), (1.0, alpha(acc, 0.5))] }, stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        }
        let ink = match (self.surface, fillc) {
            (Surface::Flat | Surface::Paper | Surface::Solid, Some(f)) => on_color(&[f]),
            _ => th.text,
        };
        let muted = if fillc.is_some() && self.surface != Surface::Glass && self.surface != Surface::Outline { alpha(ink, 0.68) } else { th.muted };
        let pad = (w.min(h) * 0.1).clamp(18.0, 56.0);
        let mut y = pad;
        let lag = 0.07 * m.pace();
        let mut step = 1.0;
        let kid = |i: f32| m.enter(t - self.delay - i * lag);
        if let Some(k) = self.icon.as_deref().filter(|k| !k.is_empty()) {
            let s = (pad * 1.35).min(52.0);
            let mut e = Vec::new();
            icon_tile(&mut e, k, pad, y, s, acc, self.surface);
            o.extend(sub_rise(e, kid(step), 10.0 * m.travel()));
            step += 1.0;
            y += s + pad * 0.6;
        }
        let tsz = self.font_size.unwrap_or_else(|| (w * 0.062).min(h * 0.15).clamp(18.0, 60.0));
        if !self.eyebrow.is_empty() {
            let es = polish::eyebrow((tsz * 0.36).max(12.0), 600, acc);
            let mut e = Vec::new();
            y += es.size;
            draw(&mut e, ctx.assets, &self.eyebrow.to_uppercase(), &es, pad, y, 0.0);
            o.extend(sub_rise(e, kid(step), 8.0 * m.travel()));
            y += es.size * 0.9;
        }
        let ts = face_style(self.face, tsz, 600, ink);
        let mut e = Vec::new();
        for l in wrap(ctx.assets, &self.title, &ts, w - 2.0 * pad) {
            y += ts.size * 1.0;
            draw(&mut e, ctx.assets, &l, &ts, pad, y, 0.0);
            y += ts.size * 0.2;
        }
        o.extend(sub_rise(e, kid(step), 12.0 * m.travel()));
        step += 1.0;
        let bs = ui((tsz * 0.6).max(14.0), 400, muted);
        y += bs.size * 0.45;
        let mut e = Vec::new();
        for l in wrap(ctx.assets, &self.body, &bs, w - 2.0 * pad) {
            y += bs.size * 1.5;
            if y > h - pad * 0.5 {
                break;
            }
            draw(&mut e, ctx.assets, &l, &bs, pad, y, 0.0);
        }
        o.extend(sub_rise(e, kid(step), 12.0 * m.travel()));
        reveal(o, p, exit, w * 0.5, h, 30.0 * m.travel())
    }
}

// ------------------------------------------------------------------ button

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Button {
    pub label: String,
    /// "primary", "secondary", "ghost", "gradient", "outline".
    pub variant: String,
    /// Font size; the button sizes itself around the label.
    pub size: f32,
    pub color: Option<Color>,
    /// Second gradient colour for `gradient` (default: a lighter shade of `color`).
    pub color2: Option<Color>,
    /// Times (s) at which it is pressed.
    pub press: Vec<f32>,
    /// Trailing arrow icon.
    pub arrow: bool,
    /// Label colour (default: white or near-black, whichever contrasts best with the fill).
    pub label_color: Option<Color>,
    pub delay: f32,
    pub out: Option<f32>,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
    /// Corner radius (default: pill).
    pub radius: Option<f32>,
    pub motion: Motion,
    pub face: Option<Face>,
    /// Light sweep across the button after it appears.
    pub sheen: bool,
    pub weight: u16,
}
impl Default for Button {
    fn default() -> Self {
        Button { label: "Button".into(), variant: "primary".into(), size: 26.0, color: None, color2: None, press: Vec::new(), arrow: false, delay: 0.0, out: None, align: 0.5, label_color: None, radius: None, motion: Motion::default(), face: None, sheen: false, weight: 600 }
    }
}
impl Prepare for Button {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if !["primary", "secondary", "ghost", "gradient", "outline"].contains(&self.variant.as_str()) {
            return Err(super::Error(format!("button variant must be primary, secondary, ghost, gradient or outline, not {:?}", self.variant)));
        }
        Ok(())
    }
}
impl Component for Button {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let m = self.motion;
        let (p, exit) = presence(m, t, self.delay, self.out, 0.3);
        if p <= 0.0 {
            return Vec::new();
        }
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let acc2 = self.color2.map(|c| c.0).unwrap_or(shade(acc, 0.3));
        let fs = self.size;
        let fills = match self.variant.as_str() {
            "gradient" => vec![acc, acc2],
            "primary" => vec![shade(acc, 0.1), shade(acc, -0.1)],
            _ => vec![],
        };
        let fg = match (self.label_color, fills.is_empty()) {
            (Some(c), _) => c.0,
            (None, false) => on_color(&fills),
            (None, true) if self.variant == "outline" => acc,
            (None, true) => th.text,
        };
        let st = face_style(self.face, fs, self.weight, fg);
        let tw = ctx.assets.measure(&self.label, &st);
        let (px, py) = (fs * 1.15, fs * 0.7);
        let aw = if self.arrow { fs * 1.25 } else { 0.0 };
        let (w, h) = (tw + 2.0 * px + aw, fs + 2.0 * py);
        let r = self.radius.unwrap_or(h * 0.5).min(h * 0.5);
        // Press: quick dip, spring back.
        let mut s = 1.0;
        let mut nudge = 0.0f32;
        let mut flash = 0.0f32;
        for &pt in &self.press {
            let d = t - pt;
            if d >= 0.0 {
                let down = if d < 0.08 { ease_out(d / 0.08) } else { 1.0 - spring_z(d - 0.08, 1.2, 0.5) };
                s = 1.0 - 0.05 * down;
                nudge = nudge.max(down);
                flash = flash.max((1.0 - d / 0.35).max(0.0));
            }
        }
        let mut o = Vec::new();
        match self.variant.as_str() {
            "primary" | "gradient" => {
                polish::tinted_shadow(&mut o, 0.0, 0.0, w, h, r, 8.0 * s, 20.0, alpha(shade(acc, -0.3), 0.55));
                shadow(&mut o, 0.0, 0.0, w, h, r, 1.0, 2.0, 0.3);
                let paint = if self.variant == "gradient" {
                    Paint::Linear { p0: [0.0, 0.0], p1: [w, h], stops: vec![(0.0, acc), (1.0, acc2)] }
                } else {
                    vgrad(0.0, h, fills[0], fills[1])
                };
                o.push(rect(0.0, 0.0, w, h, r, paint));
                polish::bevel(&mut o, 0.0, 0.0, w, h, r, 1.8);
                // Bottom inner shade for a pressed-in edge.
                o.push(Op::Stroke { path: rrect_path(0.5, 0.5, w - 1.0, h - 1.0, r), paint: vgrad(h * 0.5, h, Rgba([0.0, 0.0, 0.0, 0.0]), Rgba([0.0, 0.0, 0.0, 0.25])), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
                let sw = seg(t, self.delay + 0.25, 0.9);
                if self.sheen && sw > 0.0 && sw < 1.0 {
                    o.push(rect(0.0, 0.0, w, h, r, Paint::Linear { p0: [0.0, 0.0], p1: [w, h * 0.4], stops: polish::sheen_stops(lerp(-0.25, 1.25, ease_in_out(sw)), 0.12, Rgba([1.0, 1.0, 1.0, 0.35])) }));
                }
            }
            "secondary" => {
                polish::elevation(&mut o, th, 0.0, 0.0, w, h, r, 0.4);
                o.push(rect(0.0, 0.0, w, h, r, vgrad(0.0, h, shade(th.surface2, 0.05), th.surface2)));
                border(&mut o, 0.0, 0.0, w, h, r, th.border);
                polish::bevel(&mut o, 0.0, 0.0, w, h, r, if th.dark { 1.0 } else { 0.0 });
            }
            "outline" => {
                o.push(Op::Stroke { path: rrect_path(0.75, 0.75, w - 1.5, h - 1.5, r), paint: solid(acc), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
            }
            _ => {}
        }
        draw(&mut o, ctx.assets, &self.label, &st, px, h * 0.5 + fs * 0.36, 0.0);
        if self.arrow {
            icon(&mut o, "arrow", px + tw + aw * 0.62 + nudge * fs * 0.15, h * 0.5, fs * 0.78, fg);
        }
        if flash > 0.0 {
            o.push(rect(0.0, 0.0, w, h, r, solid(Rgba([1.0, 1.0, 1.0, 0.16 * flash]))));
        }
        transform_ops(&mut o, &scale_about(s, w * 0.5, h * 0.5));
        transform_ops(&mut o, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        let (cx, cy) = ((ctx.width - w) * self.align + w * 0.5, (ctx.height - h).max(0.0) * 0.5 + h * 0.5);
        reveal(o, p, exit, cx, cy, 14.0 * m.travel())
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
    pub motion: Motion,
    /// "solid" (lit knob, default) or "flat".
    pub surface: Surface,
}
impl Default for Toggle {
    fn default() -> Self {
        Toggle { at: 0.5, on: false, size: 44.0, color: None, label: None, motion: Motion::default(), surface: Surface::Solid }
    }
}
impl Prepare for Toggle {}
impl Component for Toggle {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let h = self.size;
        let w = h * 1.65;
        let k = match self.motion {
            Motion::Smooth => spring_z(t - self.at, 1.3, 0.75),
            m => m.enter(t - self.at),
        };
        let k = if self.on { 1.0 - k } else { k };
        let kc = clamp01(k);
        let acc = self.color.map(|c| c.0).unwrap_or(th.positive);
        let off = if th.dark { Rgba([1.0, 1.0, 1.0, 0.14]) } else { Rgba([0.0, 0.0, 0.0, 0.12]) };
        let flat = matches!(self.surface, Surface::Flat | Surface::Outline | Surface::Paper);
        let mut o = Vec::new();
        if !flat {
            polish::tinted_shadow(&mut o, 0.0, 0.0, w, h, h * 0.5, h * 0.12, h * 0.35, alpha(acc, 0.4 * kc));
        }
        if self.surface == Surface::Outline {
            o.push(Op::Stroke { path: rrect_path(0.75, 0.75, w - 1.5, h - 1.5, h * 0.5), paint: solid(mix(alpha(th.text, 0.5), acc, kc)), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        } else {
            o.push(rect(0.0, 0.0, w, h, h * 0.5, solid(mix(off, acc, kc))));
        }
        if !flat {
            // Inset: darker top inner edge.
            o.push(Op::Stroke { path: rrect_path(0.5, 0.5, w - 1.0, h - 1.0, h * 0.5), paint: vgrad(0.0, h * 0.6, Rgba([0.0, 0.0, 0.0, 0.28]), Rgba([0.0, 0.0, 0.0, 0.0])), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        }
        let pad = h * 0.09;
        let d = h - 2.0 * pad;
        // The knob stretches while moving.
        let moving = (kc * (1.0 - kc) * 4.0).clamp(0.0, 1.0);
        let kw = d * (1.0 + 0.28 * moving);
        let kx = pad + (w - 2.0 * pad - kw) * k;
        let knob = if self.surface == Surface::Outline { mix(alpha(th.text, 0.7), acc, kc) } else { white() };
        if !flat {
            shadow(&mut o, kx, pad, kw, d, d * 0.5, d * 0.06, d * 0.12, 0.25);
            shadow(&mut o, kx, pad, kw, d, d * 0.5, d * 0.12, d * 0.3, 0.25);
            o.push(rect(kx, pad, kw, d, d * 0.5, vgrad(pad, pad + d, white(), Rgba([0.93, 0.93, 0.95, 1.0]))));
        } else {
            o.push(rect(kx, pad, kw, d, d * 0.5, solid(knob)));
        }
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &ui(h * 0.5, 500, th.text), w + h * 0.45, h * 0.5 + h * 0.18, 0.0);
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
    /// Label colour (default: by contrast with the fill).
    pub label_color: Option<Color>,
    pub delay: f32,
    pub out: Option<f32>,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
    /// Corner radius (default: pill).
    pub radius: Option<f32>,
    /// Uppercase, tracked label (a "tag" look).
    pub caps: bool,
    /// Outline only (no fill).
    pub outline: bool,
    pub motion: Motion,
    pub face: Option<Face>,
}
impl Default for Badge {
    fn default() -> Self {
        Badge { label: "New".into(), color: None, size: 20.0, dot: false, solid: false, delay: 0.0, out: None, align: 0.5, label_color: None, radius: None, caps: false, outline: false, motion: Motion::default(), face: None }
    }
}
impl Prepare for Badge {}
impl Component for Badge {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (p, exit) = presence(self.motion, t, self.delay, self.out, 0.25);
        if p <= 0.0 {
            return Vec::new();
        }
        let c = self.color.map(|c| c.0).unwrap_or(th.accent);
        let fs = self.size;
        let tint_bg = mix(th.background, c, 0.16);
        let fg = match self.label_color {
            Some(l) => l.0,
            None if self.solid => on_color(&[shade(c, 0.08), shade(c, -0.08)]),
            // Tinted badge: the label is the hue itself, pushed towards the
            // background's opposite until it reads (≥ 4.5:1 where possible).
            None => {
                let mut l = shade(c, if th.dark { 0.35 } else { -0.2 });
                for _ in 0..6 {
                    if contrast(l, tint_bg) >= 4.5 {
                        break;
                    }
                    l = shade(l, if th.dark { 0.25 } else { -0.25 });
                }
                l
            }
        };
        let label = if self.caps { self.label.to_uppercase() } else { self.label.clone() };
        let st = if self.caps { polish::eyebrow(fs * 0.85, 650, fg).tracking(0.08) } else { face_style(self.face, fs, 600, fg) };
        let tw = ctx.assets.measure(&label, &st);
        let dw = if self.dot { fs * 0.85 } else { 0.0 };
        let (px, h) = (fs * 0.7, fs * 1.75);
        let w = tw + 2.0 * px + dw;
        let r = self.radius.unwrap_or(h * 0.5).min(h * 0.5);
        let mut o = Vec::new();
        if self.outline {
            o.push(Op::Stroke { path: rrect_path(0.75, 0.75, w - 1.5, h - 1.5, r), paint: solid(alpha(c, 0.9)), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        } else if self.solid {
            o.push(rect(0.0, 0.0, w, h, r, vgrad(0.0, h, shade(c, 0.08), shade(c, -0.08))));
            polish::bevel(&mut o, 0.0, 0.0, w, h, r, 1.2);
        } else {
            o.push(rect(0.0, 0.0, w, h, r, solid(alpha(c, 0.15))));
            border(&mut o, 0.0, 0.0, w, h, r, alpha(c, 0.38));
        }
        if self.dot {
            let (cx, cy) = (px + fs * 0.22, h * 0.5);
            let dc = if self.solid { fg } else { c };
            if let Some(ph) = pulse(t - self.delay, 1.0) {
                o.push(circle(cx, cy, fs * 0.22 + ease_out(ph) * fs * 0.45, solid(alpha(dc, 0.45 * (1.0 - ph)))));
            }
            o.push(circle(cx, cy, fs * 0.42, solid(alpha(dc, 0.18))));
            o.push(circle(cx, cy, fs * 0.22, solid(dc)));
        }
        let lab = if self.caps { st.size * 0.36 } else { fs * 0.36 };
        draw(&mut o, ctx.assets, &label, &st, px + dw, h * 0.5 + lab, 0.0);
        transform_ops(&mut o, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        let (cx, cy) = ((ctx.width - w) * self.align + w * 0.5, (ctx.height - h).max(0.0) * 0.5 + h * 0.5);
        reveal(o, p, exit, cx, cy, 8.0 * self.motion.travel())
    }
}

// ------------------------------------------------------------------ avatars

fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

fn draw_avatar(o: &mut Vec<Op>, ctx: &Ctx, name: &str, src: Option<&str>, x: f32, y: f32, s: f32, ring: Rgba, square: bool) {
    let th = ctx.theme;
    let h = fnv(&[name.as_bytes()]) as usize;
    let (c1, c2) = (th.pal(h), th.pal(h / 7 + 1));
    let (cx, cy, r) = (x + s * 0.5, y + s * 0.5, s * 0.5);
    let cr = if square { s * 0.24 } else { r };
    let ringw = s * 0.045;
    shadow(o, x - ringw, y - ringw, s + 2.0 * ringw, s + 2.0 * ringw, cr + ringw, s * 0.05, s * 0.1, if th.dark { 0.45 } else { 0.15 });
    o.push(rect(x - ringw, y - ringw, s + 2.0 * ringw, s + 2.0 * ringw, cr + ringw, solid(ring)));
    if let Some((id, iw, ih)) = src.and_then(|s| ctx.assets.media(s, 0.0)) {
        // Cover-fit the image in the shape.
        let k = s / iw.min(ih) as f32;
        let (dw, dh) = (iw as f32 * k, ih as f32 * k);
        o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(x, y, s, s, cr), IDENTITY)), cache_key: None });
        o.push(Op::Image { image: id, dst: [cx - dw * 0.5, cy - dh * 0.5, dw, dh], opacity: 1.0, transform: IDENTITY });
        o.push(Op::PopLayer);
    } else {
        let (a, b) = (shade(c1, 0.12), shade(c2, -0.18));
        o.push(rect(x, y, s, s, cr, Paint::Linear { p0: [x, y], p1: [x + s, y + s], stops: vec![(0.0, a), (1.0, b)] }));
        o.push(rect(x, y, s, s, cr, radial(x + s * 0.3, y + s * 0.2, s * 0.8, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.18])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
        draw(o, ctx.assets, &initials(name), &headline(s * 0.38, 600, on_color(&[a, b])), cx, cy + s * 0.135, 0.5);
    }
    polish::bevel(o, x, y, s, s, cr, 1.0);
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
    /// Rounded square instead of a circle.
    pub square: bool,
    /// Ring colour (default: the background, a cut-out).
    pub ring: Option<Color>,
    pub motion: Motion,
}
impl Default for Avatar {
    fn default() -> Self {
        Avatar { name: "Ada Lovelace".into(), src: None, size: 96.0, status: None, delay: 0.0, align: 0.5, square: false, ring: None, motion: Motion::Bouncy }
    }
}
impl Prepare for Avatar {}
impl Component for Avatar {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let p = self.motion.enter(t - self.delay);
        if p <= 0.0 {
            return Vec::new();
        }
        let s = self.size;
        let mut o = Vec::new();
        draw_avatar(&mut o, ctx, &self.name, self.src.as_deref(), 0.0, 0.0, s, self.ring.map(|c| c.0).unwrap_or(ctx.theme.background), self.square);
        if let Some(st) = &self.status {
            let c = match st.as_str() {
                "away" => parse_color("#ffb547").unwrap(),
                "busy" => ctx.theme.negative,
                _ => ctx.theme.positive,
            };
            let (cx, cy) = (s * 0.86, s * 0.86);
            let k = self.motion.enter(t - self.delay - 0.25);
            o.push(circle(cx, cy, s * 0.13 * k, solid(ctx.theme.background)));
            o.push(circle(cx, cy, s * 0.09 * k, solid(c)));
        }
        transform_ops(&mut o, &scale_about(lerp(0.6, 1.0, p), s * 0.5, s * 0.5));
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
    pub square: bool,
    pub ring: Option<Color>,
    pub motion: Motion,
}
impl Default for AvatarStack {
    fn default() -> Self {
        AvatarStack { names: Vec::new(), srcs: Vec::new(), size: 72.0, overlap: 0.3, max: 5, delay: 0.0, stagger: 0.07, align: 0.5, square: false, ring: None, motion: Motion::Bouncy }
    }
}
impl Prepare for AvatarStack {}
impl Component for AvatarStack {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let s = self.size;
        let step = s * (1.0 - self.overlap.clamp(0.0, 0.6));
        let n = self.names.len().min(self.max.max(1));
        let ring = self.ring.map(|c| c.0).unwrap_or(ctx.theme.background);
        let mut o = Vec::new();
        let total = n + usize::from(self.names.len() > n);
        for i in 0..total {
            let p = self.motion.enter(t - self.delay - i as f32 * self.stagger * self.motion.pace());
            if p <= 0.0 {
                continue;
            }
            let x = i as f32 * step;
            let mut a = Vec::new();
            if i < n {
                draw_avatar(&mut a, ctx, &self.names[i], self.srcs.get(i).map(|s| s.as_str()), x, 0.0, s, ring, self.square);
            } else {
                let cr = if self.square { s * 0.24 } else { s * 0.5 };
                let rw = s * 0.045;
                a.push(rect(x - rw, -rw, s + 2.0 * rw, s + 2.0 * rw, cr + rw, solid(ring)));
                a.push(rect(x, 0.0, s, s, cr, solid(ctx.theme.surface2)));
                polish::bevel(&mut a, x, 0.0, s, s, cr, 1.0);
                draw(&mut a, ctx.assets, &format!("+{}", self.names.len() - n), &ui(s * 0.32, 600, ctx.theme.text).tabular(), x + s * 0.5, s * 0.5 + s * 0.115, 0.5);
            }
            // Slide in from the left of its slot and pop.
            transform_ops(&mut a, &crate::dl::mul(&translate(-(1.0 - p) * s * 0.35, 0.0), &scale_about(lerp(0.7, 1.0, p), x + s * 0.5, s * 0.5)));
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
    /// Second fill colour (gradient); default a lighter accent.
    pub color2: Option<Color>,
    /// Shimmer travelling along the fill while it moves.
    pub shimmer: bool,
    /// Corner radius (default: fully rounded).
    pub radius: Option<f32>,
    /// Number of segments (0 = continuous): a stepped "battery" look.
    pub segments: usize,
    pub motion: Motion,
}
impl Default for ProgressBar {
    fn default() -> Self {
        ProgressBar { from: 0.0, to: 1.0, duration: 1.6, delay: 0.0, thickness: 12.0, color: None, label: None, percent: true, color2: None, shimmer: true, radius: None, segments: 0, motion: Motion::Soft }
    }
}
impl Prepare for ProgressBar {}
impl Component for ProgressBar {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let w = ctx.width.min(4000.0);
        let k = self.motion.ease(seg(t, self.delay, self.duration));
        let v = lerp(self.from, self.to, k).clamp(0.0, 1.0);
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let acc2 = self.color2.map(|c| c.0).unwrap_or(shade(acc, 0.3));
        let h = self.thickness;
        let r = self.radius.unwrap_or(h * 0.5).min(h * 0.5);
        let mut o = Vec::new();
        let ty = if self.label.is_some() || self.percent { h.max(14.0) * 2.4 } else { 0.0 };
        let st = ui(h.max(14.0) * 1.15, 500, th.text);
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &st, 0.0, ty - st.size * 0.65, 0.0);
        }
        if self.percent {
            draw(&mut o, ctx.assets, &format!("{}%", (v * 100.0).round()), &ui(st.size, 600, th.muted).tabular(), w, ty - st.size * 0.65, 1.0);
        }
        let track = if th.dark { Rgba([1.0, 1.0, 1.0, 0.07]) } else { Rgba([0.0, 0.0, 0.0, 0.07]) };
        if self.segments > 1 {
            let n = self.segments;
            let gap = (h * 0.35).max(3.0);
            let sw = (w - gap * (n - 1) as f32) / n as f32;
            for i in 0..n {
                let x = i as f32 * (sw + gap);
                let f = clamp01(v * n as f32 - i as f32);
                o.push(rect(x, ty, sw, h, r.min(sw * 0.5), solid(track)));
                if f > 0.0 {
                    o.push(rect(x, ty, sw, h, r.min(sw * 0.5), solid(alpha(mix(acc, acc2, i as f32 / n as f32), f))));
                }
            }
            return o;
        }
        o.push(rect(0.0, ty, w, h, r, solid(track)));
        // Inset top edge.
        o.push(Op::Stroke { path: rrect_path(0.5, ty + 0.5, w - 1.0, h - 1.0, r), paint: vgrad(ty, ty + h, Rgba([0.0, 0.0, 0.0, 0.3]), Rgba([1.0, 1.0, 1.0, 0.04])), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        let fw = (w * v).max(if v > 0.0 { h.min(w) } else { 0.0 });
        if fw > 0.0 {
            o.push(rect(0.0, ty, fw, h, r, Paint::Linear { p0: [0.0, 0.0], p1: [fw, 0.0], stops: vec![(0.0, acc), (1.0, acc2)] }));
            o.push(rect(0.0, ty, fw, h * 0.5, r.min(h * 0.25), vgrad(ty, ty + h * 0.5, Rgba([1.0, 1.0, 1.0, 0.22]), Rgba([1.0, 1.0, 1.0, 0.0]))));
            let moving = k > 0.0 && k < 1.0;
            if self.shimmer && moving {
                let ph = ((t - self.delay) * 0.9).fract();
                o.push(rect(0.0, ty, fw, h, r, Paint::Linear { p0: [0.0, 0.0], p1: [fw, 0.0], stops: polish::sheen_stops(lerp(-0.2, 1.2, ph), 0.12, Rgba([1.0, 1.0, 1.0, 0.35])) }));
            }
            // Glint at the leading edge.
            if moving {
                polish::bloom(&mut o, vec![circle(fw - h * 0.5, ty + h * 0.5, h * 0.7, solid(alpha(acc2, 0.9)))], h * 0.6, 0.6 * (1.0 - k).min(1.0).sqrt());
            }
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
    /// Second gradient colour (default: a lighter accent).
    pub color2: Option<Color>,
    /// Caption under the number.
    pub label: Option<String>,
    /// Show the percentage in the centre (false: an empty ring, e.g. behind a countdown).
    pub percent: bool,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
    /// Soft bloom around the arc.
    pub glow: bool,
    /// Butt (square) arc ends instead of round.
    pub square: bool,
    pub motion: Motion,
    pub face: Option<Face>,
}
impl Default for ProgressRing {
    fn default() -> Self {
        ProgressRing { from: 0.0, to: 1.0, duration: 1.6, delay: 0.0, size: 240.0, thickness: 18.0, color: None, color2: None, label: None, percent: true, align: 0.5, glow: false, square: false, motion: Motion::Soft, face: None }
    }
}
impl Prepare for ProgressRing {}
impl Component for ProgressRing {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let v = lerp(self.from, self.to, self.motion.ease(seg(t, self.delay, self.duration))).clamp(0.0, 1.0);
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let acc2 = self.color2.map(|c| c.0).unwrap_or(shade(acc, 0.35));
        let s = self.size;
        let r = (s - self.thickness) * 0.5;
        let (cx, cy) = (s * 0.5, s * 0.5);
        let ring = arc_path(cx, cy, r, 0.0, 1.0);
        let mut o = vec![stroke_op(ring, solid(alpha(th.text, 0.07)), self.thickness, [0.0, 1.0])];
        if v > 0.001 {
            let arc = arc_path(cx, cy, r, 0.0, v);
            let paint = Paint::Linear { p0: [cx, cy - r], p1: [cx, cy + r], stops: vec![(0.0, acc), (1.0, acc2)] };
            let mut st = line(self.thickness, [0.0, 1.0]);
            if self.square {
                st.cap = 0;
            }
            if self.glow {
                polish::bloom(&mut o, vec![Op::Stroke { path: arc.clone(), paint: paint.clone(), stroke: st.clone(), transform: IDENTITY }], self.thickness * 0.9, 0.55);
            }
            o.push(Op::Stroke { path: arc, paint, stroke: st, transform: IDENTITY });
        }
        let nsz = s * 0.24;
        let st = match self.face {
            None => headline(nsz, 600, th.text),
            f => face_style(f, nsz, 600, th.text),
        }
        .tabular();
        let dy = if self.label.is_some() { -s * 0.01 } else { nsz * 0.36 };
        if self.percent {
            let num = format!("{}", (v * 100.0).round());
            let pst = TextStyle { size: nsz * 0.5, ..st.clone() }.color(th.muted);
            let nw = ctx.assets.measure(&num, &st);
            let pw = ctx.assets.measure("%", &pst);
            let x0 = cx - (nw + pw + nsz * 0.04) * 0.5;
            draw(&mut o, ctx.assets, &num, &st, x0, cy + dy, 0.0);
            draw(&mut o, ctx.assets, "%", &pst, x0 + nw + nsz * 0.04, cy + dy - nsz * 0.3, 0.0);
        }
        if let Some(l) = &self.label {
            draw(&mut o, ctx.assets, l, &ui(s * 0.075, 500, th.muted), cx, cy + s * 0.14, 0.5);
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
    pub surface: Surface,
    pub motion: Motion,
    pub radius: Option<f32>,
    /// Icon colour (default: by `kind`).
    pub color: Option<Color>,
    /// Countdown line along the bottom while it holds.
    pub timer: bool,
}
impl Default for Toast {
    fn default() -> Self {
        Toast { title: String::new(), body: String::new(), kind: "success".into(), delay: 0.0, hold: 0.0, from: "top".into(), width: 520.0, surface: Surface::Solid, motion: Motion::Smooth, radius: None, color: None, timer: true }
    }
}
impl Prepare for Toast {}
impl Component for Toast {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let out = (self.hold > 0.0).then_some(self.delay + self.hold);
        let p = self.motion.enter(t - self.delay);
        let exit = out.map(|o| ease_in(seg(t, o, 0.35))).unwrap_or(0.0);
        if p <= 0.0 || exit >= 1.0 {
            return Vec::new();
        }
        let w = self.width;
        // Metrics scale with the width (designed at 400 px).
        let u = (w / 400.0).max(0.5);
        let pad = 18.0 * u;
        let ts = ui(18.0 * u, 600, th.text);
        let bs = ui(15.0 * u, 400, th.muted);
        let lines = wrap(ctx.assets, &self.body, &bs, w - pad * 2.0 - 48.0 * u);
        let h = pad * 2.0 + 22.0 * u + if self.body.is_empty() { 0.0 } else { lines.len() as f32 * 21.0 * u + 2.0 * u };
        let r = self.radius.unwrap_or(16.0 * u);
        let mut o = Vec::new();
        finish(&mut o, th, self.surface, 0.0, 0.0, w, h, r, 1.4, None);
        let c = self.color.map(|c| c.0).unwrap_or(match self.kind.as_str() {
            "error" => th.negative,
            "warning" => parse_color("#ffb547").unwrap(),
            "info" => th.accent,
            _ => th.positive,
        });
        let (ix, iy, ir) = (pad + 16.0 * u, pad + 15.0 * u, 16.0 * u);
        let ip = self.motion.enter(t - self.delay - 0.12);
        let mut ic = vec![circle(ix, iy, ir, solid(alpha(c, 0.16))), Op::Stroke { path: circle_path(ix, iy, ir - 0.5), paint: solid(alpha(c, 0.35)), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY }];
        let mut glyph = Vec::new();
        icon(&mut glyph, &self.kind, ix, iy, 17.0 * u, c);
        if self.kind == "success" {
            // Draw the tick on.
            for op in &mut glyph {
                if let Op::Stroke { stroke, .. } = op {
                    stroke.trim = [0.0, ease_out(seg(t, self.delay + 0.2, 0.4))];
                }
            }
        }
        ic.extend(glyph);
        transform_ops(&mut ic, &scale_about(lerp(0.5, 1.0, ip), ix, iy));
        o.extend(with_opacity(ic, clamp01(ip * 2.0)));
        let mut txt = Vec::new();
        draw(&mut txt, ctx.assets, &self.title, &ts, pad + 46.0 * u, pad + 20.0 * u, 0.0);
        for (i, l) in lines.iter().enumerate() {
            draw(&mut txt, ctx.assets, l, &bs, pad + 46.0 * u, pad + (44.0 + i as f32 * 21.0) * u, 0.0);
        }
        o.extend(sub_rise(txt, self.motion.enter(t - self.delay - 0.06), 6.0 * u));
        if self.timer && self.hold > 0.0 {
            let k = 1.0 - seg(t, self.delay + 0.3, self.hold - 0.3);
            o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(0.0, 0.0, w, h, r), IDENTITY)), cache_key: None });
            o.push(rect(0.0, h - 2.5 * u, w * k, 2.5 * u, 0.0, solid(alpha(c, 0.7))));
            o.push(Op::PopLayer);
        }
        let k = 1.0 - p + exit;
        let (dx, dy) = match self.from.as_str() {
            "bottom" => (0.0, k * (h * 0.6 + 40.0)),
            "right" => (k * (w * 0.5 + 40.0), 0.0),
            _ => (0.0, -k * (h * 0.6 + 40.0)),
        };
        transform_ops(&mut o, &scale_about(lerp(0.94, 1.0, p.min(1.0)), w * 0.5, h * 0.5));
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
    /// "arrow" (default), "hand" or "dot" (a soft touch/presenter dot).
    pub style: String,
    pub ripple: Option<Color>,
    /// Pointer fill (default white; the outline contrasts automatically).
    pub color: Option<Color>,
    /// Faint motion trail on fast moves.
    pub trail: bool,
}
impl Default for Cursor {
    fn default() -> Self {
        Cursor { path: vec![[0.0, 0.0]], times: Vec::new(), clicks: Vec::new(), size: 36.0, style: "arrow".into(), ripple: None, color: None, trail: true }
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
                let k = ease_in_out_quint(((t - self.times[i - 1]) / (self.times[i] - self.times[i - 1]).max(1e-3)).powf(0.85));
                let (a, b) = (self.path[i - 1], self.path[i]);
                // Slight arc, like a real hand.
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let bow = (k * (1.0 - k)) * 0.16;
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
        let fillc = self.color.map(|c| c.0).unwrap_or(white());
        let edge = on_color(&[fillc]);
        let mut squash = 1.0f32;
        for &c in &self.clicks {
            let d = t - c;
            if (0.0..0.7).contains(&d) {
                let (rx, ry) = self.pos(c);
                for (lag, a) in [(0.0, 1.0), (0.1, 0.5)] {
                    let k = ease_out_expo(seg(d, lag, 0.6));
                    if k > 0.0 && k < 1.0 {
                        let rr = 4.0 + k * self.size * 1.3;
                        o.push(circle(rx, ry, rr, solid(alpha(rc, 0.22 * a * (1.0 - k)))));
                        o.push(Op::Stroke { path: circle_path(rx, ry, rr), paint: solid(alpha(rc, 0.85 * a * (1.0 - k))), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
                    }
                }
            }
            if d >= 0.0 {
                let down = if d < 0.07 { ease_out(d / 0.07) } else { 1.0 - spring_z(d - 0.07, 1.3, 0.55) };
                squash = squash.min(1.0 - 0.14 * down);
            }
        }
        let s = self.size / 24.0;
        let mut c = Vec::new();
        if self.style == "dot" {
            let r = self.size * 0.42;
            c.push(circle(x, y, r * 1.6, solid(alpha(fillc, 0.12))));
            c.push(circle(x, y, r, solid(alpha(fillc, 0.55))));
            c.push(Op::Stroke { path: circle_path(x, y, r), paint: solid(alpha(fillc, 0.9)), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        } else {
            let mut p = Path::new();
            let pt = |px: f32, py: f32| ((x + px * s) as f64, (y + py * s) as f64);
            if self.style == "hand" {
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
                // macOS-style arrow.
                p.move_to(pt(0.0, 0.0));
                p.line_to(pt(0.0, 17.2));
                p.line_to(pt(4.3, 13.3));
                p.line_to(pt(7.1, 19.7));
                p.line_to(pt(9.9, 18.5));
                p.line_to(pt(7.2, 12.3));
                p.line_to(pt(12.8, 12.3));
            }
            p.close_path();
            push_layer(&mut c, 1.0, Blend::Normal, vec![crate::dl::Filter::Blur(2.2 * s)]);
            let mut sh = vec![fill(p.clone(), solid(Rgba([0.0, 0.0, 0.0, 0.4])))];
            transform_ops(&mut sh, &translate(0.6 * s, 2.0 * s));
            c.extend(sh);
            c.push(Op::PopLayer);
            c.push(Op::Stroke { path: p.clone(), paint: solid(edge), stroke: line(1.7 * s, [0.0, 1.0]), transform: IDENTITY });
            c.push(fill(p, solid(fillc)));
        }
        transform_ops(&mut c, &scale_about(squash, x, y));
        if self.trail {
            let dt = 1.0 / ctx.fps.max(1.0);
            let (px, py) = self.pos(t - dt);
            c = polish::smear(c, x - px, y - py);
        }
        o.extend(c);
        o
    }
    fn duration(&self) -> Option<f32> {
        let a = self.times.last().copied().unwrap_or(0.0);
        let b = self.clicks.iter().fold(0.0f32, |m, c| m.max(*c + 0.7));
        Some(a.max(b))
    }
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
    /// Bubble colour (default: inverted theme, high contrast).
    pub color: Option<Color>,
    pub radius: Option<f32>,
    pub motion: Motion,
    /// Keyboard shortcut hint shown dimmed after the text, e.g. "⌘C".
    pub shortcut: String,
}
impl Default for Tooltip {
    fn default() -> Self {
        Tooltip { text: "Tooltip".into(), at: [160.0, 80.0], side: "top".into(), size: 22.0, delay: 0.0, out: None, color: None, radius: None, motion: Motion::Snappy, shortcut: String::new() }
    }
}
impl Prepare for Tooltip {}
impl Component for Tooltip {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (p, exit) = presence(self.motion, t, self.delay, self.out, 0.2);
        if p <= 0.0 {
            return Vec::new();
        }
        let bg = self.color.map(|c| c.0).unwrap_or(if th.dark { Rgba([0.97, 0.97, 0.98, 1.0]) } else { parse_color("#16171d").unwrap() });
        let fg = on_color(&[bg]);
        let st = ui(self.size, 500, fg);
        let ks = ui(self.size * 0.92, 500, alpha(fg, 0.5));
        let tw = ctx.assets.measure(&self.text, &st);
        let kw = if self.shortcut.is_empty() { 0.0 } else { ctx.assets.measure(&self.shortcut, &ks) + self.size * 0.6 };
        let (px, py) = (self.size * 0.7, self.size * 0.5);
        let (w, h) = (tw + kw + 2.0 * px, self.size + 2.0 * py);
        let [ax, ay] = self.at;
        let up = self.side != "bottom";
        let a = self.size * 0.32;
        let y0 = if up { ay - a - h } else { ay + a };
        let x0 = ax - w * 0.5;
        let r = self.radius.unwrap_or(h * 0.28).min(h * 0.5);
        let mut o = Vec::new();
        polish::elevation(&mut o, th, x0, y0, w, h, r, 0.6);
        o.push(rect(x0, y0, w, h, r, solid(bg)));
        let mut tri = Path::new();
        let by = if up { y0 + h - 0.5 } else { y0 + 0.5 };
        tri.move_to(((ax - a * 1.2) as f64, by as f64));
        tri.quad_to((ax as f64, by as f64 + (ay - by) as f64 * 0.5), (ax as f64, ay as f64));
        tri.quad_to((ax as f64, by as f64 + (ay - by) as f64 * 0.5), ((ax + a * 1.2) as f64, by as f64));
        tri.close_path();
        o.push(fill(tri, solid(bg)));
        draw(&mut o, ctx.assets, &self.text, &st, x0 + px, y0 + h * 0.5 + self.size * 0.36, 0.0);
        if kw > 0.0 {
            draw(&mut o, ctx.assets, &self.shortcut, &ks, x0 + px + tw + self.size * 0.6, y0 + h * 0.5 + self.size * 0.34, 0.0);
        }
        let s = lerp(0.82, 1.0, p);
        transform_ops(&mut o, &crate::dl::mul(&translate(0.0, (1.0 - p) * if up { 6.0 } else { -6.0 }), &scale_about(s, ax, ay)));
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
    /// Label finish: solid (pill), glass, outline, flat, paper.
    pub surface: Surface,
    /// "elbow" (default) or "straight" leader.
    pub leader: String,
    pub face: Option<Face>,
}
impl Default for Callout {
    fn default() -> Self {
        Callout { from: [100.0, 100.0], to: [300.0, 60.0], text: String::new(), color: None, size: 26.0, delay: 0.0, out: None, surface: Surface::Glass, leader: "elbow".into(), face: None }
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
        let dot = spring_z(t, 1.2, 0.5);
        if let Some(ph) = pulse(t, 0.8) {
            o.push(Op::Stroke { path: circle_path(fx, fy, 6.0 + ease_out(ph) * 20.0), paint: solid(alpha(c, 0.6 * (1.0 - ph))), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
        }
        o.push(circle(fx, fy, 11.0 * dot, solid(alpha(c, 0.2))));
        o.push(circle(fx, fy, 6.0 * dot, solid(c)));
        o.push(circle(fx, fy, 2.4 * dot, solid(white())));
        let mut p = Path::new();
        let ex = tx - (tx - fx).signum() * 6.0;
        p.move_to((fx as f64, fy as f64));
        if self.leader != "straight" {
            let elbow = (fx + (ty - fy).abs() * (tx - fx).signum() * 0.6, ty);
            p.line_to((elbow.0 as f64, elbow.1 as f64));
        }
        p.line_to((ex as f64, ty as f64));
        let k = ease_in_out(seg(t, 0.12, 0.5));
        o.push(stroke_op(p, Paint::Linear { p0: [fx, fy], p1: [tx, ty], stops: vec![(0.0, alpha(c, 0.95)), (1.0, alpha(c, 0.5))] }, 1.5, [0.0, k]));
        let lp = spring_z(t - 0.5, 1.0, 0.8);
        if lp > 0.0 {
            let st = face_style(self.face, self.size, 600, th.text);
            let tw = ctx.assets.measure(&self.text, &st);
            let right = tx >= fx;
            let (pad, h) = (self.size * 0.62, self.size * 1.9);
            let w = tw + 2.0 * pad;
            let x0 = if right { tx } else { tx - w };
            let mut l = Vec::new();
            finish(&mut l, th, self.surface, x0, ty - h * 0.5, w, h, h * 0.5, 0.6, None);
            if self.surface == Surface::Glass {
                border(&mut l, x0, ty - h * 0.5, w, h, h * 0.5, alpha(c, 0.45));
            }
            draw(&mut l, ctx.assets, &self.text, &st, x0 + pad, ty + self.size * 0.36, 0.0);
            let m = scale_about(lerp(0.85, 1.0, lp), if right { x0 } else { x0 + w }, ty);
            transform_ops(&mut l, &m);
            o.extend(with_opacity(l, clamp01(lp * 1.5)));
        }
        with_opacity(o, 1.0 - exit)
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 1.1)
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
    /// Head style: "open" (chevron, default), "solid" (filled triangle) or "dot".
    pub head: String,
    /// Wobbly, hand-drawn stroke.
    pub sketch: bool,
}
impl Default for Arrow {
    fn default() -> Self {
        Arrow { from: [100.0, 300.0], to: [400.0, 120.0], bend: 0.35, color: None, width: 3.0, delay: 0.0, duration: 0.7, dashed: false, head: "open".into(), sketch: false }
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
        let tip = |u: f32| {
            let v = 1.0 - u;
            (v * v * ax + 2.0 * v * u * mx + u * u * bx, v * v * ay + 2.0 * v * u * my + u * u * by)
        };
        let mut p = Path::new();
        if self.sketch {
            let len = (dx * dx + dy * dy).sqrt();
            let n = 24;
            for i in 0..=n {
                let u = i as f32 / n as f32;
                let (x, y) = tip(u);
                let wob = (u * 13.0 + 1.3).sin() * len * 0.006 * (u * (1.0 - u) * 4.0);
                let q = ((x - dy / len * wob) as f64, (y + dx / len * wob) as f64);
                if i == 0 {
                    p.move_to(q);
                } else {
                    p.line_to(q);
                }
            }
        } else {
            p.move_to((ax as f64, ay as f64));
            p.quad_to((mx as f64, my as f64), (bx as f64, by as f64));
        }
        let mut st = line(self.width, [0.0, k]);
        if self.dashed {
            st.dash = vec![self.width * 3.0, self.width * 2.5];
        }
        let mut o = vec![Op::Stroke { path: p, paint: solid(c), stroke: st, transform: IDENTITY }];
        // Head along the tangent at the current tip, growing in at the end.
        let (hx, hy) = tip(k);
        let (qx, qy) = tip((k - 0.02).max(0.0));
        let ang = (hy - qy).atan2(hx - qx);
        let hl = self.width * 4.2 * lerp(0.4, 1.0, ease_out(seg(k, 0.6, 0.4)));
        match self.head.as_str() {
            "dot" => o.push(circle(hx, hy, self.width * 1.8, solid(c))),
            "solid" => {
                let mut h = Path::new();
                h.move_to(((hx + ang.cos() * hl * 0.35) as f64, (hy + ang.sin() * hl * 0.35) as f64));
                for s in [-1.0f32, 1.0] {
                    let a = ang + std::f32::consts::PI + s * 0.45;
                    h.line_to(((hx + a.cos() * hl) as f64, (hy + a.sin() * hl) as f64));
                }
                h.close_path();
                o.push(fill(h, solid(c)));
            }
            _ => {
                let mut h = Path::new();
                for s in [-1.0f32, 1.0] {
                    let a = ang + std::f32::consts::PI + s * 0.5;
                    h.move_to(((hx + a.cos() * hl) as f64, (hy + a.sin() * hl) as f64));
                    h.line_to((hx as f64, hy as f64));
                }
                o.push(stroke_op(h, solid(c), self.width, [0.0, 1.0]));
            }
        }
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
    /// "left" (default), "center" or "right".
    pub align: String,
    /// Margin from the bottom-left of the box.
    pub margin: f32,
    /// "plate" (broadcast name plate over an accent strip, default),
    /// "minimal" (accent rule and type only, no panels) or "glass".
    pub style: String,
    /// Plate colour (default: theme surface).
    pub color: Option<Color>,
    pub face: Option<Face>,
    pub radius: Option<f32>,
    pub motion: Motion,
    /// Light sweep across the name after it lands.
    pub sheen: bool,
}
impl Default for LowerThird {
    fn default() -> Self {
        LowerThird { title: String::new(), subtitle: String::new(), accent: None, size: 44.0, delay: 0.0, out: None, align: "left".into(), margin: 72.0, style: "plate".into(), color: None, face: None, radius: None, motion: Motion::Smooth, sheen: true }
    }
}
impl Prepare for LowerThird {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if !["plate", "minimal", "glass"].contains(&self.style.as_str()) {
            return Err(super::Error(format!("lower-third style must be plate, minimal or glass, not {:?}", self.style)));
        }
        Ok(())
    }
}
impl Component for LowerThird {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let acc = self.accent.map(|c| c.0).unwrap_or(th.accent);
        let t = t - self.delay;
        if t <= 0.0 {
            return Vec::new();
        }
        let pace = self.motion.pace();
        let tout = self.out.map(|o| t + self.delay - o).unwrap_or(-1.0);
        if tout > 0.75 * pace {
            return Vec::new();
        }
        let wipe = |start: f32, dur: f32| -> f32 {
            let i = ease_in_out_quint(seg(t, start * pace, dur * pace));
            let o = if tout > 0.0 { ease_in_out_quint(seg(tout, (0.25 - start * 0.5).max(0.0) * pace, 0.45 * pace)) } else { 0.0 };
            (i - o).max(0.0)
        };
        let s = self.size;
        let ts = match self.face {
            None => headline(s, 700, th.text),
            f => face_style(f, s, 700, th.text),
        };
        let minimal = self.style == "minimal";
        let plate_c = self.color.map(|c| c.0).unwrap_or(th.surface);
        let name_ink = if minimal || self.style == "glass" { th.text } else { on_color(&[plate_c]) };
        let ts = ts.color(name_ink);
        let sub_fill = acc;
        let sub_ink = if minimal { th.muted } else { on_color(&[sub_fill]) };
        let ss = polish::eyebrow(s * 0.36, 600, sub_ink).tracking(0.1);
        let sub = self.subtitle.to_uppercase();
        let tw = ctx.assets.measure(&self.title, &ts);
        let sw = ctx.assets.measure(&sub, &ss);
        let pad = s * 0.55;
        let (subw, subh) = (sw + pad * 1.6, s * 0.88);
        let (nw, nh) = ((tw + pad * 2.0).max(subw + s * 0.5), s * 1.6);
        let total_w = nw.max(subw);
        let x0 = match self.align.as_str() {
            "center" => (ctx.width - total_w) * 0.5,
            "right" => ctx.width - self.margin - total_w,
            _ => self.margin,
        };
        let y0 = ctx.height - self.margin - nh - subh;
        let r = self.radius.unwrap_or(s * 0.12);
        let mut o = Vec::new();
        // Accent rule: grows first, everything wipes out of it.
        let rule = wipe(0.0, 0.4);
        let bar_w = (s * 0.1).max(3.0);
        let bx = x0 - bar_w - s * 0.14;
        let full_h = nh + subh;
        let rh = full_h * rule;
        if rule > 0.0 {
            o.push(rect(bx, y0 + (full_h - rh) * 0.5, bar_w, rh, bar_w * 0.5, vgrad(y0, y0 + full_h, shade(acc, 0.15), shade(acc, -0.1))));
        }
        // Name plate.
        let np = wipe(0.12, 0.6);
        if np > 0.0 {
            let pw = nw * np;
            let mut plate = Vec::new();
            match self.style.as_str() {
                "minimal" => {}
                "glass" => polish::glass(&mut plate, th, x0, y0, nw, nh, r, 0.6),
                _ => {
                    polish::elevation(&mut plate, th, x0, y0, nw, nh, r, 0.8);
                    plate.push(rect(x0, y0, nw, nh, r, vgrad(y0, y0 + nh, shade(plate_c, 0.05), shade(plate_c, -0.04))));
                    polish::bevel(&mut plate, x0, y0, nw, nh, r, if th.dark { 1.0 } else { 0.4 });
                }
            }
            let tp = self.motion.ease(seg(t, 0.32 * pace, 0.8 * pace)) * (1.0 - if tout > 0.0 { ease_in(seg(tout, 0.0, 0.3 * pace)) } else { 0.0 });
            let base = y0 + nh * 0.5 + s * 0.36;
            let mut name = Vec::new();
            draw(&mut name, ctx.assets, &self.title, &ts, x0 + pad, base, 0.0);
            if self.sheen {
                let sw = seg(t, 1.0 * pace, 1.0);
                if sw > 0.0 && sw < 1.0 && tout < 0.0 {
                    let mut gl = Vec::new();
                    draw(&mut gl, ctx.assets, &self.title, &ts, x0 + pad, base, 0.0);
                    polish::repaint(&mut gl, [x0 + pad, y0], [x0 + pad + tw, y0 + nh * 0.6], &polish::sheen_stops(lerp(-0.2, 1.2, ease_in_out(sw)), 0.14, alpha(if th.dark { white() } else { acc }, 0.85)));
                    name.extend(gl);
                }
            }
            plate.extend(masked_rise(name, [x0, y0, nw, nh], nh * 0.7, tp));
            o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(x0 - s, y0 - s, pw + s, nh + 2.0 * s, 0.0), IDENTITY)), cache_key: None });
            o.extend(plate);
            o.push(Op::PopLayer);
        }
        // Subtitle strip, tucked under the plate.
        if !self.subtitle.is_empty() {
            let sp = wipe(0.26, 0.55);
            if sp > 0.0 {
                let sy = y0 + nh;
                let mut strip = Vec::new();
                if !minimal {
                    strip.push(rect(x0, sy, subw, subh, r * 0.6, Paint::Linear { p0: [x0, 0.0], p1: [x0 + subw, 0.0], stops: vec![(0.0, sub_fill), (1.0, shade(sub_fill, -0.12))] }));
                }
                let tp = self.motion.ease(seg(t, 0.45 * pace, 0.7 * pace));
                let mut txt = Vec::new();
                draw(&mut txt, ctx.assets, &sub, &ss, x0 + pad * 0.8, sy + subh * 0.5 + ss.size * 0.36, 0.0);
                strip.extend(masked_rise(txt, [x0, sy, subw, subh], -subh * 0.8, tp));
                o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(x0 - s, sy - 1.0, subw * sp + s, subh + s, 0.0), IDENTITY)), cache_key: None });
                o.extend(strip);
                o.push(Op::PopLayer);
            }
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 1.2 * self.motion.pace())
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
    /// Gradient title fill (two or more colours, left to right).
    pub gradient: Option<Vec<Color>>,
    /// "word" (default: masked word rise), "line" (masked line rise), "fade"
    /// (soft focus pull, no mask) or "type" (letters appear left to right).
    pub reveal: String,
    pub stagger: f32,
    pub delay: f32,
    pub out: Option<f32>,
    /// 0 left, 0.5 centre (default), 1 right.
    pub align: f32,
    /// Title face: display (default), sans, serif, mono.
    pub face: Option<Face>,
    pub weight: u16,
    /// Title colour (default: theme text).
    pub color: Option<Color>,
    /// Light sweep across the title once it lands.
    pub sheen: bool,
    /// Eyebrow as a pill chip (true) or plain tracked caps (false).
    pub chip: bool,
    pub motion: Motion,
    /// Uppercase title.
    pub uppercase: bool,
    /// Line height multiplier.
    pub leading: f32,
}
impl Default for TitleCard {
    fn default() -> Self {
        TitleCard { title: String::new(), eyebrow: String::new(), subtitle: String::new(), size: None, gradient: None, reveal: "word".into(), stagger: 0.06, delay: 0.0, out: None, align: 0.5, face: None, weight: 700, color: None, sheen: false, chip: false, motion: Motion::Smooth, uppercase: false, leading: 1.04 }
    }
}
impl Prepare for TitleCard {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if !["word", "line", "fade", "type"].contains(&self.reveal.as_str()) {
            return Err(super::Error(format!("title-card reveal must be word, line, fade or type, not {:?}", self.reveal)));
        }
        if self.gradient.as_ref().is_some_and(|g| g.len() < 2) {
            return Err(super::Error("title-card gradient needs at least two colours".into()));
        }
        Ok(())
    }
}
impl Component for TitleCard {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let m = self.motion;
        let size = self.size.unwrap_or((w * 0.062).clamp(28.0, 132.0));
        let ink = self.color.map(|c| c.0).unwrap_or(th.text);
        let ts = match self.face {
            None => headline(size, self.weight, ink),
            f => face_style(f, size, self.weight, ink),
        };
        let title = if self.uppercase { self.title.to_uppercase() } else { self.title.clone() };
        let lines = wrap(ctx.assets, &title, &ts, w * 0.86);
        let es = polish::eyebrow((size * 0.17).max(13.0), 600, if self.chip { shade(th.accent, if th.dark { 0.45 } else { -0.15 }) } else { th.accent });
        let ss = ui(size * 0.3, 400, th.muted);
        let sub = wrap(ctx.assets, &self.subtitle, &ss, w * 0.7);
        let lh = size * self.leading.max(0.7);
        let eh = if self.eyebrow.is_empty() { 0.0 } else if self.chip { es.size * 2.3 + size * 0.38 } else { es.size * 2.4 };
        let block = eh + lines.len() as f32 * lh + if sub.is_empty() { 0.0 } else { size * 0.4 + sub.len() as f32 * ss.size * 1.45 };
        let mut y = (h - block) * 0.5;
        let ax = w * (0.07 + 0.86 * self.align);
        let t0 = t - self.delay;
        let stag = self.stagger * m.pace();
        let dur = 0.85 * m.pace().sqrt();
        // Staggered exit: earlier pieces leave first.
        let exit_at = |k: f32| self.out.map(|o| ease_in_out(seg(t, o + k * stag * 0.5, 0.45))).unwrap_or(0.0);
        let mut o = Vec::new();
        let mut k = 0usize;
        if !self.eyebrow.is_empty() {
            let txt = self.eyebrow.to_uppercase();
            let tw = ctx.assets.measure(&txt, &es);
            let mut e = Vec::new();
            if self.chip {
                let (ph, padx) = (es.size * 2.3, es.size * 1.1);
                let dot = es.size * 0.9;
                let pw = tw + padx * 2.0 + dot;
                let px = ax - pw * self.align;
                e.push(rect(px, y, pw, ph, ph * 0.5, solid(alpha(th.accent, 0.12))));
                border(&mut e, px, y, pw, ph, ph * 0.5, alpha(th.accent, 0.35));
                e.push(circle(px + padx + es.size * 0.2, y + ph * 0.5, es.size * 0.22, solid(th.accent)));
                draw(&mut e, ctx.assets, &txt, &es, px + padx + dot, y + ph * 0.5 + es.size * 0.36, 0.0);
            } else {
                draw(&mut e, ctx.assets, &txt, &es, ax, y + es.size, self.align);
            }
            let p = m.enter(t0) * (1.0 - exit_at(0.0));
            o.extend(sub_rise(e, p, 12.0 * m.travel()));
            y += eh;
            k += 1;
        }
        let grad: Option<Vec<Rgba>> = self.gradient.as_ref().map(|g| g.iter().map(|c| alpha(c.0, ink.0[3])).collect());
        let widths: Vec<f32> = lines.iter().map(|l| ctx.assets.measure(l, &ts)).collect();
        let bw = widths.iter().copied().fold(0.0, f32::max);
        let bx = ax - bw * self.align;
        let gy0 = y;
        let paint_grad = |ops: &mut Vec<Op>| {
            if let Some(g) = &grad {
                let n = g.len() - 1;
                let stops: Vec<(f32, Rgba)> = g.iter().enumerate().map(|(i, c)| (i as f32 / n as f32, *c)).collect();
                polish::repaint(ops, [bx, gy0], [bx + bw, gy0 + lh * lines.len() as f32 * 0.5], &stops);
            }
        };
        let space = ctx.assets.measure(" ", &ts);
        let mut last_start = 0.0f32;
        let total_chars = title.chars().count().max(1) as f32;
        let mut char_i = 0usize;
        for (li, l) in lines.iter().enumerate() {
            let base = y + size * 0.93;
            let lw = widths[li];
            let lx = ax - lw * self.align;
            let clip = [lx - size * 0.3, base - size * 1.05, lw + size * 0.6, size * 1.5];
            let mut lo = Vec::new();
            match self.reveal.as_str() {
                "line" | "fade" => {
                    let start = li as f32 * stag * 2.5;
                    last_start = last_start.max(start);
                    let mut e = Vec::new();
                    draw(&mut e, ctx.assets, l, &ts, lx, base, 0.0);
                    paint_grad(&mut e);
                    let p = m.ease(seg(t0, start, dur));
                    if self.reveal == "fade" {
                        let mut e2 = e;
                        transform_ops(&mut e2, &scale_about(lerp(1.04, 1.0, p), lx + lw * 0.5, base));
                        lo.extend(polish::blurred(e2, (1.0 - p) * size * 0.12, clamp01(p * 1.3)));
                    } else {
                        lo.extend(masked_rise(e, clip, size * 1.1 * m.travel().min(1.0), p));
                    }
                }
                "type" => {
                    let mut x = lx;
                    for ch in l.chars() {
                        let start = char_i as f32 / total_chars * stag * 18.0;
                        last_start = last_start.max(start);
                        let p = ease_out(seg(t0, start, 0.25));
                        let s = ch.to_string();
                        let adv = ctx.assets.measure(&s, &ts);
                        if p > 0.0 && !ch.is_whitespace() {
                            let mut e = Vec::new();
                            draw(&mut e, ctx.assets, &s, &ts, x, base, 0.0);
                            paint_grad(&mut e);
                            transform_ops(&mut e, &translate(0.0, (1.0 - p) * size * 0.12));
                            lo.extend(with_opacity(e, p));
                        }
                        x += adv;
                        char_i += 1;
                    }
                    char_i += 1;
                }
                _ => {
                    let mut x = lx;
                    for wd in l.split(' ') {
                        let start = k as f32 * stag;
                        last_start = last_start.max(start);
                        let mut e = Vec::new();
                        let adv = draw(&mut e, ctx.assets, wd, &ts, x, base, 0.0);
                        paint_grad(&mut e);
                        let p = m.ease(seg(t0, start, dur));
                        lo.extend(masked_rise(e, clip, size * 1.1 * m.travel().min(1.0), p));
                        x += adv + space;
                        k += 1;
                    }
                }
            }
            if self.sheen && self.out.is_none_or(|o| t < o) {
                let sw = seg(t0, last_start + dur * 0.8 + li as f32 * 0.08, 1.3);
                if sw > 0.0 && sw < 1.0 {
                    let mut gl = Vec::new();
                    draw(&mut gl, ctx.assets, l, &ts, lx, base, 0.0);
                    polish::repaint(&mut gl, [lx, base - size], [lx + lw, base], &polish::sheen_stops(lerp(-0.25, 1.25, ease_in_out(sw)), 0.1, Rgba([1.0, 1.0, 1.0, if th.dark { 0.75 } else { 0.5 }])));
                    lo.extend(gl);
                }
            }
            let e = exit_at(li as f32 + 1.0);
            if e > 0.0 {
                // Lines leave upwards through their own masks.
                o.extend(masked_rise(lo, clip, -size * 1.1, 1.0 - e));
            } else {
                o.extend(lo);
            }
            y += lh;
            k += 1;
        }
        y += size * 0.4;
        for (i, l) in sub.iter().enumerate() {
            let base = y + ss.size * 1.1;
            let start = last_start + 0.12 + i as f32 * stag;
            let mut e = Vec::new();
            draw(&mut e, ctx.assets, l, &ss, ax, base, self.align);
            let p = m.ease(seg(t0, start, dur)) * (1.0 - exit_at(lines.len() as f32 + 1.0 + i as f32));
            let lw = ctx.assets.measure(l, &ss);
            let lx = ax - lw * self.align;
            o.extend(with_opacity(masked_rise(e, [lx - ss.size, base - ss.size * 1.1, lw + ss.size * 2.0, ss.size * 1.5], ss.size * 1.2, p.max(0.001)), clamp01(p * 1.5)));
            y += ss.size * 1.45;
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        let words = self.title.split_whitespace().count() + 2;
        Some(self.delay + words as f32 * self.stagger * self.motion.pace() + 1.0)
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
