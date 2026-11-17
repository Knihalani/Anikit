//! Content blocks for product and launch videos: chat threads, an AI prompt
//! box, checklists, testimonial quotes, tables, timelines and marquees.
//!
//! Like the UI kit, blocks take look keys (`surface`, `motion`, `radius`,
//! `face`, colours) so the same block can be dressed very differently.

use super::polish::{self, ease_out_expo, face_style, finish, headline, spring_z, ui, Motion, Surface};
use super::style::*;
use super::text::{draw, wrap, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Error, Prepare};
use crate::dl::{Op, Paint, Path, Rgba, IDENTITY};
use kurbo::Shape;
use serde::Deserialize;

/// Rise + fade entrance for one item.
fn rise(ops: Vec<Op>, p: f32, dy: f32) -> Vec<Op> {
    if p <= 0.0 {
        return Vec::new();
    }
    let mut ops = ops;
    transform_ops(&mut ops, &translate(0.0, (1.0 - p) * dy));
    with_opacity(ops, clamp01(p * 1.4))
}

/// Rounded rect with per-corner radii (tl, tr, br, bl).
fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Path {
    let m = (w.min(h) * 0.5).max(0.0);
    let c = |v: f32| v.clamp(0.0, m) as f64;
    kurbo::RoundedRect::from_rect(kurbo::Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64), kurbo::RoundedRectRadii::new(c(r[0]), c(r[1]), c(r[2]), c(r[3]))).to_path(0.1)
}

// ------------------------------------------------------------------ chat

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// "user" (right, accent bubble) or "assistant" (left, surface bubble); any other name shows as a left bubble with that name.
    #[serde(default = "user")]
    pub from: String,
    pub text: String,
    /// Arrival time (s); default: after the previous message.
    #[serde(default)]
    pub at: Option<f32>,
}
fn user() -> String {
    "user".into()
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub font_size: f32,
    /// Assistant replies stream in word by word at this many words/s (0 = whole).
    pub stream: f32,
    /// Typing-dots duration before each assistant message (s).
    pub thinking: f32,
    /// Gap between automatic message times (s).
    pub gap: f32,
    pub delay: f32,
    /// Max bubble width as a fraction of the box.
    pub max_width: f32,
    /// "bubbles" (messenger, default) or "plain" (assistant replies as
    /// plain text beside a small mark, like an AI chat app).
    pub style: String,
    /// User bubble colour (default: theme accent).
    pub user_color: Option<Color>,
    /// Bubble corner radius (default: from the font size).
    pub radius: Option<f32>,
    pub motion: Motion,
    pub face: Option<Face>,
    /// Bubble finish for other speakers: solid, glass, outline, flat, paper.
    pub surface: Surface,
    #[serde(skip)]
    times: Vec<f32>,
}
impl Default for Chat {
    fn default() -> Self {
        Chat { messages: Vec::new(), font_size: 30.0, stream: 12.0, thinking: 0.7, gap: 0.5, delay: 0.0, max_width: 0.72, style: "bubbles".into(), user_color: None, radius: None, motion: Motion::Smooth, face: None, surface: Surface::Flat, times: Vec::new() }
    }
}
impl Chat {
    fn is_user(m: &Message) -> bool {
        m.from == "user" || m.from == "me"
    }
}
impl Prepare for Chat {
    fn prepare(&mut self) -> Result<(), Error> {
        if !["bubbles", "plain"].contains(&self.style.as_str()) {
            return Err(Error(format!("chat style must be bubbles or plain, not {:?}", self.style)));
        }
        let mut t = self.delay;
        for m in &self.messages {
            let start = m.at.unwrap_or(t);
            let think = if Chat::is_user(m) { 0.0 } else { self.thinking };
            let stream = if Chat::is_user(m) || self.stream <= 0.0 { 0.0 } else { m.text.split_whitespace().count() as f32 / self.stream };
            self.times.push(start + think);
            t = start + think + stream + self.gap;
        }
        Ok(())
    }
}
impl Component for Chat {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let pad = fs * 0.75;
        let maxw = w * self.max_width.clamp(0.3, 1.0);
        let plain = self.style == "plain";
        let ucol = self.user_color.map(|c| c.0).unwrap_or(th.accent);
        let ufill = [shade(ucol, 0.06), shade(ucol, -0.06)];
        let rad = self.radius.unwrap_or(fs * 0.85);
        let other_fill = if th.dark { th.surface2 } else { shade(th.surface2, -0.02) };
        let m = self.motion;
        // Lay out the visible bubbles bottom-up so the thread scrolls like a real chat.
        struct B {
            ops: Vec<Op>,
            h: f32,
            p: f32,
            mine: bool,
        }
        let mut bubbles: Vec<B> = Vec::new();
        let mark = fs * 1.15;
        for (i, msg) in self.messages.iter().enumerate() {
            let at = self.times[i];
            let mine = Chat::is_user(msg);
            let think_start = at - if mine { 0.0 } else { self.thinking };
            if t < think_start {
                break;
            }
            let ink = if mine { on_color(&ufill) } else { th.text };
            let st = face_style(self.face, fs, 400, ink);
            let indent = if plain && !mine { mark + fs * 0.6 } else { 0.0 };
            let mut o = Vec::new();
            if !mine && plain {
                // Assistant mark: a small accent orb.
                o.push(circle(mark * 0.5, mark * 0.5, mark * 0.5, Paint::Linear { p0: [0.0, 0.0], p1: [mark, mark], stops: vec![(0.0, shade(th.accent, 0.2)), (1.0, shade(th.accent, -0.2))] }));
                polish::bevel(&mut o, 0.0, 0.0, mark, mark, mark * 0.5, 1.2);
            }
            if t < at {
                // Typing indicator: three dots in a travelling wave.
                let (bw, bh) = (fs * 3.4, fs * 2.1);
                let x0 = indent;
                if !plain {
                    o.push(Op::Fill { path: rrect4(x0, 0.0, bw, bh, [rad, rad, rad, rad * 0.3]), paint: solid(other_fill), rule: crate::dl::FillRule::NonZero, transform: IDENTITY });
                }
                for k in 0..3 {
                    let ph = (((t - think_start) * 1.6 - k as f32 * 0.16).rem_euclid(1.0) * std::f32::consts::TAU).sin().max(0.0);
                    o.push(circle(x0 + bw * (0.3 + 0.2 * k as f32), bh * 0.5 - ph * fs * 0.16, fs * 0.15, solid(alpha(th.muted, 0.45 + 0.55 * ph))));
                }
                bubbles.push(B { ops: o, h: bh, p: m.enter(t - think_start), mine });
                continue;
            }
            // Streamed words: the newest word fades in.
            let words: Vec<&str> = msg.text.split_whitespace().collect();
            let (shown, frac) = if mine || self.stream <= 0.0 {
                (words.len(), 1.0)
            } else {
                let f = (t - at) * self.stream;
                ((f.floor() as usize + 1).min(words.len()), if (f.floor() as usize + 1) >= words.len() { 1.0 } else { clamp01(f.fract() * 2.5) })
            };
            let full = wrap(ctx.assets, &msg.text, &st, maxw - 2.0 * pad - indent);
            let bw = full.iter().map(|l| ctx.assets.measure(l, &st)).fold(0.0, f32::max) + 2.0 * pad;
            let lh = fs * 1.42;
            let bh = full.len() as f32 * lh + pad * 1.1;
            let x0 = if mine { w - bw } else { indent };
            if mine {
                polish::tinted_shadow(&mut o, x0, 0.0, bw, bh, rad, fs * 0.2, fs * 0.5, alpha(shade(ucol, -0.4), 0.35));
                o.push(Op::Fill { path: rrect4(x0, 0.0, bw, bh, [rad, rad, rad * 0.3, rad]), paint: vgrad(0.0, bh, ufill[0], ufill[1]), rule: crate::dl::FillRule::NonZero, transform: IDENTITY });
                polish::bevel(&mut o, x0, 0.0, bw, bh, rad, 1.0);
            } else if !plain {
                match self.surface {
                    Surface::Flat => o.push(Op::Fill { path: rrect4(x0, 0.0, bw, bh, [rad, rad, rad, rad * 0.3]), paint: solid(other_fill), rule: crate::dl::FillRule::NonZero, transform: IDENTITY }),
                    s => finish(&mut o, th, s, x0, 0.0, bw, bh, rad, 0.6, None),
                }
            }
            // Lay out the shown words on the final line breaks so text never re-wraps.
            let mut left = shown;
            let tx = if plain && !mine { x0 } else { x0 + pad };
            for (k, l) in full.iter().enumerate() {
                if left == 0 {
                    break;
                }
                let lw: Vec<&str> = l.split(' ').collect();
                let take = lw.len().min(left);
                left -= take;
                let by = pad * 0.55 + lh * k as f32 + fs * 1.0;
                let done = lw[..take.saturating_sub(1)].join(" ");
                let mut x = tx;
                if !done.is_empty() {
                    x += draw(&mut o, ctx.assets, &done, &st, x, by, 0.0) + ctx.assets.measure(" ", &st);
                }
                if take > 0 {
                    let last = lw[take - 1];
                    let a = if left == 0 { frac } else { 1.0 };
                    draw(&mut o, ctx.assets, last, &st.clone().color(alpha(ink, a)), x, by, 0.0);
                }
            }
            if !mine && !matches!(msg.from.as_str(), "assistant" | "ai" | "bot") {
                draw(&mut o, ctx.assets, &msg.from, &ui(fs * 0.6, 600, th.muted), x0 + pad * 0.3, -fs * 0.4, 0.0);
            }
            bubbles.push(B { ops: o, h: bh, p: m.enter(t - if mine { at } else { think_start }), mine });
        }
        let gap = fs * 0.65;
        let mut y = h;
        let mut out = Vec::new();
        for b in bubbles.into_iter().rev() {
            // Height animates with the entrance so earlier bubbles glide up.
            y -= (b.h + gap) * b.p.min(1.0);
            if y + b.h < 0.0 {
                break;
            }
            let mut ops = b.ops;
            let px = if b.mine { w } else { 0.0 };
            transform_ops(&mut ops, &crate::dl::mul(&translate(0.0, y), &scale_about(lerp(0.9, 1.0, b.p.min(1.0)), px, b.h)));
            out.extend(rise(ops, b.p, fs * 0.6 * m.travel()));
        }
        out
    }
    fn duration(&self) -> Option<f32> {
        let last = self.messages.len().checked_sub(1)?;
        let words = self.messages[last].text.split_whitespace().count() as f32;
        Some(self.times[last] + if self.stream > 0.0 { words / self.stream } else { 0.0 } + 0.4)
    }
}

// ------------------------------------------------------------------ prompt

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Prompt {
    /// The text typed into the box.
    pub text: String,
    pub placeholder: String,
    /// Characters per second.
    pub typing: f32,
    pub delay: f32,
    /// When the send button is pressed (default: just after typing ends).
    pub send_at: Option<f32>,
    pub font_size: f32,
    /// Animated gradient rim in the accent colour (the "AI" look). Off by default.
    pub glow: bool,
    /// Rim colours when `glow` is on (default: shades of the accent).
    pub colors: Vec<Color>,
    pub surface: Surface,
    /// Corner radius (default: pill).
    pub radius: Option<f32>,
    pub face: Option<Face>,
    pub motion: Motion,
    #[serde(skip)]
    end: f32,
}
impl Default for Prompt {
    fn default() -> Self {
        Prompt { text: String::new(), placeholder: "Ask anything…".into(), typing: 30.0, delay: 0.0, send_at: None, font_size: 30.0, glow: false, colors: Vec::new(), surface: Surface::Solid, radius: None, face: None, motion: Motion::Smooth, end: 0.0 }
    }
}
impl Prepare for Prompt {
    fn prepare(&mut self) -> Result<(), Error> {
        self.end = self.delay + self.text.chars().count() as f32 / self.typing.max(1.0);
        Ok(())
    }
}
impl Component for Prompt {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let fs = self.font_size;
        let (w, h) = (ctx.width.min(4000.0), (fs * 3.2).min(ctx.height));
        let r = self.radius.unwrap_or(h * 0.5).min(h * 0.5);
        let send = self.send_at.unwrap_or(self.end + 0.35);
        let mut o = Vec::new();
        if self.glow {
            let cols: Vec<Rgba> = if self.colors.is_empty() { vec![th.accent, shade(th.accent, 0.5), shade(th.accent, -0.2)] } else { self.colors.iter().map(|c| c.0).collect() };
            let n = (cols.len() - 1).max(1) as f32;
            let stops: Vec<(f32, Rgba)> = cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect();
            let a = t * 1.2;
            let (c, s) = (a.cos(), a.sin());
            let paint = Paint::Linear { p0: [w * 0.5 - c * w * 0.5, h * 0.5 - s * h * 2.0], p1: [w * 0.5 + c * w * 0.5, h * 0.5 + s * h * 2.0], stops };
            polish::bloom(&mut o, vec![rect(-2.0, -2.0, w + 4.0, h + 4.0, r + 2.0, paint.clone())], 18.0, 0.45);
            o.push(rect(-1.5, -1.5, w + 3.0, h + 3.0, r + 1.5, paint));
            o.push(rect(0.0, 0.0, w, h, r, vgrad(0.0, h, shade(th.surface, 0.04), th.surface)));
            polish::bevel(&mut o, 0.0, 0.0, w, h, r, 1.0);
        } else {
            finish(&mut o, th, self.surface, 0.0, 0.0, w, h, r, 1.0, None);
        }
        let n = self.text.chars().count();
        let k = ((t - self.delay) * self.typing).floor().clamp(0.0, n as f32) as usize;
        let sent = t >= send;
        let typed: String = if sent { String::new() } else { self.text.chars().take(k).collect() };
        let st = face_style(self.face, fs, 400, th.text);
        let x0 = r.max(h * 0.35) * 0.9 + fs * 0.2;
        let base = h * 0.5 + fs * 0.36;
        // Keep the caret in view: scroll long text left.
        let maxw = w - x0 - h * 1.3;
        let tw = ctx.assets.measure(&typed, &st);
        let off = (tw - maxw).max(0.0);
        push_clip(&mut o, x0, 0.0, maxw + 4.0, h, 0.0);
        if typed.is_empty() {
            draw(&mut o, ctx.assets, &self.placeholder, &st.clone().color(alpha(th.muted, 0.75)), x0, base, 0.0);
        } else {
            draw(&mut o, ctx.assets, &typed, &st, x0 - off, base, 0.0);
        }
        // Smooth caret: solid while typing, soft fade blink when idle.
        let blink = if t < self.end && t > self.delay { 1.0 } else { smoothstep01(0.5 + 0.5 * (t * std::f32::consts::TAU / 1.1).cos()) };
        if !sent {
            o.push(rect(x0 - off + tw + 2.0, h * 0.5 - fs * 0.62, (fs * 0.08).max(2.0), fs * 1.24, 1.0, solid(alpha(th.accent, blink))));
        }
        o.push(Op::PopLayer);
        // Send button: fills when there is text, pops when pressed.
        let bs = h * 0.64;
        let (bx, by) = (w - (h - bs) * 0.5 - bs, (h - bs) * 0.5);
        let ready = if sent { 1.0 - ease_out(seg(t, send + 0.3, 0.3)) } else { ease_out(clamp01(k as f32)) };
        let pd = t - send;
        let press = if pd < 0.0 { 1.0 } else if pd < 0.08 { 1.0 - 0.14 * ease_out(pd / 0.08) } else { 0.86 + 0.14 * spring_z(pd - 0.08, 1.2, 0.5) };
        let (cx, cy) = (bx + bs * 0.5, by + bs * 0.5);
        let fillb = mix(th.surface2, th.accent, ready);
        let mut b = vec![circle(cx, cy, bs * 0.5, vgrad(by, by + bs, shade(fillb, 0.08), shade(fillb, -0.06)))];
        let mut arrow = Path::new();
        arrow.move_to((cx as f64, (cy + bs * 0.2) as f64));
        arrow.line_to((cx as f64, (cy - bs * 0.2) as f64));
        arrow.move_to(((cx - bs * 0.17) as f64, (cy - bs * 0.03) as f64));
        arrow.line_to((cx as f64, (cy - bs * 0.2) as f64));
        arrow.line_to(((cx + bs * 0.17) as f64, (cy - bs * 0.03) as f64));
        let ink = mix(th.muted, on_color(&[th.accent]), ready);
        let fly = if pd > 0.0 { ease_out_expo(seg(pd, 0.05, 0.4)) } else { 0.0 };
        let mut ar = vec![stroke_op(arrow, solid(alpha(ink, 1.0 - fly)), bs * 0.075, [0.0, 1.0])];
        transform_ops(&mut ar, &translate(0.0, -fly * bs * 0.5));
        b.extend(ar);
        transform_ops(&mut b, &scale_about(press, cx, cy));
        o.extend(b);
        let p = self.motion.enter(t);
        transform_ops(&mut o, &crate::dl::mul(&translate(0.0, (1.0 - p) * 16.0 * self.motion.travel()), &scale_about(lerp(0.96, 1.0, p), w * 0.5, h * 0.5)));
        with_opacity(o, clamp01(p * 2.0))
    }
    fn duration(&self) -> Option<f32> {
        Some(self.send_at.unwrap_or(self.end + 0.35) + 0.4)
    }
}

fn smoothstep01(x: f32) -> f32 {
    polish::smoothstep(0.0, 1.0, x)
}

// ------------------------------------------------------------------ checklist

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Checklist {
    pub items: Vec<String>,
    /// Seconds between items appearing.
    pub stagger: f32,
    /// Each item is checked this long after it appears (negative = never).
    pub check_after: f32,
    pub font_size: f32,
    pub color: Option<Color>,
    pub delay: f32,
    /// Strike through checked items (to-do style) instead of highlighting them.
    pub strike: bool,
    /// Horizontal placement of the list in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
    /// Box shape: "square" (default), "circle" or "none" (just a tick).
    pub shape: String,
    /// Each row on its own surface (solid, glass, outline, flat, paper); default none.
    pub surface: Option<Surface>,
    pub motion: Motion,
    pub face: Option<Face>,
    /// Row spacing multiplier.
    pub spacing: f32,
}
impl Default for Checklist {
    fn default() -> Self {
        Checklist { items: Vec::new(), stagger: 0.35, check_after: 0.25, font_size: 36.0, color: None, delay: 0.0, strike: false, align: 0.5, shape: "square".into(), surface: None, motion: Motion::Smooth, face: None, spacing: 1.0 }
    }
}
impl Prepare for Checklist {}
impl Component for Checklist {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let c = self.color.map(|c| c.0).unwrap_or(th.positive);
        let fs = self.font_size;
        let rowed = self.surface.is_some();
        let lh = fs * if rowed { 2.4 } else { 1.9 } * self.spacing.max(0.5);
        let s = fs * 1.1;
        let st0 = face_style(self.face, fs, 500, th.text);
        let text_w = self.items.iter().map(|it| ctx.assets.measure(it, &st0)).fold(0.0, f32::max);
        let rpad = if rowed { fs * 0.6 } else { 0.0 };
        let bw = rpad * 2.0 + s + fs * 0.6 + text_w;
        let mut out = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            let t0 = self.delay + i as f32 * self.stagger * self.motion.pace();
            let p = self.motion.enter(t - t0);
            if p <= 0.0 {
                break;
            }
            let y = i as f32 * lh;
            let k = if self.check_after >= 0.0 { ease_out(seg(t, t0 + self.check_after, 0.3)) } else { 0.0 };
            let pop = if self.check_after >= 0.0 { spring_z(t - t0 - self.check_after, 1.3, 0.45) } else { 0.0 };
            let mut o = Vec::new();
            if let Some(sf) = self.surface {
                finish(&mut o, th, sf, 0.0, y, bw, lh - fs * 0.45, fs * 0.5, 0.6, None);
            }
            let by = y + (lh - if rowed { fs * 0.45 } else { 0.0 }) * 0.5 - s * 0.5;
            let bx = rpad;
            let r = match self.shape.as_str() {
                "circle" => s * 0.5,
                _ => s * 0.28,
            };
            let mut bx_ops = Vec::new();
            if self.shape != "none" {
                if k > 0.0 {
                    polish::tinted_shadow(&mut bx_ops, bx, by, s, s, r, s * 0.1, s * 0.3, alpha(c, 0.35 * k));
                }
                bx_ops.push(rect(bx, by, s, s, r, solid(mix(alpha(th.text, 0.06), c, k))));
                if k < 1.0 {
                    border(&mut bx_ops, bx, by, s, s, r, alpha(th.text, 0.28 * (1.0 - k)));
                }
                if k > 0.0 {
                    polish::bevel(&mut bx_ops, bx, by, s, s, r, k);
                }
            }
            if k > 0.0 {
                let ink = if self.shape == "none" { c } else { on_color(&[c]) };
                bx_ops.push(stroke_op(polish::check_path(bx, by, s), solid(ink), s * 0.11, [0.0, k]));
            }
            transform_ops(&mut bx_ops, &scale_about(lerp(0.8, 1.0, pop.max(1.0 - k)), bx + s * 0.5, by + s * 0.5));
            o.extend(bx_ops);
            let tc = if self.strike { mix(th.text, th.muted, k) } else { th.text };
            let st = st0.clone().color(tc);
            let tx = bx + s + fs * 0.6;
            let tw = draw(&mut o, ctx.assets, item, &st, tx, by + s * 0.5 + fs * 0.36, 0.0);
            if self.strike && k > 0.0 {
                o.push(rect(tx, by + s * 0.5 - fs * 0.02, tw * k, (fs * 0.07).max(1.5), 1.0, solid(th.muted)));
            }
            out.extend(rise(o, p, fs * 0.6 * self.motion.travel()));
        }
        let bh = (self.items.len() as f32 - 1.0).max(0.0) * lh + if rowed { lh - fs * 0.45 } else { s };
        transform_ops(&mut out, &translate((ctx.width - bw) * self.align, (ctx.height - bh).max(0.0) * 0.5));
        out
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.items.len() as f32 * self.stagger * self.motion.pace() + self.check_after.max(0.0) + 0.5)
    }
}

// ------------------------------------------------------------------ quote

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Quote {
    pub text: String,
    pub name: String,
    pub title: String,
    /// Avatar image src (initials otherwise).
    pub avatar: Option<String>,
    pub font_size: f32,
    /// Stars out of 5 (0 = none).
    pub stars: u8,
    pub delay: f32,
    /// Words revealed per second (0 = whole quote at once).
    pub reveal: f32,
    /// Quote face: display (default), sans, serif (editorial), mono.
    pub face: Option<Face>,
    /// Big opening quote mark.
    pub mark: bool,
    /// Mark / star colour (default: theme accent / gold).
    pub color: Option<Color>,
    /// 0 left (default), 0.5 centred.
    pub align: f32,
    pub motion: Motion,
}
impl Default for Quote {
    fn default() -> Self {
        Quote { text: String::new(), name: String::new(), title: String::new(), avatar: None, font_size: 44.0, stars: 0, delay: 0.0, reveal: 10.0, face: None, mark: true, color: None, align: 0.0, motion: Motion::Smooth }
    }
}
impl Prepare for Quote {}
impl Component for Quote {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let w = ctx.width;
        let fs = self.font_size;
        let t = t - self.delay;
        if t <= 0.0 {
            return Vec::new();
        }
        let al = self.align.clamp(0.0, 1.0);
        let ax = w * al;
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let mut o = Vec::new();
        let mut y = 0.0;
        if self.mark {
            let qp = self.motion.enter(t);
            let mut q = Vec::new();
            let qs = TextStyle::new(Face::Serif, fs * 3.6, 400, acc);
            draw(&mut q, ctx.assets, "\u{201C}", &qs, ax - fs * 0.1 * (1.0 - al * 2.0), fs * 2.7, al);
            transform_ops(&mut q, &translate(0.0, (1.0 - qp) * fs * 0.5));
            o.extend(with_opacity(q, clamp01(qp) * 0.9));
            y = fs * 2.3;
        }
        if self.stars > 0 {
            let n = self.stars.min(5) as f32;
            let sw = n * fs * 0.95;
            let sx = ax - sw * al;
            let gold = self.color.map(|c| c.0).unwrap_or(parse_color("#ffc53d").unwrap());
            for i in 0..self.stars.min(5) {
                let p = spring_z(t - 0.15 - i as f32 * 0.06, 1.3, 0.45);
                if p > 0.0 {
                    let cx = sx + fs * 0.45 + i as f32 * fs * 0.95;
                    let mut s = vec![star(cx, y - fs * 0.3, fs * 0.4, gold)];
                    transform_ops(&mut s, &scale_about(p, cx, y - fs * 0.3));
                    o.extend(s);
                }
            }
            y += fs * 1.1;
        }
        let st = match self.face {
            None => headline(fs, 500, th.text),
            f => face_style(f, fs, 500, th.text),
        };
        let lines = wrap(ctx.assets, &self.text, &st, w);
        let mut word_i = 0usize;
        let space = ctx.assets.measure(" ", &st);
        for l in &lines {
            y += fs * 1.28;
            let lw = ctx.assets.measure(l, &st);
            let mut x = ax - lw * al;
            for word in l.split(' ') {
                let start = if self.reveal > 0.0 { 0.3 + word_i as f32 / self.reveal } else { 0.3 };
                let p = self.motion.ease(seg(t, start, 0.6));
                let mut e = Vec::new();
                let adv = draw(&mut e, ctx.assets, word, &st, x, y, 0.0);
                transform_ops(&mut e, &translate(0.0, (1.0 - p) * fs * 0.3));
                o.extend(polish::blurred(e, (1.0 - p) * fs * 0.08, clamp01(p * 1.2)));
                x += adv + space;
                word_i += 1;
            }
        }
        let words = self.text.split_whitespace().count() as f32;
        let ap = self.motion.enter(t - 0.3 - if self.reveal > 0.0 { words / self.reveal } else { 0.3 });
        y += fs * 1.3;
        let s = fs * 1.7;
        let mut a = Vec::new();
        if !self.name.is_empty() {
            let ns = ui(fs * 0.5, 600, th.text);
            let tsy = ui(fs * 0.44, 400, th.muted);
            let tw = ctx.assets.measure(&self.name, &ns).max(ctx.assets.measure(&self.title, &tsy));
            let bwid = s + fs * 0.5 + tw;
            let x0 = ax - bwid * al;
            let h = fnv(&[self.name.as_bytes()]) as usize;
            let (cx, cy) = (x0 + s * 0.5, y + s * 0.5);
            if let Some((id, iw, ih)) = self.avatar.as_deref().and_then(|src| ctx.assets.media(src, 0.0)) {
                let k = s / iw.min(ih) as f32;
                a.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((circle_path(cx, cy, s * 0.5), IDENTITY)), cache_key: None });
                a.push(Op::Image { image: id, dst: [cx - iw as f32 * k * 0.5, cy - ih as f32 * k * 0.5, iw as f32 * k, ih as f32 * k], opacity: 1.0, transform: IDENTITY });
                a.push(Op::PopLayer);
            } else {
                let (c1, c2) = (shade(th.pal(h), 0.1), shade(th.pal(h / 7 + 1), -0.2));
                a.push(circle(cx, cy, s * 0.5, Paint::Linear { p0: [x0, y], p1: [x0 + s, y + s], stops: vec![(0.0, c1), (1.0, c2)] }));
                let ini: String = self.name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect();
                draw(&mut a, ctx.assets, &ini.to_uppercase(), &headline(s * 0.38, 600, on_color(&[c1, c2])), cx, cy + s * 0.135, 0.5);
            }
            polish::bevel(&mut a, x0, y, s, s, s * 0.5, 1.0);
            draw(&mut a, ctx.assets, &self.name, &ns, x0 + s + fs * 0.5, y + s * 0.47, 0.0);
            draw(&mut a, ctx.assets, &self.title, &tsy, x0 + s + fs * 0.5, y + s * 0.47 + fs * 0.64, 0.0);
        }
        o.extend(rise(a, ap, fs * 0.4));
        o
    }
    fn duration(&self) -> Option<f32> {
        let words = self.text.split_whitespace().count() as f32;
        Some(self.delay + 0.8 + if self.reveal > 0.0 { words / self.reveal } else { 0.3 } + 0.6)
    }
}

/// Five-point star with softly rounded points.
fn star(cx: f32, cy: f32, r: f32, c: Rgba) -> Op {
    let mut p = Path::new();
    for i in 0..10 {
        let a = i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
        let rr = if i % 2 == 0 { r } else { r * 0.47 };
        let pt = ((cx + a.cos() * rr) as f64, (cy + a.sin() * rr) as f64);
        if i == 0 {
            p.move_to(pt);
        } else {
            p.line_to(pt);
        }
    }
    p.close_path();
    fill(p, solid(c))
}

// ------------------------------------------------------------------ table

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Table {
    pub columns: Vec<String>,
    /// Rows of cells; `"✓"`/`"✗"` (or `true`/`false`) render as check/cross icons.
    pub rows: Vec<Vec<serde_json::Value>>,
    /// Column to emphasise (e.g. "us" in a comparison), 0-based.
    pub highlight: Option<usize>,
    pub font_size: f32,
    pub delay: f32,
    pub stagger: f32,
    /// Highlight colour (default: theme accent).
    pub color: Option<Color>,
    /// Table finish: none (default, rules only), solid, glass, outline, flat, paper.
    pub surface: Option<Surface>,
    /// Alternate row tint.
    pub zebra: bool,
    /// Small tag above the highlighted column, e.g. "Recommended".
    pub tag: String,
    pub motion: Motion,
    pub face: Option<Face>,
}
impl Default for Table {
    fn default() -> Self {
        Table { columns: Vec::new(), rows: Vec::new(), highlight: None, font_size: 28.0, delay: 0.0, stagger: 0.12, color: None, surface: None, zebra: false, tag: String::new(), motion: Motion::Smooth, face: None }
    }
}
impl Prepare for Table {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.columns.is_empty() {
            return Err(Error("table needs `columns`".into()));
        }
        Ok(())
    }
}
impl Component for Table {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let w = ctx.width;
        let fs = self.font_size;
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let nc = self.columns.len();
        let first = w * if nc > 2 { 0.34 } else { 0.5 };
        let cw = (w - first) / (nc - 1).max(1) as f32;
        let col_x = |c: usize| if c == 0 { 0.0 } else { first + (c - 1) as f32 * cw };
        let col_w = |c: usize| if c == 0 { first } else { cw };
        let rh = fs * 2.4;
        let rows = self.rows.len();
        let total_h = rh * (rows + 1) as f32;
        let m = self.motion;
        let mut o = Vec::new();
        if let Some(sf) = self.surface {
            let p = m.enter(t - self.delay);
            let mut s = Vec::new();
            finish(&mut s, th, sf, 0.0, 0.0, w, total_h, th.radius, 1.0, None);
            o.extend(with_opacity(s, clamp01(p * 1.5)));
        }
        if let Some(hc) = self.highlight.filter(|c| *c < nc) {
            let p = m.enter(t - self.delay - 0.15);
            let (hx, hw) = (col_x(hc) + 6.0, col_w(hc) - 12.0);
            let ext = fs * 0.35;
            let mut hl = Vec::new();
            polish::tinted_shadow(&mut hl, hx, -ext, hw, total_h + 2.0 * ext, th.radius * 0.7, fs * 0.4, fs * 1.2, alpha(acc, 0.25));
            hl.push(rect(hx, -ext, hw, total_h + 2.0 * ext, th.radius * 0.7, vgrad(-ext, total_h, mix(th.surface, acc, 0.16), mix(th.surface, acc, 0.07))));
            border(&mut hl, hx, -ext, hw, total_h + 2.0 * ext, th.radius * 0.7, alpha(acc, 0.55));
            polish::bevel(&mut hl, hx, -ext, hw, total_h + 2.0 * ext, th.radius * 0.7, 1.0);
            if !self.tag.is_empty() {
                let ts = polish::eyebrow(fs * 0.5, 700, on_color(&[acc]));
                let tw = ctx.assets.measure(&self.tag.to_uppercase(), &ts) + fs;
                let (tx, ty) = (hx + hw * 0.5 - tw * 0.5, -ext - fs * 0.55);
                hl.push(rect(tx, ty, tw, fs * 1.1, fs * 0.55, solid(acc)));
                draw(&mut hl, ctx.assets, &self.tag.to_uppercase(), &ts, hx + hw * 0.5, ty + fs * 0.55 + ts.size * 0.36, 0.5);
            }
            transform_ops(&mut hl, &scale_about(lerp(0.96, 1.0, p), hx + hw * 0.5, total_h * 0.5));
            o.extend(with_opacity(hl, clamp01(p * 1.4)));
        }
        let hp = m.enter(t - self.delay);
        let mut hdr = Vec::new();
        for (c, name) in self.columns.iter().enumerate() {
            let hl = self.highlight == Some(c);
            let st = if c == 0 { polish::eyebrow(fs * 0.62, 600, th.muted) } else { face_style(self.face, fs * 0.85, 650, if hl { th.text } else { th.muted }) };
            let (x, al) = if c == 0 { (fs * 0.6, 0.0) } else { (col_x(c) + col_w(c) * 0.5, 0.5) };
            let label = if c == 0 { name.to_uppercase() } else { name.clone() };
            draw(&mut hdr, ctx.assets, &label, &st, x, rh * 0.6, al);
        }
        o.extend(rise(hdr, hp, 8.0));
        for (r, row) in self.rows.iter().enumerate() {
            let rt = self.delay + 0.2 + r as f32 * self.stagger * m.pace();
            let p = m.enter(t - rt);
            if p <= 0.0 {
                break;
            }
            let y = rh * (r + 1) as f32;
            let mut e = Vec::new();
            if self.zebra && r % 2 == 0 {
                e.push(rect(0.0, y, w, rh, fs * 0.3, solid(alpha(th.text, 0.035))));
            }
            // Hairline that draws across with the row.
            e.push(rect(fs * 0.4, y, (w - fs * 0.8) * ease_out_expo(seg(t, rt, 0.6)), 1.0, 0.0, solid(th.border)));
            for (c, cell) in row.iter().enumerate().take(nc) {
                let cy = y + rh * 0.5;
                let cx = if c == 0 { fs * 0.6 } else { col_x(c) + col_w(c) * 0.5 };
                let tick = match cell {
                    serde_json::Value::Bool(b) => Some(*b),
                    serde_json::Value::String(s) if s == "✓" || s == "yes" => Some(true),
                    serde_json::Value::String(s) if s == "✗" || s == "×" || s == "no" => Some(false),
                    _ => None,
                };
                let cp = spring_z(t - rt - 0.08 * c as f32, 1.3, 0.5);
                match tick {
                    Some(true) => {
                        let hl = self.highlight == Some(c);
                        let tc = if hl { acc } else { th.positive };
                        let mut g = vec![circle(cx, cy, fs * 0.52, solid(alpha(tc, if hl { 1.0 } else { 0.16 })))];
                        g.push(stroke_op(polish::check_path(cx - fs * 0.52, cy - fs * 0.52, fs * 1.04), solid(if hl { on_color(&[acc]) } else { tc }), fs * 0.1, [0.0, ease_out(seg(t, rt + 0.1, 0.35))]));
                        transform_ops(&mut g, &scale_about(lerp(0.5, 1.0, cp), cx, cy));
                        e.extend(g);
                    }
                    Some(false) => {
                        let mut pth = Path::new();
                        let k = fs * 0.15;
                        pth.move_to(((cx - k) as f64, (cy - k) as f64));
                        pth.line_to(((cx + k) as f64, (cy + k) as f64));
                        pth.move_to(((cx + k) as f64, (cy - k) as f64));
                        pth.line_to(((cx - k) as f64, (cy + k) as f64));
                        e.push(stroke_op(pth, solid(alpha(th.muted, 0.6)), fs * 0.075, [0.0, 1.0]));
                    }
                    None => {
                        let s = match cell {
                            serde_json::Value::String(s) => s.clone(),
                            v => v.to_string(),
                        };
                        let hl = self.highlight == Some(c);
                        let st = face_style(self.face, fs, if c == 0 { 500 } else if hl { 650 } else { 400 }, if c == 0 || hl { th.text } else { th.muted }).tabular();
                        draw(&mut e, ctx.assets, &s, &st, cx, cy + fs * 0.36, if c == 0 { 0.0 } else { 0.5 });
                    }
                }
            }
            transform_ops(&mut e, &translate(0.0, (1.0 - p) * fs * 0.5 * m.travel()));
            o.extend(with_opacity(e, clamp01(p * 1.4)));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 0.8 + self.rows.len() as f32 * self.stagger * self.motion.pace())
    }
}

// ------------------------------------------------------------------ timeline

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    pub label: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Timeline {
    pub items: Vec<Milestone>,
    /// Index of the "now" milestone (pulses; later ones are dimmed).
    pub current: Option<usize>,
    pub vertical: bool,
    pub font_size: f32,
    pub delay: f32,
    /// Seconds per milestone as the line draws on.
    pub step: f32,
    /// Line / node colour (default: theme accent).
    pub color: Option<Color>,
    /// Node style: "dot" (default), "ring" or "square".
    pub node: String,
    /// Line thickness.
    pub thickness: f32,
    /// Labels all on one side instead of alternating (horizontal).
    pub one_side: bool,
    pub motion: Motion,
    pub face: Option<Face>,
}
impl Default for Timeline {
    fn default() -> Self {
        Timeline { items: Vec::new(), current: None, vertical: false, font_size: 26.0, delay: 0.0, step: 0.45, color: None, node: "dot".into(), thickness: 3.0, one_side: false, motion: Motion::Smooth, face: None }
    }
}
impl Prepare for Timeline {}
impl Component for Timeline {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let n = self.items.len().max(1);
        let fs = self.font_size;
        let acc = self.color.map(|c| c.0).unwrap_or(th.accent);
        let step = self.step * self.motion.pace();
        let k = clamp01((t - self.delay) / (step * n as f32));
        let mut o = Vec::new();
        let pos = |i: usize| -> (f32, f32) {
            let f = (i as f32 + 0.5) / n as f32;
            if self.vertical {
                (fs * 1.2, h * f)
            } else {
                (w * f, h * if self.one_side { 0.3 } else { 0.42 })
            }
        };
        let (a, b) = (pos(0), pos(n - 1));
        let mut ln = Path::new();
        ln.move_to((a.0 as f64, a.1 as f64));
        ln.line_to((b.0 as f64, b.1 as f64));
        let th_w = self.thickness;
        o.push(stroke_op(ln.clone(), solid(alpha(th.text, 0.1)), th_w, [0.0, 1.0]));
        let kk = ease_in_out(k) * if n > 1 { 1.0 } else { 0.0 };
        let reach = match self.current {
            Some(c) if n > 1 => (c as f32 / (n - 1) as f32).min(1.0),
            _ => 1.0,
        };
        let prog = kk.min(reach);
        if prog > 0.0 {
            let paint = Paint::Linear { p0: [a.0, a.1], p1: [b.0, b.1], stops: vec![(0.0, shade(acc, -0.15)), (1.0, shade(acc, 0.25))] };
            o.push(stroke_op(ln, paint, th_w, [0.0, prog]));
            // Bright head travelling along the line.
            if prog < reach {
                let hx = a.0 + (b.0 - a.0) * prog;
                let hy = a.1 + (b.1 - a.1) * prog;
                polish::bloom(&mut o, vec![circle(hx, hy, th_w * 2.5, solid(shade(acc, 0.4)))], th_w * 2.0, 0.8);
            }
        }
        for (i, ms) in self.items.iter().enumerate() {
            let ti = self.delay + step * i as f32;
            let p = self.motion.enter(t - ti);
            if p <= 0.0 {
                continue;
            }
            let (x, y) = pos(i);
            let future = self.current.is_some_and(|c| i > c);
            let c = if future { alpha(th.text, 0.35) } else { acc };
            let mut e = Vec::new();
            if self.current == Some(i) {
                if let Some(ph) = pulse(t - ti, 0.8) {
                    e.push(Op::Stroke { path: circle_path(x, y, fs * 0.5 + ease_out(ph) * fs * 0.9), paint: solid(alpha(acc, 0.6 * (1.0 - ph))), stroke: line(1.5, [0.0, 1.0]), transform: IDENTITY });
                }
                e.push(circle(x, y, fs * 0.85, solid(alpha(acc, 0.16))));
            }
            let nr = fs * 0.42;
            let np = spring_z(t - ti, 1.4, 0.5);
            let mut node = vec![circle(x, y, nr + fs * 0.14, solid(th.background))];
            match self.node.as_str() {
                "ring" => node.push(Op::Stroke { path: circle_path(x, y, nr * 0.85), paint: solid(c), stroke: line(fs * 0.12, [0.0, 1.0]), transform: IDENTITY }),
                "square" => node.push(rect(x - nr * 0.85, y - nr * 0.85, nr * 1.7, nr * 1.7, nr * 0.2, solid(c))),
                _ => {
                    node.push(circle(x, y, nr, solid(c)));
                    node.push(circle(x, y, nr * 0.38, solid(if future { th.background } else { on_color(&[acc]) })));
                }
            }
            transform_ops(&mut node, &scale_about(np, x, y));
            e.extend(node);
            let tc = if future { alpha(th.text, 0.5) } else { th.text };
            let ls = polish::eyebrow(fs * 0.62, 650, if future { th.muted } else { acc });
            let tst = face_style(self.face, fs * 1.05, 600, tc);
            let bst = ui(fs * 0.72, 400, th.muted);
            let label = ms.label.to_uppercase();
            if self.vertical {
                let lx = x + fs * 1.4;
                draw(&mut e, ctx.assets, &label, &ls, lx, y - fs * 0.55, 0.0);
                draw(&mut e, ctx.assets, &ms.title, &tst, lx, y + fs * 0.6, 0.0);
                draw(&mut e, ctx.assets, &ms.body, &bst, lx, y + fs * 1.65, 0.0);
            } else {
                let up = !self.one_side && i % 2 == 0;
                let ly = if up { y - fs * 1.35 } else { y + fs * 2.0 };
                draw(&mut e, ctx.assets, &label, &ls, x, ly, 0.5);
                draw(&mut e, ctx.assets, &ms.title, &tst, x, ly + if up { -fs * 1.25 } else { fs * 1.35 }, 0.5);
                if !ms.body.is_empty() {
                    let maxw = w / n as f32 * 0.9;
                    let bl = wrap(ctx.assets, &ms.body, &bst, maxw);
                    let nb = bl.len().min(2);
                    for (j, l) in bl.iter().enumerate().take(2) {
                        let dy = if up { -fs * 2.35 - (nb - 1 - j) as f32 * fs * 0.95 } else { fs * 2.35 + j as f32 * fs * 0.95 };
                        draw(&mut e, ctx.assets, l, &bst, x, ly + dy, 0.5);
                    }
                }
            }
            o.extend(rise(e, p, fs * 0.5 * self.motion.travel()));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.step * self.motion.pace() * self.items.len() as f32 + 0.5)
    }
}

// ------------------------------------------------------------------ marquee

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Marquee {
    /// Text items (or image `src`s when `images` is true).
    pub items: Vec<String>,
    pub images: bool,
    /// Pixels per second (negative scrolls right).
    pub speed: f32,
    pub font_size: f32,
    pub gap: f32,
    /// Fade the left and right edges.
    pub fade: bool,
    /// Separator drawn between text items.
    pub separator: String,
    pub color: Option<Color>,
    /// Separator colour (default: theme muted).
    pub separator_color: Option<Color>,
    pub face: Option<Face>,
    pub weight: u16,
    pub uppercase: bool,
    /// Outlined (stroked) letters instead of filled.
    pub outline: bool,
    /// Each item in a pill chip.
    pub chips: bool,
}
impl Default for Marquee {
    fn default() -> Self {
        Marquee { items: Vec::new(), images: false, speed: 90.0, font_size: 40.0, gap: 80.0, fade: true, separator: "•".into(), color: None, separator_color: None, face: None, weight: 600, uppercase: false, outline: false, chips: false }
    }
}
impl Prepare for Marquee {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.items.is_empty() {
            return Err(Error("marquee needs `items`".into()));
        }
        if self.uppercase {
            for it in &mut self.items {
                if !self.images {
                    *it = it.to_uppercase();
                }
            }
        }
        Ok(())
    }
}
impl Component for Marquee {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height.min(self.font_size * 3.0));
        let c = self.color.map(|c| c.0).unwrap_or(alpha(th.text, 0.8));
        let st = match self.face {
            None => headline(self.font_size, self.weight, c),
            f => face_style(f, self.font_size, self.weight, c),
        };
        let chip_pad = if self.chips { self.font_size * 0.8 } else { 0.0 };
        // One cycle's width.
        let widths: Vec<f32> = self
            .items
            .iter()
            .map(|it| {
                if self.images {
                    ctx.assets.media(it, 0.0).map(|(_, iw, ih)| iw as f32 * (h * 0.6) / ih as f32).unwrap_or(h)
                } else {
                    ctx.assets.measure(it, &st) + chip_pad * 2.0
                }
            })
            .collect();
        let cycle: f32 = widths.iter().map(|w| w + self.gap).sum::<f32>().max(1.0);
        let mut x = -(t * self.speed).rem_euclid(cycle);
        let mut o = Vec::new();
        let mut i = 0usize;
        let base = h * 0.5 + self.font_size * 0.36;
        while x < w {
            let k = i % self.items.len();
            let iw = widths[k];
            if x + iw > -10.0 {
                if self.images {
                    if let Some((id, _, _)) = ctx.assets.media(&self.items[k], 0.0) {
                        o.push(Op::Image { image: id, dst: [x, h * 0.2, iw, h * 0.6], opacity: 0.85, transform: IDENTITY });
                    }
                } else {
                    if self.chips {
                        let ch = self.font_size * 1.9;
                        o.push(rect(x, (h - ch) * 0.5, iw, ch, ch * 0.5, solid(alpha(th.text, 0.06))));
                        border(&mut o, x, (h - ch) * 0.5, iw, ch, ch * 0.5, th.border);
                    }
                    let mut g = Vec::new();
                    draw(&mut g, ctx.assets, &self.items[k], &st, x + chip_pad, base, 0.0);
                    if self.outline {
                        for op in &mut g {
                            if let Op::Glyphs { paint, .. } = op {
                                *paint = solid(Rgba([0.0, 0.0, 0.0, 0.0]));
                            }
                        }
                        // Outline via a slightly larger translucent copy under a background-coloured fill.
                        let mut ol = Vec::new();
                        draw(&mut ol, ctx.assets, &self.items[k], &st, x + chip_pad, base, 0.0);
                        let mut inner = ol.clone();
                        for op in &mut inner {
                            if let Op::Glyphs { paint, .. } = op {
                                *paint = solid(th.background);
                            }
                        }
                        let off = (self.font_size * 0.03).max(1.0);
                        for (dx, dy) in [(-off, 0.0), (off, 0.0), (0.0, -off), (0.0, off)] {
                            let mut c2 = ol.clone();
                            transform_ops(&mut c2, &translate(dx, dy));
                            o.extend(c2);
                        }
                        o.extend(inner);
                    } else {
                        o.extend(g);
                    }
                    if !self.separator.is_empty() {
                        let sc = self.separator_color.map(|c| c.0).unwrap_or(alpha(th.muted, 0.8));
                        draw(&mut o, ctx.assets, &self.separator, &st.clone().color(sc).tracking(0.0), x + iw + self.gap * 0.5, base - self.font_size * 0.06, 0.5);
                    }
                }
            }
            x += iw + self.gap;
            i += 1;
            if i > 10_000 {
                break;
            }
        }
        if self.fade {
            // Edge fades painted in the background colour.
            let bg = th.background;
            o.push(rect(0.0, 0.0, w * 0.14, h, 0.0, Paint::Linear { p0: [0.0, 0.0], p1: [w * 0.14, 0.0], stops: vec![(0.0, bg), (1.0, alpha(bg, 0.0))] }));
            o.push(rect(w * 0.86, 0.0, w * 0.14, h, 0.0, Paint::Linear { p0: [w * 0.86, 0.0], p1: [w, 0.0], stops: vec![(0.0, alpha(bg, 0.0)), (1.0, bg)] }));
        }
        o
    }
}

// ------------------------------------------------------------------ spotlight

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Spotlight {
    /// Focus rectangles `[x, y, w, h]` in box px; the spotlight glides between them.
    pub targets: Vec<[f32; 4]>,
    /// Time each target becomes active (default 1.2 s apart from `delay`).
    pub times: Vec<f32>,
    /// Darkness outside the focus (0..1).
    pub dim: f32,
    pub radius: f32,
    /// Padding around each target.
    pub padding: f32,
    /// Ring colour (default: the theme accent).
    pub color: Option<Color>,
    pub delay: f32,
    pub out: Option<f32>,
    /// Soft edge width in px (0 = hard edge).
    pub feather: f32,
    /// Draw a ring around the focus.
    pub ring: bool,
    /// Dim colour (default black).
    pub tint: Option<Color>,
}
impl Default for Spotlight {
    fn default() -> Self {
        Spotlight { targets: vec![[100.0, 100.0, 300.0, 120.0]], times: Vec::new(), dim: 0.6, radius: 16.0, padding: 10.0, color: None, delay: 0.0, out: None, feather: 24.0, ring: true, tint: None }
    }
}
impl Prepare for Spotlight {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.targets.is_empty() {
            return Err(Error("spotlight needs `targets`".into()));
        }
        if self.times.is_empty() {
            self.times = (0..self.targets.len()).map(|i| self.delay + i as f32 * 1.2).collect();
        }
        if self.times.len() != self.targets.len() {
            return Err(Error(format!("spotlight: {} times for {} targets", self.times.len(), self.targets.len())));
        }
        Ok(())
    }
}
impl Component for Spotlight {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let on = ease_out(seg(t, self.times[0], 0.5)) * self.out.map(|o| 1.0 - ease_in_out(seg(t, o, 0.4))).unwrap_or(1.0);
        if on <= 0.0 {
            return Vec::new();
        }
        // Current rect: glide between targets.
        let mut r = self.targets[0];
        for i in 1..self.targets.len() {
            let k = polish::ease_in_out_quint(seg(t, self.times[i] - 0.6, 0.6));
            for j in 0..4 {
                r[j] = lerp(r[j], self.targets[i][j], k);
            }
        }
        let p = self.padding;
        // Opening: the hole starts larger and settles on the target.
        let grow = (1.0 - ease_out_expo(seg(t, self.times[0], 0.8))) * 80.0;
        let (x, y, w, h) = (r[0] - p - grow, r[1] - p - grow, r[2] + 2.0 * (p + grow), r[3] + 2.0 * (p + grow));
        let tint = self.tint.map(|c| c.0).unwrap_or(Rgba([0.0, 0.0, 0.0, 1.0]));
        let mut o = Vec::new();
        // Feathered edge: a few nested even-odd fills with growing holes.
        let steps = if self.feather > 0.5 { 5 } else { 1 };
        for s in 0..steps {
            let f = if steps == 1 { 0.0 } else { s as f32 / (steps - 1) as f32 };
            let e = self.feather * (f - 0.5);
            let mut path = rrect_path(-10.0, -10.0, ctx.width + 20.0, ctx.height + 20.0, 0.0);
            path.extend(rrect_path(x - e, y - e, w + 2.0 * e, h + 2.0 * e, (self.radius + e).max(0.0)).elements().iter().copied());
            o.push(Op::Fill { path, paint: solid(alpha(tint, self.dim * on / steps as f32)), rule: crate::dl::FillRule::EvenOdd, transform: IDENTITY });
        }
        if self.ring {
            let c = self.color.map(|c| c.0).unwrap_or(th.accent);
            o.push(Op::Stroke { path: rrect_path(x, y, w, h, self.radius), paint: solid(alpha(c, 0.9 * on)), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.times.last().copied().unwrap_or(0.0) + 0.6)
    }
}

// ------------------------------------------------------------------ keys

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Keys {
    /// Keycap labels, e.g. `["⌘", "K"]` or `["Ctrl", "Shift", "P"]`.
    pub keys: Vec<String>,
    /// Times the combo is pressed.
    pub press: Vec<f32>,
    pub size: f32,
    pub delay: f32,
    pub out: Option<f32>,
    /// Horizontal placement in the box: 0 left, 0.5 centre (default), 1 right. Vertically centred.
    pub align: f32,
    /// Keycap finish: solid (3D, default), flat, outline, glass, paper.
    pub surface: Surface,
    /// Keycap colour (default: theme surface).
    pub color: Option<Color>,
    /// Show "+" between keys.
    pub plus: bool,
    pub motion: Motion,
}
impl Default for Keys {
    fn default() -> Self {
        Keys { keys: vec!["⌘".into(), "K".into()], press: vec![0.6], size: 64.0, delay: 0.0, out: None, align: 0.5, surface: Surface::Solid, color: None, plus: true, motion: Motion::Bouncy }
    }
}
impl Prepare for Keys {}
impl Component for Keys {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let s = self.size;
        let cap = self.color.map(|c| c.0).unwrap_or(if th.dark { shade(th.surface2, 0.06) } else { th.surface });
        let ink = if self.surface == Surface::Outline || self.surface == Surface::Glass { th.text } else { on_color(&[cap]) };
        let st = ui(s * 0.4, 600, ink);
        let gap = if self.plus { s * 0.6 } else { s * 0.2 };
        let mut x = 0.0;
        let mut out = Vec::new();
        let exit = self.out.map(|o| ease_in(seg(t, o, 0.3))).unwrap_or(0.0);
        for (i, k) in self.keys.iter().enumerate() {
            let p = self.motion.enter(t - self.delay - i as f32 * 0.07);
            if p <= 0.0 {
                break;
            }
            let kw = (ctx.assets.measure(k, &st) + s * 0.6).max(s);
            // All keys go down together, staggered slightly like real fingers.
            let mut down = 0.0f32;
            for &pt in &self.press {
                let d = t - pt - i as f32 * 0.035;
                if d >= 0.0 {
                    let v = if d < 0.07 { ease_out(d / 0.07) } else { 1.0 - spring_z(d - 0.07, 1.1, 0.6) };
                    down = down.max(v);
                }
            }
            let depth = s * 0.1;
            let dy = depth * down;
            let r = s * 0.2;
            let mut e = Vec::new();
            match self.surface {
                Surface::Solid => {
                    shadow(&mut e, x, depth, kw, s, r, s * 0.08 * (1.0 - down * 0.6), s * 0.18, if th.dark { 0.6 } else { 0.18 });
                    e.push(rect(x, depth, kw, s, r, solid(shade(cap, -0.35))));
                    e.push(rect(x, dy, kw, s, r, vgrad(dy, dy + s, shade(cap, 0.1), shade(cap, -0.04))));
                    // Concave top face.
                    e.push(rect(x + s * 0.08, dy + s * 0.05, kw - s * 0.16, s * 0.8, r * 0.8, vgrad(dy, dy + s * 0.85, shade(cap, -0.02), shade(cap, 0.05))));
                    polish::bevel(&mut e, x, dy, kw, s, r, 1.3);
                    border(&mut e, x, dy, kw, s, r, if down > 0.3 { alpha(th.accent, 0.8 * down) } else { Rgba([0.0, 0.0, 0.0, 0.2]) });
                }
                sf => {
                    finish(&mut e, th, sf, x, dy, kw, s, r, 0.5, Some(cap));
                    if down > 0.0 {
                        e.push(rect(x, dy, kw, s, r, solid(alpha(th.accent, 0.25 * down))));
                    }
                }
            }
            draw(&mut e, ctx.assets, k, &st, x + kw * 0.5, dy + s * 0.46 + st.size * 0.36, 0.5);
            if self.plus && i + 1 < self.keys.len() {
                draw(&mut e, ctx.assets, "+", &ui(s * 0.38, 500, th.muted), x + kw + gap * 0.5, s * 0.5 + s * 0.14, 0.5);
            }
            transform_ops(&mut e, &scale_about(lerp(0.7, 1.0, p), x + kw * 0.5, s * 0.5));
            out.extend(rise(e, p, s * 0.25));
            x += kw + gap;
        }
        let (w, h) = ((x - gap).max(0.0), s * 1.1);
        transform_ops(&mut out, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        with_opacity(out, 1.0 - exit)
    }
    fn duration(&self) -> Option<f32> {
        Some(self.press.iter().fold(self.delay + 0.5, |m, p| m.max(p + 0.5)))
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
            ("chat", json!({"x": 30, "y": 20, "width": 440, "height": 330, "messages": [
                {"from": "user", "text": "Make a 30s launch video for our API"},
                {"from": "assistant", "text": "Done. 1080p60, rendered in 4.2 s. Want a vertical cut for social too?"},
                {"from": "user", "text": "Yes please!"}]})),
            ("prompt", json!({"x": 30, "y": 380, "width": 440, "text": "Animate the revenue chart with a spring", "font_size": 18})),
            ("checklist", json!({"x": 520, "y": 30, "items": ["Native renderer", "Parallel encode", "Beat-synced cuts"], "font_size": 22})),
            ("quote", json!({"x": 520, "y": 200, "width": 400, "text": "We replaced our whole browser render farm with one binary.", "name": "Grace Hopper", "title": "CTO, Compilers Inc", "stars": 5, "font_size": 22})),
        ];
        for (name, t) in [("blocks-end", 9.0f32), ("blocks-mid", 2.2)] {
            let mut ops = Vec::new();
            for (k, p) in &items {
                ops.extend(build(k, p).unwrap().eval(t, 960.0, 540.0, 30.0, &th, a));
            }
            preview::validate(&ops).unwrap();
            preview::snapshot(name, 960, 540, th.background, ops, a);
        }
        let items = [
            ("table", json!({"x": 40, "y": 20, "width": 560, "height": 300, "columns": ["", "Anikit", "Browser"], "rows": [["Native render", true, false], ["1080p60 encode", "4 s", "3 min"], ["Agent-first JSON", "✓", "✗"]], "highlight": 1})),
            ("timeline", json!({"x": 40, "y": 300, "width": 880, "height": 220, "items": [{"label": "Q1", "title": "Prototype"}, {"label": "Q2", "title": "Beta", "body": "500 teams"}, {"label": "Q3", "title": "Launch"}, {"label": "Q4", "title": "Scale"}], "current": 2})),
            ("marquee", json!({"x": 620, "y": 120, "width": 320, "height": 80, "items": ["Stripe", "Linear", "Vercel", "Figma"], "font_size": 26})),
            ("keys", json!({"x": 640, "y": 220, "width": 300, "height": 60, "align": 0, "keys": ["⌘", "Shift", "K"], "press": [2.9], "size": 52})),
            ("spotlight", json!({"targets": [[230, 60, 180, 220]], "times": [2.0]})),
        ];
        let mut ops = Vec::new();
        for (k, p) in &items {
            ops.extend(build(k, p).unwrap().eval(3.0, 960.0, 540.0, 30.0, &th, a));
        }
        preview::validate(&ops).unwrap();
        preview::snapshot("blocks-2", 960, 540, th.background, ops, a);
    }
}
