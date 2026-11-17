//! Generated app screens for device mockups: `dashboard`, `chat`, `feed`,
//! `editor` and `settings`. They lay themselves out for portrait (phone) or
//! landscape (laptop, browser) screens, take the product `title` and an
//! `accent`, and build in with a short stagger so a device "comes alive".

use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::Ctx;
use crate::dl::{Op, Path, Rgba};

/// Screen kinds, for validation and docs.
pub const KINDS: &[&str] = &["app", "dashboard", "chat", "feed", "editor", "settings"];

/// Product copy for generated screens.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kpi {
    pub label: String,
    pub value: String,
}

struct S<'a> {
    ctx: &'a Ctx<'a>,
    acc: Rgba,
    bg: Rgba,
    card: Rgba,
    line: Rgba,
    text: Rgba,
    muted: Rgba,
    fs: f32,
    t: f32,
    items: &'a [String],
    kpis: &'a [Kpi],
}

impl S<'_> {
    /// Item `i` of the product copy, or `fallback`.
    fn it<'b>(&'b self, i: usize, fallback: &'b str) -> &'b str {
        if self.items.is_empty() {
            fallback
        } else {
            &self.items[i % self.items.len()]
        }
    }
}

impl S<'_> {
    fn txt(&self, o: &mut Vec<Op>, s: &str, size: f32, weight: u16, c: Rgba, x: f32, y: f32, align: f32) -> f32 {
        draw(o, self.ctx.assets, s, &TextStyle::new(Face::Sans, size, weight, c), x, y, align)
    }
    /// Text fitted to `maxw`: shrinks up to 20 %, then ellipsizes.
    fn fit(&self, o: &mut Vec<Op>, text: &str, size: f32, weight: u16, c: Rgba, x: f32, y: f32, maxw: f32) -> f32 {
        let a = self.ctx.assets;
        let mut st = TextStyle::new(Face::Sans, size, weight, c);
        let w = a.measure(text, &st);
        if w <= maxw || maxw <= 0.0 {
            return draw(o, a, text, &st, x, y, 0.0);
        }
        st.size = (size * maxw / w).max(size * 0.8);
        if a.measure(text, &st) <= maxw {
            return draw(o, a, text, &st, x, y, 0.0);
        }
        // Longest prefix (at a char boundary) that fits with an ellipsis.
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cand: String = chars[..mid].iter().collect::<String>().trim_end().to_string() + "…";
            if a.measure(&cand, &st) <= maxw {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let cut: String = chars[..lo].iter().collect::<String>().trim_end().to_string() + "…";
        draw(o, a, &cut, &st, x, y, 0.0)
    }
    /// Placeholder text line.
    fn bar(&self, o: &mut Vec<Op>, x: f32, y: f32, w: f32, c: Rgba) {
        let h = self.fs * 0.55;
        o.push(rect(x, y, w.max(h), h, h * 0.5, solid(c)));
    }
    /// Entrance progress of item `i`.
    fn k(&self, i: usize) -> f32 {
        ease_out(seg(self.t, 0.1 + i as f32 * 0.07, 0.5))
    }
    /// Push `ops` with item `i`'s entrance (rise + fade).
    fn item(&self, o: &mut Vec<Op>, i: usize, ops: Vec<Op>) {
        let k = self.k(i);
        if k <= 0.0 {
            return;
        }
        let mut ops = ops;
        super::transform_ops(&mut ops, &translate(0.0, (1.0 - k) * self.fs));
        o.extend(super::with_opacity(ops, k));
    }
    fn card(&self, o: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32) {
        o.push(rect(x, y, w, h, self.fs * 0.7, solid(self.card)));
        border(o, x, y, w, h, self.fs * 0.7, self.line);
    }
}

/// Draw screen `kind` into `(x, y, w, h)`; `top` is a status-bar inset.
pub fn draw_screen(o: &mut Vec<Op>, ctx: &Ctx, kind: &str, title: &str, items: &[String], kpis: &[Kpi], accent: Option<Rgba>, t: f32, x: f32, y: f32, w: f32, h: f32, top: f32) {
    let th = ctx.theme;
    let s = S {
        ctx,
        acc: accent.unwrap_or(th.accent),
        bg: th.background,
        card: th.surface,
        line: th.border,
        text: th.text,
        muted: th.muted,
        fs: (w.min(h * 0.8) / 24.0).clamp(6.0, 40.0),
        t,
        items,
        kpis,
    };
    let fs = s.fs;
    o.push(rect(x, y, w, h, 0.0, solid(s.bg)));
    let wide = w > h * 1.2;
    let title = if title.is_empty() { "Acme" } else { title };
    // Header: logo dot, product name, avatar.
    let hy = y + top;
    let hh = fs * 3.0;
    let mut hdr = Vec::new();
    hdr.push(rect(x + fs, hy + hh * 0.5 - fs * 0.6, fs * 1.2, fs * 1.2, fs * 0.35, crate::dl::Paint::Linear { p0: [x + fs, hy], p1: [x + fs * 2.2, hy + hh], stops: vec![(0.0, shade(s.acc, 0.2)), (1.0, shade(s.acc, -0.2))] }));
    s.txt(&mut hdr, title, fs * 1.05, 700, s.text, x + fs * 2.8, hy + hh * 0.5 + fs * 0.38, 0.0);
    hdr.push(circle(x + w - fs * 1.8, hy + hh * 0.5, fs * 0.8, crate::dl::Paint::Linear { p0: [x + w - fs * 2.6, hy], p1: [x + w - fs, hy + hh], stops: vec![(0.0, th.pal(2)), (1.0, th.pal(4))] }));
    hdr.push(rect(x, hy + hh - 1.0, w, 1.0, 0.0, solid(s.line)));
    s.item(o, 0, hdr);
    let (mut cx, cy, mut cw) = (x + fs, hy + hh + fs, w - 2.0 * fs);
    let ch = y + h - cy - fs;
    // Sidebar on wide screens.
    if wide {
        let sw = (w * 0.2).min(fs * 14.0);
        let mut side = vec![rect(x, hy + hh, sw, h - top - hh, 0.0, solid(shade(s.bg, if th.dark { 0.03 } else { -0.02 })))];
        let nav = if s.items.is_empty() { 6 } else { s.items.len().min(8) };
        for i in 0..nav {
            let yy = hy + hh + fs * (1.2 + i as f32 * 2.2);
            if i == 0 {
                side.push(rect(x + fs * 0.6, yy - fs * 0.5, sw - fs * 1.2, fs * 1.9, fs * 0.5, solid(alpha(s.acc, 0.16))));
            }
            side.push(rect(x + fs * 1.2, yy + fs * 0.1, fs * 0.8, fs * 0.8, fs * 0.25, solid(if i == 0 { s.acc } else { alpha(s.muted, 0.6) })));
            if s.items.is_empty() {
                s.bar(&mut side, x + fs * 2.6, yy + fs * 0.22, sw * (0.35 + 0.25 * hash01(i as u32 + 3)), if i == 0 { s.text } else { alpha(s.muted, 0.55) });
            } else if i < s.items.len() {
                s.fit(&mut side, &s.items[i], fs * 0.7, 500, if i == 0 { s.text } else { s.muted }, x + fs * 2.6, yy + fs * 0.65, sw - fs * 3.4);
            }
        }
        side.push(rect(x + sw - 1.0, hy + hh, 1.0, h - top - hh, 0.0, solid(s.line)));
        s.item(o, 1, side);
        cx = x + sw + fs;
        cw = w - sw - 2.0 * fs;
    }
    match kind {
        "chat" => chat(o, &s, cx, cy, cw, ch),
        "feed" | "list" => feed(o, &s, cx, cy, cw, ch, wide),
        "editor" => editor(o, &s, cx, cy, cw, ch),
        "settings" => settings(o, &s, cx, cy, cw, ch),
        "app" => app(o, &s, cx, cy, cw, ch, wide),
        _ => dashboard(o, &s, cx, cy, cw, ch, wide),
    }
}

fn dashboard(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, wide: bool) {
    let fs = s.fs;
    let n = if s.kpis.is_empty() { if wide { 3 } else { 2 } } else { s.kpis.len().min(if wide { 4 } else { 2 }) };
    let gap = fs * 0.8;
    let tw = (w - gap * (n - 1) as f32) / n as f32;
    let th = fs * 5.0;
    let kpis = [("Revenue", 48_200.0, "$"), ("Active users", 12_840.0, ""), ("Conversion", 18.4, "%"), ("Retention", 92.0, "%")];
    for (i, (label, v, unit)) in kpis.iter().enumerate().take(n) {
        let tx = x + i as f32 * (tw + gap);
        let mut e = Vec::new();
        s.card(&mut e, tx, y, tw, th);
        let k = ease_out(seg(s.t, 0.3 + i as f32 * 0.1, 1.2)) as f64;
        let (label, body) = match s.kpis.get(i) {
            Some(kp) => (kp.label.as_str(), count_up(&kp.value, k)),
            None => {
                let val = v * k;
                (*label, if *unit == "%" { format!("{:.1}%", val) } else { format!("{unit}{}", compact_number(val)) })
            }
        };
        s.fit(&mut e, label, fs * 0.75, 500, s.muted, tx + fs, y + fs * 1.6, tw - fs * if s.kpis.is_empty() { 4.0 } else { 2.0 });
        draw(&mut e, s.ctx.assets, &body, &TextStyle::new(Face::Display, fs * 1.7, 700, s.text).tabular(), tx + fs, y + fs * 3.9, 0.0);
        if s.kpis.is_empty() {
            let up = format!("+{:.0}%", 8.0 + 5.0 * i as f32);
            s.txt(&mut e, &up, fs * 0.7, 600, s.ctx.theme.positive, tx + tw - fs, y + fs * 1.6, 1.0);
        }
        s.item(o, 2 + i, e);
    }
    // Area chart card.
    let cy = y + th + gap;
    let chh = (h - th - gap) * if wide { 1.0 } else { 0.55 };
    let cwid = if wide { w * 0.62 } else { w };
    let mut e = Vec::new();
    s.card(&mut e, x, cy, cwid, chh);
    if s.kpis.is_empty() {
        s.txt(&mut e, "Growth", fs * 0.85, 600, s.text, x + fs, cy + fs * 1.7, 0.0);
    }
    let (px, py, pw, ph) = (x + fs, cy + fs * 2.6, cwid - 2.0 * fs, chh - fs * 3.6);
    let data = [3.0f32, 4.2, 3.8, 5.1, 6.0, 5.6, 7.2, 8.1, 7.6, 9.4];
    let pts: Vec<(f32, f32)> = data.iter().enumerate().map(|(i, v)| (px + pw * i as f32 / (data.len() - 1) as f32, py + ph - ph * v / 10.0)).collect();
    let line = smooth_path(&pts, 1.0);
    let k = ease_in_out(seg(s.t, 0.5, 1.4));
    let mut area = line.clone();
    area.line_to(((px + pw) as f64, (py + ph) as f64));
    area.line_to((px as f64, (py + ph) as f64));
    area.close_path();
    e.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![], clip: Some((rrect_path(px - 2.0, py - fs, pw * k + 4.0, ph + fs * 2.0, 0.0), crate::dl::IDENTITY)), cache_key: None });
    e.push(fill(area, vgrad(py, py + ph, alpha(s.acc, 0.35), alpha(s.acc, 0.0))));
    e.push(stroke_op(line, solid(s.acc), (fs * 0.18).max(1.5), [0.0, 1.0]));
    e.push(Op::PopLayer);
    s.item(o, 5, e);
    // Recent list.
    let (lx, ly, lw, lh) = if wide { (x + cwid + gap, cy, w - cwid - gap, chh) } else { (x, cy + chh + gap, w, h - th - chh - 2.0 * gap) };
    if lh > fs * 4.0 {
        let mut e = Vec::new();
        s.card(&mut e, lx, ly, lw, lh);
        s.txt(&mut e, if s.items.is_empty() { "Recent" } else { "Highlights" }, fs * 0.85, 600, s.text, lx + fs, ly + fs * 1.7, 0.0);
        let rows = ((lh - fs * 2.6) / (fs * 2.4)).floor().max(0.0) as usize;
        let rows = if s.items.is_empty() { rows.min(6) } else { rows.min(s.items.len()) };
        for r in 0..rows {
            let ry = ly + fs * 2.8 + r as f32 * fs * 2.4;
            e.push(circle(lx + fs * 1.6, ry + fs * 0.8, fs * 0.7, solid(s.ctx.theme.pal(r + 1))));
            if s.items.is_empty() {
                s.bar(&mut e, lx + fs * 2.8, ry + fs * 0.2, lw * 0.35, alpha(s.text, 0.8));
                s.bar(&mut e, lx + fs * 2.8, ry + fs * 1.0, lw * 0.22, alpha(s.muted, 0.5));
                s.txt(&mut e, &format!("${}", 120 + r * 37), fs * 0.75, 600, s.text, lx + lw - fs, ry + fs * 1.0, 1.0);
            } else {
                s.fit(&mut e, s.it(r, ""), fs * 0.75, 500, s.text, lx + fs * 2.8, ry + fs * 1.05, lw - fs * 3.8);
            }
        }
        s.item(o, 6, e);
    }
}

/// Count a value string up from zero when it holds a number ("$4.2K", "98%", "12,000+").
fn count_up(v: &str, k: f64) -> String {
    let start = v.find(|c: char| c.is_ascii_digit());
    let Some(a) = start else { return v.to_string() };
    let end = v[a..].find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ',')).map_or(v.len(), |e| a + e);
    let num: String = v[a..end].chars().filter(|c| *c != ',').collect();
    let Ok(n) = num.parse::<f64>() else { return v.to_string() };
    let dec = num.split_once('.').map_or(0, |(_, d)| d.len());
    let body = format_number(n * k, dec, v[a..end].contains(','));
    format!("{}{}{}", &v[..a], body, &v[end..])
}

/// Neutral screen: the product name and its own items, no invented data.
/// A hero card for the first item (headline, skeleton paragraph, button)
/// and a grid of the others, sized to fill the screen.
fn app(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, wide: bool) {
    let fs = s.fs;
    let gap = fs * 0.8;
    let rest: Vec<String> = if s.items.len() > 1 { s.items[1..].to_vec() } else { Vec::new() };
    let n = if rest.is_empty() { 4 } else { rest.len().min(if wide { 6 } else { 5 }) };
    // Hero takes ~45 % of the height on wide screens, ~38 % on phones.
    let hh = (h * if wide { 0.45 } else { 0.38 }).max(fs * 6.0);
    let mut e = Vec::new();
    e.push(rect(x, y, w, hh, fs * 0.9, crate::dl::Paint::Linear { p0: [x, y], p1: [x + w, y + hh], stops: vec![(0.0, alpha(s.acc, 0.38)), (1.0, alpha(s.acc, 0.07))] }));
    border(&mut e, x, y, w, hh, fs * 0.9, alpha(s.acc, 0.4));
    let pad = fs * 1.4;
    let hsz = (fs * if wide { 1.8 } else { 1.3 }).min(hh * 0.22);
    s.fit(&mut e, s.it(0, "Welcome"), hsz, 700, s.text, x + pad, y + pad + hsz * 0.9, w - 2.0 * pad);
    // Skeleton paragraph under the headline.
    let mut ly = y + pad + hsz * 1.6;
    for (k, f) in [0.86f32, 0.78, 0.55].iter().enumerate() {
        if ly + fs * 2.6 > y + hh - fs * 2.4 {
            break;
        }
        s.bar(&mut e, x + pad, ly, (w - 2.0 * pad) * f * if wide { 0.7 } else { 1.0 }, alpha(s.text, if k == 0 { 0.32 } else { 0.22 }));
        ly += fs * 1.1;
    }
    let bh = fs * 1.6;
    e.push(rect(x + pad, y + hh - pad - bh, fs * 6.5, bh, bh * 0.5, solid(s.acc)));
    e.push(rect(x + pad + fs * 7.2, y + hh - pad - bh, fs * 5.0, bh, bh * 0.5, solid(alpha(s.text, 0.12))));
    if wide {
        // An illustrative panel on the right of the hero.
        let (px, py, pw, ph) = (x + w * 0.62, y + pad, w * 0.38 - pad, hh - 2.0 * pad);
        e.push(rect(px, py, pw, ph, fs * 0.6, solid(alpha(s.bg, 0.55))));
        for r in 0..4 {
            let yy = py + fs * (1.0 + r as f32 * 1.6);
            if yy + fs > py + ph {
                break;
            }
            e.push(circle(px + fs * 1.2, yy + fs * 0.3, fs * 0.45, solid(s.ctx.theme.pal(r + 1))));
            s.bar(&mut e, px + fs * 2.2, yy, pw * (0.55 - 0.08 * r as f32), alpha(s.text, 0.3));
        }
    }
    s.item(o, 2, e);
    // Grid of the other items, filling the remaining height.
    let cols = if wide { 3usize } else { 1 };
    let rows = n.div_ceil(cols).max(1);
    let gy = y + hh + gap;
    let ch = ((y + h - gy) - gap * (rows - 1) as f32) / rows as f32;
    if ch < fs * 2.4 {
        return;
    }
    let cw = (w - gap * (cols - 1) as f32) / cols as f32;
    for i in 0..n {
        let (c, r) = (i % cols, i / cols);
        let (cx, cy) = (x + c as f32 * (cw + gap), gy + r as f32 * (ch + gap));
        let mut e = Vec::new();
        s.card(&mut e, cx, cy, cw, ch);
        let ic = s.ctx.theme.pal(i + 1);
        let isz = (fs * 1.5).min(ch * 0.4);
        e.push(rect(cx + fs, cy + fs * 0.9, isz, isz, isz * 0.3, solid(alpha(ic, 0.85))));
        let tx = cx + fs + isz + fs * 0.8;
        let ty = cy + fs * 0.9 + isz * 0.7;
        match rest.get(i) {
            Some(t) => {
                s.fit(&mut e, t, fs * 0.85, 600, s.text, tx, ty, cx + cw - fs - tx);
            }
            None => s.bar(&mut e, tx, ty - fs * 0.45, cw * 0.4, alpha(s.text, 0.75)),
        }
        // Body lines where there is room.
        let mut by = cy + fs * 0.9 + isz + fs * 0.9;
        let mut k = 0;
        while by + fs * 0.6 < cy + ch - fs * 0.6 && k < 3 {
            s.bar(&mut e, cx + fs, by, (cw - 2.0 * fs) * [0.9, 0.75, 0.5][k], alpha(s.muted, 0.35));
            by += fs * 1.0;
            k += 1;
        }
        s.item(o, 3 + i, e);
    }
}

fn chat(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let fs = s.fs;
    let custom;
    let msgs: Vec<(bool, &str)> = if s.items.is_empty() {
        vec![(false, "Hi! How can I help today?"), (true, "Summarise last week's sales"), (false, "Revenue grew 18% week over week, led by the new Pro plan."), (true, "Make it a chart")]
    } else {
        // The product's own features as a short exchange.
        custom = ["What's new?".to_string(), s.items.iter().take(3).cloned().collect::<Vec<_>>().join(" · "), s.it(1, "").to_string(), "Love it!".to_string()];
        vec![(true, custom[0].as_str()), (false, custom[1].as_str()), (true, custom[2].as_str()), (false, custom[3].as_str())]
    };
    let mut yy = y + fs * 0.5;
    let maxw = w * 0.75;
    for (i, (mine, text)) in msgs.iter().enumerate() {
        let st = TextStyle::new(Face::Sans, fs * 0.85, 400, if *mine { Rgba([1.0, 1.0, 1.0, 1.0]) } else { s.text });
        let lines = super::text::wrap(s.ctx.assets, text, &st, maxw - fs * 1.6);
        let bw = lines.iter().map(|l| s.ctx.assets.measure(l, &st)).fold(0.0, f32::max) + fs * 1.6;
        let bh = lines.len() as f32 * fs * 1.25 + fs * 0.9;
        if yy + bh > y + h - fs * 3.5 {
            break;
        }
        let bx = if *mine { x + w - bw } else { x };
        let mut e = vec![rect(bx, yy, bw, bh, fs * 0.8, if *mine { solid(s.acc) } else { solid(s.card) })];
        if !*mine {
            border(&mut e, bx, yy, bw, bh, fs * 0.8, s.line);
        }
        for (j, l) in lines.iter().enumerate() {
            draw(&mut e, s.ctx.assets, l, &st, bx + fs * 0.8, yy + fs * (1.25 + 1.25 * j as f32), 0.0);
        }
        s.item(o, 2 + i * 2, e);
        yy += bh + fs * 0.7;
    }
    // Composer.
    let mut e = Vec::new();
    let (iy, ih) = (y + h - fs * 2.8, fs * 2.6);
    e.push(rect(x, iy, w, ih, ih * 0.5, solid(s.card)));
    border(&mut e, x, iy, w, ih, ih * 0.5, s.line);
    s.txt(&mut e, "Message…", fs * 0.85, 400, s.muted, x + fs, iy + ih * 0.5 + fs * 0.3, 0.0);
    e.push(circle(x + w - ih * 0.5, iy + ih * 0.5, ih * 0.36, solid(s.acc)));
    s.item(o, 1, e);
}

fn feed(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, wide: bool) {
    let fs = s.fs;
    let cols = if wide { 2 } else { 1 };
    let gap = fs * 0.8;
    let cw = (w - gap * (cols - 1) as f32) / cols as f32;
    let chh = fs * 9.0;
    let names = ["Ada Lovelace", "Grace Hopper", "Alan Turing", "Katherine J.", "Linus T.", "Margaret H."];
    let mut i = 0;
    let mut yy = y;
    while yy + chh <= y + h + chh * 0.4 && i < 8 {
        for c in 0..cols {
            let cx = x + c as f32 * (cw + gap);
            let mut e = Vec::new();
            s.card(&mut e, cx, yy, cw, chh);
            e.push(circle(cx + fs * 1.6, yy + fs * 1.6, fs * 0.8, crate::dl::Paint::Linear { p0: [cx, yy], p1: [cx + fs * 3.0, yy + fs * 3.0], stops: vec![(0.0, s.ctx.theme.pal(i)), (1.0, s.ctx.theme.pal(i + 3))] }));
            s.fit(&mut e, s.it(i, names[i % names.len()]), fs * 0.8, 600, s.text, cx + fs * 2.8, yy + fs * 1.45, cw - fs * 6.0);
            s.txt(&mut e, &format!("{}m", 3 + i * 7), fs * 0.65, 400, s.muted, cx + cw - fs, yy + fs * 1.45, 1.0);
            s.bar(&mut e, cx + fs, yy + fs * 3.0, cw * 0.8, alpha(s.text, 0.7));
            s.bar(&mut e, cx + fs, yy + fs * 3.9, cw * 0.55, alpha(s.muted, 0.5));
            let (ix, iy, iw, ih) = (cx + fs, yy + fs * 5.0, cw - 2.0 * fs, chh - fs * 6.0);
            e.push(rect(ix, iy, iw, ih, fs * 0.5, crate::dl::Paint::Linear { p0: [ix, iy], p1: [ix + iw, iy + ih], stops: vec![(0.0, alpha(s.ctx.theme.pal(i + 1), 0.55)), (1.0, alpha(s.acc, 0.35))] }));
            s.item(o, 2 + i, e);
            i += 1;
        }
        yy += chh + gap;
    }
}

fn editor(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let fs = s.fs;
    let mut e = Vec::new();
    s.card(&mut e, x, y, w, h);
    // Tabs.
    e.push(rect(x + fs * 0.6, y + fs * 0.5, fs * 7.0, fs * 1.8, fs * 0.45, solid(alpha(s.acc, 0.16))));
    s.fit(&mut e, s.it(0, "main.ts"), fs * 0.7, 500, s.text, x + fs * 1.2, y + fs * 1.65, fs * 6.0);
    s.txt(&mut e, "README.md", fs * 0.7, 500, s.muted, x + fs * 8.4, y + fs * 1.65, 0.0);
    s.item(o, 2, e);
    // Code lines as coloured token bars (reads as code at any size).
    let lh = fs * 1.35;
    let rows = ((h - fs * 3.2) / lh).floor().max(0.0) as usize;
    let pal = [s.ctx.theme.pal(0), s.ctx.theme.pal(1), s.ctx.theme.pal(2), s.ctx.theme.pal(4), alpha(s.text, 0.75)];
    let reveal = ((s.t - 0.4).max(0.0) * 14.0) as usize;
    let mut e = Vec::new();
    for r in 0..rows.min(reveal) {
        let ly = y + fs * 3.0 + r as f32 * lh;
        s.txt(&mut e, &(r + 1).to_string(), fs * 0.6, 400, alpha(s.muted, 0.6), x + fs * 1.8, ly + fs * 0.55, 1.0);
        let indent = [0, 1, 1, 2, 2, 1, 0, 1, 2, 3, 2, 1][r % 12] as f32 * fs * 1.2;
        let mut lx = x + fs * 2.6 + indent;
        let toks = 1 + (hash01(r as u32 * 31 + 7) * 4.0) as usize;
        for k in 0..toks {
            let tw = fs * (1.5 + 4.0 * hash01((r * 13 + k) as u32));
            if lx + tw > x + w - fs {
                break;
            }
            let h2 = fs * 0.5;
            e.push(rect(lx, ly + fs * 0.2, tw, h2, h2 * 0.5, solid(pal[(r + k * 3) % pal.len()])));
            lx += tw + fs * 0.45;
        }
        if r == 4 {
            e.insert(0, rect(x + 1.0, ly - fs * 0.15, w - 2.0, lh, 0.0, solid(alpha(s.acc, 0.12))));
        }
    }
    o.extend(e);
}

fn settings(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let fs = s.fs;
    let rows = ["Dark mode", "Notifications", "Auto-render previews", "Beat sync", "Share analytics", "Two-factor auth"];
    let rh = fs * 3.0;
    let mut e = Vec::new();
    let n = ((h - fs * 6.0) / rh).floor().clamp(1.0, rows.len() as f32) as usize;
    s.card(&mut e, x, y, w, rh * n as f32);
    s.item(o, 1, e);
    for (i, label) in rows.iter().enumerate().take(n) {
        let label = &s.it(i, label);
        let ry = y + i as f32 * rh;
        let mut e = Vec::new();
        if i > 0 {
            e.push(rect(x + fs, ry, w - 2.0 * fs, 1.0, 0.0, solid(s.line)));
        }
        s.fit(&mut e, label, fs * 0.85, 500, s.text, x + fs, ry + rh * 0.5 + fs * 0.3, w - fs * 5.0);
        // Toggle: some on, one flips on during the shot.
        let on0 = [true, false, true, false, true, false][i % 6];
        let k = if i == 1 { ease_in_out(seg(s.t, 1.2, 0.3)) } else if on0 { 1.0 } else { 0.0 };
        let (tw, tht) = (fs * 2.4, fs * 1.4);
        let (tx, ty) = (x + w - fs - tw, ry + (rh - tht) * 0.5);
        e.push(rect(tx, ty, tw, tht, tht * 0.5, solid(mix(alpha(s.muted, 0.35), s.acc, k))));
        let d = tht - fs * 0.3;
        e.push(circle(tx + fs * 0.15 + d * 0.5 + (tw - d - fs * 0.3) * k, ty + tht * 0.5, d * 0.5, solid(Rgba([1.0, 1.0, 1.0, 1.0]))));
        s.item(o, 2 + i, e);
    }
    // A form field and a primary button.
    let fy = y + n as f32 * rh + fs * 1.2;
    if fy + fs * 4.6 < y + h {
        let mut e = Vec::new();
        e.push(rect(x, fy, w, fs * 2.4, fs * 0.6, solid(s.card)));
        border(&mut e, x, fy, w, fs * 2.4, fs * 0.6, alpha(s.acc, 0.6));
        s.txt(&mut e, "you@company.com", fs * 0.8, 400, s.text, x + fs, fy + fs * 1.5, 0.0);
        let bw = fs * 7.0;
        e.push(rect(x + w - bw, fy + fs * 3.0, bw, fs * 2.2, fs * 1.1, solid(s.acc)));
        s.txt(&mut e, "Save", fs * 0.8, 600, Rgba([1.0, 1.0, 1.0, 1.0]), x + w - bw * 0.5, fy + fs * 4.4, 0.5);
        s.item(o, 2 + n, e);
    }
    let _ = Path::new();
}
