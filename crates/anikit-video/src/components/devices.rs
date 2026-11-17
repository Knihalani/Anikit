//! Device mockups: phone, laptop and browser window, with an image or video
//! screen (cover-fit, optional scroll through a tall screenshot), glass
//! reflection and a soft shadow. Each keeps its aspect ratio, centred in the box.

use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::{transform_ops, with_opacity, Component, Ctx, Prepare};
use crate::dl::{Op, Rgba, IDENTITY};
use serde::Deserialize;

/// Screen content shared by all devices.
#[derive(Deserialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Screen {
    /// Image or video `src` (as registered with the spec's assets).
    pub src: Option<String>,
    /// Placeholder gradient when there is no `src`.
    pub colors: Vec<Color>,
    /// Scroll a tall screenshot: fraction of its overflow (0 top, 1 bottom) to reach…
    pub scroll_to: f32,
    /// …starting at this time…
    pub scroll_at: f32,
    /// …over this long (0 = no scroll).
    pub scroll_duration: f32,
    /// Generated app screen when there is no `src`: dashboard, chat, feed, editor, settings.
    pub screen: Option<String>,
    /// Product name shown on generated screens.
    pub title: String,
    /// The product's own copy (features, changelog items): sidebar entries,
    /// list and card titles, chat lines, settings rows.
    pub items: Vec<String>,
    /// Dashboard numbers: `[{ "label": "Notes", "value": "12,400" }]` (numbers count up).
    pub kpis: Vec<super::screens::Kpi>,
    /// Accent colour of generated screens (default: theme accent).
    pub accent: Option<Color>,
}

/// Draw the screen into the rounded rect `(x, y, w, h, r)`.
fn screen(o: &mut Vec<Op>, ctx: &Ctx, s: &Screen, t: f32, x: f32, y: f32, w: f32, h: f32, r: f32) {
    push_clip(o, x, y, w, h, r);
    if let Some((id, iw, ih)) = s.src.as_deref().and_then(|src| ctx.assets.media(src, t)) {
        // Fit width; tall images scroll, short ones cover.
        let k = (w / iw as f32).max(h / ih as f32);
        let (dw, dh) = (iw as f32 * k, ih as f32 * k);
        let over = (dh - h).max(0.0);
        let f = if s.scroll_duration > 0.0 { s.scroll_to * ease_in_out(seg(t, s.scroll_at, s.scroll_duration)) } else { 0.0 };
        o.push(Op::Image { image: id, dst: [x + (w - dw) * 0.5, y - over * f, dw, dh], opacity: 1.0, transform: IDENTITY });
    } else if let Some(kind) = s.screen.as_deref() {
        // Portrait screens leave room for the phone's status bar.
        let top = if h > w * 1.3 { w * 0.13 } else { 0.0 };
        super::screens::draw_screen(o, ctx, kind, &s.title, &s.items, &s.kpis, s.accent.map(|c| c.0), t, x, y, w, h, top);
    } else {
        let th = ctx.theme;
        let cols: Vec<Rgba> = if s.colors.is_empty() { vec![shade(th.accent, -0.35), shade(th.pal(2), -0.3), shade(th.pal(1), -0.45)] } else { s.colors.iter().map(|c| c.0).collect() };
        let n = (cols.len() - 1).max(1) as f32;
        o.push(rect(x, y, w, h, 0.0, crate::dl::Paint::Linear { p0: [x, y], p1: [x + w, y + h], stops: cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect() }));
        // A few skeleton UI blocks so empty screens still read as "an app".
        let p = w * 0.08;
        let wh = Rgba([1.0, 1.0, 1.0, 0.16]);
        o.push(rect(x + p, y + h * 0.12, w * 0.45, h * 0.035, h * 0.017, solid(Rgba([1.0, 1.0, 1.0, 0.85]))));
        o.push(rect(x + p, y + h * 0.18, w * 0.7, h * 0.02, h * 0.01, solid(Rgba([1.0, 1.0, 1.0, 0.4]))));
        for i in 0..3 {
            o.push(rect(x + p, y + h * (0.27 + 0.2 * i as f32), w - 2.0 * p, h * 0.16, (w * 0.04).min(18.0), solid(wh)));
        }
    }
    o.push(Op::PopLayer);
}

/// Diagonal glass sheen.
fn reflection(o: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, strength: f32) {
    if strength <= 0.0 {
        return;
    }
    push_clip(o, x, y, w, h, r);
    let mut p = crate::dl::Path::new();
    p.move_to((x as f64, y as f64));
    p.line_to(((x + w * 0.62) as f64, y as f64));
    p.line_to(((x + w * 0.18) as f64, (y + h) as f64));
    p.line_to((x as f64, (y + h) as f64));
    p.close_path();
    o.push(fill(p, crate::dl::Paint::Linear { p0: [x, y], p1: [x + w * 0.5, y + h * 0.5], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.16 * strength])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] }));
    o.push(Op::PopLayer);
}

/// Shared entrance + float, applied about the device centre.
fn present(o: Vec<Op>, t: f32, delay: f32, float: f32, cx: f32, cy: f32) -> Vec<Op> {
    let p = spring(t - delay, 0.8);
    if p <= 0.0 {
        return Vec::new();
    }
    let mut o = o;
    // Whole-pixel bob: the device only moves when the rounded offset changes,
    // so most frames repeat exactly (cheap to render and encode).
    let bob = (float * ((t - delay) * 1.1).sin()).round();
    let m = crate::dl::mul(&translate(0.0, (1.0 - p) * 60.0 + bob), &scale_about(lerp(0.92, 1.0, p), cx, cy));
    transform_ops(&mut o, &m);
    with_opacity(o, clamp01(p * 1.5))
}

/// Contact shadow under a device: the shadow of a pill hidden behind the
/// device's own silhouette (sprite-cached by the renderer, so it is cheap).
/// Call it before drawing the device so the device covers the pill.
fn ground_shadow(o: &mut Vec<Op>, cx: f32, y: f32, w: f32, h: f32, dy: f32, blur: f32, a: f32) {
    o.push(Op::PushLayer { opacity: 1.0, blend: crate::dl::Blend::Normal, filters: vec![crate::dl::Filter::Shadow { dx: 0.0, dy, blur, color: Rgba([0.0, 0.0, 0.0, a]) }], clip: None, cache_key: None });
    o.push(rect(cx - w * 0.5, y, w, h, h * 0.5, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
    o.push(Op::PopLayer);
}

/// Fit an aspect ratio into the box, centred.
fn fit(ctx: &Ctx, aspect: f32, margin: f32) -> (f32, f32, f32, f32) {
    let (bw, bh) = (ctx.width * (1.0 - margin), ctx.height * (1.0 - margin));
    let (w, h) = if bw / bh > aspect { (bh * aspect, bh) } else { (bw, bw / aspect) };
    ((ctx.width - w) * 0.5, (ctx.height - h) * 0.5, w, h)
}

// ------------------------------------------------------------------ phone

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Phone {
    #[serde(flatten)]
    pub screen: Screen,
    /// Frame colour.
    pub color: Option<Color>,
    pub reflection: f32,
    pub shadow: bool,
    pub delay: f32,
    /// Gentle floating amplitude in pixels.
    pub float: f32,
    /// Status bar time text ("" to hide).
    pub time: String,
}
impl Default for Phone {
    fn default() -> Self {
        Phone { screen: Screen::default(), color: None, reflection: 1.0, shadow: true, delay: 0.0, float: 0.0, time: "9:41".into() }
    }
}
fn check_screen(s: &Screen) -> Result<(), super::Error> {
    match s.screen.as_deref() {
        Some(k) if !super::screens::KINDS.contains(&k) && k != "list" => Err(super::Error(match closest(k, super::screens::KINDS) {
            Some(c) => format!("unknown screen `{k}`; did you mean `{c}`?"),
            None => format!("unknown screen `{k}`; use one of {}", super::screens::KINDS.join(", ")),
        })),
        _ => Ok(()),
    }
}

impl Prepare for Phone {
    fn prepare(&mut self) -> Result<(), super::Error> {
        check_screen(&self.screen)
    }
}
impl Component for Phone {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (x, y, w, h) = fit(ctx, 9.0 / 19.5, 0.08);
        let frame = self.color.map(|c| c.0).unwrap_or(parse_color("#1b1c21").unwrap());
        let r = w * 0.17;
        let bz = w * 0.035;
        let mut o = Vec::new();
        if self.shadow {
            ground_shadow(&mut o, x + w * 0.5, y + h - w * 0.09, w * 0.6, w * 0.06, w * 0.1, w * 0.06, 0.6);
            shadow(&mut o, x, y, w, h, r, w * 0.06, w * 0.12, 0.5);
        }
        // Metal band, then black bezel, then screen.
        o.push(rect(x, y, w, h, r, crate::dl::Paint::Linear { p0: [x, y], p1: [x + w, y], stops: vec![(0.0, shade(frame, 0.25)), (0.08, frame), (0.92, frame), (1.0, shade(frame, 0.2))] }));
        o.push(rect(x + 2.0, y + 2.0, w - 4.0, h - 4.0, r - 2.0, solid(parse_color("#050506").unwrap())));
        let (sx, sy, sw, sh) = (x + bz, y + bz, w - 2.0 * bz, h - 2.0 * bz);
        screen(&mut o, ctx, &self.screen, t, sx, sy, sw, sh, r - bz);
        // Status bar and dynamic island.
        let iw = sw * 0.32;
        o.push(rect(x + (w - iw) * 0.5, sy + sw * 0.035, iw, sw * 0.09, sw * 0.045, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
        if !self.time.is_empty() {
            draw(&mut o, ctx.assets, &self.time, &TextStyle::new(Face::Sans, sw * 0.045, 600, Rgba([1.0, 1.0, 1.0, 0.95])), sx + sw * 0.15, sy + sw * 0.095, 0.5);
            // Battery.
            let (bx, by) = (sx + sw * 0.8, sy + sw * 0.062);
            o.push(rect(bx, by, sw * 0.065, sw * 0.032, sw * 0.008, solid(Rgba([1.0, 1.0, 1.0, 0.9]))));
        }
        // Home indicator.
        o.push(rect(x + w * 0.35, sy + sh - sw * 0.04, w * 0.3, sw * 0.012, sw * 0.006, solid(Rgba([1.0, 1.0, 1.0, 0.8]))));
        reflection(&mut o, sx, sy, sw, sh, r - bz, self.reflection);
        // Side buttons.
        o.push(rect(x + w - 1.0, y + h * 0.25, 3.0, h * 0.1, 1.5, solid(shade(frame, 0.1))));
        o.push(rect(x - 2.0, y + h * 0.2, 3.0, h * 0.06, 1.5, solid(shade(frame, 0.1))));
        o.push(rect(x - 2.0, y + h * 0.28, 3.0, h * 0.06, 1.5, solid(shade(frame, 0.1))));
        present(o, t, self.delay, self.float, x + w * 0.5, y + h * 0.5)
    }
}

// ------------------------------------------------------------------ laptop

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Laptop {
    #[serde(flatten)]
    pub screen: Screen,
    pub color: Option<Color>,
    pub reflection: f32,
    pub shadow: bool,
    pub delay: f32,
    pub float: f32,
    /// Lid opens from closed over this many seconds (0 = already open).
    pub open: f32,
}
impl Default for Laptop {
    fn default() -> Self {
        Laptop { screen: Screen::default(), color: None, reflection: 0.6, shadow: true, delay: 0.0, float: 0.0, open: 0.0 }
    }
}
impl Prepare for Laptop {
    fn prepare(&mut self) -> Result<(), super::Error> {
        check_screen(&self.screen)
    }
}
impl Component for Laptop {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (x, y, w, h) = fit(ctx, 1.6, 0.06);
        let body = self.color.map(|c| c.0).unwrap_or(parse_color("#c9ccd3").unwrap());
        let lid_h = h * 0.9;
        let lid_w = w * 0.84;
        let lx = x + (w - lid_w) * 0.5;
        let open = if self.open > 0.0 { ease_in_out(seg(t, self.delay, self.open)) } else { 1.0 };
        let mut o = Vec::new();
        let base_y = y + lid_h;
        if self.shadow {
            let bh0 = h * 0.045;
            ground_shadow(&mut o, x + w * 0.5, base_y + bh0 * 0.1, w * 0.8, bh0 * 0.8, bh0 * 1.6, w * 0.03, 0.6);
        }
        // Lid (scaled vertically while opening, hinged at the bottom).
        let mut lid = Vec::new();
        let r = lid_w * 0.025;
        lid.push(rect(lx, y, lid_w, lid_h, r, solid(parse_color("#0a0a0c").unwrap())));
        border(&mut lid, lx, y, lid_w, lid_h, r, Rgba([1.0, 1.0, 1.0, 0.12]));
        let bz = lid_w * 0.022;
        let (sx, sy, sw, sh) = (lx + bz, y + bz * 1.3, lid_w - 2.0 * bz, lid_h - bz * 2.3);
        screen(&mut lid, ctx, &self.screen, t, sx, sy, sw, sh, r * 0.3);
        lid.push(circle(lx + lid_w * 0.5, y + bz * 0.65, bz * 0.18, solid(parse_color("#26272c").unwrap())));
        reflection(&mut lid, sx, sy, sw, sh, r * 0.3, self.reflection);
        transform_ops(&mut lid, &[1.0, 0.0, 0.0, open.max(0.02), 0.0, base_y * (1.0 - open.max(0.02))]);
        o.extend(lid);
        // Base: thin wedge with a lip and a notch.
        let bh = h * 0.045;
        o.push(rect(x, base_y, w, bh, bh * 0.45, vgrad(base_y, base_y + bh, shade(body, 0.2), shade(body, -0.35))));
        o.push(rect(x + w * 0.42, base_y, w * 0.16, bh * 0.35, bh * 0.2, solid(shade(body, -0.25))));
        o.push(rect(x + 2.0, base_y, w - 4.0, 1.5, 0.0, solid(Rgba([1.0, 1.0, 1.0, 0.6]))));
        present(o, t, self.delay, self.float, x + w * 0.5, y + h * 0.5)
    }
}

// ------------------------------------------------------------------ browser

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Browser {
    #[serde(flatten)]
    pub screen: Screen,
    pub url: String,
    /// Tab titles; the first is active.
    pub tabs: Vec<String>,
    /// Type the URL in over this many seconds (0 = shown).
    pub type_url: f32,
    pub delay: f32,
    pub float: f32,
    pub shadow: bool,
    /// Aspect ratio of the whole window (default 16:10).
    pub aspect: f32,
}
impl Default for Browser {
    fn default() -> Self {
        Browser { screen: Screen::default(), url: "anikit.dev".into(), tabs: Vec::new(), type_url: 0.0, delay: 0.0, float: 0.0, shadow: true, aspect: 1.6 }
    }
}
impl Prepare for Browser {
    fn prepare(&mut self) -> Result<(), super::Error> {
        check_screen(&self.screen)
    }
}
impl Component for Browser {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let th = ctx.theme;
        let (x, y, w, h) = fit(ctx, self.aspect.max(0.3), 0.04);
        let r = (w * 0.014).max(8.0);
        let chrome_c = if th.dark { parse_color("#1a1b21").unwrap() } else { parse_color("#eceef2").unwrap() };
        let bar = (w * 0.05).clamp(30.0, 64.0);
        let mut o = Vec::new();
        if self.shadow {
            shadow(&mut o, x, y, w, h, r, w * 0.02, w * 0.05, if th.dark { 0.6 } else { 0.2 });
        }
        o.push(rect(x, y, w, h, r, solid(chrome_c)));
        let dot = bar * 0.13;
        for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
            o.push(circle(x + bar * 0.55 + i as f32 * dot * 3.2, y + bar * 0.5, dot, solid(parse_color(c).unwrap())));
        }
        let fs = bar * 0.34;
        // Tabs (when given), else a centred address bar.
        let ax = x + bar * 0.55 + dot * 9.0;
        let st = TextStyle::new(Face::Sans, fs, 500, th.muted);
        if !self.tabs.is_empty() {
            let mut tx = ax;
            for (i, tab) in self.tabs.iter().enumerate() {
                let tw = (ctx.assets.measure(tab, &st) + fs * 3.0).min(w * 0.22);
                if i == 0 {
                    o.push(rect(tx, y + bar * 0.18, tw, bar * 0.64, bar * 0.2, solid(if th.dark { parse_color("#2a2b33").unwrap() } else { Rgba([1.0, 1.0, 1.0, 1.0]) })));
                }
                o.push(circle(tx + fs * 0.9, y + bar * 0.5, fs * 0.3, solid(th.pal(i))));
                draw(&mut o, ctx.assets, tab, &st.clone().color(if i == 0 { th.text } else { th.muted }), tx + fs * 1.6, y + bar * 0.5 + fs * 0.35, 0.0);
                tx += tw + fs * 0.4;
            }
        }
        // Address bar.
        let (uw, uh) = (w * 0.42, bar * 0.6);
        let (ux, uy) = (x + (w - uw) * 0.5, y + bar + bar * 0.15);
        let content_y = if self.tabs.is_empty() { y + bar } else { uy + uh + bar * 0.15 };
        let (ux, uy) = if self.tabs.is_empty() { (x + (w - uw) * 0.5, y + (bar - uh) * 0.5) } else { (ux, uy) };
        o.push(rect(ux, uy, uw, uh, uh * 0.5, solid(if th.dark { parse_color("#0f1014").unwrap() } else { Rgba([1.0, 1.0, 1.0, 1.0]) })));
        let n = self.url.chars().count();
        let k = if self.type_url > 0.0 { ((seg(t, self.delay + 0.3, self.type_url)) * n as f32).round() as usize } else { n };
        let url: String = self.url.chars().take(k).collect();
        let lock = fs * 0.8;
        let us = st.clone().color(th.text);
        let tw = ctx.assets.measure(&url, &us);
        let cx = ux + uw * 0.5 - (tw + lock * 1.4) * 0.5;
        o.push(rect(cx, uy + uh * 0.5 - lock * 0.2, lock * 0.75, lock * 0.6, lock * 0.12, solid(th.muted)));
        o.push(Op::Stroke { path: rrect_path(cx + lock * 0.15, uy + uh * 0.5 - lock * 0.5, lock * 0.45, lock * 0.6, lock * 0.22), paint: solid(th.muted), stroke: line(lock * 0.12, [0.0, 0.62]), transform: IDENTITY });
        draw(&mut o, ctx.assets, &url, &us, cx + lock * 1.4, uy + uh * 0.5 + fs * 0.35, 0.0);
        o.push(rect(x, content_y - 1.0, w, 1.0, 0.0, solid(th.border)));
        // Content: rounded only at the bottom (clip with the window, inset by the bar).
        push_clip(&mut o, x, y, w, h, r);
        screen(&mut o, ctx, &self.screen, t, x, content_y, w, y + h - content_y, 0.0);
        o.push(Op::PopLayer);
        border(&mut o, x, y, w, h, r, th.border);
        present(o, t, self.delay, self.float, x + w * 0.5, y + h * 0.5)
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
        // A tall "screenshot" to scroll through.
        let (iw, ih) = (360u32, 1400u32);
        let mut px = Vec::with_capacity((iw * ih * 4) as usize);
        for y in 0..ih {
            for x in 0..iw {
                let band = (y / 120) % 2 == 0;
                px.extend_from_slice(&[if band { 40 } else { 90 }, (x * 255 / iw) as u8, (y * 255 / ih) as u8, 255]);
            }
        }
        a.add_media("shot.png", iw, ih, px);
        let mut ops = Vec::new();
        for (k, p) in [
            ("phone", json!({"x": 20, "y": 20, "width": 240, "height": 500, "src": "shot.png", "scroll_to": 1, "scroll_duration": 2})),
            ("laptop", json!({"x": 260, "y": 30, "width": 680, "height": 240, "screen": "dashboard", "title": "Northwind"})),
            ("browser", json!({"x": 300, "y": 290, "width": 620, "height": 240, "url": "anikit.dev/video", "tabs": ["Anikit Video", "Docs"], "screen": "editor", "title": "Anikit"})),
        ] {
            ops.extend(build(k, &p).unwrap().eval(1.5, 960.0, 540.0, 30.0, &th, a));
        }
        preview::validate(&ops).unwrap();
        preview::snapshot("devices", 960, 540, th.background, ops, a);
        // Every generated screen, phone and laptop.
        for (i, kind) in ["app", "dashboard", "chat", "feed", "editor", "settings"].iter().enumerate() {
            let mut ops = Vec::new();
            ops.extend(build("phone", &json!({"x": 0, "y": 0, "width": 300, "height": 540, "screen": kind, "title": "Northwind", "accent": "#ff6fb1"})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
            ops.extend(build("laptop", &json!({"x": 300, "y": 60, "width": 660, "height": 420, "screen": kind, "title": "Northwind"})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
            preview::validate(&ops).unwrap();
            preview::snapshot(&format!("screen-{i}-{kind}"), 960, 540, th.background, ops, a);
        }
        assert!(build("phone", &json!({"screen": "dashbord"})).err().unwrap().0.contains("dashboard"));
        // Product copy instead of invented data.
        let items = json!(["Capture in one keystroke", "Handwriting search in sketches", "Offline mode", "Shared notebooks with live cursors", "Templates", "Version history"]);
        let mut ops = Vec::new();
        ops.extend(build("phone", &json!({"x": 0, "y": 0, "width": 300, "height": 540, "screen": "app", "title": "Quill", "items": items})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        ops.extend(build("browser", &json!({"x": 300, "y": 20, "width": 660, "height": 420, "screen": "app", "title": "Quill", "items": items})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        preview::validate(&ops).unwrap();
        preview::snapshot("screen-quill", 960, 540, th.background, ops, a);
        let ops = build("laptop", &json!({"screen": "dashboard", "title": "Quill", "items": items, "kpis": [{"label": "Notes captured this week", "value": "12,400"}]})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a);
        preview::validate(&ops).unwrap();
        preview::snapshot("screen-quill-kpi", 960, 540, th.background, ops, a);
    }
}
