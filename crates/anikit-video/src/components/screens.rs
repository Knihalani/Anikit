//! Generated app screens for device mockups: `app`, `dashboard`, `chat`,
//! `feed`, `editor` and `settings`.
//!
//! Each screen is laid out like a real product in design points (a desktop
//! app ~1280 pt wide, an iPhone 390 pt wide) and scaled to the device, so
//! type sizes, hairlines and density match real software. Desktop screens
//! get a sidebar and toolbar; phone screens get iOS navigation and a tab
//! bar. Look parameters ([`Look`]) change appearance, density, corners,
//! heading type, grey family and motion, so two products never look alike.
//! Product copy (`title`, `items`, `kpis`) always wins over sample content.

use super::style::*;
use super::text::{draw, Face, TextStyle};
use super::uikit::{self as ui, ts, Pal, Tint};
use super::Ctx;
use crate::dl::{Blend, Op, Paint, Path, Rgba, IDENTITY};
use kurbo::ParamCurve;

/// Screen kinds, for validation and docs.
pub const KINDS: &[&str] = &["app", "dashboard", "chat", "feed", "editor", "settings"];

/// Product copy for generated screens.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kpi {
    pub label: String,
    pub value: String,
}

/// How dense the UI is (design width of the screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Density {
    /// Real-app density (small type, lots on screen).
    Compact,
    #[default]
    Comfortable,
    /// Big type for small embeds and vertical video.
    Large,
}

/// How much the screen moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Motion {
    /// No build-in, no live motion: a still product shot.
    Still,
    /// Soft build-in and small live details (default).
    #[default]
    Subtle,
    /// Snappier build-in, more live activity (hover glides, likes, typing).
    Lively,
}

/// Which device the screen is shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Phone,
    Laptop,
    Browser,
}

/// Appearance of a generated screen.
#[derive(Clone, Debug, Default)]
pub struct Look {
    /// `Some(true)` dark UI, `Some(false)` light UI, `None` follow the theme.
    pub dark: Option<bool>,
    pub density: Density,
    /// Card corner radius in design points (default 10 desktop, 14 phone).
    pub radius: Option<f32>,
    /// Face for headings and big numbers.
    pub font: Face,
    pub tint: Tint,
    pub motion: Motion,
}

impl Look {
    /// Parse the string params; errors name the bad value.
    pub fn parse(ui: &str, density: &str, font: &str, tint: &str, motion: &str, radius: Option<f32>) -> Result<Look, String> {
        let bad = |k: &str, v: &str, opts: &[&str]| -> String {
            match closest(v, opts) {
                Some(s) => format!("unknown {k} `{v}`; did you mean `{s}`?"),
                None => format!("unknown {k} `{v}`; use one of {}", opts.join(", ")),
            }
        };
        Ok(Look {
            dark: match ui {
                "" | "auto" | "theme" => None,
                "dark" => Some(true),
                "light" => Some(false),
                v => return Err(bad("ui", v, &["auto", "light", "dark"])),
            },
            density: match density {
                "" | "comfortable" | "default" | "regular" => Density::Comfortable,
                "compact" | "dense" => Density::Compact,
                "large" | "spacious" | "big" => Density::Large,
                v => return Err(bad("density", v, &["compact", "comfortable", "large"])),
            },
            font: match font {
                "" | "sans" | "inter" => Face::Sans,
                "display" => Face::Display,
                "serif" => Face::Serif,
                "mono" => Face::Mono,
                v => return Err(bad("font", v, &["sans", "display", "serif", "mono"])),
            },
            tint: if tint.is_empty() { Tint::Neutral } else { Tint::parse(tint).ok_or_else(|| bad("tint", tint, &["neutral", "warm", "cool", "accent"]))? },
            motion: match motion {
                "" | "subtle" | "default" => Motion::Subtle,
                "still" | "none" | "static" => Motion::Still,
                "lively" | "playful" | "busy" => Motion::Lively,
                v => return Err(bad("motion", v, &["still", "subtle", "lively"])),
            },
            radius: radius.map(|r| r.clamp(0.0, 32.0)),
        })
    }
}

/// What the screen shows.
pub struct Content<'a> {
    pub kind: &'a str,
    pub title: &'a str,
    pub items: &'a [String],
    pub kpis: &'a [Kpi],
    pub accent: Option<Rgba>,
}

/// Whether the screen will render dark (for status-bar ink and glass).
pub fn is_dark(ctx: &Ctx, look: &Look) -> bool {
    look.dark.unwrap_or(ctx.theme.dark)
}

/// Background colour of the screen (what shows under the status bar).
pub fn background(ctx: &Ctx, look: &Look, accent: Option<Rgba>) -> Rgba {
    Pal::new(is_dark(ctx, look), accent.unwrap_or(ctx.theme.accent), look.tint).bg
}

struct S<'a> {
    ctx: &'a Ctx<'a>,
    p: Pal,
    /// Pixels per design point.
    u: f32,
    t: f32,
    title: &'a str,
    items: &'a [String],
    kpis: &'a [Kpi],
    /// Card radius (px).
    r: f32,
    head: Face,
    motion: Motion,
    host: Host,
    kind: &'a str,
}

impl S<'_> {
    fn a(&self) -> &dyn super::Assets {
        self.ctx.assets
    }
    /// Item `i` of the product copy, or `fallback`.
    fn it<'b>(&'b self, i: usize, fallback: &'b str) -> &'b str {
        if self.items.is_empty() {
            fallback
        } else {
            &self.items[i % self.items.len()]
        }
    }
    fn st(&self, size: f32, weight: u16, c: Rgba) -> TextStyle {
        ts(Face::Sans, size * self.u, weight, c)
    }
    /// Heading style in the look's face.
    fn hst(&self, size: f32, weight: u16, c: Rgba) -> TextStyle {
        match self.head {
            Face::Serif => ts(Face::Serif, size * 1.22 * self.u, 400, c),
            Face::Mono => ts(Face::Mono, size * 0.9 * self.u, weight.min(600), c).tracking(-0.01),
            f => ts(f, size * self.u, weight, c),
        }
    }
    /// Big-number style (tabular).
    fn nst(&self, size: f32, c: Rgba) -> TextStyle {
        match self.head {
            Face::Serif => ts(Face::Serif, size * 1.2 * self.u, 400, c),
            Face::Mono => ts(Face::Mono, size * 0.88 * self.u, 500, c),
            _ => ts(Face::Display, size * self.u, 600, c).tabular(),
        }
    }
    fn txt(&self, o: &mut Vec<Op>, s: &str, size: f32, weight: u16, c: Rgba, x: f32, y: f32, align: f32) -> f32 {
        draw(o, self.a(), s, &self.st(size, weight, c), x, y, align)
    }
    fn fit(&self, o: &mut Vec<Op>, s: &str, size: f32, weight: u16, c: Rgba, x: f32, y: f32, maxw: f32) -> f32 {
        ui::fit(o, self.a(), s, &self.st(size, weight, c), x, y, maxw, 0.0)
    }
    fn ic(&self, o: &mut Vec<Op>, name: &str, x: f32, y: f32, size: f32, c: Rgba) {
        ui::icon(o, name, x, y, size * self.u, c, 1.8);
    }
    fn live(&self) -> f32 {
        match self.motion {
            Motion::Still => 0.0,
            Motion::Subtle => 1.0,
            Motion::Lively => 1.6,
        }
    }
    /// Time used by live details; frozen at a good-looking moment when still.
    fn lt(&self, still_at: f32) -> f32 {
        if self.motion == Motion::Still {
            still_at
        } else {
            self.t
        }
    }
    /// Entrance progress of element `i`.
    fn k(&self, i: usize) -> f32 {
        match self.motion {
            Motion::Still => 1.0,
            Motion::Subtle => ease_out(seg(self.t, 0.08 + i as f32 * 0.045, 0.6)),
            Motion::Lively => ease_out(seg(self.t, 0.05 + i as f32 * 0.06, 0.45)),
        }
    }
    /// Push `ops` with element `i`'s entrance (short rise + fade).
    fn item(&self, o: &mut Vec<Op>, i: usize, mut ops: Vec<Op>) {
        let k = self.k(i);
        if k <= 0.0 || ops.is_empty() {
            return;
        }
        if k < 1.0 {
            let rise = if self.motion == Motion::Lively { 14.0 } else { 8.0 };
            super::transform_ops(&mut ops, &translate(0.0, (1.0 - k) * rise * self.u));
            o.extend(super::with_opacity(ops, k));
        } else {
            o.extend(ops);
        }
    }
    fn card(&self, o: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32) {
        ui::card(o, &self.p, x, y, w, h, self.r, true);
    }
    fn hair(&self, o: &mut Vec<Op>, x: f32, y: f32, w: f32) {
        o.push(rect(x, y, w, 1.0, 0.0, solid(self.p.line)));
    }
    fn vhair(&self, o: &mut Vec<Op>, x: f32, y: f32, h: f32) {
        o.push(rect(x, y, 1.0, h, 0.0, solid(self.p.line)));
    }
    fn slug(&self) -> String {
        let s: String = self.title.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
        if s.is_empty() {
            "acme".into()
        } else {
            s
        }
    }
}

/// Draw a generated screen into `(x, y, w, h)`; `top` is a status-bar inset
/// (phones). `t` is seconds since the device arrived.
pub fn draw_screen(o: &mut Vec<Op>, ctx: &Ctx, c: &Content, look: &Look, host: Host, t: f32, x: f32, y: f32, w: f32, h: f32, top: f32) {
    if !(w.is_finite() && h.is_finite() && x.is_finite() && y.is_finite() && t.is_finite() && w >= 2.0 && h >= 2.0) {
        return;
    }
    let dark = is_dark(ctx, look);
    let p = Pal::new(dark, c.accent.unwrap_or(ctx.theme.accent), look.tint);
    let mobile = h > w * 1.05;
    let u = if mobile {
        let d = match look.density {
            Density::Compact => 420.0,
            Density::Comfortable => 390.0,
            Density::Large => 340.0,
        };
        (w / d).min(h / (d * 1.55))
    } else {
        let d = match look.density {
            Density::Compact => 1440.0,
            Density::Comfortable => 1240.0,
            Density::Large => 1000.0,
        };
        (w / d).min(h / (d * 0.5))
    };
    let r = look.radius.unwrap_or(if mobile { 14.0 } else { 10.0 }) * u;
    let title = if c.title.trim().is_empty() { "Acme" } else { c.title.trim() };
    let s = S { ctx, p, u, t, title, items: c.items, kpis: c.kpis, r, head: look.font, motion: look.motion, host, kind: c.kind };
    o.push(rect(x, y, w, h, 0.0, solid(s.p.bg)));
    push_clip(o, x, y, w, h, 0.0);
    if mobile {
        let y0 = y + top;
        let (cx, cy, cw, ch) = (x, y0, w, h - top);
        match c.kind {
            "chat" => m_chat(o, &s, cx, cy, cw, ch),
            "feed" | "list" => m_feed(o, &s, cx, cy, cw, ch),
            "editor" => m_editor(o, &s, cx, cy, cw, ch),
            "settings" => m_settings(o, &s, cx, cy, cw, ch),
            "app" => m_app(o, &s, cx, cy, cw, ch),
            _ => m_dashboard(o, &s, cx, cy, cw, ch),
        }
    } else if c.kind == "editor" {
        d_editor(o, &s, x, y, w, h);
    } else {
        let (mx, my, mw, mh) = d_chrome(o, &s, x, y, w, h);
        match c.kind {
            "chat" => d_chat(o, &s, mx, my, mw, mh),
            "feed" | "list" => d_feed(o, &s, mx, my, mw, mh),
            "settings" => d_settings(o, &s, mx, my, mw, mh),
            "app" => d_app(o, &s, mx, my, mw, mh),
            _ => d_dashboard(o, &s, mx, my, mw, mh),
        }
    }
    o.push(Op::PopLayer);
}

// ====================================================================== desktop chrome

/// Sidebar + top bar. Returns the content rect below the top bar.
fn d_chrome(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let u = s.u;
    let p = &s.p;
    let a = s.a();
    let sbw = if w / u >= 860.0 { 232.0 * u } else { 0.0 };
    if sbw > 0.0 {
        let mut e = Vec::new();
        e.push(rect(x, y, sbw, h, 0.0, solid(p.side)));
        s.vhair(&mut e, x + sbw - 1.0, y, h);
        let mut yy = y + 14.0 * u;
        if s.host == Host::Laptop {
            for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
                let cc = parse_color(c).unwrap();
                e.push(circle(x + (20.0 + i as f32 * 20.0) * u, yy + 6.0 * u, 6.0 * u, solid(cc)));
                e.push(stroke_op(circle_path(x + (20.0 + i as f32 * 20.0) * u, yy + 6.0 * u, 5.6 * u), solid(Rgba([0.0, 0.0, 0.0, 0.14])), 0.8 * u, [0.0, 1.0]));
            }
            yy += 26.0 * u;
        }
        // Workspace switcher.
        let rowc = yy + 16.0 * u;
        ui::logo(&mut e, a, s.title, p.acc, x + 14.0 * u, rowc - 11.0 * u, 22.0 * u);
        let tw = s.fit(&mut e, s.title, 14.0, 600, p.text, x + 44.0 * u, ui::mid(rowc, 14.0 * u), sbw - 110.0 * u);
        s.ic(&mut e, "chevron-down", x + 48.0 * u + tw, rowc - 6.0 * u, 12.0, p.muted);
        s.ic(&mut e, "edit", x + sbw - 34.0 * u, rowc - 8.0 * u, 16.0, p.muted);
        yy += 42.0 * u;
        // Search.
        ui::field(&mut e, a, p, "Search", Some("search"), x + 12.0 * u, yy, sbw - 24.0 * u, 30.0 * u, 7.0 * u, Some("⌘K"));
        yy += 44.0 * u;
        // Primary nav.
        let nav: [(&str, &str); 5] = [("home", "Home"), ("inbox", "Inbox"), ("chart", "Analytics"), ("message", "Messages"), ("layers", "Projects")];
        let active = match s.kind {
            "dashboard" => 2,
            "chat" => 3,
            "settings" => 99,
            _ => 0,
        };
        for (i, (icn, label)) in nav.iter().enumerate() {
            let cy = yy + 15.0 * u;
            if i == active {
                e.push(rect(x + 8.0 * u, yy, sbw - 16.0 * u, 30.0 * u, 7.0 * u, solid(p.hover)));
            }
            let c = if i == active { p.text } else { p.text2 };
            s.ic(&mut e, icn, x + 18.0 * u, cy - 8.0 * u, 16.0, if i == active { p.text } else { p.muted });
            s.txt(&mut e, label, 13.5, 500, c, x + 44.0 * u, ui::mid(cy, 13.5 * u), 0.0);
            if i == 1 {
                let st = s.st(11.0, 600, p.muted);
                let bw = 20.0 * u;
                e.push(rect(x + sbw - 18.0 * u - bw, cy - 9.0 * u, bw, 18.0 * u, 5.0 * u, solid(p.sunken)));
                draw(&mut e, a, "4", &st, x + sbw - 18.0 * u - bw * 0.5, ui::mid(cy, 11.0 * u), 0.5);
            }
            yy += 32.0 * u;
        }
        // Workspace section: the product's own items.
        yy += 16.0 * u;
        s.txt(&mut e, "Workspace", 12.0, 500, p.faint, x + 18.0 * u, yy + 4.0 * u, 0.0);
        s.ic(&mut e, "plus", x + sbw - 32.0 * u, yy - 7.0 * u, 14.0, p.faint);
        yy += 14.0 * u;
        let defaults = ["Product", "Design", "Growth", "Engineering", "Operations"];
        let n = if s.items.is_empty() { defaults.len() } else { s.items.len() };
        let bottom = y + h - 96.0 * u;
        for i in 0..n {
            if yy + 30.0 * u > bottom {
                break;
            }
            let cy = yy + 15.0 * u;
            let label = if s.items.is_empty() { defaults[i] } else { s.items[i].as_str() };
            let hc = ui::hue(i + 1);
            e.push(rect(x + 21.0 * u, cy - 5.0 * u, 10.0 * u, 10.0 * u, 3.0 * u, solid(hc)));
            s.fit(&mut e, label, 13.5, 500, p.text2, x + 44.0 * u, ui::mid(cy, 13.5 * u), sbw - 60.0 * u);
            yy += 30.0 * u;
        }
        // Bottom: settings + account.
        let by = y + h - 88.0 * u;
        let sel = s.kind == "settings";
        if sel {
            e.push(rect(x + 8.0 * u, by, sbw - 16.0 * u, 30.0 * u, 7.0 * u, solid(p.hover)));
        }
        s.ic(&mut e, "settings", x + 18.0 * u, by + 7.0 * u, 16.0, if sel { p.text } else { p.muted });
        s.txt(&mut e, "Settings", 13.5, 500, if sel { p.text } else { p.text2 }, x + 44.0 * u, ui::mid(by + 15.0 * u, 13.5 * u), 0.0);
        s.hair(&mut e, x + 12.0 * u, by + 40.0 * u, sbw - 24.0 * u);
        let ay = by + 62.0 * u;
        e.push(circle(x + 30.0 * u, ay, 13.0 * u, solid(p.sunken)));
        s.ic(&mut e, "user", x + 22.0 * u, ay - 8.0 * u, 16.0, p.muted);
        s.txt(&mut e, "Your account", 13.0, 500, p.text2, x + 52.0 * u, ui::mid(ay, 13.0 * u), 0.0);
        s.ic(&mut e, "chevrons-updown", x + sbw - 32.0 * u, ay - 7.0 * u, 14.0, p.faint);
        s.item(o, 0, e);
    }
    // Top bar.
    let (mx, mw) = (x + sbw, w - sbw);
    let bh = 52.0 * u;
    let mut e = Vec::new();
    s.hair(&mut e, mx, y + bh - 1.0, mw);
    let (page, picon) = match s.kind {
        "dashboard" => ("Analytics", "chart"),
        "chat" => ("general", "hash"),
        "feed" | "list" => ("Home", "home"),
        "settings" => ("Settings", "settings"),
        _ => ("Updates", "sparkles"),
    };
    let cy = y + bh * 0.5;
    let mut bx = mx + 24.0 * u;
    if sbw == 0.0 {
        ui::logo(&mut e, s.a(), s.title, p.acc, bx, cy - 11.0 * u, 22.0 * u);
        bx += 32.0 * u;
        bx += s.txt(&mut e, s.title, 14.0, 600, p.text, bx, ui::mid(cy, 14.0 * u), 0.0) + 8.0 * u;
        s.ic(&mut e, "chevron-right", bx, cy - 7.0 * u, 14.0, p.faint);
        bx += 22.0 * u;
    }
    s.ic(&mut e, picon, bx, cy - 8.0 * u, 16.0, p.muted);
    bx += 24.0 * u;
    let pw = s.txt(&mut e, page, 14.0, 500, p.text, bx, ui::mid(cy, 14.0 * u), 0.0);
    if s.kind == "chat" {
        s.txt(&mut e, "· 18 members", 13.0, 400, p.muted, bx + pw + 8.0 * u, ui::mid(cy, 13.0 * u), 0.0);
    }
    // Right actions.
    let mut rx = mx + mw - 20.0 * u;
    let bh2 = 30.0 * u;
    let br = (s.r * 0.7).max(4.0 * u);
    match s.kind {
        "dashboard" => {
            rx -= ui::button(&mut e, s.a(), p, "Export", Some("download"), rx, cy - bh2 * 0.5, bh2, false, br, true) + 10.0 * u;
            let sel = 1.0 + if s.motion == Motion::Lively { ease_in_out(seg(s.t, 2.2, 0.4)) } else { 0.0 };
            ui::segmented(&mut e, s.a(), p, &["7D", "30D", "3M", "12M"], sel, rx, cy - bh2 * 0.5, bh2, true);
        }
        "settings" => {}
        "chat" => {
            rx -= ui::button(&mut e, s.a(), p, "", Some("search"), rx, cy - bh2 * 0.5, bh2, false, br, true) + 12.0 * u;
            for (i, n) in ["Sam Park", "Jordan Lee", "Maya Chen"].iter().enumerate() {
                ui::avatar(&mut e, s.a(), n, rx - 12.0 * u - i as f32 * 18.0 * u, cy, 12.0 * u, Some(p.bg));
            }
        }
        _ => {
            let label = if s.kind == "app" { "New" } else { "Post" };
            rx -= ui::button(&mut e, s.a(), p, label, Some("plus"), rx, cy - bh2 * 0.5, bh2, true, br, true) + 10.0 * u;
            rx -= ui::button(&mut e, s.a(), p, "Filter", Some("filter"), rx, cy - bh2 * 0.5, bh2, false, br, true) + 14.0 * u;
            if s.kind == "app" {
                for (i, n) in ["Sam Park", "Jordan Lee", "Maya Chen"].iter().enumerate() {
                    ui::avatar(&mut e, s.a(), n, rx - 12.0 * u - i as f32 * 18.0 * u, cy, 12.0 * u, Some(p.bg));
                }
            }
        }
    }
    s.item(o, 1, e);
    (mx, y + bh, mw, h - bh)
}

// ====================================================================== shared data

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn series(prev: bool) -> Vec<f32> {
    (0..12)
        .map(|i| {
            let f = i as f32;
            if prev {
                34.0 + f * 2.6 + 5.0 * (f * 0.9 + 1.3).sin()
            } else {
                38.0 + f * 4.1 + 6.0 * (f * 0.8).sin() + 5.0 * hash01(i as u32 * 7 + 3) - (if i == 5 { 7.0 } else { 0.0 })
            }
        })
        .collect()
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

/// KPI tiles: (label, value text, delta) for slot `i`, counting up with `k`.
fn kpi(s: &S, i: usize, k: f64) -> (String, String, Option<f32>) {
    if let Some(kp) = s.kpis.get(i) {
        return (kp.label.clone(), count_up(&kp.value, k), None);
    }
    let d = [("Revenue", 48_290.0, "$", 12.4f32), ("Active users", 12_840.0, "", 8.1), ("Conversion", 4.62, "%", -0.4), ("Avg. order", 86.4, "$", 3.2)][i % 4];
    let v = d.1 * k;
    let body = match d.2 {
        "%" => format!("{:.2}%", v),
        "$" if d.1 < 1000.0 => format!("${:.2}", v),
        "$" => format!("${}", format_number(v, 0, true)),
        _ => format_number(v, 0, true),
    };
    (d.0.to_string(), body, Some(d.3))
}

/// Delta chip "↑ 12.4%".
fn delta(o: &mut Vec<Op>, s: &S, d: f32, x: f32, cy: f32, size: f32) -> f32 {
    let u = s.u;
    let c = if d >= 0.0 { s.p.pos } else { s.p.neg };
    let label = format!("{:.1}%", d.abs());
    let st = s.st(size, 600, c).tabular();
    let tw = s.a().measure(&label, &st);
    let h = size * 1.7 * u;
    let isz = size * 0.95 * u;
    let w = tw + isz + size * 1.3 * u;
    o.push(rect(x, cy - h * 0.5, w, h, h * 0.32, solid(alpha(c, 0.12))));
    let mut ar = ui::icon_path("arrow-up", x + size * 0.45 * u, cy - isz * 0.5, isz);
    if d < 0.0 {
        ar.apply_affine(kurbo::Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, 2.0 * cy as f64]));
    }
    o.push(Op::Stroke { path: ar, paint: solid(c), stroke: line(2.4 * isz / 24.0, [0.0, 1.0]), transform: IDENTITY });
    draw(o, s.a(), &label, &st, x + size * 0.55 * u + isz, ui::mid(cy, size * u), 0.0);
    w
}

/// Area chart with axes, gridlines, comparison line, draw-on and a hover
/// crosshair with tooltip.
fn area_chart(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, money: bool, yticks: usize, xstep: usize) {
    let u = s.u;
    let p = &s.p;
    let cur = series(false);
    let prev = series(true);
    let maxv = 100.0f32;
    let lab_w = 40.0 * u;
    let (px, py, pw, ph) = (x + lab_w, y + 6.0 * u, w - lab_w, h - 30.0 * u);
    // Grid + y labels.
    for i in 0..=yticks {
        let f = i as f32 / yticks as f32;
        let gy = py + ph - ph * f;
        let v = maxv * f;
        let label = if money { if v == 0.0 { "$0".to_string() } else { format!("${}k", v as i32) } } else { format!("{}", v as i32) };
        s.txt(o, &label, 11.0, 400, p.faint, px - 10.0 * u, ui::mid(gy, 11.0 * u), 1.0);
        if i == 0 {
            o.push(rect(px, gy, pw, 1.0, 0.0, solid(p.line2)));
        } else {
            o.push(Op::Stroke { path: { let mut l = Path::new(); l.move_to((px as f64, gy as f64)); l.line_to(((px + pw) as f64, gy as f64)); l }, paint: solid(p.line), stroke: crate::dl::Stroke { width: 1.0, cap: 0, join: 0, miter_limit: 4.0, dash: vec![3.0 * u, 3.0 * u], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY });
        }
    }
    let n = cur.len();
    let xs = |i: usize| px + 8.0 * u + (pw - 16.0 * u) * i as f32 / (n - 1) as f32;
    for i in (0..n).step_by(xstep.max(1)) {
        s.txt(o, MONTHS[i], 11.0, 400, p.faint, xs(i), py + ph + 19.0 * u, 0.5);
    }
    let pts = |d: &[f32]| -> Vec<(f32, f32)> { d.iter().enumerate().map(|(i, v)| (xs(i), py + ph - ph * v / maxv)).collect() };
    let cp = pts(&cur);
    let line_c = smooth_path(&cp, 1.0);
    let line_p = smooth_path(&pts(&prev), 1.0);
    let k = if s.motion == Motion::Still { 1.0 } else { ease_in_out(seg(s.t, 0.45, 1.3)) };
    if k <= 0.0 {
        return;
    }
    o.push(Op::PushLayer { opacity: 1.0, blend: Blend::Normal, filters: vec![], clip: Some((rrect_path(px - 4.0, py - 20.0 * u, (pw + 8.0) * k, ph + 22.0 * u, 0.0), IDENTITY)), cache_key: None });
    o.push(Op::Stroke { path: line_p, paint: solid(alpha(p.muted, 0.55)), stroke: crate::dl::Stroke { width: 1.5 * u, cap: 1, join: 1, miter_limit: 4.0, dash: vec![4.0 * u, 4.0 * u], dash_offset: 0.0, trim: [0.0, 1.0] }, transform: IDENTITY });
    let mut area = line_c.clone();
    area.line_to((cp[n - 1].0 as f64, (py + ph) as f64));
    area.line_to((cp[0].0 as f64, (py + ph) as f64));
    area.close_path();
    o.push(fill(area, vgrad(py, py + ph, alpha(p.acc, if p.dark { 0.28 } else { 0.18 }), alpha(p.acc, 0.0))));
    o.push(stroke_op(line_c.clone(), solid(p.acc), 2.0 * u, [0.0, 1.0]));
    o.push(Op::PopLayer);
    // Hover crosshair, gliding between points once drawn.
    let lt = s.lt(3.0);
    let hk = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(lt, 1.7, 0.35)) };
    if hk <= 0.0 {
        return;
    }
    let stops = [9.0f32, 7.0, 10.0, 8.0];
    let period = if s.motion == Motion::Lively { 1.4 } else { 2.2 };
    let ph_t = ((lt - 1.7) / period).max(0.0);
    let seg_i = ph_t.floor() as usize;
    let f = ease_in_out(((ph_t.fract() - 0.55) / 0.45).clamp(0.0, 1.0));
    let fi = if s.live() == 0.0 { stops[0] } else { lerp(stops[seg_i % 4], stops[(seg_i + 1) % 4], f) };
    let i0 = (fi.floor() as usize).min(n - 2);
    let ff = (fi - i0 as f32) as f64;
    let pt = line_c.segments().nth(i0).map(|sg| sg.eval(ff)).unwrap_or(kurbo::Point::new(cp[i0].0 as f64, cp[i0].1 as f64));
    let (hx, hy) = (pt.x as f32, pt.y as f32);
    let mut e = vec![rect(hx - 0.5, py, 1.0, ph, 0.0, solid(p.line2)), circle(hx, hy, 6.5 * u, solid(alpha(p.acc, 0.22))), circle(hx, hy, 4.0 * u, solid(p.bg)), circle(hx, hy, 3.0 * u, solid(p.acc))];
    let vi = lerp(cur[i0], cur[(i0 + 1).min(n - 1)], ff as f32);
    let label = if money { format!("${:.1}k", vi) } else { format!("{:.0}", vi * 10.0) };
    let month = MONTHS[(fi.round() as usize).min(11)];
    let tw = 112.0 * u;
    let th = 50.0 * u;
    let tx = (hx + 14.0 * u).min(px + pw - tw);
    let tx = if tx < hx && hx - 14.0 * u - tw > px { hx - 14.0 * u - tw } else { tx };
    let ty = (hy - th - 14.0 * u).max(py - 4.0 * u);
    shadow(&mut e, tx, ty, tw, th, 8.0 * u, 4.0 * u, 12.0 * u, if p.dark { 0.5 } else { 0.14 });
    e.push(rect(tx, ty, tw, th, 8.0 * u, solid(if p.dark { p.sunken } else { p.panel })));
    border(&mut e, tx, ty, tw, th, 8.0 * u, p.line2);
    s.txt(&mut e, &format!("{month} 2026"), 11.0, 500, p.muted, tx + 12.0 * u, ty + 19.0 * u, 0.0);
    e.push(circle(tx + 16.0 * u, ty + 35.0 * u, 3.5 * u, solid(p.acc)));
    draw(&mut e, s.a(), &label, &s.st(13.0, 600, p.text).tabular(), tx + 26.0 * u, ty + 39.5 * u, 0.0);
    o.extend(super::with_opacity(e, hk));
}

// ====================================================================== desktop screens

fn page_title(o: &mut Vec<Op>, s: &S, title: &str, sub: &str, x: f32, y: f32, maxw: f32) {
    let u = s.u;
    let st = s.hst(22.0, 600, s.p.text);
    ui::fit(o, s.a(), title, &st, x, y + 26.0 * u, maxw, 0.0);
    if !sub.is_empty() {
        s.fit(o, sub, 13.5, 400, s.p.muted, x, y + 50.0 * u, maxw);
    }
}

fn d_dashboard(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let pad = 28.0 * u;
    let (x, w) = (x + pad, w - 2.0 * pad);
    let mut e = Vec::new();
    page_title(&mut e, s, "Overview", "", x, y + 10.0 * u, w * 0.6);
    // Live dot + updated.
    let pulse = 0.5 + 0.5 * (s.lt(0.0) * 3.0).sin() * s.live().min(1.0);
    let lx = x + s.a().measure("Overview", &s.hst(22.0, 600, p.text)) + 16.0 * u;
    e.push(circle(lx + 4.0 * u, y + 28.0 * u, 6.0 * u * (0.6 + 0.4 * pulse), solid(alpha(p.pos, 0.25 * pulse))));
    e.push(circle(lx + 4.0 * u, y + 28.0 * u, 3.5 * u, solid(p.pos)));
    s.txt(&mut e, "Live", 12.5, 500, p.muted, lx + 14.0 * u, ui::mid(y + 28.0 * u, 12.5 * u), 0.0);
    s.item(o, 2, e);
    // KPI row.
    let ky = y + 58.0 * u;
    let n = if s.kpis.is_empty() { if w / u > 760.0 { 4 } else { 3 } } else { s.kpis.len().clamp(1, 4) };
    let gap = 16.0 * u;
    let kw = (w - gap * (n - 1) as f32) / n as f32;
    let kh = 112.0 * u;
    for i in 0..n {
        let kx = x + i as f32 * (kw + gap);
        let mut e = Vec::new();
        s.card(&mut e, kx, ky, kw, kh);
        let k = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(s.t, 0.35 + i as f32 * 0.08, 1.3)) as f64 };
        let (label, val, d) = kpi(s, i, k);
        s.fit(&mut e, &label, 13.0, 500, p.muted, kx + 18.0 * u, ky + 30.0 * u, kw - if d.is_some() && kw > 230.0 * u { 110.0 * u } else { 36.0 * u });
        ui::fit(&mut e, s.a(), &val, &s.nst(28.0, p.text), kx + 18.0 * u, ky + 70.0 * u, kw - 36.0 * u, 0.0);
        if let Some(d) = d {
            let cw = delta(&mut e, s, d, kx + 18.0 * u, ky + 92.0 * u, 11.5);
            s.fit(&mut e, "vs last month", 12.0, 400, p.faint, kx + 26.0 * u + cw, ui::mid(ky + 92.0 * u, 12.0 * u), kw - cw - 50.0 * u);
            if kw > 230.0 * u {
                let data: Vec<f32> = (0..10).map(|j| 3.0 + j as f32 * 0.35 * d.signum() + 1.6 * hash01((i * 31 + j) as u32)).collect();
                ui::sparkline(&mut e, &data, kx + kw - 78.0 * u, ky + 16.0 * u, 60.0 * u, 20.0 * u, if d >= 0.0 { p.pos } else { p.neg }, 1.5 * u, k as f32);
            }
        }
        s.item(o, 3 + i, e);
    }
    // Chart + side list.
    let cy = ky + kh + gap;
    let avail = y + h - cy - pad;
    let table_h = if avail > 520.0 * u { 210.0 * u } else { 0.0 };
    let ch = (avail - if table_h > 0.0 { table_h + gap } else { 0.0 }).max(120.0 * u);
    let side = w / u > 820.0;
    let cw = if side { (w - gap) * 0.64 } else { w };
    let mut e = Vec::new();
    s.card(&mut e, x, cy, cw, ch);
    let ctitle = s.kpis.first().map_or("Revenue", |k| k.label.as_str());
    s.fit(&mut e, ctitle, 14.0, 600, p.text, x + 20.0 * u, cy + 30.0 * u, cw * 0.5);
    // Legend.
    let mut lx = x + cw - 20.0 * u;
    for (lab, c, dashed) in [("Last year", p.muted, true), ("This year", p.acc, false)] {
        let tw = s.a().measure(lab, &s.st(12.0, 500, p.muted));
        s.txt(&mut e, lab, 12.0, 500, p.muted, lx, ui::mid(cy + 25.0 * u, 12.0 * u), 1.0);
        lx -= tw + 8.0 * u;
        if dashed {
            e.push(rect(lx - 12.0 * u, cy + 24.0 * u, 5.0 * u, 2.0 * u, u, solid(c)));
            e.push(rect(lx - 5.0 * u, cy + 24.0 * u, 5.0 * u, 2.0 * u, u, solid(c)));
        } else {
            e.push(rect(lx - 12.0 * u, cy + 24.0 * u, 12.0 * u, 2.5 * u, u, solid(c)));
        }
        lx -= 30.0 * u;
    }
    let (ck, cv, _) = kpi(s, 0, 1.0);
    let _ = ck;
    if s.kpis.is_empty() {
        ui::fit(&mut e, s.a(), "$612,480", &s.nst(24.0, p.text), x + 20.0 * u, cy + 64.0 * u, cw * 0.5, 0.0);
        delta(&mut e, s, 18.2, x + 24.0 * u + s.a().measure("$612,480", &s.nst(24.0, p.text)), cy + 56.0 * u, 11.5);
    } else {
        ui::fit(&mut e, s.a(), &cv, &s.nst(24.0, p.text), x + 20.0 * u, cy + 64.0 * u, cw * 0.5, 0.0);
    }
    let chart_top = cy + 84.0 * u;
    let money = s.kpis.is_empty();
    area_chart(&mut e, s, x + 14.0 * u, chart_top, cw - 34.0 * u, cy + ch - chart_top - 10.0 * u, money, 4, if cw / u > 560.0 { 1 } else { 2 });
    s.item(o, 7, e);
    if side {
        let (sx, sw) = (x + cw + gap, w - cw - gap);
        let mut e = Vec::new();
        s.card(&mut e, sx, cy, sw, ch);
        let pad2 = 20.0 * u;
        if s.items.is_empty() {
            s.txt(&mut e, "Top channels", 14.0, 600, p.text, sx + pad2, cy + 30.0 * u, 0.0);
            s.ic(&mut e, "more", sx + sw - pad2 - 16.0 * u, cy + 17.0 * u, 16.0, p.muted);
            let rows = [("Direct", 0.42f32, "globe"), ("Organic search", 0.28, "search"), ("Referral", 0.17, "share"), ("Social", 0.09, "users"), ("Email", 0.04, "inbox")];
            let mut ry = cy + 58.0 * u;
            for (i, (lab, f, icn)) in rows.iter().enumerate() {
                if ry + 44.0 * u > cy + ch {
                    break;
                }
                s.ic(&mut e, icn, sx + pad2, ry - 2.0 * u, 15.0, p.muted);
                s.fit(&mut e, lab, 13.0, 500, p.text2, sx + pad2 + 24.0 * u, ry + 10.0 * u, sw * 0.5);
                let kk = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(s.t, 0.6 + i as f32 * 0.08, 1.0)) };
                draw(&mut e, s.a(), &format!("{:.0}%", f * 100.0 * kk), &s.st(13.0, 600, p.text).tabular(), sx + sw - pad2, ry + 10.0 * u, 1.0);
                let by = ry + 20.0 * u;
                let bw = sw - 2.0 * pad2;
                e.push(rect(sx + pad2, by, bw, 5.0 * u, 2.5 * u, solid(p.sunken)));
                e.push(rect(sx + pad2, by, (bw * f / 0.42 * kk).max(5.0 * u), 5.0 * u, 2.5 * u, solid(if i == 0 { p.acc } else { alpha(p.acc, 0.55 - i as f32 * 0.08) })));
                ry += 48.0 * u;
            }
        } else {
            s.txt(&mut e, "Highlights", 14.0, 600, p.text, sx + pad2, cy + 30.0 * u, 0.0);
            let mut ry = cy + 52.0 * u;
            for (i, item) in s.items.iter().enumerate() {
                if ry + 40.0 * u > cy + ch {
                    break;
                }
                let rc = ry + 20.0 * u;
                e.push(rect(sx + pad2, rc - 14.0 * u, 28.0 * u, 28.0 * u, 7.0 * u, solid(alpha(ui::hue(i + 1), 0.14))));
                let icn = ["zap", "sparkles", "layers", "shield", "globe", "users"][i % 6];
                s.ic(&mut e, icn, sx + pad2 + 7.0 * u, rc - 7.0 * u, 14.0, ui::hue(i + 1));
                s.fit(&mut e, item, 13.0, 500, p.text2, sx + pad2 + 40.0 * u, ui::mid(rc, 13.0 * u), sw - 2.0 * pad2 - 40.0 * u);
                if i > 0 {
                    s.hair(&mut e, sx + pad2 + 40.0 * u, ry, sw - 2.0 * pad2 - 40.0 * u);
                }
                ry += 44.0 * u;
            }
        }
        // Weekday bars fill what is left of the card.
        let used = cy + if s.items.is_empty() { 58.0 + 48.0 * 5.0 } else { 52.0 + 44.0 * s.items.len().min(8) as f32 } * u;
        let room = cy + ch - used;
        if room > 150.0 * u {
            let (bx, bw) = (sx + pad2, sw - 2.0 * pad2);
            let top = used + 14.0 * u;
            s.hair(&mut e, bx, top - 8.0 * u, bw);
            s.txt(&mut e, "Sessions by weekday", 13.0, 600, p.text, bx, top + 16.0 * u, 0.0);
            let (b0, b1) = (top + 32.0 * u, cy + ch - 34.0 * u);
            let vals = [0.55f32, 0.72, 0.68, 0.84, 1.0, 0.46, 0.38];
            let n = vals.len() as f32;
            let slot = bw / n;
            for (j, v) in vals.iter().enumerate() {
                let kk = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(s.t, 0.8 + j as f32 * 0.05, 0.8)) };
                let hh = (b1 - b0) * v * kk;
                let xx = bx + slot * j as f32 + slot * 0.2;
                e.push(rect(xx, b1 - (b1 - b0), slot * 0.6, b1 - b0, 4.0 * u, solid(p.sunken)));
                e.push(rect(xx, b1 - hh, slot * 0.6, hh, 4.0 * u, solid(if j == 4 { p.acc } else { alpha(p.acc, 0.45) })));
                s.txt(&mut e, ["M", "T", "W", "T", "F", "S", "S"][j], 11.0, 500, p.faint, xx + slot * 0.3, b1 + 18.0 * u, 0.5);
            }
        }
        s.item(o, 8, e);
    }
    if table_h > 0.0 {
        let ty = cy + ch + gap;
        let mut e = Vec::new();
        s.card(&mut e, x, ty, w, table_h);
        s.txt(&mut e, "Recent orders", 14.0, 600, p.text, x + 20.0 * u, ty + 30.0 * u, 0.0);
        let cols = [0.0f32, 0.38, 0.58, 0.78];
        let hy = ty + 60.0 * u;
        for (j, hd) in ["Customer", "Status", "Date", "Amount"].iter().enumerate() {
            let cx = if j == 3 { x + w - 20.0 * u } else { x + 20.0 * u + (w - 40.0 * u) * cols[j] };
            s.txt(&mut e, hd, 12.0, 500, p.faint, cx, hy, if j == 3 { 1.0 } else { 0.0 });
        }
        let rows = [("Acme Inc.", 0, "Nov 14", "$2,400.00"), ("Globex", 1, "Nov 13", "$1,180.50"), ("Initech", 0, "Nov 13", "$860.00"), ("Umbrella Co.", 2, "Nov 12", "$3,920.00")];
        let mut ry = hy + 12.0 * u;
        for (name, st, date, amt) in rows {
            if ry + 36.0 * u > ty + table_h {
                break;
            }
            s.hair(&mut e, x + 20.0 * u, ry, w - 40.0 * u);
            let rc = ry + 18.0 * u;
            ui::avatar(&mut e, s.a(), name, x + 32.0 * u, rc, 11.0 * u, None);
            s.txt(&mut e, name, 13.0, 500, p.text, x + 52.0 * u, ui::mid(rc, 13.0 * u), 0.0);
            let (sl, sc) = [("Paid", p.pos), ("Pending", p.warn), ("Refunded", p.muted)][st];
            ui::chip(&mut e, s.a(), sl, sc, x + 20.0 * u + (w - 40.0 * u) * cols[1], rc, 11.5 * u, true);
            s.txt(&mut e, date, 13.0, 400, p.muted, x + 20.0 * u + (w - 40.0 * u) * cols[2], ui::mid(rc, 13.0 * u), 0.0);
            draw(&mut e, s.a(), amt, &s.st(13.0, 500, p.text).tabular(), x + w - 20.0 * u, ui::mid(rc, 13.0 * u), 1.0);
            ry += 36.0 * u;
        }
        s.item(o, 9, e);
    }
}

/// One chat message (desktop): avatar, name, time, wrapped text. Returns its height.
fn d_message(o: &mut Vec<Op>, s: &S, name: &str, time: &str, body: &[String], bot: bool, x: f32, y: f32, maxw: f32, reaction: Option<(&str, u32)>) -> f32 {
    let u = s.u;
    let p = &s.p;
    if bot {
        ui::logo(o, s.a(), s.title, p.acc, x, y + 2.0 * u, 34.0 * u);
    } else {
        ui::avatar(o, s.a(), name, x + 17.0 * u, y + 19.0 * u, 17.0 * u, None);
    }
    let tx = x + 48.0 * u;
    let nw = s.txt(o, name, 14.0, 600, p.text, tx, y + 13.0 * u, 0.0);
    let mut ax = tx + nw + 8.0 * u;
    if bot {
        ax += ui::chip(o, s.a(), "APP", p.acc_text, ax, y + 8.5 * u, 10.0 * u, false) + 8.0 * u;
    }
    s.txt(o, time, 12.0, 400, p.faint, ax, y + 13.0 * u, 0.0);
    let st = s.st(14.0, 400, p.text2);
    let lh = 22.0 * u;
    let mut yy = y + 36.0 * u;
    for l in body {
        if let Some(rest) = l.strip_prefix("[file]") {
            let (name, meta) = rest.split_once('|').unwrap_or((rest, ""));
            let (fw, fh) = (maxw.min(380.0 * u), 56.0 * u);
            let fy = yy - 14.0 * u;
            o.push(rect(tx, fy, fw, fh, 9.0 * u, solid(p.panel)));
            border(o, tx, fy, fw, fh, 9.0 * u, p.line2);
            o.push(rect(tx + 10.0 * u, fy + 10.0 * u, 36.0 * u, 36.0 * u, 8.0 * u, solid(alpha(ui::hue(1), 0.16))));
            s.ic(o, "image", tx + 18.0 * u, fy + 18.0 * u, 20.0, ui::hue(1));
            s.fit(o, name, 13.5, 600, p.text, tx + 56.0 * u, fy + 24.0 * u, fw - 100.0 * u);
            s.fit(o, meta, 12.0, 400, p.muted, tx + 56.0 * u, fy + 42.0 * u, fw - 100.0 * u);
            s.ic(o, "download", tx + fw - 32.0 * u, fy + 19.0 * u, 18.0, p.muted);
            yy += lh * 3.0;
            continue;
        }
        if let Some(rest) = l.strip_prefix("• ") {
            o.push(circle(tx + 5.0 * u, yy - 5.0 * u, 2.5 * u, solid(p.acc)));
            ui::fit(o, s.a(), rest, &st, tx + 16.0 * u, yy, maxw - 16.0 * u, 0.0);
        } else {
            draw(o, s.a(), l, &st, tx, yy, 0.0);
        }
        yy += lh;
    }
    let files = body.iter().filter(|l| l.starts_with("[file]")).count() as f32;
    let mut hgt = 36.0 * u + lh * (body.len() as f32 - 1.0 + 2.0 * files) + 10.0 * u;
    if let Some((icn, n)) = reaction {
        let ry = yy - 8.0 * u;
        let rw = 50.0 * u;
        o.push(rect(tx, ry, rw, 24.0 * u, 12.0 * u, solid(p.acc_soft)));
        border(o, tx, ry, rw, 24.0 * u, 12.0 * u, alpha(p.acc, 0.45));
        ui::icon_filled(o, icn, tx + 9.0 * u, ry + 5.0 * u, 14.0 * u, p.acc_text);
        draw(o, s.a(), &n.to_string(), &s.st(12.0, 600, p.acc_text).tabular(), tx + 29.0 * u, ry + 16.5 * u, 0.0);
        hgt += 28.0 * u;
    }
    hgt
}

fn typing_dots(o: &mut Vec<Op>, s: &S, x: f32, cy: f32, r: f32, c: Rgba) {
    let lt = s.lt(0.4);
    for i in 0..3 {
        let ph = ((lt * 5.0 - i as f32 * 0.9).sin() * 0.5 + 0.5) * s.live().min(1.0);
        o.push(circle(x + i as f32 * r * 3.0, cy - ph * r * 0.8, r, solid(alpha(c, 0.45 + 0.55 * ph))));
    }
}

/// One scripted message: author, from the product itself, text, reaction.
type Line = (String, bool, String, Option<(&'static str, u32)>);

fn chat_script(s: &S) -> Vec<Line> {
    if s.items.is_empty() {
        vec![
            ("Sam Park".into(), false, "Morning all! Design review moved to 2pm today.".into(), None),
            ("Jordan Lee".into(), false, "Here's the latest file for it:\n[file]Onboarding flow v3|Figma · edited 5 min ago".into(), None),
            ("Maya Chen".into(), false, "Pushed the new onboarding flow to staging. Would love a few eyes on it before the 2pm sync.".into(), None),
            ("Jordan Lee".into(), false, "Just went through it end to end. The empty states are so much clearer now, and the import step finally feels fast.".into(), Some(("heart", 4))),
            ("Maya Chen".into(), false, "Thank you! Merging once QA signs off.".into(), None),
        ]
    } else {
        let mut list = String::from("Here's what's new this week:");
        for it in s.items.iter().take(4) {
            list.push('\n');
            list.push_str("• ");
            list.push_str(it);
        }
        vec![
            ("Sam Park".into(), false, "Morning all! Release notes go out at noon.".into(), None),
            ("Maya Chen".into(), false, format!("What shipped in {} this week?", s.title), None),
            (s.title.to_string(), true, list, Some(("heart", 6))),
            ("Jordan Lee".into(), false, format!("{} is on fire. Rolling it out to the whole team today.", s.it(0, "")).trim().to_string(), None),
        ]
    }
}

fn d_chat(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let pad = 28.0 * u;
    let maxw = (w - 2.0 * pad - 48.0 * u).min(660.0 * u);
    // Composer.
    let comp_h = 92.0 * u;
    let cy = y + h - comp_h - 20.0 * u;
    let mut e = Vec::new();
    s.card(&mut e, x + pad, cy, w - 2.0 * pad, comp_h);
    let placeholder = "Message #general";
    s.txt(&mut e, placeholder, 14.0, 400, p.faint, x + pad + 16.0 * u, cy + 28.0 * u, 0.0);
    if s.live() > 0.0 && (s.t * 1.8).fract() < 0.55 {
        e.push(rect(x + pad + 15.0 * u, cy + 14.0 * u, 1.5 * u, 18.0 * u, 0.0, solid(p.acc)));
    }
    let ty = cy + comp_h - 38.0 * u;
    for (i, icn) in ["plus", "paperclip", "smile", "mic"].iter().enumerate() {
        s.ic(&mut e, icn, x + pad + (14.0 + i as f32 * 30.0) * u, ty + 6.0 * u, 18.0, p.muted);
    }
    let sb = 30.0 * u;
    let sx = x + w - pad - 12.0 * u - sb;
    e.push(rect(sx, ty, sb, sb, 8.0 * u, solid(p.acc)));
    s.ic(&mut e, "arrow-up", sx + 7.0 * u, ty + 7.0 * u, 16.0, p.on_acc);
    // Typing indicator.
    let name = "Sam Park";
    let tiy = cy - 14.0 * u;
    typing_dots(&mut e, s, x + pad + 4.0 * u, tiy - 4.0 * u, 2.6 * u, p.muted);
    s.txt(&mut e, &format!("{name} is typing…"), 12.0, 400, p.muted, x + pad + 30.0 * u, tiy, 0.0);
    s.item(o, 2, e);
    // Messages, bottom-anchored above the typing line.
    let script = chat_script(s);
    let st = s.st(14.0, 400, p.text2);
    let bodies: Vec<Vec<String>> = script.iter().map(|(_, _, text, _)| text.split('\n').flat_map(|para| if para.starts_with("• ") || para.starts_with("[file]") { vec![para.to_string()] } else { ui::wrap_lines(s.a(), para, &st, maxw, 3) }).collect()).collect();
    let heights: Vec<f32> = script.iter().zip(&bodies).map(|((_, _, _, r), b)| 36.0 * u + 22.0 * u * (b.len() as f32 - 1.0 + 2.0 * b.iter().filter(|l| l.starts_with("[file]")).count() as f32) + 10.0 * u + if r.is_some() { 28.0 * u } else { 0.0 } + 20.0 * u).collect();
    let total: f32 = heights.iter().sum();
    let mut yy = (tiy - 30.0 * u - total).max(y + 70.0 * u);
    // Day divider.
    let mut e = Vec::new();
    let dy = yy - 30.0 * u;
    if dy > y + 10.0 * u {
        let lw = s.a().measure("Today", &s.st(12.0, 500, p.muted)) + 24.0 * u;
        s.hair(&mut e, x + pad, dy, (w - 2.0 * pad - lw) * 0.5);
        s.hair(&mut e, x + w * 0.5 + lw * 0.5, dy, (w - 2.0 * pad - lw) * 0.5);
        s.txt(&mut e, "Today", 12.0, 500, p.muted, x + w * 0.5, ui::mid(dy, 12.0 * u), 0.5);
    }
    s.item(o, 3, e);
    let times = ["9:58 AM", "10:02 AM", "10:24 AM", "10:26 AM", "10:31 AM"];
    for (i, ((name, bot, _, react), body)) in script.iter().zip(&bodies).enumerate() {
        let mut e = Vec::new();
        d_message(&mut e, s, name, times[(i + 5 - script.len().min(5)) % 5], body, *bot, x + pad, yy, maxw, *react);
        s.item(o, 4 + i * 2, e);
        yy += heights[i];
    }
}

/// Abstract "photo": layered soft gradients in a hue.
fn photo(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, r: f32, seed: usize) {
    let dark = s.p.dark;
    let c1 = s.p.acc;
    let c2 = ui::hue(seed);
    o.push(rect(x, y, w, h, r, Paint::Linear { p0: [x, y], p1: [x + w, y + h], stops: vec![(0.0, shade(c1, if dark { -0.3 } else { 0.25 })), (1.0, shade(c2, if dark { -0.5 } else { 0.0 }))] }));
    push_clip(o, x, y, w, h, r);
    let mut rng = Rng::new(seed as u64 + 11);
    for k in 0..3 {
        let (cx, cy) = (x + w * rng.range(0.1, 0.9), y + h * rng.range(0.1, 0.9));
        let rr = w.max(h) * rng.range(0.3, 0.55);
        let c = if k % 2 == 0 { shade(c1, 0.5) } else { shade(c2, 0.35) };
        o.push(rect(x, y, w, h, 0.0, radial(cx, cy, rr, vec![(0.0, alpha(c, 0.5)), (1.0, alpha(c, 0.0))])));
    }
    // A product card floating in the shot: logo, name, a live metric line.
    let (cw, ch) = (w * 0.46, h * 0.5);
    let (cx, cy) = (x + (w - cw) * 0.5, y + (h - ch) * 0.55);
    let cr = ch * 0.1;
    shadow(o, cx, cy, cw, ch, cr, ch * 0.1, ch * 0.25, 0.28);
    o.push(rect(cx, cy, cw, ch, cr, solid(Rgba([1.0, 1.0, 1.0, 0.96]))));
    let ink = Rgba([0.09, 0.09, 0.11, 1.0]);
    let ls = ch * 0.2;
    ui::logo(o, s.a(), s.title, c1, cx + ch * 0.12, cy + ch * 0.12, ls);
    let fs = ch * 0.1;
    ui::fit(o, s.a(), s.title, &ts(Face::Sans, fs, 600, ink), cx + ch * 0.12 + ls + fs * 0.7, cy + ch * 0.12 + ls * 0.5 + fs * 0.36, cw * 0.5, 0.0);
    let data = [3.0f32, 3.6, 3.3, 4.4, 4.1, 5.2, 5.0, 6.3];
    ui::sparkline(o, &data, cx + ch * 0.12, cy + ch * 0.48, cw - ch * 0.24, ch * 0.36, c1, (ch * 0.025).max(1.0), 1.0);
    o.push(Op::PopLayer);
    border(o, x, y, w, h, r, Rgba([1.0, 1.0, 1.0, if dark { 0.06 } else { 0.0 }]));
}

struct Post {
    name: String,
    handle: String,
    when: &'static str,
    text: String,
    brand: bool,
    media: bool,
    counts: [u32; 4],
}

fn posts(s: &S) -> Vec<Post> {
    if s.items.is_empty() {
        vec![
            Post { name: "Maya Chen".into(), handle: "@mayachen".into(), when: "2h", text: "Shipped a full redesign of our onboarding today. Fewer steps, clearer copy, and the first-week activation is already up.".into(), brand: false, media: true, counts: [24, 12, 148, 9] },
            Post { name: "Jordan Lee".into(), handle: "@jordanlee".into(), when: "5h", text: "The best product docs read like a conversation, not a manual.".into(), brand: false, media: false, counts: [41, 30, 392, 18] },
            Post { name: "Sam Park".into(), handle: "@sampark".into(), when: "8h", text: "We're hiring product designers who care about the details. Remote friendly, DMs open.".into(), brand: false, media: false, counts: [8, 22, 97, 4] },
        ]
    } else {
        let handle = format!("@{}", s.slug());
        s.items.iter().take(4).enumerate().map(|(i, t)| Post { name: s.title.to_string(), handle: handle.clone(), when: ["1h", "4h", "1d", "2d"][i], text: t.clone(), brand: true, media: i == 0, counts: [18 + 7 * i as u32, 9 + 3 * i as u32, 214 - 31 * i as u32, 12] }).collect()
    }
}

/// One feed post; returns its height.
fn post(o: &mut Vec<Op>, s: &S, ps: &Post, x: f32, y: f32, w: f32, size: f32, idx: usize, measure_only: bool) -> f32 {
    let u = s.u;
    let p = &s.p;
    let av = 40.0 * u * size / 15.0;
    let tx = x + av + 12.0 * u;
    let tw = w - (tx - x);
    let st = s.st(size, 400, p.text);
    let lines = ui::wrap_lines(s.a(), &ps.text, &st, tw, 4);
    let lh = size * 1.42 * u;
    let mh = if ps.media { (tw * 0.5).min(260.0 * u) } else { 0.0 };
    let total = 26.0 * u + lh * lines.len() as f32 + if ps.media { mh + 12.0 * u } else { 0.0 } + 44.0 * u;
    if measure_only {
        return total;
    }
    if ps.brand {
        ui::logo(o, s.a(), s.title, p.acc, x, y, av);
    } else {
        ui::avatar(o, s.a(), &ps.name, x + av * 0.5, y + av * 0.5, av * 0.5, None);
    }
    let ny = y + 15.0 * u;
    let nw = ui::fit(o, s.a(), &ps.name, &s.st(size, 600, p.text), tx, ny, tw * 0.5, 0.0);
    let mut ax = tx + nw + 4.0 * u;
    if ps.brand || idx == 0 {
        let b = size * 1.0 * u;
        o.push(circle(ax + b * 0.5, ny - size * 0.36 * u, b * 0.5, solid(p.acc)));
        let mut ck = Path::new();
        ck.move_to(((ax + b * 0.28) as f64, (ny - size * 0.36 * u) as f64));
        ck.line_to(((ax + b * 0.45) as f64, (ny - size * 0.36 * u + b * 0.16) as f64));
        ck.line_to(((ax + b * 0.74) as f64, (ny - size * 0.36 * u - b * 0.16) as f64));
        o.push(stroke_op(ck, solid(p.on_acc), b * 0.13, [0.0, 1.0]));
        ax += b + 4.0 * u;
    }
    ui::fit(o, s.a(), &format!("{} · {}", ps.handle, ps.when), &s.st(size, 400, p.muted), ax, ny, x + w - ax - 24.0 * u, 0.0);
    ui::icon(o, "more", x + w - 18.0 * u, ny - 13.0 * u, 18.0 * u, p.faint, 1.8);
    let mut yy = y + 26.0 * u + lh * 0.75;
    for l in &lines {
        draw(o, s.a(), l, &st, tx, yy, 0.0);
        yy += lh;
    }
    yy -= lh * 0.75;
    if ps.media {
        yy += 8.0 * u;
        photo(o, s, tx, yy, tw, mh, s.r, idx + 1 + s.title.len());
        yy += mh + 4.0 * u;
    }
    // Actions.
    let ay = yy + 20.0 * u;
    let isz = 17.0 * size / 15.0;
    let liked_at = 1.5;
    let lt = s.lt(3.0);
    for (j, icn) in ["message", "repeat", "heart", "chart"].iter().enumerate() {
        let ix = tx + (tw - 30.0 * u) * j as f32 / 4.0;
        let mut n = ps.counts[j];
        let mut c = p.muted;
        if j == 2 && idx == 0 && s.live() > 0.0 || (j == 2 && idx == 0 && s.motion == Motion::Still) {
            let k = seg(lt, liked_at, 0.35);
            if k > 0.0 {
                n += 1;
                c = p.neg;
                let pop = 1.0 + 0.35 * (k * std::f32::consts::PI).sin();
                let sz = isz * u * pop;
                ui::icon_filled(o, "heart", ix + (isz * u - sz) * 0.5, ay - sz * 0.5, sz, c);
                if k < 1.0 {
                    o.push(stroke_op(circle_path(ix + isz * u * 0.5, ay, isz * u * (0.6 + k)), solid(alpha(c, 0.5 * (1.0 - k))), 1.5 * u, [0.0, 1.0]));
                }
            } else {
                ui::icon(o, icn, ix, ay - isz * u * 0.5, isz * u, c, 1.8);
            }
        } else {
            ui::icon(o, icn, ix, ay - isz * u * 0.5, isz * u, c, 1.8);
        }
        draw(o, s.a(), &compact_number(n as f64), &s.st(size * 0.87, 400, c).tabular(), ix + isz * u + 7.0 * u, ui::mid(ay, size * 0.87 * u), 0.0);
    }
    ui::icon(o, "bookmark", x + w - 44.0 * u, ay - isz * u * 0.5, isz * u, p.muted, 1.8);
    ui::icon(o, "share", x + w - 18.0 * u, ay - isz * u * 0.5, isz * u, p.muted, 1.8);
    total
}

fn d_feed(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let rail = w / u > 900.0;
    let rw = if rail { 300.0 * u } else { 0.0 };
    let gap = 28.0 * u;
    let colw = (w - rw - gap * if rail { 3.0 } else { 2.0 }).min(640.0 * u);
    let cx = x + gap;
    // Tabs.
    let mut e = Vec::new();
    let ty = y + 40.0 * u;
    let mut tx = cx;
    for (i, lab) in ["For you", "Following", "Launches"].iter().enumerate() {
        let tw = s.txt(&mut e, lab, 14.0, if i == 0 { 600 } else { 500 }, if i == 0 { p.text } else { p.muted }, tx, ty, 0.0);
        if i == 0 {
            e.push(rect(tx, ty + 13.0 * u, tw, 2.0 * u, u, solid(p.acc)));
        }
        tx += tw + 28.0 * u;
    }
    s.hair(&mut e, cx, ty + 15.0 * u, colw);
    // Composer.
    let cy = ty + 30.0 * u;
    ui::avatar(&mut e, s.a(), "You", cx + 20.0 * u, cy + 20.0 * u, 20.0 * u, None);
    s.txt(&mut e, "What's new?", 15.0, 400, p.faint, cx + 52.0 * u, ui::mid(cy + 20.0 * u, 15.0 * u), 0.0);
    ui::button(&mut e, s.a(), p, "Post", None, cx + colw, cy + 4.0 * u, 32.0 * u, true, 16.0 * u, true);
    s.hair(&mut e, cx, cy + 54.0 * u, colw);
    s.item(o, 2, e);
    let mut yy = cy + 74.0 * u;
    for (i, ps) in posts(s).iter().enumerate() {
        if yy > y + h {
            break;
        }
        let mut e = Vec::new();
        let ph = post(&mut e, s, ps, cx, yy, colw, 15.0, i, false);
        s.hair(&mut e, cx, yy + ph + 4.0 * u, colw);
        s.item(o, 3 + i, e);
        yy += ph + 22.0 * u;
    }
    if rail {
        let rx = x + w - rw - gap;
        let mut e = Vec::new();
        ui::field(&mut e, s.a(), p, "Search", Some("search"), rx, y + 20.0 * u, rw, 36.0 * u, 18.0 * u, None);
        let ry = y + 72.0 * u;
        let tags: Vec<(String, String, String)> = if s.items.is_empty() {
            vec![("Trending in Design".into(), "#launchweek".into(), "12.4K posts".into()), ("Technology".into(), "Local-first apps".into(), "8,210 posts".into()), ("Trending".into(), "#buildinpublic".into(), "31.9K posts".into())]
        } else {
            s.items.iter().skip(1).take(3).map(|t| (format!("Trending · {}", s.title), t.clone(), String::new())).collect()
        };
        let rh = 76.0 + 62.0 * tags.len() as f32;
        s.card(&mut e, rx, ry, rw, rh * u);
        s.hst(18.0, 700, p.text);
        ui::fit(&mut e, s.a(), "What's happening", &s.hst(17.0, 700, p.text), rx + 18.0 * u, ry + 34.0 * u, rw - 36.0 * u, 0.0);
        for (i, (k, t, n)) in tags.iter().enumerate() {
            let by = ry + (62.0 + i as f32 * 62.0) * u;
            s.fit(&mut e, k, 12.0, 400, p.muted, rx + 18.0 * u, by + 14.0 * u, rw - 60.0 * u);
            s.fit(&mut e, t, 14.5, 600, p.text, rx + 18.0 * u, by + 34.0 * u, rw - 50.0 * u);
            s.txt(&mut e, n, 12.0, 400, p.muted, rx + 18.0 * u, by + 52.0 * u, 0.0);
            s.ic(&mut e, "more", rx + rw - 34.0 * u, by + 4.0 * u, 16.0, p.faint);
        }
        // Who to follow.
        let fy = ry + rh * u + 18.0 * u;
        if fy + 180.0 * u < y + h {
            s.card(&mut e, rx, fy, rw, 172.0 * u);
            ui::fit(&mut e, s.a(), "Who to follow", &s.hst(17.0, 700, p.text), rx + 18.0 * u, fy + 34.0 * u, rw - 36.0 * u, 0.0);
            for (i, (n, hnd)) in [("Ada Park", "@adapark"), ("Leo Grant", "@leogrant")].iter().enumerate() {
                let by = fy + (76.0 + i as f32 * 54.0) * u;
                ui::avatar(&mut e, s.a(), n, rx + 38.0 * u, by, 20.0 * u, None);
                s.txt(&mut e, n, 14.0, 600, p.text, rx + 68.0 * u, by - 3.0 * u, 0.0);
                s.txt(&mut e, hnd, 12.5, 400, p.muted, rx + 68.0 * u, by + 14.0 * u, 0.0);
                let followed = i == 1 && s.lt(3.0) > 1.9;
                ui::button(&mut e, s.a(), p, if followed { "Following" } else { "Follow" }, None, rx + rw - 18.0 * u, by - 15.0 * u, 30.0 * u, !followed, 15.0 * u, true);
            }
        }
        s.item(o, 3, e);
    }
}

/// Syntax-highlighted code (tiny TypeScript tokenizer).
fn code_lines(s: &S) -> Vec<String> {
    let slug = s.slug();
    let up = slug.to_uppercase();
    vec![
        format!("import {{ createClient }} from \"@{slug}/sdk\";"),
        "import type { Workspace } from \"./types\";".into(),
        String::new(),
        "const client = createClient({".into(),
        format!("  apiKey: process.env.{up}_KEY,"),
        "  region: \"eu-west-1\",".into(),
        "  retries: 3,".into(),
        "});".into(),
        String::new(),
        "// Index every item in a workspace.".into(),
        "export async function sync(workspace: Workspace) {".into(),
        "  const items = await client.items.list({ workspace });".into(),
        "  for (const item of items) {".into(),
        "    await client.index(item, { priority: \"high\" });".into(),
        "  }".into(),
        "  return items.length;".into(),
        "}".into(),
        String::new(),
        "export const schedule = \"*/5 * * * *\";".into(),
    ]
}

fn code_colors(dark: bool) -> [Rgba; 7] {
    // text, keyword, string, function, number, comment, type/property
    let h = |c: &str| parse_color(c).unwrap();
    if dark {
        [h("#d4d7de"), h("#ff7b72"), h("#a5d6ff"), h("#d2a8ff"), h("#79c0ff"), h("#6e7681"), h("#ffa657")]
    } else {
        [h("#24292f"), h("#cf222e"), h("#0a3069"), h("#8250df"), h("#0550ae"), h("#6e7781"), h("#953800")]
    }
}

/// Draw one line of code with highlighting; returns the end x.
fn code_line(o: &mut Vec<Op>, s: &S, line: &str, x: f32, y: f32, size: f32) -> f32 {
    let cols = code_colors(s.p.dark);
    const KW: &[&str] = &["import", "from", "type", "const", "let", "export", "async", "function", "await", "for", "of", "return", "if", "else", "new"];
    let chars: Vec<char> = line.chars().collect();
    let mut toks: Vec<(String, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            toks.push((chars[i..].iter().collect(), 5));
            break;
        } else if c == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                j += 1;
            }
            let j = (j + 1).min(chars.len());
            toks.push((chars[i..j].iter().collect(), 2));
            i = j;
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            toks.push((chars[i..j].iter().collect(), 4));
            i = j;
        } else if c.is_alphabetic() || c == '_' {
            let mut j = i;
            while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                j += 1;
            }
            let w: String = chars[i..j].iter().collect();
            let next = chars[j..].iter().find(|c| **c != ' ');
            let kind = if KW.contains(&w.as_str()) {
                1
            } else if next == Some(&'(') {
                3
            } else if w.chars().next().is_some_and(|c| c.is_uppercase()) {
                6
            } else if next == Some(&':') && line.trim_start().starts_with(&w) {
                4
            } else {
                0
            };
            toks.push((w, kind));
            i = j;
        } else {
            let mut j = i;
            while j < chars.len() && !(chars[j].is_alphanumeric() || chars[j] == '"' || chars[j] == '_' || (chars[j] == '/' && chars.get(j + 1) == Some(&'/'))) {
                j += 1;
            }
            toks.push((chars[i..j].iter().collect(), 0));
            i = j;
        }
    }
    let mut cx = x;
    for (t, k) in toks {
        let st = ts(Face::Mono, size, if k == 1 { 500 } else { 400 }, if k == 0 && t.trim().chars().all(|c| !c.is_alphanumeric()) { alpha(cols[0], 0.7) } else { cols[k] });
        cx += draw(o, s.a(), &t, &st, cx, y, 0.0).max(s.a().measure(&t, &st));
    }
    cx
}

fn d_editor(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let tree_w = if w / u > 760.0 { 236.0 * u } else { 0.0 };
    let status_h = 26.0 * u;
    let mut e = Vec::new();
    // Explorer.
    if tree_w > 0.0 {
        e.push(rect(x, y, tree_w, h, 0.0, solid(p.side)));
        s.vhair(&mut e, x + tree_w - 1.0, y, h);
        let mut yy = y + 16.0 * u;
        if s.host == Host::Laptop {
            for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
                e.push(circle(x + (20.0 + i as f32 * 20.0) * u, yy + 6.0 * u, 6.0 * u, solid(parse_color(c).unwrap())));
            }
            yy += 28.0 * u;
        }
        ui::logo(&mut e, s.a(), s.title, p.acc, x + 14.0 * u, yy, 20.0 * u);
        s.fit(&mut e, &s.slug(), 13.5, 600, p.text, x + 42.0 * u, ui::mid(yy + 10.0 * u, 13.5 * u), tree_w - 80.0 * u);
        s.ic(&mut e, "branch", x + tree_w - 30.0 * u, yy + 3.0 * u, 14.0, p.faint);
        yy += 38.0 * u;
        let tree: [(usize, &str, bool, usize); 13] = [
            (0, "src", true, 9),
            (1, "lib", true, 9),
            (2, "client.ts", false, 1),
            (2, "types.ts", false, 1),
            (2, "sync.ts", false, 1),
            (1, "components", false, 9),
            (1, "index.ts", false, 1),
            (0, "tests", false, 9),
            (0, "package.json", false, 3),
            (0, "README.md", false, 0),
            (0, "tsconfig.json", false, 3),
            (0, ".env", false, 0),
            (0, "vite.config.ts", false, 1),
        ];
        for (i, (depth, name, open, kind)) in tree.iter().enumerate() {
            if yy + 26.0 * u > y + h - status_h {
                break;
            }
            let rc = yy + 13.0 * u;
            let ix = x + (16.0 + *depth as f32 * 14.0) * u;
            let active = i == 4;
            if active {
                e.push(rect(x + 6.0 * u, yy, tree_w - 12.0 * u, 26.0 * u, 6.0 * u, solid(p.acc_soft)));
            }
            if *kind == 9 {
                s.ic(&mut e, if *open { "chevron-down" } else { "chevron-right" }, ix, rc - 6.0 * u, 12.0, p.faint);
                s.ic(&mut e, "folder", ix + 16.0 * u, rc - 7.0 * u, 14.0, p.muted);
            } else {
                let fc = match kind {
                    1 => parse_color("#3e7be0").unwrap(),
                    3 => parse_color("#d4a12a").unwrap(),
                    _ => p.muted,
                };
                s.ic(&mut e, "file", ix + 16.0 * u, rc - 7.0 * u, 14.0, fc);
            }
            s.txt(&mut e, name, 13.0, if active { 500 } else { 400 }, if active { p.text } else { p.text2 }, ix + 38.0 * u, ui::mid(rc, 13.0 * u), 0.0);
            yy += 26.0 * u;
        }
    }
    s.item(o, 0, e);
    // Tabs.
    let (ex, ew) = (x + tree_w, w - tree_w);
    let tab_h = 38.0 * u;
    let mut e = Vec::new();
    e.push(rect(ex, y, ew, tab_h, 0.0, solid(p.side)));
    s.hair(&mut e, ex, y + tab_h - 1.0, ew);
    let mut tx = ex;
    for (i, name) in ["sync.ts", "client.ts", "README.md"].iter().enumerate() {
        let tw = s.a().measure(name, &s.st(13.0, 500, p.text)) + 64.0 * u;
        if i == 0 {
            e.push(rect(tx, y, tw, tab_h, 0.0, solid(p.bg)));
            e.push(rect(tx, y, tw, 2.0 * u, 0.0, solid(p.acc)));
            s.ic(&mut e, "x", tx + tw - 24.0 * u, y + tab_h * 0.5 - 6.0 * u, 12.0, p.muted);
        }
        s.vhair(&mut e, tx + tw - 1.0, y, tab_h);
        s.ic(&mut e, "file", tx + 14.0 * u, y + tab_h * 0.5 - 7.0 * u, 14.0, if name.ends_with(".ts") { parse_color("#3e7be0").unwrap() } else { p.muted });
        s.txt(&mut e, name, 13.0, 500, if i == 0 { p.text } else { p.muted }, tx + 34.0 * u, ui::mid(y + tab_h * 0.5, 13.0 * u), 0.0);
        tx += tw;
    }
    // Breadcrumb.
    let by = y + tab_h + 16.0 * u;
    let mut bx = ex + 20.0 * u;
    for (i, part) in ["src", "lib", "sync.ts"].iter().enumerate() {
        bx += s.txt(&mut e, part, 12.0, 400, if i == 2 { p.text2 } else { p.muted }, bx, ui::mid(by, 12.0 * u), 0.0);
        if i < 2 {
            s.ic(&mut e, "chevron-right", bx + 4.0 * u, by - 5.0 * u, 10.0, p.faint);
            bx += 18.0 * u;
        }
    }
    s.item(o, 1, e);
    // Bottom terminal when tall.
    let term_h = if h / u > 560.0 { 150.0 * u } else { 0.0 };
    let code_top = by + 18.0 * u;
    let code_bot = y + h - status_h - term_h;
    let size = 13.5 * u;
    let lh = 22.0 * u;
    let lines = code_lines(s);
    let gutter = 58.0 * u;
    let cur_line = 13usize;
    // Typing: line 13 types in.
    let type_k = if s.motion == Motion::Still { 1.0 } else { seg(s.t, 0.7, 1.4) };
    let mut e = Vec::new();
    let caret_on = s.live() == 0.0 || (s.t * 1.7).fract() < 0.6 || type_k < 1.0;
    for (i, l) in lines.iter().enumerate() {
        let ly = code_top + lh * (i as f32 + 0.5);
        if ly + lh * 0.5 > code_bot {
            break;
        }
        if i == cur_line {
            e.push(rect(ex, ly - lh * 0.5, ew, lh, 0.0, solid(p.hover)));
        }
        draw(&mut e, s.a(), &(i + 1).to_string(), &ts(Face::Mono, 12.5 * u, 400, if i == cur_line { p.text2 } else { p.faint }), ex + gutter - 16.0 * u, ui::mid(ly, 12.5 * u), 1.0);
        let shown: String = if i == cur_line { let n = (l.chars().count() as f32 * type_k).round() as usize; l.chars().take(n).collect() } else { l.clone() };
        let end = code_line(&mut e, s, &shown, ex + gutter, ui::mid(ly, size), size);
        if i == cur_line && caret_on {
            e.push(rect(end + 1.0, ly - lh * 0.38, 2.0 * u, lh * 0.76, 0.0, solid(p.acc)));
        }
        // Indent guides.
        let ind = l.chars().take_while(|c| *c == ' ').count() / 2;
        for g in 0..ind {
            let gx = ex + gutter + s.a().measure("  ", &ts(Face::Mono, size, 400, p.text)) * g as f32 + 1.0;
            e.push(rect(gx, ly - lh * 0.5, 1.0, lh, 0.0, solid(p.line)));
        }
    }
    // Inline hint (ghost text) once typed.
    if type_k >= 1.0 && s.motion != Motion::Still {
        let ly = code_top + lh * (cur_line as f32 + 0.5);
        let k = ease_out(seg(s.t, 2.3, 0.4));
        if k > 0.0 && ly < code_bot {
            let lw = s.a().measure(&lines[cur_line], &ts(Face::Mono, size, 400, p.text));
            draw(&mut e, s.a(), "  // retried 3×", &ts(Face::Mono, size, 400, alpha(p.faint, k)), ex + gutter + lw + 4.0 * u, ui::mid(ly, size), 0.0);
        }
    }
    s.item(o, 2, e);
    // Terminal.
    if term_h > 0.0 {
        let ty = y + h - status_h - term_h;
        let mut e = Vec::new();
        e.push(rect(ex, ty, ew, term_h, 0.0, solid(p.side)));
        s.hair(&mut e, ex, ty, ew);
        let mut tx = ex + 20.0 * u;
        for (i, lab) in ["Terminal", "Problems", "Output"].iter().enumerate() {
            let tw = s.txt(&mut e, lab, 12.0, 500, if i == 0 { p.text } else { p.muted }, tx, ty + 24.0 * u, 0.0);
            if i == 0 {
                e.push(rect(tx, ty + 32.0 * u, tw, 1.5 * u, 0.0, solid(p.acc)));
            }
            tx += tw + 22.0 * u;
        }
        let tl = [("$ ", "npm run dev", p.text), ("", "  VITE v6.0.3  ready in 412 ms", p.pos), ("", "  ➜  Local:   http://localhost:5173/", p.text2)];
        for (i, (pre, l, c)) in tl.iter().enumerate() {
            let ly = ty + (60.0 + i as f32 * 21.0) * u;
            if ly > ty + term_h - 8.0 * u {
                break;
            }
            let pw = draw(&mut e, s.a(), pre, &ts(Face::Mono, 12.5 * u, 400, p.acc_text), ex + 20.0 * u, ly, 0.0);
            draw(&mut e, s.a(), l, &ts(Face::Mono, 12.5 * u, 400, *c), ex + 20.0 * u + pw, ly, 0.0);
        }
        s.item(o, 3, e);
    }
    // Status bar.
    let sy = y + h - status_h;
    let mut e = vec![rect(x, sy, w, status_h, 0.0, solid(p.side))];
    s.hair(&mut e, x, sy, w);
    let c = sy + status_h * 0.5;
    s.ic(&mut e, "branch", x + 14.0 * u, c - 6.0 * u, 12.0, p.muted);
    s.txt(&mut e, "main", 12.0, 500, p.muted, x + 32.0 * u, ui::mid(c, 12.0 * u), 0.0);
    s.ic(&mut e, "check", x + 80.0 * u, c - 6.0 * u, 12.0, p.pos);
    s.txt(&mut e, "0 problems", 12.0, 400, p.muted, x + 98.0 * u, ui::mid(c, 12.0 * u), 0.0);
    let col = 1 + (lines[cur_line].chars().count() as f32 * type_k) as usize;
    s.txt(&mut e, &format!("Ln {}, Col {}    Spaces: 2    UTF-8    TypeScript", cur_line + 1, col), 12.0, 400, p.muted, x + w - 16.0 * u, ui::mid(c, 12.0 * u), 1.0);
    s.item(o, 1, e);
}

fn d_settings(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let pad = 40.0 * u;
    let cw = (w - 2.0 * pad).min(760.0 * u);
    let x = x + pad;
    let mut e = Vec::new();
    page_title(&mut e, s, "Settings", "Manage your workspace, appearance and notifications.", x, y + 14.0 * u, cw);
    let ty = y + 98.0 * u;
    let mut tx = x;
    for (i, lab) in ["General", "Members", "Billing", "Notifications", "Security"].iter().enumerate() {
        let tw = s.txt(&mut e, lab, 13.5, 500, if i == 0 { p.text } else { p.muted }, tx, ty, 0.0);
        if i == 0 {
            e.push(rect(tx, ty + 12.0 * u, tw, 2.0 * u, u, solid(p.acc)));
        }
        tx += tw + 26.0 * u;
    }
    s.hair(&mut e, x, ty + 14.0 * u, cw);
    s.item(o, 2, e);
    // Appearance card.
    let mut yy = ty + 40.0 * u;
    let row_h = 66.0 * u;
    let mut e = Vec::new();
    s.txt(&mut e, "Appearance", 15.0, 600, p.text, x, yy + 6.0 * u, 0.0);
    yy += 22.0 * u;
    s.card(&mut e, x, yy, cw, row_h * 3.0);
    let rows: [(&str, &str); 3] = [("Interface theme", "Select or customise your UI theme."), ("Accent colour", "Used for buttons, links and highlights."), ("Font size", "Adjust the size of text across the app.")];
    for (i, (l, d)) in rows.iter().enumerate() {
        let ry = yy + row_h * i as f32;
        if i > 0 {
            s.hair(&mut e, x + 20.0 * u, ry, cw - 40.0 * u);
        }
        s.txt(&mut e, l, 13.5, 500, p.text, x + 20.0 * u, ry + 28.0 * u, 0.0);
        s.fit(&mut e, d, 12.5, 400, p.muted, x + 20.0 * u, ry + 47.0 * u, cw * 0.5);
        let rc = ry + row_h * 0.5;
        let right = x + cw - 20.0 * u;
        match i {
            0 => {
                let base = if p.dark { 1.0 } else { 0.0 };
                let sel = if s.motion == Motion::Lively { lerp(2.0, base, ease_in_out(seg(s.t, 1.4, 0.4))) } else { base };
                ui::segmented(&mut e, s.a(), p, &["Light", "Dark", "System"], sel, right, rc - 15.0 * u, 30.0 * u, true);
            }
            1 => {
                let cols = [p.acc, ui::hue(1), ui::hue(2), ui::hue(0), ui::hue(4), ui::hue(5)];
                for (j, c) in cols.iter().enumerate() {
                    let cx = right - 10.0 * u - (5 - j) as f32 * 28.0 * u;
                    if j == 0 {
                        e.push(stroke_op(circle_path(cx, rc, 12.5 * u), solid(*c), 2.0 * u, [0.0, 1.0]));
                    }
                    e.push(circle(cx, rc, 9.0 * u, solid(*c)));
                }
            }
            _ => {
                let bw = 140.0 * u;
                e.push(rect(right - bw, rc - 16.0 * u, bw, 32.0 * u, 7.0 * u, solid(if p.dark { p.sunken } else { p.panel })));
                border(&mut e, right - bw, rc - 16.0 * u, bw, 32.0 * u, 7.0 * u, p.line2);
                s.txt(&mut e, "Default", 13.0, 500, p.text, right - bw + 12.0 * u, ui::mid(rc, 13.0 * u), 0.0);
                s.ic(&mut e, "chevrons-updown", right - 26.0 * u, rc - 7.0 * u, 14.0, p.muted);
            }
        }
    }
    s.item(o, 3, e);
    // Notifications / feature toggles.
    yy += row_h * 3.0 + 30.0 * u;
    let defaults: [(&str, &str, bool); 4] = [("Email digest", "A summary of activity every Monday.", true), ("Push notifications", "Mentions, replies and assignments.", false), ("Product updates", "News about features and improvements.", true), ("Weekly report", "Your team's metrics, every Friday.", false)];
    let n_rows = if s.items.is_empty() { 4 } else { s.items.len().min(5) };
    let rh2 = if s.items.is_empty() { 66.0 * u } else { 54.0 * u };
    let fits = (((y + h - 70.0 * u) - (yy + 22.0 * u)) / rh2).floor().max(0.0) as usize;
    let n_rows = n_rows.min(fits);
    if n_rows > 0 {
        let mut e = Vec::new();
        s.txt(&mut e, if s.items.is_empty() { "Notifications" } else { "Features" }, 15.0, 600, p.text, x, yy + 6.0 * u, 0.0);
        yy += 22.0 * u;
        s.card(&mut e, x, yy, cw, rh2 * n_rows as f32);
        for i in 0..n_rows {
            let ry = yy + rh2 * i as f32;
            if i > 0 {
                s.hair(&mut e, x + 20.0 * u, ry, cw - 40.0 * u);
            }
            let rc = ry + rh2 * 0.5;
            let on0 = if s.items.is_empty() { defaults[i].2 } else { i % 3 != 1 };
            if s.items.is_empty() {
                s.txt(&mut e, defaults[i].0, 13.5, 500, p.text, x + 20.0 * u, ry + 28.0 * u, 0.0);
                s.fit(&mut e, defaults[i].1, 12.5, 400, p.muted, x + 20.0 * u, ry + 47.0 * u, cw - 120.0 * u);
            } else {
                s.fit(&mut e, &s.items[i], 13.5, 500, p.text, x + 20.0 * u, ui::mid(rc, 13.5 * u), cw - 120.0 * u);
            }
            let k = if i == 1 && s.motion != Motion::Still { ease_in_out(seg(s.t, 1.3, 0.28)) } else if on0 || (i == 1 && s.motion == Motion::Still) { 1.0 } else { 0.0 };
            ui::toggle(&mut e, p, x + cw - 20.0 * u - 36.0 * u, rc, 20.0 * u, k, false);
        }
        yy += rh2 * n_rows as f32;
        s.item(o, 4, e);
    }
    // Footer buttons.
    let fy = yy + 20.0 * u;
    if fy + 34.0 * u < y + h {
        let mut e = Vec::new();
        let br = (s.r * 0.7).max(4.0 * u);
        let w1 = ui::button(&mut e, s.a(), p, "Save changes", None, x + cw, fy, 34.0 * u, true, br, true);
        ui::button(&mut e, s.a(), p, "Cancel", None, x + cw - w1 - 10.0 * u, fy, 34.0 * u, false, br, true);
        s.item(o, 5, e);
    }
}

fn d_app(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let pad = 28.0 * u;
    let (x, w) = (x + pad, w - 2.0 * pad);
    // Featured card.
    let fy = y + 24.0 * u;
    let fh = (h * 0.42).min(250.0 * u).max(170.0 * u);
    let mut e = Vec::new();
    s.card(&mut e, x, fy, w, fh);
    push_clip(&mut e, x, fy, w, fh, s.r);
    e.push(rect(x, fy, w, fh, 0.0, radial(x + w * 0.85, fy, w * 0.55, vec![(0.0, alpha(p.acc, if p.dark { 0.22 } else { 0.12 })), (1.0, alpha(p.acc, 0.0))])));
    e.push(Op::PopLayer);
    let ip = 28.0 * u;
    let text_w = if w / u > 700.0 { w * 0.56 } else { w - 2.0 * ip };
    ui::chip(&mut e, s.a(), "New", p.acc_text, x + ip, fy + ip + 10.0 * u, 12.0 * u, true);
    let head = if s.items.is_empty() { format!("Welcome to {}", s.title) } else { s.items[0].clone() };
    let hst = s.hst(26.0, 650, p.text);
    let lines = ui::wrap_lines(s.a(), &head, &hst, text_w, 2);
    let hl = hst.size * 1.18;
    let mut ly = fy + ip + 34.0 * u + hst.size * 0.8;
    for l in &lines {
        draw(&mut e, s.a(), l, &hst, x + ip, ly, 0.0);
        ly += hl;
    }
    let sub = if s.items.is_empty() { "Set up your workspace in a few minutes.".to_string() } else { format!("Now available to everyone on {}.", s.title) };
    s.fit(&mut e, &sub, 14.0, 400, p.muted, x + ip, ly - hl + 26.0 * u, text_w);
    let by = fy + fh - ip - 34.0 * u;
    let br = (s.r * 0.7).max(4.0 * u);
    let b1 = ui::button(&mut e, s.a(), p, if s.items.is_empty() { "Get started" } else { "Try it now" }, None, x + ip, by, 34.0 * u, true, br, false);
    ui::button(&mut e, s.a(), p, "Learn more", Some("arrow-up-right"), x + ip + b1 + 10.0 * u, by, 34.0 * u, false, br, false);
    if w / u > 700.0 {
        // Preview panel: the next items as a checklist.
        let (px, py, pw, ph) = (x + w * 0.62, fy + ip, w * 0.38 - ip, fh - 2.0 * ip);
        e.push(rect(px, py, pw, ph, s.r * 0.8, solid(if p.dark { p.sunken } else { p.side })));
        border(&mut e, px, py, pw, ph, s.r * 0.8, p.line);
        let list: Vec<String> = if s.items.len() > 1 { s.items[1..].iter().take(4).cloned().collect() } else { vec!["Invite your team".into(), "Connect your data".into(), "Create a project".into(), "Share a link".into()] };
        let rh = (ph - 16.0 * u) / 4.0;
        for (i, l) in list.iter().enumerate() {
            let rc = py + 8.0 * u + rh * (i as f32 + 0.5);
            let done_at = 0.9 + i as f32 * 0.35;
            let done = if s.motion == Motion::Still { i < 2 } else { s.t > done_at && i < 3 };
            ui::status(&mut e, p, if done { 4 } else { 1 }, px + 22.0 * u, rc, 8.0 * u);
            s.fit(&mut e, l, 13.0, 500, if done { p.muted } else { p.text }, px + 40.0 * u, ui::mid(rc, 13.0 * u), pw - 56.0 * u);
        }
    }
    s.item(o, 2, e);
    // List.
    let ly = fy + fh + 30.0 * u;
    let mut e = Vec::new();
    let lt = s.txt(&mut e, if s.items.is_empty() { "Recent" } else { "All updates" }, 14.0, 600, p.text, x, ly, 0.0);
    let defaults = ["Invite your team", "Connect your data", "Create your first project", "Customise your workspace", "Explore templates", "Set up notifications", "Add a custom domain", "Import from CSV"];
    let count = if s.items.is_empty() { defaults.len() } else { s.items.len() };
    ui::chip(&mut e, s.a(), &count.to_string(), p.muted, x + lt + 8.0 * u, ly - 5.0 * u, 11.0 * u, false);
    s.item(o, 3, e);
    let prefix: String = s.slug().chars().take(3).collect::<String>().to_uppercase();
    let row_h = 44.0 * u;
    let top = ly + 14.0 * u;
    let n = ((y + h - top - 10.0 * u) / row_h).floor().max(0.0) as usize;
    let n = n.min(count);
    // Hover glide.
    let lt2 = s.lt(0.0);
    let hov = if s.live() > 0.0 && n > 1 {
        let ph = ((lt2 - 1.6) / 1.3).max(0.0);
        let i0 = ph.floor() as usize % n;
        let f = ease_in_out(((ph.fract() - 0.6) / 0.4).clamp(0.0, 1.0));
        Some(lerp(i0 as f32, ((i0 + 1) % n) as f32, if (i0 + 1).is_multiple_of(n) { 0.0 } else { f }))
    } else {
        None
    };
    let mut e = Vec::new();
    if let (Some(hv), true) = (hov, s.t > 1.6) {
        e.push(rect(x - 10.0 * u, top + hv * row_h, w + 20.0 * u, row_h, 8.0 * u, solid(p.hover)));
    }
    o.extend(e);
    let labels = [("Feature", 1usize), ("Improvement", 2), ("Fix", 0)];
    let dates = ["Today", "Today", "Yesterday", "2d ago", "3d ago", "Last week"];
    for i in 0..n {
        let ry = top + row_h * i as f32;
        let rc = ry + row_h * 0.5;
        let mut e = Vec::new();
        if i > 0 {
            s.hair(&mut e, x, ry, w);
        }
        let st = if s.items.is_empty() { [4u8, 2, 1, 1, 0, 0, 1, 0][i % 8] } else { [4u8, 4, 2, 3, 1, 0][i % 6] };
        ui::status(&mut e, p, st, x + 8.0 * u, rc, 7.5 * u);
        let id = format!("{prefix}-{}", 142 - i * 3);
        let idw = draw(&mut e, s.a(), &id, &ts(Face::Mono, 12.0 * u, 400, p.faint), x + 26.0 * u, ui::mid(rc, 12.0 * u), 0.0);
        let label = if s.items.is_empty() { defaults[i % defaults.len()] } else { s.items[i].as_str() };
        let right_w = 220.0 * u;
        s.fit(&mut e, label, 13.5, 500, p.text, x + 36.0 * u + idw, ui::mid(rc, 13.5 * u), w - right_w - idw - 40.0 * u);
        draw(&mut e, s.a(), dates[i % 6], &s.st(12.5, 400, p.muted), x + w, ui::mid(rc, 12.5 * u), 1.0);
        let (lab, hc) = labels[i % 3];
        ui::label_chip(&mut e, s.a(), p, lab, ui::hue(hc), x + w - 86.0 * u, rc, 11.5 * u, true);
        s.item(o, 4 + i, e);
    }
}

// ====================================================================== mobile chrome

/// iOS large-title header: optional left/right icons; returns the y below it.
fn m_header(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, title: &str, right: Option<&str>, avatar: bool) -> f32 {
    let u = s.u;
    let p = &s.p;
    let mut e = Vec::new();
    let row = y + 22.0 * u;
    if avatar {
        ui::avatar(&mut e, s.a(), "You", x + w - 34.0 * u, row + 34.0 * u, 17.0 * u, None);
    } else if let Some(icn) = right {
        s.ic(&mut e, icn, x + w - 42.0 * u, row - 11.0 * u, 22.0, p.acc_text);
    }
    let st = s.hst(32.0, 700, p.text);
    ui::fit(&mut e, s.a(), title, &st, x + 20.0 * u, row + 46.0 * u, w - 90.0 * u, 0.0);
    s.item(o, 0, e);
    row + 62.0 * u
}

/// iOS tab bar with `active` selected; returns its top y.
fn m_tabbar(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32, tabs: &[(&str, &str)], active: usize) -> f32 {
    let u = s.u;
    let p = &s.p;
    let th = 84.0 * u;
    let ty = y + h - th;
    let mut e = Vec::new();
    e.push(rect(x, ty, w, th, 0.0, solid(alpha(p.side, 0.96))));
    s.hair(&mut e, x, ty, w);
    let n = tabs.len() as f32;
    for (i, (icn, lab)) in tabs.iter().enumerate() {
        let cx = x + w * (i as f32 + 0.5) / n;
        let c = if i == active { p.acc_text } else { p.faint };
        if i == active && !matches!(*icn, "search" | "chart" | "settings" | "edit" | "code" | "terminal") {
            ui::icon_filled(&mut e, icn, cx - 12.0 * u, ty + 9.0 * u, 24.0 * u, c);
        } else {
            ui::icon(&mut e, icn, cx - 12.0 * u, ty + 9.0 * u, 24.0 * u, c, if i == active { 2.2 } else { 1.8 });
        }
        s.txt(&mut e, lab, 10.0, 500, c, cx, ty + 46.0 * u, 0.5);
    }
    s.item(o, 1, e);
    ty
}

fn m_dashboard(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let tb = m_tabbar(o, s, x, y, w, h, &[("home", "Home"), ("chart", "Analytics"), ("inbox", "Inbox"), ("settings", "Settings")], 1);
    let mut yy = m_header(o, s, x, y, w, "Overview", None, true);
    let pad = 16.0 * u;
    let (x, w) = (x + pad, w - 2.0 * pad);
    let mut e = Vec::new();
    let sel = 2.0 - if s.motion == Motion::Lively { 1.0 - ease_in_out(seg(s.t, 1.6, 0.35)) } else { 0.0 };
    ui::segmented_w(&mut e, s.a(), p, &["Day", "Week", "Month", "Year"], sel, x, yy + 6.0 * u, w, 32.0 * u);
    s.item(o, 2, e);
    yy += 52.0 * u;
    // KPI grid 2×2 (or kpis).
    let n = if s.kpis.is_empty() { 4 } else { s.kpis.len().min(4) };
    let gap = 12.0 * u;
    let kw = (w - gap) / 2.0;
    let kh = 96.0 * u;
    for i in 0..n {
        let (c, r) = (i % 2, i / 2);
        let (kx, ky) = (x + c as f32 * (kw + gap), yy + r as f32 * (kh + gap));
        let (kx, kw2) = if n == 1 || (n == 3 && i == 2) { (x, w) } else { (kx, kw) };
        let mut e = Vec::new();
        s.card(&mut e, kx, ky, kw2, kh);
        let k = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(s.t, 0.35 + i as f32 * 0.08, 1.3)) as f64 };
        let (label, val, d) = kpi(s, i, k);
        s.fit(&mut e, &label, 13.0, 500, p.muted, kx + 14.0 * u, ky + 26.0 * u, kw2 - 28.0 * u);
        ui::fit(&mut e, s.a(), &val, &s.nst(23.0, p.text), kx + 14.0 * u, ky + 58.0 * u, kw2 - 28.0 * u, 0.0);
        if let Some(d) = d {
            delta(&mut e, s, d, kx + 14.0 * u, ky + 78.0 * u, 11.0);
        }
        s.item(o, 3 + i, e);
    }
    yy += n.div_ceil(2) as f32 * (kh + gap) + 4.0 * u;
    // Chart card.
    let room = tb - yy - 16.0 * u;
    let ch = if room - 276.0 * u < 140.0 * u { room } else { 260.0 * u };
    if ch > 120.0 * u {
        let mut e = Vec::new();
        s.card(&mut e, x, yy, w, ch);
        let ct = s.kpis.first().map_or("Revenue", |k| k.label.as_str());
        s.fit(&mut e, ct, 15.0, 600, p.text, x + 14.0 * u, yy + 28.0 * u, w * 0.6);
        if s.kpis.is_empty() {
            delta(&mut e, s, 18.2, x + w - 14.0 * u - 66.0 * u, yy + 23.0 * u, 11.0);
        }
        area_chart(&mut e, s, x + 6.0 * u, yy + 48.0 * u, w - 20.0 * u, ch - 56.0 * u, s.kpis.is_empty(), 4, 3);
        s.item(o, 8, e);
        yy += ch + 16.0 * u;
    }
    // List.
    if tb - yy > 120.0 * u {
        let mut e = Vec::new();
        let rows: Vec<String> = if s.items.is_empty() { vec!["Direct".into(), "Organic search".into(), "Referral".into(), "Social".into()] } else { s.items.to_vec() };
        let rh = 50.0 * u;
        let nr = (((tb - yy - 16.0 * u) / rh).floor() as usize).min(rows.len());
        if nr > 0 {
            s.card(&mut e, x, yy, w, rh * nr as f32);
            for (i, r) in rows.iter().take(nr).enumerate() {
                let rc = yy + rh * (i as f32 + 0.5);
                if i > 0 {
                    s.hair(&mut e, x + 52.0 * u, yy + rh * i as f32, w - 52.0 * u);
                }
                e.push(rect(x + 12.0 * u, rc - 15.0 * u, 30.0 * u, 30.0 * u, 8.0 * u, solid(alpha(ui::hue(i + 1), 0.15))));
                s.ic(&mut e, ["globe", "search", "share", "users", "zap", "layers"][i % 6], x + 19.0 * u, rc - 8.0 * u, 16.0, ui::hue(i + 1));
                s.fit(&mut e, r, 15.0, 500, p.text, x + 54.0 * u, ui::mid(rc, 15.0 * u), w - 120.0 * u);
                if s.items.is_empty() {
                    draw(&mut e, s.a(), &format!("{}%", [42, 28, 17, 9][i]), &s.st(15.0, 500, p.muted).tabular(), x + w - 32.0 * u, ui::mid(rc, 15.0 * u), 1.0);
                }
                s.ic(&mut e, "chevron-right", x + w - 26.0 * u, rc - 7.0 * u, 14.0, p.faint);
            }
            s.item(o, 9, e);
        }
    }
}

fn m_chat(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let has_items = !s.items.is_empty();
    let (peer, bot) = if has_items { (s.title.to_string(), true) } else { ("Maya Chen".to_string(), false) };
    // Header.
    let hh = 96.0 * u;
    let mut e = vec![rect(x, y - 80.0 * u, w, hh + 80.0 * u, 0.0, solid(p.side))];
    s.hair(&mut e, x, y + hh, w);
    s.ic(&mut e, "chevron-left", x + 10.0 * u, y + 30.0 * u, 26.0, p.acc_text);
    let cx = x + w * 0.5;
    if bot {
        ui::logo(&mut e, s.a(), s.title, p.acc, cx - 22.0 * u, y + 10.0 * u, 44.0 * u);
    } else {
        ui::avatar(&mut e, s.a(), &peer, cx, y + 32.0 * u, 22.0 * u, None);
    }
    let nw = ui::fit(&mut e, s.a(), &peer, &s.st(12.0, 500, p.text), cx, y + 78.0 * u, w * 0.5, 0.5);
    s.ic(&mut e, "chevron-right", cx + nw * 0.5 + 2.0 * u, y + 70.0 * u, 10.0, p.faint);
    s.ic(&mut e, "video", x + w - 40.0 * u, y + 32.0 * u, 26.0, p.acc_text);
    s.item(o, 0, e);
    // Composer.
    let comp_y = y + h - 34.0 * u - 50.0 * u;
    let mut e = Vec::new();
    e.push(circle(x + 30.0 * u, comp_y + 25.0 * u, 17.0 * u, solid(p.sunken)));
    s.ic(&mut e, "plus", x + 21.0 * u, comp_y + 16.0 * u, 18.0, p.muted);
    let (fx, fw) = (x + 56.0 * u, w - 72.0 * u);
    e.push(rect(fx, comp_y + 8.0 * u, fw, 34.0 * u, 17.0 * u, solid(p.bg)));
    border(&mut e, fx, comp_y + 8.0 * u, fw, 34.0 * u, 17.0 * u, p.line2);
    s.txt(&mut e, "Message", 16.0, 400, p.faint, fx + 14.0 * u, ui::mid(comp_y + 25.0 * u, 16.0 * u), 0.0);
    s.ic(&mut e, "mic", fx + fw - 30.0 * u, comp_y + 15.0 * u, 18.0, p.faint);
    s.item(o, 1, e);
    // Bubbles: (mine, text).
    let msgs: Vec<(bool, String)> = if has_items {
        let mut v = vec![(false, format!("Hi! I'm the {} assistant. Ask me anything.", s.title)), (true, format!("What's new in {}?", s.title))];
        for it in s.items.iter().take(3) {
            v.push((false, it.clone()));
        }
        v.push((true, "Love it. Sharing with the team!".into()));
        v
    } else {
        vec![(true, "Sending the slides over tonight".into()), (false, "Perfect, thank you!".into()), (false, "Morning! Did the new build go out?".into()), (true, "Yep, it's in review now".into()), (true, "Should be live by lunch".into()), (false, "Are we still on for the launch review at 3?".into()), (true, "Yes! Deck is almost done".into()), (false, "Amazing. Can you add the new retention chart?".into()), (true, "Already in there, slide 6".into())]
    };
    let maxw = w * 0.72;
    let size = 16.0;
    let st_in = s.st(size, 400, p.text);
    let them = if p.dark { parse_color("#26262b").unwrap() } else { parse_color("#e9e9eb").unwrap() };
    let pad_x = 13.0 * u;
    let lh = 21.0 * u;
    let laid: Vec<(bool, Vec<String>, f32, f32)> = msgs
        .iter()
        .map(|(mine, t)| {
            let lines = ui::wrap_lines(s.a(), t, &st_in, maxw - 2.0 * pad_x, 4);
            let bw = lines.iter().map(|l| s.a().measure(l, &st_in)).fold(0.0, f32::max) + 2.0 * pad_x;
            let bh = lh * lines.len() as f32 + 15.0 * u;
            (*mine, lines, bw, bh)
        })
        .collect();
    // Typing bubble at the bottom; messages stacked upward from it.
    let typing_h = 38.0 * u;
    let bottom = comp_y - 10.0 * u;
    let mut yy = bottom - typing_h - 8.0 * u;
    let mut tops = vec![0.0; laid.len()];
    for i in (0..laid.len()).rev() {
        let gap = if i + 1 < laid.len() && laid[i + 1].0 == laid[i].0 { 3.0 * u } else { 10.0 * u };
        yy -= laid[i].3;
        tops[i] = yy;
        yy -= gap;
    }
    let mut e = Vec::new();
    if yy > y + hh + 30.0 * u {
        s.txt(&mut e, "Today 9:41 AM", 11.5, 500, p.faint, x + w * 0.5, yy - 6.0 * u, 0.5);
    }
    s.item(o, 2, e);
    for (i, (mine, lines, bw, bh)) in laid.iter().enumerate() {
        if tops[i] < y + hh {
            continue;
        }
        let bx = if *mine { x + w - 14.0 * u - bw } else { x + 14.0 * u };
        let mut e = Vec::new();
        let fillc = if *mine { solid(p.acc) } else { solid(them) };
        e.push(rect(bx, tops[i], *bw, *bh, 18.0 * u, fillc));
        let tc = if *mine { p.on_acc } else { p.text };
        for (j, l) in lines.iter().enumerate() {
            draw(&mut e, s.a(), l, &s.st(size, 400, tc), bx + pad_x, tops[i] + 8.0 * u + lh * (j as f32 + 0.78), 0.0);
        }
        s.item(o, 3 + i * 2, e);
    }
    // Typing bubble.
    let mut e = Vec::new();
    let ty = bottom - typing_h;
    e.push(rect(x + 14.0 * u, ty, 60.0 * u, typing_h, 19.0 * u, solid(them)));
    typing_dots(&mut e, s, x + 34.0 * u, ty + typing_h * 0.5, 3.6 * u, p.muted);
    s.item(o, 3 + laid.len() * 2, e);
}

fn m_feed(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let tb = m_tabbar(o, s, x, y, w, h, &[("home", "Home"), ("search", "Search"), ("bell", "Activity"), ("inbox", "Inbox")], 0);
    // Top bar: avatar, logo, sparkles.
    let mut e = Vec::new();
    let row = y + 26.0 * u;
    ui::avatar(&mut e, s.a(), "You", x + 32.0 * u, row, 16.0 * u, None);
    ui::logo(&mut e, s.a(), s.title, p.acc, x + w * 0.5 - 15.0 * u, row - 15.0 * u, 30.0 * u);
    s.ic(&mut e, "sliders", x + w - 44.0 * u, row - 11.0 * u, 22.0, p.text2);
    let ty = row + 46.0 * u;
    for (i, lab) in ["For you", "Following"].iter().enumerate() {
        let cx = x + w * (0.25 + 0.5 * i as f32);
        let tw = s.txt(&mut e, lab, 15.0, if i == 0 { 600 } else { 500 }, if i == 0 { p.text } else { p.muted }, cx, ty, 0.5);
        if i == 0 {
            e.push(rect(cx - tw * 0.5 - 4.0 * u, ty + 13.0 * u, tw + 8.0 * u, 3.0 * u, 1.5 * u, solid(p.acc)));
        }
    }
    s.hair(&mut e, x, ty + 16.0 * u, w);
    s.item(o, 0, e);
    let mut yy = ty + 32.0 * u;
    for (i, ps) in posts(s).iter().enumerate() {
        if yy > tb {
            break;
        }
        let mut e = Vec::new();
        let ph = post(&mut e, s, ps, x + 14.0 * u, yy, w - 28.0 * u, 15.0, i, false);
        s.hair(&mut e, x, yy + ph + 4.0 * u, w);
        s.item(o, 2 + i, e);
        yy += ph + 20.0 * u;
    }
    // Compose button.
    let mut e = Vec::new();
    let (bx, by) = (x + w - 74.0 * u, tb - 74.0 * u);
    shadow(&mut e, bx, by, 56.0 * u, 56.0 * u, 28.0 * u, 6.0 * u, 16.0 * u, 0.3);
    e.push(circle(bx + 28.0 * u, by + 28.0 * u, 28.0 * u, solid(p.acc)));
    s.ic(&mut e, "plus", bx + 16.0 * u, by + 16.0 * u, 24.0, p.on_acc);
    s.item(o, 8, e);
}

fn m_editor(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let mut e = Vec::new();
    let row = y + 24.0 * u;
    s.ic(&mut e, "chevron-left", x + 10.0 * u, row - 13.0 * u, 26.0, p.acc_text);
    s.txt(&mut e, "sync.ts", 17.0, 600, p.text, x + w * 0.5, ui::mid(row, 17.0 * u), 0.5);
    let bw = 34.0 * u;
    e.push(rect(x + w - 16.0 * u - bw, row - bw * 0.5, bw, bw, bw * 0.5, solid(p.acc)));
    let mut tri = Path::new();
    let (tcx, tcy) = (x + w - 16.0 * u - bw * 0.5 + 1.5 * u, row);
    tri.move_to(((tcx - 4.5 * u) as f64, (tcy - 6.5 * u) as f64));
    tri.line_to(((tcx + 6.5 * u) as f64, tcy as f64));
    tri.line_to(((tcx - 4.5 * u) as f64, (tcy + 6.5 * u) as f64));
    tri.close_path();
    e.push(fill(tri, solid(p.on_acc)));
    s.hair(&mut e, x, row + 26.0 * u, w);
    s.item(o, 0, e);
    // Accessory bar above the home indicator.
    let ab = y + h - 34.0 * u - 46.0 * u;
    let mut e = vec![rect(x, ab, w, h - (ab - y), 0.0, solid(p.side))];
    s.hair(&mut e, x, ab, w);
    let keys = ["tab", "{", "}", "(", ")", ";", "=", "\""];
    let kw = (w - 16.0 * u) / keys.len() as f32;
    for (i, k) in keys.iter().enumerate() {
        let kx = x + 8.0 * u + kw * i as f32 + 3.0 * u;
        e.push(rect(kx, ab + 8.0 * u, kw - 6.0 * u, 30.0 * u, 6.0 * u, solid(if p.dark { p.sunken } else { p.panel })));
        border(&mut e, kx, ab + 8.0 * u, kw - 6.0 * u, 30.0 * u, 6.0 * u, p.line);
        draw(&mut e, s.a(), k, &ts(Face::Mono, 13.0 * u, 500, p.text2), kx + (kw - 6.0 * u) * 0.5, ui::mid(ab + 23.0 * u, 13.0 * u), 0.5);
    }
    s.item(o, 1, e);
    // Console above the accessory bar.
    let con_h = 150.0 * u;
    let con_y = ab - con_h;
    let mut e = vec![rect(x, con_y, w, con_h, 0.0, solid(p.side))];
    s.hair(&mut e, x, con_y, w);
    s.txt(&mut e, "Console", 13.0, 600, p.text, x + 16.0 * u, con_y + 26.0 * u, 0.0);
    ui::chip(&mut e, s.a(), "Running", p.pos, x + w - 16.0 * u - 76.0 * u, con_y + 21.0 * u, 11.0 * u, true);
    let out = [("$ ", "node sync.ts", p.text), ("", "Connected to eu-west-1", p.muted), ("", "Indexed 1,284 items in 2.1s", p.pos)];
    let shown = if s.motion == Motion::Still { 3 } else { ((s.t - 0.9) / 0.45).clamp(0.0, 3.0) as usize };
    for (i, (pre, l, c)) in out.iter().enumerate().take(shown) {
        let ly = con_y + (56.0 + i as f32 * 22.0) * u;
        let pw = draw(&mut e, s.a(), pre, &ts(Face::Mono, 12.0 * u, 400, p.acc_text), x + 16.0 * u, ly, 0.0);
        draw(&mut e, s.a(), l, &ts(Face::Mono, 12.0 * u, 400, *c), x + 16.0 * u + pw, ly, 0.0);
    }
    s.item(o, 3, e);
    // Code, soft-wrapped to the width (continuations indented, unnumbered).
    let lines = code_lines(s);
    let size = 11.5 * u;
    let lh = 19.0 * u;
    let gutter = 34.0 * u;
    let top = row + 44.0 * u;
    let mono = ts(Face::Mono, size, 400, p.text);
    let cw = s.a().measure("0000000000", &mono) / 10.0;
    let cols = (((w - gutter - 10.0 * u) / cw).floor() as usize).max(12);
    let type_k = if s.motion == Motion::Still { 1.0 } else { seg(s.t, 0.7, 1.4) };
    let cur = 13usize;
    let mut e = Vec::new();
    push_clip(&mut e, x, top - 16.0 * u, w, con_y - top + 10.0 * u, 0.0);
    let mut row_i = 0usize;
    for (i, l) in lines.iter().enumerate() {
        let full: String = if i == cur { l.chars().take((l.chars().count() as f32 * type_k).round() as usize).collect() } else { l.clone() };
        // Split into visual rows.
        let indent = l.chars().take_while(|c| *c == ' ').count() + 4;
        let mut rows: Vec<String> = Vec::new();
        let mut rest: Vec<char> = full.chars().collect();
        let mut first = true;
        loop {
            let lim = if first { cols } else { cols.saturating_sub(indent).max(8) };
            if rest.len() <= lim {
                rows.push(rest.iter().collect());
                break;
            }
            let cut = rest[..lim].iter().rposition(|c| *c == ' ').filter(|p| *p > 0).unwrap_or(lim);
            rows.push(rest[..cut].iter().collect());
            rest = rest[cut..].iter().copied().skip_while(|c| *c == ' ').collect();
            first = false;
        }
        for (j, r) in rows.iter().enumerate() {
            let ly = top + lh * row_i as f32;
            if i == cur {
                e.push(rect(x, ly - lh * 0.72, w, lh, 0.0, solid(p.hover)));
            }
            if j == 0 {
                draw(&mut e, s.a(), &(i + 1).to_string(), &ts(Face::Mono, size * 0.92, 400, if i == cur { p.text2 } else { p.faint }), x + gutter - 10.0 * u, ly, 1.0);
            }
            let rx = x + gutter + if j == 0 { 0.0 } else { cw * indent as f32 };
            let end = code_line(&mut e, s, r, rx, ly, size);
            if i == cur && j + 1 == rows.len() && (s.live() == 0.0 || (s.t * 1.7).fract() < 0.6 || type_k < 1.0) {
                e.push(rect(end + 1.0, ly - lh * 0.62, 1.8 * u, lh * 0.8, 0.0, solid(p.acc)));
            }
            row_i += 1;
        }
    }
    e.push(Op::PopLayer);
    s.item(o, 2, e);
}

fn m_settings(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    // Grouped background.
    let gbg = if p.dark { parse_color("#000000").map(|c| mix(c, p.bg, 0.4)).unwrap() } else { p.side };
    o.push(rect(x, y - 80.0 * u, w, h + 80.0 * u, 0.0, solid(gbg)));
    let mut yy = m_header(o, s, x, y, w, "Settings", None, false);
    let pad = 16.0 * u;
    let (x, w) = (x + pad, w - 2.0 * pad);
    let r = s.r.max(10.0 * u);
    let cell = if p.dark { p.panel } else { p.bg };
    let mut e = Vec::new();
    e.push(rect(x, yy + 4.0 * u, w, 36.0 * u, 10.0 * u, solid(if p.dark { p.sunken } else { mix(p.side, Rgba([0.0, 0.0, 0.05, 1.0]), 0.06) })));
    s.ic(&mut e, "search", x + 10.0 * u, yy + 13.0 * u, 18.0, p.muted);
    s.txt(&mut e, "Search", 16.0, 400, p.muted, x + 34.0 * u, ui::mid(yy + 22.0 * u, 16.0 * u), 0.0);
    s.item(o, 2, e);
    yy += 56.0 * u;
    // Profile cell.
    let mut e = Vec::new();
    let ph = 76.0 * u;
    e.push(rect(x, yy, w, ph, r, solid(cell)));
    ui::logo(&mut e, s.a(), s.title, p.acc, x + 14.0 * u, yy + 14.0 * u, 48.0 * u);
    s.fit(&mut e, s.title, 19.0, 600, p.text, x + 74.0 * u, yy + 35.0 * u, w - 110.0 * u);
    s.fit(&mut e, "Account, sync & billing", 13.0, 400, p.muted, x + 74.0 * u, yy + 55.0 * u, w - 110.0 * u);
    s.ic(&mut e, "chevron-right", x + w - 26.0 * u, yy + ph * 0.5 - 7.0 * u, 14.0, p.faint);
    s.item(o, 3, e);
    yy += ph + 26.0 * u;
    // Group 1.
    let defaults: [(&str, &str, usize, u8); 5] = [("Notifications", "bell", 0, 1), ("Appearance", "sun", 1, 2), ("Focus mode", "moon", 6, 0), ("Sync", "repeat", 2, 1), ("Privacy", "lock", 5, 2)];
    let rh = 46.0 * u;
    let n = if s.items.is_empty() { 5 } else { s.items.len().min(6) };
    let avail = ((y + h - 40.0 * u - yy) / rh).floor().max(0.0) as usize;
    let n = n.min(avail);
    if n == 0 {
        return;
    }
    let mut e = Vec::new();
    e.push(rect(x, yy, w, rh * n as f32, r, solid(cell)));
    for i in 0..n {
        let rc = yy + rh * (i as f32 + 0.5);
        let (label, icn, hue, ctl) = if s.items.is_empty() {
            let d = defaults[i];
            (d.0, d.1, d.2, d.3)
        } else {
            (s.items[i].as_str(), ["zap", "sparkles", "layers", "shield", "globe", "users"][i % 6], i + 1, if i % 2 == 0 { 1 } else { 2 })
        };
        if i > 0 {
            s.hair(&mut e, x + 54.0 * u, yy + rh * i as f32, w - 54.0 * u);
        }
        let hc = ui::hue(hue);
        e.push(rect(x + 14.0 * u, rc - 14.5 * u, 29.0 * u, 29.0 * u, 7.0 * u, solid(hc)));
        s.ic(&mut e, icn, x + 20.5 * u, rc - 8.0 * u, 16.0, Rgba([1.0, 1.0, 1.0, 1.0]));
        let right_w = if ctl == 1 { 70.0 * u } else { 90.0 * u };
        s.fit(&mut e, label, 16.0, 400, p.text, x + 56.0 * u, ui::mid(rc, 16.0 * u), w - 56.0 * u - right_w);
        match ctl {
            1 => {
                let k = if i == 0 && s.motion != Motion::Still { ease_in_out(seg(s.t, 1.3, 0.3)) } else { 1.0 };
                ui::toggle(&mut e, p, x + w - 14.0 * u - 51.0 * u, rc, 31.0 * u, k, true);
            }
            2 => {
                let v = if s.items.is_empty() { if i == 1 { if p.dark { "Dark" } else { "Light" } } else { "On" } } else { "On" };
                s.txt(&mut e, v, 16.0, 400, p.muted, x + w - 32.0 * u, ui::mid(rc, 16.0 * u), 1.0);
                s.ic(&mut e, "chevron-right", x + w - 26.0 * u, rc - 7.0 * u, 14.0, p.faint);
            }
            _ => s.ic(&mut e, "chevron-right", x + w - 26.0 * u, rc - 7.0 * u, 14.0, p.faint),
        }
    }
    s.item(o, 4, e);
    yy += rh * n as f32 + 26.0 * u;
    // Group 2.
    if yy + rh * 2.0 < y + h - 30.0 * u {
        let mut e = Vec::new();
        e.push(rect(x, yy, w, rh * 2.0, r, solid(cell)));
        for (i, (l, icn, hue)) in [("Help & feedback", "help", 1usize), ("About", "globe", 7)].iter().enumerate() {
            let rc = yy + rh * (i as f32 + 0.5);
            if i > 0 {
                s.hair(&mut e, x + 54.0 * u, yy + rh, w - 54.0 * u);
            }
            e.push(rect(x + 14.0 * u, rc - 14.5 * u, 29.0 * u, 29.0 * u, 7.0 * u, solid(ui::hue(*hue))));
            s.ic(&mut e, icn, x + 20.5 * u, rc - 8.0 * u, 16.0, Rgba([1.0, 1.0, 1.0, 1.0]));
            s.txt(&mut e, l, 16.0, 400, p.text, x + 56.0 * u, ui::mid(rc, 16.0 * u), 0.0);
            s.ic(&mut e, "chevron-right", x + w - 26.0 * u, rc - 7.0 * u, 14.0, p.faint);
        }
        s.item(o, 5, e);
        yy += rh * 2.0 + 26.0 * u;
    }
    // Storage usage.
    if yy + 110.0 * u < y + h - 30.0 * u {
        let mut e = Vec::new();
        let ch = 104.0 * u;
        e.push(rect(x, yy, w, ch, r, solid(cell)));
        s.txt(&mut e, "Storage", 16.0, 500, p.text, x + 16.0 * u, yy + 30.0 * u, 0.0);
        draw(&mut e, s.a(), "38.4 GB of 50 GB", &s.st(14.0, 400, p.muted).tabular(), x + w - 16.0 * u, yy + 30.0 * u, 1.0);
        let (bx, by, bw, bh) = (x + 16.0 * u, yy + 46.0 * u, w - 32.0 * u, 10.0 * u);
        e.push(rect(bx, by, bw, bh, bh * 0.5, solid(if p.dark { p.sunken } else { mix(p.side, Rgba([0.0, 0.0, 0.0, 1.0]), 0.05) })));
        push_clip(&mut e, bx, by, bw, bh, bh * 0.5);
        let parts = [(0.34f32, ui::hue(1)), (0.22, ui::hue(3)), (0.12, ui::hue(4)), (0.09, ui::hue(2))];
        let kk = if s.motion == Motion::Still { 1.0 } else { ease_out(seg(s.t, 0.9, 1.0)) };
        let mut cx = bx;
        for (f, c) in parts {
            let pw = bw * f * kk;
            e.push(rect(cx, by, pw - 1.5 * u, bh, 0.0, solid(c)));
            cx += pw;
        }
        e.push(Op::PopLayer);
        let mut lx = bx;
        for (j, lab) in ["Photos", "Docs", "Mail", "Apps"].iter().enumerate() {
            e.push(circle(lx + 4.0 * u, yy + 78.0 * u, 4.0 * u, solid(parts[j].1)));
            lx += 14.0 * u + s.txt(&mut e, lab, 12.5, 400, p.muted, lx + 12.0 * u, ui::mid(yy + 78.0 * u, 12.5 * u), 0.0) + 12.0 * u;
        }
        s.item(o, 6, e);
    }
}

fn m_app(o: &mut Vec<Op>, s: &S, x: f32, y: f32, w: f32, h: f32) {
    let u = s.u;
    let p = &s.p;
    let tb = m_tabbar(o, s, x, y, w, h, &[("home", "Home"), ("search", "Search"), ("inbox", "Inbox"), ("user", "Account")], 0);
    let mut yy = m_header(o, s, x, y, w, s.title, None, true);
    let pad = 16.0 * u;
    let (x, w) = (x + pad, w - 2.0 * pad);
    // Featured card.
    let mut e = Vec::new();
    let head = if s.items.is_empty() { format!("Welcome to {}", s.title) } else { s.items[0].clone() };
    let hst = s.hst(22.0, 700, p.text);
    let lines = ui::wrap_lines(s.a(), &head, &hst, w - 36.0 * u, 3);
    let fh = 146.0 * u + hst.size * 1.18 * lines.len() as f32;
    s.card(&mut e, x, yy + 6.0 * u, w, fh);
    push_clip(&mut e, x, yy + 6.0 * u, w, fh, s.r);
    e.push(rect(x, yy, w, fh + 6.0 * u, 0.0, radial(x + w, yy, w * 0.9, vec![(0.0, alpha(p.acc, if p.dark { 0.3 } else { 0.16 })), (1.0, alpha(p.acc, 0.0))])));
    e.push(Op::PopLayer);
    ui::chip(&mut e, s.a(), "New", p.acc_text, x + 18.0 * u, yy + 32.0 * u, 12.0 * u, true);
    let mut ly = yy + 52.0 * u + hst.size * 0.85;
    for l in &lines {
        draw(&mut e, s.a(), l, &hst, x + 18.0 * u, ly, 0.0);
        ly += hst.size * 1.18;
    }
    let sub = if s.items.is_empty() { "Set up your workspace in minutes.".to_string() } else { format!("Now available on {}.", s.title) };
    s.fit(&mut e, &sub, 14.0, 400, p.muted, x + 18.0 * u, ly - hst.size * 1.18 + 26.0 * u, w - 36.0 * u);
    let br = 10.0 * u;
    ui::button(&mut e, s.a(), p, if s.items.is_empty() { "Get started" } else { "Try it now" }, None, x + 18.0 * u, yy + 6.0 * u + fh - 54.0 * u, 36.0 * u, true, br, false);
    s.item(o, 2, e);
    yy += fh + 30.0 * u;
    // List of the rest.
    let rest: Vec<String> = if s.items.len() > 1 { s.items[1..].to_vec() } else if s.items.is_empty() { vec!["Invite your team".into(), "Connect your data".into(), "Create a project".into(), "Explore templates".into()] } else { Vec::new() };
    if rest.is_empty() {
        return;
    }
    let mut e = Vec::new();
    s.txt(&mut e, if s.items.is_empty() { "Get set up" } else { "Also new" }, 13.0, 600, p.muted, x + 4.0 * u, yy, 0.0);
    s.item(o, 3, e);
    yy += 12.0 * u;
    let rh = 64.0 * u;
    let n = (((tb - yy - 12.0 * u) / rh).floor().max(0.0) as usize).min(rest.len());
    if n == 0 {
        return;
    }
    let mut e = Vec::new();
    e.push(rect(x, yy, w, rh * n as f32, s.r, solid(p.panel)));
    border(&mut e, x, yy, w, rh * n as f32, s.r, p.line);
    for (i, l) in rest.iter().take(n).enumerate() {
        let rc = yy + rh * (i as f32 + 0.5);
        if i > 0 {
            s.hair(&mut e, x + 56.0 * u, yy + rh * i as f32, w - 56.0 * u);
        }
        e.push(rect(x + 14.0 * u, rc - 15.0 * u, 30.0 * u, 30.0 * u, 8.0 * u, solid(alpha(ui::hue(i + 1), 0.15))));
        s.ic(&mut e, ["zap", "sparkles", "layers", "shield", "globe", "users"][i % 6], x + 21.0 * u, rc - 8.0 * u, 16.0, ui::hue(i + 1));
        s.fit(&mut e, l, 15.0, 500, p.text, x + 56.0 * u, rc - 2.0 * u, w - 90.0 * u);
        let meta = if s.items.is_empty() { ["About 2 min", "About 5 min", "About 1 min", "Browse 40+", "About 1 min"][i % 5] } else { ["New", "Improved", "New", "Improved", "New"][i % 5] };
        s.txt(&mut e, meta, 12.5, 400, p.muted, x + 56.0 * u, rc + 15.0 * u, 0.0);
        s.ic(&mut e, "chevron-right", x + w - 26.0 * u, rc - 7.0 * u, 14.0, p.faint);
    }
    s.item(o, 4, e);
}
