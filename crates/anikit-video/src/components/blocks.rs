//! Content blocks for product and launch videos: chat threads, an AI prompt
//! box, checklists, testimonial quotes, tables, timelines and marquees.

use super::style::*;
use super::text::{draw, wrap, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Error, Prepare};
use crate::dl::{Op, Path, Rgba, IDENTITY};
use serde::Deserialize;

fn sans(size: f32, weight: u16, c: Rgba) -> TextStyle {
    TextStyle::new(Face::Sans, size, weight, c)
}

/// Rise + fade entrance for one item.
fn rise(ops: Vec<Op>, p: f32, dy: f32) -> Vec<Op> {
    if p <= 0.0 {
        return Vec::new();
    }
    let mut ops = ops;
    transform_ops(&mut ops, &translate(0.0, (1.0 - p) * dy));
    with_opacity(ops, clamp01(p * 1.4))
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
    #[serde(skip)]
    times: Vec<f32>,
}
impl Default for Chat {
    fn default() -> Self {
        Chat { messages: Vec::new(), font_size: 30.0, stream: 12.0, thinking: 0.7, gap: 0.5, delay: 0.0, max_width: 0.72, times: Vec::new() }
    }
}
impl Chat {
    fn is_user(m: &Message) -> bool {
        m.from == "user" || m.from == "me"
    }
}
impl Prepare for Chat {
    fn prepare(&mut self) -> Result<(), Error> {
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
        let pad = fs * 0.8;
        let maxw = w * self.max_width.clamp(0.3, 1.0);
        // Lay out the visible bubbles bottom-up so the thread scrolls like a real chat.
        struct B {
            ops: Vec<Op>,
            h: f32,
            p: f32,
        }
        let mut bubbles: Vec<B> = Vec::new();
        for (i, m) in self.messages.iter().enumerate() {
            let at = self.times[i];
            let mine = Chat::is_user(m);
            let think_start = at - if mine { 0.0 } else { self.thinking };
            if t < think_start {
                break;
            }
            let st = sans(fs, 400, if mine { Rgba([1.0, 1.0, 1.0, 1.0]) } else { th.text });
            let mut o = Vec::new();
            if t < at {
                // Typing indicator.
                let (bw, bh) = (fs * 3.6, fs * 2.2);
                o.push(rect(0.0, 0.0, bw, bh, bh * 0.5, solid(th.surface2)));
                for k in 0..3 {
                    let ph = ((t - think_start) * 5.0 - k as f32 * 0.7).sin() * 0.5 + 0.5;
                    o.push(circle(bw * (0.3 + 0.2 * k as f32), bh * 0.5 - ph * fs * 0.15, fs * 0.16, solid(alpha(th.muted, 0.5 + 0.5 * ph))));
                }
                bubbles.push(B { ops: o, h: bh, p: spring(t - think_start, 1.3) });
                continue;
            }
            // Streamed words.
            let words: Vec<&str> = m.text.split_whitespace().collect();
            let shown = if mine || self.stream <= 0.0 { words.len() } else { (((t - at) * self.stream).floor() as usize + 1).min(words.len()) };
            let full = wrap(ctx.assets, &m.text, &st, maxw - 2.0 * pad);
            let bw = full.iter().map(|l| ctx.assets.measure(l, &st)).fold(0.0, f32::max) + 2.0 * pad;
            let partial = words[..shown].join(" ");
            let lines = wrap(ctx.assets, &partial, &st, maxw - 2.0 * pad);
            let lh = fs * 1.4;
            let bh = full.len() as f32 * lh + pad * 1.2;
            let x0 = if mine { w - bw } else { 0.0 };
            let fill_p = if mine { vgrad(0.0, bh, shade(th.accent, 0.1), shade(th.accent, -0.08)) } else { solid(th.surface2) };
            o.push(rect(x0, 0.0, bw, bh, fs * 0.9, fill_p));
            if !mine {
                border(&mut o, x0, 0.0, bw, bh, fs * 0.9, th.border);
            }
            for (k, l) in lines.iter().enumerate() {
                draw(&mut o, ctx.assets, l, &st, x0 + pad, pad * 0.6 + lh * k as f32 + fs * 1.02, 0.0);
            }
            if !mine && !matches!(m.from.as_str(), "assistant" | "ai" | "bot") {
                draw(&mut o, ctx.assets, &m.from, &sans(fs * 0.65, 600, th.muted), x0 + pad * 0.3, -fs * 0.4, 0.0);
            }
            bubbles.push(B { ops: o, h: bh, p: spring(t - if mine { at } else { think_start }, 1.3) });
        }
        let gap = fs * 0.7;
        let mut y = h;
        let mut out = Vec::new();
        for b in bubbles.into_iter().rev() {
            // Height animates with the entrance so earlier bubbles glide up.
            y -= (b.h + gap) * b.p.min(1.0);
            if y + b.h < 0.0 {
                break;
            }
            let mut ops = b.ops;
            transform_ops(&mut ops, &translate(0.0, y));
            out.extend(rise(ops, b.p, fs));
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
    /// Animated gradient border (the "AI" look).
    pub glow: bool,
    #[serde(skip)]
    end: f32,
}
impl Default for Prompt {
    fn default() -> Self {
        Prompt { text: String::new(), placeholder: "Ask anything…".into(), typing: 30.0, delay: 0.0, send_at: None, font_size: 30.0, glow: true, end: 0.0 }
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
        let r = h * 0.5;
        let send = self.send_at.unwrap_or(self.end + 0.35);
        let mut o = Vec::new();
        if self.glow {
            // A rotating conic-ish rim: two gradient strokes offset in phase.
            let a = t * 1.4;
            let (c, s) = (a.cos(), a.sin());
            let paint = crate::dl::Paint::Linear { p0: [w * 0.5 - c * w * 0.5, h * 0.5 - s * h * 2.0], p1: [w * 0.5 + c * w * 0.5, h * 0.5 + s * h * 2.0], stops: vec![(0.0, th.accent), (0.5, th.pal(2)), (1.0, th.pal(1))] };
            glow(&mut o, 0.0, 0.0, w, h, r, 24.0, alpha(th.accent, 0.35));
            o.push(rect(-2.0, -2.0, w + 4.0, h + 4.0, r + 2.0, paint));
        } else {
            shadow(&mut o, 0.0, 0.0, w, h, r, 8.0, 20.0, 0.4);
        }
        o.push(rect(0.0, 0.0, w, h, r, solid(th.surface)));
        let n = self.text.chars().count();
        let k = ((t - self.delay) * self.typing).floor().clamp(0.0, n as f32) as usize;
        let sent = t >= send;
        let typed: String = if sent { String::new() } else { self.text.chars().take(k).collect() };
        let st = sans(fs, 400, th.text);
        let x0 = h * 0.5;
        let base = h * 0.5 + fs * 0.36;
        // Keep the caret in view: scroll long text left.
        let maxw = w - x0 - h * 1.3;
        let tw = ctx.assets.measure(&typed, &st);
        let off = (tw - maxw).max(0.0);
        push_clip(&mut o, x0, 0.0, maxw + 4.0, h, 0.0);
        if typed.is_empty() {
            draw(&mut o, ctx.assets, &self.placeholder, &st.clone().color(alpha(th.muted, 0.8)), x0, base, 0.0);
        } else {
            draw(&mut o, ctx.assets, &typed, &st, x0 - off, base, 0.0);
        }
        let blink = if t < self.end && t > self.delay { 1.0 } else { (0.5 + 0.5 * (t * std::f32::consts::TAU / 1.05).cos()).round() };
        if !sent {
            o.push(rect(x0 - off + tw + 2.0, h * 0.5 - fs * 0.6, 2.5, fs * 1.2, 1.0, solid(alpha(th.accent, blink))));
        }
        o.push(Op::PopLayer);
        // Send button: fills when there is text, pops when pressed.
        let bs = h * 0.66;
        let (bx, by) = (w - h * 0.17 - bs, (h - bs) * 0.5);
        let ready = if k > 0 { 1.0 } else { 0.0 };
        let press = 1.0 - 0.12 * (seg(t, send, 0.25) * std::f32::consts::PI).sin();
        let mut b = vec![circle(bx + bs * 0.5, by + bs * 0.5, bs * 0.5, solid(mix(th.surface2, th.accent, ready)))];
        let mut arrow = Path::new();
        let (cx, cy) = (bx + bs * 0.5, by + bs * 0.5);
        arrow.move_to((cx as f64, (cy + bs * 0.22) as f64));
        arrow.line_to((cx as f64, (cy - bs * 0.22) as f64));
        arrow.move_to(((cx - bs * 0.18) as f64, (cy - bs * 0.04) as f64));
        arrow.line_to((cx as f64, (cy - bs * 0.22) as f64));
        arrow.line_to(((cx + bs * 0.18) as f64, (cy - bs * 0.04) as f64));
        b.push(stroke_op(arrow, solid(Rgba([1.0, 1.0, 1.0, 0.6 + 0.4 * ready])), bs * 0.08, [0.0, 1.0]));
        transform_ops(&mut b, &scale_about(press, cx, cy));
        o.extend(b);
        let p = spring(t, 1.0);
        transform_ops(&mut o, &scale_about(lerp(0.95, 1.0, p), w * 0.5, h * 0.5));
        with_opacity(o, clamp01(p * 2.0))
    }
    fn duration(&self) -> Option<f32> {
        Some(self.send_at.unwrap_or(self.end + 0.35) + 0.3)
    }
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
}
impl Default for Checklist {
    fn default() -> Self {
        Checklist { items: Vec::new(), stagger: 0.35, check_after: 0.25, font_size: 36.0, color: None, delay: 0.0, strike: false }
    }
}
impl Prepare for Checklist {}
impl Component for Checklist {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let c = self.color.map(|c| c.0).unwrap_or(th.positive);
        let fs = self.font_size;
        let lh = fs * 1.9;
        let mut out = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            let t0 = self.delay + i as f32 * self.stagger;
            let p = spring(t - t0, 1.1);
            if p <= 0.0 {
                break;
            }
            let y = i as f32 * lh;
            let s = fs * 1.15;
            let k = if self.check_after >= 0.0 { ease_out(seg(t, t0 + self.check_after, 0.35)) } else { 0.0 };
            let mut o = Vec::new();
            o.push(rect(0.0, y, s, s, s * 0.3, solid(mix(th.surface2, c, k))));
            if k <= 0.0 {
                border(&mut o, 0.0, y, s, s, s * 0.3, alpha(th.text, 0.25));
            }
            let mut tick = Path::new();
            tick.move_to(((s * 0.26) as f64, (y + s * 0.52) as f64));
            tick.line_to(((s * 0.43) as f64, (y + s * 0.69) as f64));
            tick.line_to(((s * 0.75) as f64, (y + s * 0.33) as f64));
            if k > 0.0 {
                o.push(stroke_op(tick, solid(Rgba([1.0, 1.0, 1.0, 1.0])), s * 0.11, [0.0, k]));
            }
            let tc = if self.strike { mix(th.text, th.muted, k) } else { th.text };
            let st = sans(fs, 500, tc);
            let tw = draw(&mut o, ctx.assets, item, &st, s + fs * 0.6, y + s * 0.5 + fs * 0.36, 0.0);
            if self.strike && k > 0.0 {
                o.push(rect(s + fs * 0.6, y + s * 0.5, tw * k, (fs * 0.07).max(1.5), 1.0, solid(th.muted)));
            }
            out.extend(rise(o, p, fs * 0.6));
        }
        out
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.items.len() as f32 * self.stagger + self.check_after.max(0.0) + 0.4)
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
}
impl Default for Quote {
    fn default() -> Self {
        Quote { text: String::new(), name: String::new(), title: String::new(), avatar: None, font_size: 44.0, stars: 0, delay: 0.0, reveal: 10.0 }
    }
}
impl Prepare for Quote {}
impl Component for Quote {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, _h) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let t = t - self.delay;
        if t <= 0.0 {
            return Vec::new();
        }
        let mut o = Vec::new();
        // Big opening quote mark.
        let qp = ease_out(seg(t, 0.0, 0.5));
        o.extend(with_opacity(
            {
                let mut q = Vec::new();
                draw(&mut q, ctx.assets, "\u{201C}", &TextStyle::new(Face::Serif, fs * 4.0, 400, th.accent), -fs * 0.1, fs * 3.0, 0.0);
                q
            },
            qp,
        ));
        let mut y = fs * 2.6;
        if self.stars > 0 {
            for i in 0..self.stars.min(5) {
                let p = spring(t - 0.15 - i as f32 * 0.07, 1.4);
                if p > 0.0 {
                    let mut s = vec![star(fs * 0.45 + i as f32 * fs * 1.0, y - fs * 0.3, fs * 0.42, parse_color("#ffc53d").unwrap())];
                    transform_ops(&mut s, &scale_about(p, fs * 0.45 + i as f32 * fs * 1.0, y - fs * 0.3));
                    o.extend(s);
                }
            }
            y += fs * 1.1;
        }
        let st = TextStyle::new(Face::Display, fs, 500, th.text).tracking(-0.015);
        let lines = wrap(ctx.assets, &self.text, &st, w);
        let mut word_i = 0usize;
        for l in &lines {
            y += fs * 1.3;
            let mut x = 0.0;
            for word in l.split(' ') {
                let p = if self.reveal > 0.0 { ease_out(seg(t, 0.3 + word_i as f32 / self.reveal, 0.4)) } else { ease_out(seg(t, 0.3, 0.5)) };
                let mut e = Vec::new();
                let adv = draw(&mut e, ctx.assets, word, &st, x, y, 0.0);
                o.extend(rise(e, p, fs * 0.25));
                x += adv + ctx.assets.measure(" ", &st);
                word_i += 1;
            }
        }
        let words = self.text.split_whitespace().count() as f32;
        let ap = ease_out(seg(t, 0.3 + if self.reveal > 0.0 { words / self.reveal } else { 0.3 }, 0.5));
        y += fs * 1.4;
        let s = fs * 1.9;
        let mut a = Vec::new();
        if !self.name.is_empty() {
            let h = fnv(&[self.name.as_bytes()]) as usize;
            let (cx, cy) = (s * 0.5, y + s * 0.5);
            if let Some((id, iw, ih)) = self.avatar.as_deref().and_then(|src| ctx.assets.media(src, 0.0)) {
                let k = s / iw.min(ih) as f32;
                a.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((circle_path(cx, cy, s * 0.5), IDENTITY)), cache_key: None });
                a.push(Op::Image { image: id, dst: [cx - iw as f32 * k * 0.5, cy - ih as f32 * k * 0.5, iw as f32 * k, ih as f32 * k], opacity: 1.0, transform: IDENTITY });
                a.push(Op::PopLayer);
            } else {
                a.push(circle(cx, cy, s * 0.5, crate::dl::Paint::Linear { p0: [0.0, y], p1: [s, y + s], stops: vec![(0.0, th.pal(h)), (1.0, shade(th.pal(h / 7 + 1), -0.2))] }));
                let ini: String = self.name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect();
                draw(&mut a, ctx.assets, &ini.to_uppercase(), &sans(s * 0.38, 600, Rgba([1.0, 1.0, 1.0, 1.0])), cx, cy + s * 0.135, 0.5);
            }
            draw(&mut a, ctx.assets, &self.name, &sans(fs * 0.55, 600, th.text), s + fs * 0.5, y + s * 0.45, 0.0);
            draw(&mut a, ctx.assets, &self.title, &sans(fs * 0.48, 400, th.muted), s + fs * 0.5, y + s * 0.45 + fs * 0.7, 0.0);
        }
        o.extend(rise(a, ap, fs * 0.4));
        o
    }
    fn duration(&self) -> Option<f32> {
        let words = self.text.split_whitespace().count() as f32;
        Some(self.delay + 0.8 + if self.reveal > 0.0 { words / self.reveal } else { 0.3 } + 0.5)
    }
}

/// Five-point star.
fn star(cx: f32, cy: f32, r: f32, c: Rgba) -> Op {
    let mut p = Path::new();
    for i in 0..10 {
        let a = i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
        let rr = if i % 2 == 0 { r } else { r * 0.45 };
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
}
impl Default for Table {
    fn default() -> Self {
        Table { columns: Vec::new(), rows: Vec::new(), highlight: None, font_size: 28.0, delay: 0.0, stagger: 0.12 }
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
        let (w, _) = (ctx.width, ctx.height);
        let fs = self.font_size;
        let nc = self.columns.len();
        let first = w * if nc > 2 { 0.34 } else { 0.5 };
        let cw = (w - first) / (nc - 1).max(1) as f32;
        let col_x = |c: usize| if c == 0 { 0.0 } else { first + (c - 1) as f32 * cw };
        let col_w = |c: usize| if c == 0 { first } else { cw };
        let rh = fs * 2.4;
        let mut o = Vec::new();
        let rows = self.rows.len();
        if let Some(hc) = self.highlight.filter(|c| *c < nc) {
            let p = ease_out(seg(t, self.delay, 0.6));
            let mut hl = Vec::new();
            glow(&mut hl, col_x(hc) + 6.0, 0.0, col_w(hc) - 12.0, rh * (rows + 1) as f32, th.radius * 0.6, 30.0, alpha(th.accent, 0.3));
            hl.push(rect(col_x(hc) + 6.0, 0.0, col_w(hc) - 12.0, rh * (rows + 1) as f32, th.radius * 0.6, solid(alpha(th.accent, 0.12))));
            border(&mut hl, col_x(hc) + 6.0, 0.0, col_w(hc) - 12.0, rh * (rows + 1) as f32, th.radius * 0.6, alpha(th.accent, 0.6));
            o.extend(with_opacity(hl, p));
        }
        let hp = ease_out(seg(t, self.delay, 0.5));
        let mut hdr = Vec::new();
        for (c, name) in self.columns.iter().enumerate() {
            let hl = self.highlight == Some(c);
            let st = sans(fs * 0.8, 600, if hl { th.text } else { th.muted }).tracking(0.02);
            let (x, al) = if c == 0 { (fs * 0.6, 0.0) } else { (col_x(c) + col_w(c) * 0.5, 0.5) };
            draw(&mut hdr, ctx.assets, name, &st, x, rh * 0.6, al);
        }
        o.extend(rise(hdr, hp, 8.0));
        for (r, row) in self.rows.iter().enumerate() {
            let p = ease_out(seg(t, self.delay + 0.2 + r as f32 * self.stagger, 0.5));
            if p <= 0.0 {
                break;
            }
            let y = rh * (r + 1) as f32;
            let mut e = vec![rect(fs * 0.4, y, w - fs * 0.8, 1.0, 0.0, solid(th.border))];
            for (c, cell) in row.iter().enumerate().take(nc) {
                let cy = y + rh * 0.5;
                let cx = if c == 0 { fs * 0.6 } else { col_x(c) + col_w(c) * 0.5 };
                let tick = match cell {
                    serde_json::Value::Bool(b) => Some(*b),
                    serde_json::Value::String(s) if s == "✓" || s == "yes" => Some(true),
                    serde_json::Value::String(s) if s == "✗" || s == "×" || s == "no" => Some(false),
                    _ => None,
                };
                match tick {
                    Some(true) => {
                        e.push(circle(cx, cy, fs * 0.55, solid(alpha(th.positive, 0.18))));
                        let mut pth = Path::new();
                        pth.move_to(((cx - fs * 0.25) as f64, cy as f64));
                        pth.line_to(((cx - fs * 0.07) as f64, (cy + fs * 0.18) as f64));
                        pth.line_to(((cx + fs * 0.27) as f64, (cy - fs * 0.18) as f64));
                        e.push(stroke_op(pth, solid(th.positive), fs * 0.1, [0.0, 1.0]));
                    }
                    Some(false) => {
                        let mut pth = Path::new();
                        let k = fs * 0.16;
                        pth.move_to(((cx - k) as f64, (cy - k) as f64));
                        pth.line_to(((cx + k) as f64, (cy + k) as f64));
                        pth.move_to(((cx + k) as f64, (cy - k) as f64));
                        pth.line_to(((cx - k) as f64, (cy + k) as f64));
                        e.push(stroke_op(pth, solid(alpha(th.muted, 0.7)), fs * 0.08, [0.0, 1.0]));
                    }
                    None => {
                        let s = match cell {
                            serde_json::Value::String(s) => s.clone(),
                            v => v.to_string(),
                        };
                        let st = sans(fs, if c == 0 { 500 } else { 400 }, if c == 0 || self.highlight == Some(c) { th.text } else { th.muted });
                        draw(&mut e, ctx.assets, &s, &st, cx, cy + fs * 0.36, if c == 0 { 0.0 } else { 0.5 });
                    }
                }
            }
            transform_ops(&mut e, &translate((1.0 - p) * -16.0, 0.0));
            o.extend(with_opacity(e, p));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + 0.7 + self.rows.len() as f32 * self.stagger)
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
}
impl Default for Timeline {
    fn default() -> Self {
        Timeline { items: Vec::new(), current: None, vertical: false, font_size: 26.0, delay: 0.0, step: 0.45 }
    }
}
impl Prepare for Timeline {}
impl Component for Timeline {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height);
        let n = self.items.len().max(1);
        let fs = self.font_size;
        let k = clamp01((t - self.delay) / (self.step * n as f32));
        let mut o = Vec::new();
        let pos = |i: usize| -> (f32, f32) {
            let f = (i as f32 + 0.5) / n as f32;
            if self.vertical {
                (fs * 1.2, h * f)
            } else {
                (w * f, h * 0.42)
            }
        };
        let (a, b) = (pos(0), pos(n - 1));
        let mut ln = Path::new();
        ln.move_to((a.0 as f64, a.1 as f64));
        ln.line_to((b.0 as f64, b.1 as f64));
        o.push(stroke_op(ln.clone(), solid(alpha(th.text, 0.12)), 3.0, [0.0, 1.0]));
        let kk = ease_in_out(k * n as f32 / (n as f32 - 0.0)) * if n > 1 { 1.0 } else { 0.0 };
        let reach = match self.current {
            Some(c) if n > 1 => (c as f32 / (n - 1) as f32).min(1.0),
            _ => 1.0,
        };
        o.push(stroke_op(ln, crate::dl::Paint::Linear { p0: [a.0, a.1], p1: [b.0, b.1], stops: vec![(0.0, th.accent), (1.0, th.pal(1))] }, 3.0, [0.0, kk.min(reach)]));
        for (i, m) in self.items.iter().enumerate() {
            let ti = self.delay + self.step * i as f32;
            let p = spring(t - ti, 1.2);
            if p <= 0.0 {
                continue;
            }
            let (x, y) = pos(i);
            let future = self.current.is_some_and(|c| i > c);
            let c = if future { th.muted } else { th.accent };
            let mut e = Vec::new();
            if self.current == Some(i) {
                let ph = ((t - ti) * 0.9).fract();
                e.push(circle(x, y, fs * 0.5 + ph * fs * 1.2, solid(alpha(c, 0.4 * (1.0 - ph)))));
            }
            e.push(circle(x, y, fs * 0.55, solid(th.background)));
            e.push(circle(x, y, fs * 0.42, solid(c)));
            let tc = if future { alpha(th.text, 0.5) } else { th.text };
            if self.vertical {
                let lx = x + fs * 1.4;
                draw(&mut e, ctx.assets, &m.label, &sans(fs * 0.75, 600, c).tracking(0.04), lx, y - fs * 0.5, 0.0);
                draw(&mut e, ctx.assets, &m.title, &sans(fs * 1.1, 600, tc), lx, y + fs * 0.75, 0.0);
                draw(&mut e, ctx.assets, &m.body, &sans(fs * 0.8, 400, th.muted), lx, y + fs * 1.9, 0.0);
            } else {
                let up = i % 2 == 0;
                let ly = if up { y - fs * 1.4 } else { y + fs * 2.2 };
                draw(&mut e, ctx.assets, &m.label, &sans(fs * 0.75, 600, c).tracking(0.04), x, ly, 0.5);
                draw(&mut e, ctx.assets, &m.title, &sans(fs * 1.05, 600, tc), x, ly + if up { -fs * 1.3 } else { fs * 1.3 }, 0.5);
                if !m.body.is_empty() {
                    let maxw = w / n as f32 * 0.9;
                    for (j, l) in wrap(ctx.assets, &m.body, &sans(fs * 0.75, 400, th.muted), maxw).iter().enumerate().take(2) {
                        let dy = if up { -fs * 2.4 - (1 - j.min(1)) as f32 * fs * 1.0 } else { fs * 2.4 + j as f32 * fs * 1.0 };
                        draw(&mut e, ctx.assets, l, &sans(fs * 0.75, 400, th.muted), x, ly + dy, 0.5);
                    }
                }
            }
            o.extend(rise(e, p, fs * 0.5));
        }
        o
    }
    fn duration(&self) -> Option<f32> {
        Some(self.delay + self.step * self.items.len() as f32 + 0.4)
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
}
impl Default for Marquee {
    fn default() -> Self {
        Marquee { items: Vec::new(), images: false, speed: 90.0, font_size: 40.0, gap: 80.0, fade: true, separator: "•".into(), color: None }
    }
}
impl Prepare for Marquee {
    fn prepare(&mut self) -> Result<(), Error> {
        if self.items.is_empty() {
            return Err(Error("marquee needs `items`".into()));
        }
        Ok(())
    }
}
impl Component for Marquee {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (w, h) = (ctx.width, ctx.height.min(self.font_size * 3.0));
        let c = self.color.map(|c| c.0).unwrap_or(alpha(th.text, 0.75));
        let st = TextStyle::new(Face::Display, self.font_size, 600, c).tracking(-0.01);
        // One cycle's width.
        let widths: Vec<f32> = self
            .items
            .iter()
            .map(|it| {
                if self.images {
                    ctx.assets.media(it, 0.0).map(|(_, iw, ih)| iw as f32 * (h * 0.6) / ih as f32).unwrap_or(h)
                } else {
                    ctx.assets.measure(it, &st)
                }
            })
            .collect();
        let cycle: f32 = widths.iter().map(|w| w + self.gap).sum::<f32>().max(1.0);
        let mut x = -(t * self.speed).rem_euclid(cycle);
        let mut o = Vec::new();
        let mut i = 0usize;
        while x < w {
            let k = i % self.items.len();
            let iw = widths[k];
            if x + iw > -10.0 {
                if self.images {
                    if let Some((id, _, _)) = ctx.assets.media(&self.items[k], 0.0) {
                        o.push(Op::Image { image: id, dst: [x, h * 0.2, iw, h * 0.6], opacity: 0.8, transform: IDENTITY });
                    }
                } else {
                    draw(&mut o, ctx.assets, &self.items[k], &st, x, h * 0.5 + self.font_size * 0.36, 0.0);
                    if !self.separator.is_empty() {
                        draw(&mut o, ctx.assets, &self.separator, &st.clone().color(alpha(th.accent, 0.9)).tracking(0.0), x + iw + self.gap * 0.5, h * 0.5 + self.font_size * 0.3, 0.5);
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
            o.push(rect(0.0, 0.0, w * 0.12, h, 0.0, crate::dl::Paint::Linear { p0: [0.0, 0.0], p1: [w * 0.12, 0.0], stops: vec![(0.0, bg), (1.0, alpha(bg, 0.0))] }));
            o.push(rect(w * 0.88, 0.0, w * 0.12, h, 0.0, crate::dl::Paint::Linear { p0: [w * 0.88, 0.0], p1: [w, 0.0], stops: vec![(0.0, alpha(bg, 0.0)), (1.0, bg)] }));
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
    pub color: Option<Color>,
    pub delay: f32,
    pub out: Option<f32>,
}
impl Default for Spotlight {
    fn default() -> Self {
        Spotlight { targets: vec![[100.0, 100.0, 300.0, 120.0]], times: Vec::new(), dim: 0.6, radius: 16.0, padding: 10.0, color: None, delay: 0.0, out: None }
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
            let k = ease_in_out(seg(t, self.times[i] - 0.5, 0.5));
            for j in 0..4 {
                r[j] = lerp(r[j], self.targets[i][j], k);
            }
        }
        let p = self.padding;
        // Opening: the hole starts larger and settles on the target.
        let grow = (1.0 - ease_out(seg(t, self.times[0], 0.6))) * 60.0;
        let (x, y, w, h) = (r[0] - p - grow, r[1] - p - grow, r[2] + 2.0 * (p + grow), r[3] + 2.0 * (p + grow));
        let mut path = rrect_path(-10.0, -10.0, ctx.width + 20.0, ctx.height + 20.0, 0.0);
        path.extend(rrect_path(x, y, w, h, self.radius).elements().iter().copied());
        let mut o = vec![Op::Fill { path, paint: solid(Rgba([0.0, 0.0, 0.0, self.dim * on])), rule: crate::dl::FillRule::EvenOdd, transform: IDENTITY }];
        let c = self.color.map(|c| c.0).unwrap_or(th.accent);
        o.push(Op::Stroke { path: rrect_path(x, y, w, h, self.radius), paint: solid(alpha(c, 0.9 * on)), stroke: line(2.0, [0.0, 1.0]), transform: IDENTITY });
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
}
impl Default for Keys {
    fn default() -> Self {
        Keys { keys: vec!["⌘".into(), "K".into()], press: vec![0.6], size: 64.0, delay: 0.0, out: None, align: 0.5 }
    }
}
impl Prepare for Keys {}
impl Component for Keys {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let s = self.size;
        let st = sans(s * 0.42, 600, th.text);
        let mut x = 0.0;
        let mut out = Vec::new();
        let exit = self.out.map(|o| ease_in(seg(t, o, 0.3))).unwrap_or(0.0);
        for (i, k) in self.keys.iter().enumerate() {
            let p = spring(t - self.delay - i as f32 * 0.08, 1.2);
            if p <= 0.0 {
                break;
            }
            let kw = (ctx.assets.measure(k, &st) + s * 0.6).max(s);
            // All keys go down together, staggered slightly like real fingers.
            let mut down = 0.0f32;
            for &pt in &self.press {
                let d = t - pt - i as f32 * 0.04;
                if (0.0..0.35).contains(&d) {
                    down = down.max((d / 0.35 * std::f32::consts::PI).sin());
                }
            }
            let depth = s * 0.09;
            let dy = depth * down;
            let mut e = Vec::new();
            // Key body (side), cap, label.
            e.push(rect(x, depth, kw, s, s * 0.2, solid(shade(th.surface2, -0.35))));
            e.push(rect(x, dy, kw, s, s * 0.2, vgrad(dy, dy + s, shade(th.surface2, 0.12), th.surface2)));
            border(&mut e, x, dy, kw, s, s * 0.2, if down > 0.3 { alpha(th.accent, 0.8) } else { th.border });
            draw(&mut e, ctx.assets, k, &st, x + kw * 0.5, dy + s * 0.5 + st.size * 0.36, 0.5);
            if i + 1 < self.keys.len() {
                draw(&mut e, ctx.assets, "+", &sans(s * 0.4, 500, th.muted), x + kw + s * 0.3, s * 0.5 + s * 0.15, 0.5);
            }
            out.extend(rise(e, p, s * 0.3));
            x += kw + s * 0.6;
        }
        let (w, h) = ((x - s * 0.6).max(0.0), s * 1.09);
        transform_ops(&mut out, &translate((ctx.width - w) * self.align, (ctx.height - h).max(0.0) * 0.5));
        with_opacity(out, 1.0 - exit)
    }
    fn duration(&self) -> Option<f32> {
        Some(self.press.iter().fold(self.delay + 0.5, |m, p| m.max(p + 0.4)))
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
