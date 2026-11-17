//! Device mockups: phone, laptop and browser window, drawn like product
//! renders (machined frames with specular edges, glass, contact shadows),
//! with an image or video screen (cover-fit, optional scroll through a tall
//! screenshot) or a generated app screen ([`super::screens`]). Each keeps its
//! aspect ratio, centred in the box. Finishes, chrome style and the screen's
//! look are all parameters, so mockups need not share one house style.

use super::screens::{self, Content, Host, Look};
use super::style::*;
use super::text::{draw, Face};
use super::uikit::{self as ui, ts};
use super::{transform_ops, with_opacity, Component, Ctx, Prepare};
use crate::dl::{Op, Paint, Path, Rgba, IDENTITY};
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
    /// Generated app screen when there is no `src`: app, dashboard, chat, feed, editor, settings.
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
    /// Generated screen appearance: `auto` (follow the theme), `light`, `dark`.
    pub ui: String,
    /// `compact`, `comfortable` (default) or `large`.
    pub density: String,
    /// Heading face: `sans` (default), `display`, `serif`, `mono`.
    pub font: String,
    /// Grey family: `neutral` (default), `warm`, `cool`, `accent`.
    pub tint: String,
    /// `still`, `subtle` (default) or `lively`.
    pub motion: String,
    /// Card corner radius of the generated UI, in design points.
    pub radius: Option<f32>,
    #[serde(skip)]
    look: Look,
}

impl Screen {
    fn prepare(&mut self) -> Result<(), super::Error> {
        if let Some(k) = self.screen.as_deref() {
            if !screens::KINDS.contains(&k) && k != "list" {
                return Err(super::Error(match closest(k, screens::KINDS) {
                    Some(c) => format!("unknown screen `{k}`; did you mean `{c}`?"),
                    None => format!("unknown screen `{k}`; use one of {}", screens::KINDS.join(", ")),
                }));
            }
        }
        self.look = Look::parse(&self.ui, &self.density, &self.font, &self.tint, &self.motion, self.radius).map_err(super::Error)?;
        Ok(())
    }
    /// Generated screen kind: explicit, or `app` when there is nothing else to show.
    fn kind(&self) -> Option<&str> {
        match self.screen.as_deref() {
            Some(k) => Some(k),
            None if self.colors.is_empty() => Some("app"),
            None => None,
        }
    }
    fn dark(&self, ctx: &Ctx) -> bool {
        screens::is_dark(ctx, &self.look)
    }
}

/// Draw the screen into the rounded rect `(x, y, w, h)`; `top` is the
/// status-bar inset for generated screens.
fn screen(o: &mut Vec<Op>, ctx: &Ctx, s: &Screen, host: Host, t: f32, x: f32, y: f32, w: f32, h: f32, r: f32, top: f32) {
    push_clip(o, x, y, w, h, r);
    if let Some((id, iw, ih)) = s.src.as_deref().and_then(|src| ctx.assets.media(src, t)) {
        // Fit width; tall images scroll, short ones cover.
        let k = (w / iw as f32).max(h / ih as f32);
        let (dw, dh) = (iw as f32 * k, ih as f32 * k);
        let over = (dh - h).max(0.0);
        let f = if s.scroll_duration > 0.0 { s.scroll_to * ease_in_out(seg(t, s.scroll_at, s.scroll_duration)) } else { 0.0 };
        o.push(rect(x, y, w, h, 0.0, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
        o.push(Op::Image { image: id, dst: [x + (w - dw) * 0.5, y - over * f, dw, dh], opacity: 1.0, transform: IDENTITY });
    } else if let Some(kind) = s.kind() {
        let c = Content { kind, title: &s.title, items: &s.items, kpis: &s.kpis, accent: s.accent.map(|c| c.0) };
        screens::draw_screen(o, ctx, &c, &s.look, host, t, x, y, w, h, top);
    } else {
        // Wallpaper from the given colours, with a soft light bloom.
        let cols: Vec<Rgba> = s.colors.iter().map(|c| c.0).collect();
        let n = (cols.len() - 1).max(1) as f32;
        o.push(rect(x, y, w, h, 0.0, Paint::Linear { p0: [x, y], p1: [x + w, y + h], stops: cols.iter().enumerate().map(|(i, c)| (i as f32 / n, *c)).collect() }));
        o.push(rect(x, y, w, h, 0.0, radial(x + w * 0.3, y + h * 0.2, w.max(h) * 0.7, vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.18])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))])));
    }
    o.push(Op::PopLayer);
}

/// Soft glass sheen: a broad falloff from the top-left corner, with no
/// hard edge (a polygon edge reads as a rendering artefact on video).
fn reflection(o: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, strength: f32) {
    if strength <= 0.0 {
        return;
    }
    let d = w.min(h) * 0.9;
    o.push(rect(x, y, w, h, r, Paint::Linear { p0: [x, y], p1: [x + d, y + d], stops: (0..=8).map(|i| { let f = i as f32 / 8.0; (f, Rgba([1.0, 1.0, 1.0, 0.09 * strength * (1.0 - f).powi(3)])) }).collect() }));
}

/// Shared entrance + float, applied about the device centre.
fn present(o: Vec<Op>, t: f32, delay: f32, float: f32, cx: f32, cy: f32) -> Vec<Op> {
    let p = spring(t - delay, 0.8);
    if p <= 0.0 {
        return Vec::new();
    }
    let mut o = o;
    let bob = float * ((t - delay) * 1.1).sin();
    let bob = if draft() { bob.round() } else { bob };
    if p < 1.0 || bob != 0.0 {
        let m = crate::dl::mul(&translate(0.0, (1.0 - p) * 60.0 + bob), &scale_about(lerp(0.94, 1.0, p), cx, cy));
        transform_ops(&mut o, &m);
    }
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

/// Metal finish: (base, highlight, shadow).
fn finish(name: &str, color: Option<Color>, laptop: bool) -> (Rgba, Rgba, Rgba) {
    if let Some(c) = color {
        let c = c.0;
        return (c, shade(c, 0.55), shade(c, -0.55));
    }
    let h = |s: &str| parse_color(s).unwrap();
    let (b, hi, sh) = match name {
        "black" | "space-black" | "graphite" => ("#3a3b3f", "#8d8f95", "#121315"),
        "white" | "silver" if !laptop => ("#d6d6d3", "#f8f8f6", "#8e8e8b"),
        "blue" | "midnight" if !laptop => ("#3e4859", "#94a2b8", "#1a2029"),
        "midnight" | "blue" => ("#2d3441", "#66738a", "#151a22"),
        "desert" | "gold" | "starlight" => ("#c2b29e", "#efe5d6", "#76695a"),
        "natural" | "titanium" if !laptop => ("#8f8b85", "#dedad3", "#46433f"),
        _ if laptop => ("#c3c6cb", "#eef0f2", "#7a7d83"),
        _ => ("#8f8b85", "#dedad3", "#46433f"),
    };
    (h(b), h(hi), h(sh))
}

fn check_finish(f: &str, laptop: bool) -> Result<(), super::Error> {
    let opts: &[&str] = if laptop { &["silver", "space-black", "black", "midnight", "starlight"] } else { &["titanium", "natural", "black", "white", "silver", "blue", "desert", "gold"] };
    if f.is_empty() || opts.contains(&f) {
        return Ok(());
    }
    Err(super::Error(match closest(f, opts) {
        Some(c) => format!("unknown finish `{f}`; did you mean `{c}`?"),
        None => format!("unknown finish `{f}`; use one of {}", opts.join(", ")),
    }))
}

// ------------------------------------------------------------------ phone

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Phone {
    #[serde(flatten)]
    pub screen: Screen,
    /// Frame colour (overrides `finish`).
    pub color: Option<Color>,
    /// Frame finish: titanium (default), black, white, blue, desert.
    pub finish: String,
    pub reflection: f32,
    pub shadow: bool,
    pub delay: f32,
    /// Gentle floating amplitude in pixels.
    pub float: f32,
    /// Status bar time text ("" hides the status bar).
    pub time: String,
}
impl Default for Phone {
    fn default() -> Self {
        Phone { screen: Screen::default(), color: None, finish: String::new(), reflection: 1.0, shadow: true, delay: 0.0, float: 0.0, time: "9:41".into() }
    }
}
impl Prepare for Phone {
    fn prepare(&mut self) -> Result<(), super::Error> {
        check_finish(&self.finish, false)?;
        self.screen.prepare()
    }
}

/// iOS status bar: time, signal, wi-fi and battery, in `ink`.
fn status_bar(o: &mut Vec<Op>, ctx: &Ctx, time: &str, sx: f32, sy: f32, sw: f32, ink: Rgba) {
    let pt = sw / 393.0;
    let cy = sy + 30.0 * pt;
    let st = ts(Face::Sans, 17.0 * pt, 600, ink).tabular();
    draw(o, ctx.assets, time, &st, sx + 64.0 * pt, ui::mid(cy, 17.0 * pt) - 0.5 * pt, 0.5);
    // Signal: four rising bars.
    let mut bx = sx + 293.0 * pt;
    for i in 0..4 {
        let bh = (4.0 + 2.6 * i as f32) * pt;
        o.push(rect(bx, cy + 5.5 * pt - bh, 3.0 * pt, bh, 0.9 * pt, solid(ink)));
        bx += 4.6 * pt;
    }
    // Wi-fi: three nested wedges.
    let (wx, wy) = (sx + 323.0 * pt, cy + 5.0 * pt);
    for (i, r) in [11.0f32, 7.4, 3.6].iter().enumerate() {
        let mut p = Path::new();
        let a0 = -std::f64::consts::FRAC_PI_2 - 0.78;
        let a1 = -std::f64::consts::FRAC_PI_2 + 0.78;
        let r0 = (r * pt) as f64;
        let ri = ((r - 2.6).max(0.0) * pt) as f64;
        let (cx, cy0) = (wx as f64, wy as f64);
        p.move_to((cx + ri * a0.cos(), cy0 + ri * a0.sin()));
        p.line_to((cx + r0 * a0.cos(), cy0 + r0 * a0.sin()));
        let n = 10;
        for k in 1..=n {
            let a = a0 + (a1 - a0) * k as f64 / n as f64;
            p.line_to((cx + r0 * a.cos(), cy0 + r0 * a.sin()));
        }
        for k in (0..=n).rev() {
            let a = a0 + (a1 - a0) * k as f64 / n as f64;
            p.line_to((cx + ri * a.cos(), cy0 + ri * a.sin()));
        }
        p.close_path();
        if i == 2 {
            let mut d = Path::new();
            d.move_to((cx, cy0));
            for k in 0..=n {
                let a = a0 + (a1 - a0) * k as f64 / n as f64;
                d.line_to((cx + r0 * a.cos(), cy0 + r0 * a.sin()));
            }
            d.close_path();
            o.push(fill(d, solid(ink)));
        } else {
            o.push(fill(p, solid(ink)));
        }
    }
    // Battery.
    let (bx, by, bw, bh) = (sx + 340.0 * pt, cy - 6.0 * pt, 25.0 * pt, 12.0 * pt);
    o.push(Op::Stroke { path: rrect_path(bx + 0.5 * pt, by + 0.5 * pt, bw - pt, bh - pt, 3.8 * pt), paint: solid(alpha(ink, 0.4)), stroke: line(1.0 * pt, [0.0, 1.0]), transform: IDENTITY });
    o.push(rect(bx + 2.0 * pt, by + 2.0 * pt, (bw - 4.0 * pt) * 0.8, bh - 4.0 * pt, 2.2 * pt, solid(ink)));
    o.push(rect(bx + bw + 1.0 * pt, by + 4.0 * pt, 1.6 * pt, 4.0 * pt, 0.8 * pt, solid(alpha(ink, 0.45))));
}

impl Component for Phone {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        // iPhone 15 Pro proportions: 70.6 × 146.6 mm.
        let (x, y, w, h) = fit(ctx, 70.6 / 146.6, 0.06);
        let (base, hi, sh) = finish(&self.finish, self.color, false);
        let r = w * 0.172;
        let band = w * 0.019;
        let bezel = w * 0.024;
        let inset = band + bezel;
        let mut o = Vec::new();
        if self.shadow {
            ground_shadow(&mut o, x + w * 0.5, y + h - w * 0.12, w * 0.74, w * 0.1, w * 0.07, w * 0.07, 0.42);
            shadow(&mut o, x, y, w, h, r, w * 0.1, w * 0.2, 0.32);
            shadow(&mut o, x, y, w, h, r, w * 0.012, w * 0.03, 0.35);
        }
        // Buttons (behind the frame, peeking out).
        let btn = |o: &mut Vec<Op>, left: bool, y0: f32, len: f32| {
            let bw = w * 0.012;
            let bx = if left { x - bw * 0.62 } else { x + w - bw * 0.38 };
            o.push(rect(bx, y + h * y0, bw, h * len, bw * 0.45, Paint::Linear { p0: [bx, 0.0], p1: [bx + bw, 0.0], stops: if left { vec![(0.0, sh), (0.35, hi), (1.0, base)] } else { vec![(0.0, base), (0.65, hi), (1.0, sh)] } }));
        };
        btn(&mut o, true, 0.165, 0.034);
        btn(&mut o, true, 0.235, 0.062);
        btn(&mut o, true, 0.315, 0.062);
        btn(&mut o, false, 0.262, 0.098);
        // Frame: brushed band, lit at the edges, with a chamfer highlight.
        o.push(rect(x, y, w, h, r, Paint::Linear { p0: [x, 0.0], p1: [x + w, 0.0], stops: vec![(0.0, sh), (0.006, hi), (0.03, base), (0.5, shade(base, -0.06)), (0.97, base), (0.994, hi), (1.0, sh)] }));
        o.push(rect(x, y, w, h, r, Paint::Linear { p0: [0.0, y], p1: [0.0, y + h], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.28])), (0.012, Rgba([1.0, 1.0, 1.0, 0.0])), (0.985, Rgba([0.0, 0.0, 0.0, 0.0])), (1.0, Rgba([0.0, 0.0, 0.0, 0.3]))] }));
        let ci = w * 0.0045;
        o.push(Op::Stroke {
            path: rrect_path(x + ci, y + ci, w - 2.0 * ci, h - 2.0 * ci, r - ci),
            paint: Paint::Linear { p0: [x, y], p1: [x + w, y + h], stops: vec![(0.0, alpha(Rgba([1.0, 1.0, 1.0, 1.0]), 0.75)), (0.3, alpha(hi, 0.25)), (0.7, alpha(hi, 0.15)), (1.0, alpha(Rgba([1.0, 1.0, 1.0, 1.0]), 0.5))] },
            stroke: line((w * 0.0035).max(0.8), [0.0, 1.0]),
            transform: IDENTITY,
        });
        // Black glass front.
        o.push(rect(x + band, y + band, w - 2.0 * band, h - 2.0 * band, r - band, solid(parse_color("#030304").unwrap())));
        o.push(Op::Stroke { path: rrect_path(x + band + 0.5, y + band + 0.5, w - 2.0 * band - 1.0, h - 2.0 * band - 1.0, r - band), paint: Paint::Linear { p0: [x, y], p1: [x, y + h], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.16])), (0.5, Rgba([1.0, 1.0, 1.0, 0.05])), (1.0, Rgba([1.0, 1.0, 1.0, 0.1]))] }, stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        let (sx, sy, sw, sh2) = (x + inset, y + inset, w - 2.0 * inset, h - 2.0 * inset);
        let sr = r - inset;
        let pt = sw / 393.0;
        let top = if self.time.is_empty() { 0.0 } else { 54.0 * pt };
        screen(&mut o, ctx, &self.screen, Host::Phone, (t - self.delay).max(0.0), sx, sy, sw, sh2, sr, top);
        let dark_ui = self.screen.src.is_some() || self.screen.kind().is_none() || self.screen.dark(ctx);
        let ink = if dark_ui { Rgba([1.0, 1.0, 1.0, 0.96]) } else { Rgba([0.0, 0.0, 0.0, 0.92]) };
        // Dynamic Island.
        let (iw, ih) = (125.0 * pt, 36.5 * pt);
        let (ix, iy) = (sx + (sw - iw) * 0.5, sy + 11.0 * pt);
        o.push(rect(ix, iy, iw, ih, ih * 0.5, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
        let (lx, ly, lr) = (ix + iw - ih * 0.5, iy + ih * 0.5, ih * 0.23);
        o.push(circle(lx, ly, lr, radial(lx - lr * 0.3, ly - lr * 0.3, lr * 1.3, vec![(0.0, parse_color("#2a3550").unwrap()), (0.6, parse_color("#0c0f18").unwrap()), (1.0, parse_color("#050608").unwrap())])));
        o.push(circle(lx - lr * 0.35, ly - lr * 0.35, lr * 0.22, solid(Rgba([0.6, 0.7, 1.0, 0.35]))));
        if !self.time.is_empty() {
            status_bar(&mut o, ctx, &self.time, sx, sy, sw, ink);
        }
        // Home indicator.
        o.push(rect(sx + (sw - 134.0 * pt) * 0.5, sy + sh2 - 13.0 * pt, 134.0 * pt, 5.0 * pt, 2.5 * pt, solid(alpha(ink, 0.85))));
        reflection(&mut o, sx, sy, sw, sh2, sr, self.reflection);
        present(o, t, self.delay, self.float, x + w * 0.5, y + h * 0.5)
    }
}

// ------------------------------------------------------------------ laptop

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Laptop {
    #[serde(flatten)]
    pub screen: Screen,
    /// Body colour (overrides `finish`).
    pub color: Option<Color>,
    /// Aluminium finish: silver (default), space-black, midnight, starlight.
    pub finish: String,
    pub reflection: f32,
    pub shadow: bool,
    pub delay: f32,
    pub float: f32,
    /// Lid opens from closed over this many seconds (0 = already open).
    pub open: f32,
    /// macOS menu bar across the top of generated screens.
    pub menu_bar: bool,
}
impl Default for Laptop {
    fn default() -> Self {
        Laptop { screen: Screen::default(), color: None, finish: String::new(), reflection: 0.6, shadow: true, delay: 0.0, float: 0.0, open: 0.0, menu_bar: true }
    }
}
impl Prepare for Laptop {
    fn prepare(&mut self) -> Result<(), super::Error> {
        check_finish(&self.finish, true)?;
        self.screen.prepare()
    }
}

/// macOS menu bar (the notch sits in its middle).
fn menu_bar(o: &mut Vec<Op>, ctx: &Ctx, s: &Screen, x: f32, y: f32, w: f32, h: f32) {
    let dark = s.dark(ctx);
    let bg = screens::background(ctx, &s.look, s.accent.map(|c| c.0));
    o.push(rect(x, y, w, h, 0.0, solid(shade(bg, if dark { -0.25 } else { -0.05 }))));
    let ink = if dark { Rgba([1.0, 1.0, 1.0, 0.92]) } else { Rgba([0.0, 0.0, 0.0, 0.86]) };
    let size = h * 0.56;
    let cy = y + h * 0.5;
    // Apple-free "system" glyph: a small filled circle.
    o.push(circle(x + h * 0.85, cy, size * 0.36, solid(ink)));
    let mut mx = x + h * 1.6;
    let title = if s.title.is_empty() { "Finder" } else { s.title.as_str() };
    mx += draw(o, ctx.assets, title, &ts(Face::Sans, size, 700, ink), mx, ui::mid(cy, size), 0.0) + size * 1.2;
    for item in ["File", "Edit", "View", "Window", "Help"] {
        mx += draw(o, ctx.assets, item, &ts(Face::Sans, size, 500, ink), mx, ui::mid(cy, size), 0.0) + size * 1.2;
    }
    let r = draw(o, ctx.assets, "Tue 17 Nov  9:41", &ts(Face::Sans, size, 500, ink).tabular(), x + w - h * 0.6, ui::mid(cy, size), 1.0);
    let mut ix = x + w - h * 0.6 - r - size * 1.3;
    for icn in ["search", "sliders"] {
        ix -= size * 1.15;
        ui::icon(o, icn, ix, cy - size * 0.55, size * 1.1, ink, 2.0);
        ix -= size * 0.9;
    }
    // Battery.
    let (bw, bh) = (size * 1.9, size * 0.9);
    let bx = ix - bw - size * 0.3;
    o.push(Op::Stroke { path: rrect_path(bx, cy - bh * 0.5, bw, bh, bh * 0.3), paint: solid(alpha(ink, 0.5)), stroke: line(size * 0.08, [0.0, 1.0]), transform: IDENTITY });
    o.push(rect(bx + size * 0.15, cy - bh * 0.5 + size * 0.15, (bw - size * 0.3) * 0.75, bh - size * 0.3, bh * 0.15, solid(ink)));
}

impl Component for Laptop {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        // MacBook-Pro-like front view: 16:10.4 display, thin even bezels, a
        // notch, an aluminium base slightly wider than the lid.
        let (x, y, w, h) = fit(ctx, 1.618, 0.05);
        let (base, hi, sh) = finish(&self.finish, self.color, true);
        let lid_w = w * 0.86;
        let side = lid_w * 0.018;
        let top_b = lid_w * 0.024;
        let bot_b = lid_w * 0.036;
        let scr_w = lid_w - 2.0 * side;
        let scr_h = scr_w / 1.54;
        let lid_h = scr_h + top_b + bot_b;
        let base_h = w * 0.026;
        let lx = x + (w - lid_w) * 0.5;
        let ly = y + (h - lid_h - base_h) * 0.5;
        let base_y = ly + lid_h;
        let open = if self.open > 0.0 { ease_in_out(seg(t, self.delay, self.open)) } else { 1.0 };
        let mut o = Vec::new();
        if self.shadow {
            ground_shadow(&mut o, x + w * 0.5, base_y + base_h * 0.4, w * 0.94, base_h * 0.5, base_h * 0.9, w * 0.02, 0.55);
            ground_shadow(&mut o, x + w * 0.5, base_y - base_h, w * 0.86, base_h * 1.6, base_h * 1.8, w * 0.06, 0.4);
        }
        // Lid.
        let mut lid = Vec::new();
        let r = lid_w * 0.026;
        lid.push(rect(lx, ly, lid_w, lid_h, r, Paint::Linear { p0: [lx, ly], p1: [lx, ly + lid_h], stops: vec![(0.0, mix(hi, base, 0.5)), (0.08, shade(base, -0.15)), (1.0, shade(sh, -0.2))] }));
        let rim = (lid_w * 0.0017).max(1.0);
        lid.push(rect(lx + rim, ly + rim, lid_w - 2.0 * rim, lid_h - 2.0 * rim, r - rim, solid(parse_color("#0a0a0b").unwrap())));
        lid.push(Op::Stroke { path: rrect_path(lx + rim + 0.5, ly + rim + 0.5, lid_w - 2.0 * rim - 1.0, lid_h - 2.0 * rim - 1.0, r - rim), paint: Paint::Linear { p0: [lx, ly], p1: [lx, ly + lid_h], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.12])), (1.0, Rgba([1.0, 1.0, 1.0, 0.03]))] }, stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        let (sx, sy) = (lx + side, ly + top_b);
        let sr = lid_w * 0.011;
        let notch_h = scr_h * 0.024;
        let mb = if self.menu_bar && self.screen.src.is_none() && self.screen.kind().is_some() { notch_h } else { 0.0 };
        let st = (t - self.delay - self.open).max(0.0);
        push_clip(&mut lid, sx, sy, scr_w, scr_h, sr);
        if mb > 0.0 {
            menu_bar(&mut lid, ctx, &self.screen, sx, sy, scr_w, mb);
        }
        screen(&mut lid, ctx, &self.screen, Host::Laptop, st, sx, sy + mb, scr_w, scr_h - mb, 0.0, 0.0);
        lid.push(Op::PopLayer);
        // Notch with camera.
        let nw = scr_w * 0.1;
        let mut np = Path::new();
        let nr = (notch_h * 0.45) as f64;
        let (n0, n1, ny0, ny1) = ((sx + (scr_w - nw) * 0.5) as f64, (sx + (scr_w + nw) * 0.5) as f64, (sy - 1.0) as f64, (sy + notch_h) as f64);
        np.move_to((n0, ny0));
        np.line_to((n1, ny0));
        np.line_to((n1, ny1 - nr));
        np.quad_to((n1, ny1), (n1 - nr, ny1));
        np.line_to((n0 + nr, ny1));
        np.quad_to((n0, ny1), (n0, ny1 - nr));
        np.close_path();
        lid.push(fill(np, solid(parse_color("#0a0a0b").unwrap())));
        let (ccx, ccy) = (sx + scr_w * 0.5, sy + notch_h * 0.45);
        lid.push(circle(ccx, ccy, notch_h * 0.2, solid(parse_color("#16181d").unwrap())));
        lid.push(circle(ccx, ccy, notch_h * 0.1, solid(parse_color("#1f2a40").unwrap())));
        reflection(&mut lid, sx, sy, scr_w, scr_h, sr, self.reflection);
        let k = open.max(0.02);
        transform_ops(&mut lid, &[1.0, 0.0, 0.0, k, 0.0, base_y * (1.0 - k)]);
        o.extend(lid);
        // Hinge shadow line.
        o.push(rect(lx + lid_w * 0.03, base_y - base_h * 0.12, lid_w * 0.94, base_h * 0.18, base_h * 0.09, solid(parse_color("#1a1b1e").unwrap())));
        // Base: lit top edge, brushed front face, rounded bottom corners.
        let (bx0, bx1, by0, by1) = (x as f64, (x + w) as f64, base_y as f64, (base_y + base_h) as f64);
        let rt = (base_h * 0.18) as f64;
        let rb = (base_h * 0.9) as f64;
        let mut bp = Path::new();
        bp.move_to((bx0 + rt, by0));
        bp.line_to((bx1 - rt, by0));
        bp.quad_to((bx1, by0), (bx1, by0 + rt));
        bp.line_to((bx1, by1 - rb));
        bp.curve_to((bx1, by1 - rb * 0.3), (bx1 - rb * 0.6, by1), (bx1 - rb * 1.6, by1));
        bp.line_to((bx0 + rb * 1.6, by1));
        bp.curve_to((bx0 + rb * 0.6, by1), (bx0, by1 - rb * 0.3), (bx0, by1 - rb));
        bp.line_to((bx0, by0 + rt));
        bp.quad_to((bx0, by0), (bx0 + rt, by0));
        bp.close_path();
        o.push(fill(bp.clone(), vgrad(base_y, base_y + base_h, shade(base, 0.2), shade(sh, -0.1))));
        o.push(fill(bp, Paint::Linear { p0: [x, 0.0], p1: [x + w, 0.0], stops: vec![(0.0, Rgba([0.0, 0.0, 0.0, 0.25])), (0.06, Rgba([0.0, 0.0, 0.0, 0.0])), (0.94, Rgba([0.0, 0.0, 0.0, 0.0])), (1.0, Rgba([0.0, 0.0, 0.0, 0.25]))] }));
        // Deck sliver and lit edge.
        o.push(rect(x + base_h * 0.2, base_y, w - base_h * 0.4, base_h * 0.22, base_h * 0.1, vgrad(base_y, base_y + base_h * 0.22, hi, base)));
        o.push(rect(x + base_h * 0.3, base_y + base_h * 0.22, w - base_h * 0.6, (base_h * 0.05).max(1.0), 0.0, Paint::Linear { p0: [x, 0.0], p1: [x + w, 0.0], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.0])), (0.2, Rgba([1.0, 1.0, 1.0, 0.75])), (0.8, Rgba([1.0, 1.0, 1.0, 0.75])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] }));
        // Thumb scoop.
        let (sw2, sh2) = (w * 0.13, base_h * 0.4);
        let scx = x + (w - sw2) * 0.5;
        o.push(rect(scx, base_y + base_h * 0.22, sw2, sh2, sh2 * 0.5, vgrad(base_y, base_y + sh2 * 1.6, shade(sh, -0.05), shade(base, -0.12))));
        o.push(rect(scx + sh2 * 0.4, base_y + base_h * 0.22 + sh2 - 1.0, sw2 - sh2 * 0.8, 1.0, 0.0, solid(alpha(hi, 0.6))));
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
    /// Window chrome: `safari` (default; unified toolbar), `chrome` (tab
    /// strip above an omnibox) or `minimal` (traffic lights and the URL).
    pub style: String,
    /// Chrome appearance: `auto` (follows the screen), `light` or `dark`.
    pub chrome: String,
}
impl Default for Browser {
    fn default() -> Self {
        Browser { screen: Screen::default(), url: "anikit.dev".into(), tabs: Vec::new(), type_url: 0.0, delay: 0.0, float: 0.0, shadow: true, aspect: 1.6, style: String::new(), chrome: String::new() }
    }
}
impl Prepare for Browser {
    fn prepare(&mut self) -> Result<(), super::Error> {
        let bad = |k: &str, v: &str, opts: &[&str]| {
            super::Error(match closest(v, opts) {
                Some(c) => format!("unknown {k} `{v}`; did you mean `{c}`?"),
                None => format!("unknown {k} `{v}`; use one of {}", opts.join(", ")),
            })
        };
        if !["", "safari", "chrome", "minimal"].contains(&self.style.as_str()) {
            return Err(bad("style", &self.style, &["safari", "chrome", "minimal"]));
        }
        if !["", "auto", "light", "dark"].contains(&self.chrome.as_str()) {
            return Err(bad("chrome", &self.chrome, &["auto", "light", "dark"]));
        }
        self.screen.prepare()
    }
}

struct Chrome {
    bar: Rgba,
    bar2: Rgba,
    field: Rgba,
    line: Rgba,
    ink: Rgba,
    muted: Rgba,
    faint: Rgba,
    active_tab: Rgba,
}

fn traffic(o: &mut Vec<Op>, x: f32, cy: f32, u: f32) {
    for (i, c) in ["#ff5f57", "#febc2e", "#28c840"].iter().enumerate() {
        let cx = x + i as f32 * 20.0 * u;
        let cc = parse_color(c).unwrap();
        o.push(circle(cx, cy, 6.0 * u, vgrad(cy - 6.0 * u, cy + 6.0 * u, shade(cc, 0.08), shade(cc, -0.04))));
        o.push(stroke_op(circle_path(cx, cy, 5.7 * u), solid(Rgba([0.0, 0.0, 0.0, 0.16])), 0.7 * u, [0.0, 1.0]));
    }
}

impl Component for Browser {
    fn eval(&self, t: f32, ctx: &Ctx) -> Vec<Op> {
        let (x, y, w, h) = fit(ctx, self.aspect.max(0.3), 0.04);
        let u = (w / 1280.0).clamp(0.3, 3.0);
        let dark = match self.chrome.as_str() {
            "light" => false,
            "dark" => true,
            _ if self.screen.src.is_some() => ctx.theme.dark,
            _ => self.screen.dark(ctx),
        };
        let h_ = |s: &str| parse_color(s).unwrap();
        let c = if dark {
            Chrome { bar: h_("#2c2c30"), bar2: h_("#252528"), field: Rgba([1.0, 1.0, 1.0, 0.08]), line: Rgba([0.0, 0.0, 0.0, 0.55]), ink: h_("#ececee"), muted: h_("#a1a1a8"), faint: h_("#6c6c73"), active_tab: h_("#38383d") }
        } else {
            Chrome { bar: h_("#f6f6f7"), bar2: h_("#ececee"), field: Rgba([0.0, 0.0, 0.05, 0.055]), line: Rgba([0.0, 0.0, 0.0, 0.11]), ink: h_("#1d1d20"), muted: h_("#6e6e75"), faint: h_("#a8a8ae"), active_tab: h_("#ffffff") }
        };
        let style = if self.style.is_empty() { "safari" } else { self.style.as_str() };
        let r = 12.0 * u;
        let mut o = Vec::new();
        if self.shadow {
            shadow(&mut o, x, y, w, h, r, 30.0 * u, 70.0 * u, if ctx.theme.dark { 0.55 } else { 0.22 });
            shadow(&mut o, x, y, w, h, r, 2.0 * u, 6.0 * u, if ctx.theme.dark { 0.4 } else { 0.12 });
        }
        o.push(rect(x, y, w, h, r, solid(c.bar)));
        // URL (typed in or shown).
        let n = self.url.chars().count();
        let k = if self.type_url > 0.0 { ((seg(t, self.delay + 0.3, self.type_url)) * n as f32).round() as usize } else { n };
        let url: String = self.url.chars().take(k).collect();
        let typing = self.type_url > 0.0 && k < n && t > self.delay + 0.3;
        let fav_name = if self.screen.title.is_empty() { self.url.as_str() } else { self.screen.title.as_str() };
        let acc = self.screen.accent.map(|c| c.0).unwrap_or(ctx.theme.accent);
        let a = ctx.assets;
        let content_y = match style {
            "chrome" => {
                let strip = 42.0 * u;
                let tool = 46.0 * u;
                o.push(rect(x, y, w, strip + r, r, solid(c.bar2)));
                traffic(&mut o, x + 22.0 * u, y + strip * 0.55, u);
                let tabs: Vec<String> = if self.tabs.is_empty() { vec![if self.screen.title.is_empty() { self.url.clone() } else { self.screen.title.clone() }] } else { self.tabs.clone() };
                let mut tx = x + 88.0 * u;
                let tw = ((w - 150.0 * u) / tabs.len() as f32).min(240.0 * u);
                for (i, tab) in tabs.iter().enumerate() {
                    let (ty, th) = (y + 8.0 * u, strip - 8.0 * u);
                    if i == 0 {
                        // Active tab with flared feet.
                        let fr = 8.0 * u;
                        let mut p = Path::new();
                        let (x0, x1, y0, y1) = (tx as f64, (tx + tw) as f64, ty as f64, (ty + th) as f64);
                        let (f, rr) = (fr as f64, 10.0 * u as f64);
                        p.move_to((x0 - f, y1));
                        p.quad_to((x0, y1), (x0, y1 - f));
                        p.line_to((x0, y0 + rr));
                        p.quad_to((x0, y0), (x0 + rr, y0));
                        p.line_to((x1 - rr, y0));
                        p.quad_to((x1, y0), (x1, y0 + rr));
                        p.line_to((x1, y1 - f));
                        p.quad_to((x1, y1), (x1 + f, y1));
                        p.close_path();
                        o.push(fill(p, solid(c.bar)));
                    } else {
                        o.push(rect(tx, ty + th * 0.28, 1.0, th * 0.44, 0.0, solid(c.line)));
                    }
                    let cy = ty + th * 0.5;
                    ui::logo(&mut o, a, if i == 0 { fav_name } else { tab }, if i == 0 { acc } else { ui::hue(i + 2) }, tx + 14.0 * u, cy - 8.0 * u, 16.0 * u);
                    ui::fit(&mut o, a, tab, &ts(Face::Sans, 12.5 * u, 500, if i == 0 { c.ink } else { c.muted }), tx + 38.0 * u, ui::mid(cy, 12.5 * u), tw - 70.0 * u, 0.0);
                    ui::icon(&mut o, "x", tx + tw - 26.0 * u, cy - 6.0 * u, 12.0 * u, c.muted, 2.0);
                    tx += tw;
                }
                ui::icon(&mut o, "plus", tx + 12.0 * u, y + 8.0 * u + (strip - 8.0 * u) * 0.5 - 8.0 * u, 16.0 * u, c.muted, 2.0);
                // Toolbar.
                let ty = y + strip;
                let cy = ty + tool * 0.5;
                for (i, icn) in ["chevron-left", "chevron-right", "reload"].iter().enumerate() {
                    ui::icon(&mut o, icn, x + (16.0 + i as f32 * 32.0) * u, cy - 9.0 * u, 18.0 * u, if i == 1 { c.faint } else { c.muted }, 2.0);
                }
                let (fx, fw, fh) = (x + 116.0 * u, w - 116.0 * u - 90.0 * u, 32.0 * u);
                o.push(rect(fx, cy - fh * 0.5, fw, fh, fh * 0.5, solid(if dark { h_("#1f1f22") } else { h_("#eceef0") })));
                ui::icon(&mut o, "sliders", fx + 14.0 * u, cy - 7.0 * u, 14.0 * u, c.muted, 2.0);
                let (host, path) = match url.find('/') {
                    Some(i) => (&url[..i], &url[i..]),
                    None => (url.as_str(), ""),
                };
                let hw = draw(&mut o, a, host, &ts(Face::Sans, 14.0 * u, 400, c.ink), fx + 40.0 * u, ui::mid(cy, 14.0 * u), 0.0);
                let pw = draw(&mut o, a, path, &ts(Face::Sans, 14.0 * u, 400, c.muted), fx + 40.0 * u + hw, ui::mid(cy, 14.0 * u), 0.0);
                if typing {
                    o.push(rect(fx + 41.0 * u + hw + pw, cy - 8.0 * u, 1.5 * u, 16.0 * u, 0.0, solid(acc)));
                }
                ui::icon(&mut o, "star", fx + fw - 30.0 * u, cy - 8.0 * u, 16.0 * u, c.muted, 2.0);
                ui::avatar(&mut o, a, "You", x + w - 58.0 * u, cy, 12.0 * u, None);
                ui::icon(&mut o, "more", x + w - 34.0 * u, cy - 9.0 * u, 18.0 * u, c.muted, 2.0);
                ty + tool
            }
            "minimal" => {
                let bar = 38.0 * u;
                traffic(&mut o, x + 20.0 * u, y + bar * 0.5, u);
                let st = ts(Face::Sans, 12.5 * u, 500, c.muted);
                let tw = a.measure(&url, &st);
                ui::icon(&mut o, "lock", x + w * 0.5 - tw * 0.5 - 16.0 * u, y + bar * 0.5 - 6.0 * u, 11.0 * u, c.faint, 2.2);
                draw(&mut o, a, &url, &st, x + w * 0.5 - tw * 0.5, ui::mid(y + bar * 0.5, 12.5 * u), 0.0);
                y + bar
            }
            _ => {
                let bar = 52.0 * u;
                o.push(rect(x, y, w, bar + r, r, vgrad(y, y + bar, c.bar, shade(c.bar, if dark { -0.03 } else { -0.015 }))));
                let cy = y + bar * 0.5;
                traffic(&mut o, x + 22.0 * u, cy, u);
                ui::icon(&mut o, "sidebar", x + 92.0 * u, cy - 9.0 * u, 18.0 * u, c.muted, 1.8);
                ui::icon(&mut o, "chevron-left", x + 132.0 * u, cy - 10.0 * u, 20.0 * u, c.muted, 2.0);
                ui::icon(&mut o, "chevron-right", x + 162.0 * u, cy - 10.0 * u, 20.0 * u, c.faint, 2.0);
                // Address field.
                let fw = (w * 0.44).min(620.0 * u).max(200.0 * u);
                let fh = 32.0 * u;
                let fx = x + (w - fw) * 0.5;
                o.push(rect(fx, cy - fh * 0.5, fw, fh, 9.0 * u, solid(c.field)));
                let st = ts(Face::Sans, 13.5 * u, 500, c.ink);
                let tw = a.measure(&url, &st).min(fw - 90.0 * u);
                let gx = fx + fw * 0.5 - (tw + 24.0 * u) * 0.5;
                ui::logo(&mut o, a, fav_name, acc, gx, cy - 8.0 * u, 16.0 * u);
                let dw = ui::fit(&mut o, a, &url, &st, gx + 24.0 * u, ui::mid(cy, 13.5 * u), fw - 90.0 * u, 0.0);
                if typing {
                    o.push(rect(gx + 25.0 * u + dw, cy - 8.0 * u, 1.5 * u, 16.0 * u, 0.0, solid(acc)));
                }
                ui::icon(&mut o, "reload", fx + fw - 26.0 * u, cy - 7.0 * u, 14.0 * u, c.muted, 2.2);
                for (i, icn) in ["copy", "plus", "share"].iter().enumerate() {
                    ui::icon(&mut o, icn, x + w - (34.0 + i as f32 * 36.0) * u, cy - 9.0 * u, 18.0 * u, c.muted, 1.8);
                }
                let mut cy2 = y + bar;
                if !self.tabs.is_empty() {
                    let th = 32.0 * u;
                    o.push(rect(x, cy2, w, th, 0.0, solid(c.bar2)));
                    o.push(rect(x, cy2, w, 1.0, 0.0, solid(c.line)));
                    let tw = w / self.tabs.len() as f32;
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let tx = x + tw * i as f32;
                        if i == 0 {
                            o.push(rect(tx + 4.0 * u, cy2 + 4.0 * u, tw - 8.0 * u, th - 8.0 * u, 7.0 * u, solid(c.active_tab)));
                            if !dark {
                                border(&mut o, tx + 4.0 * u, cy2 + 4.0 * u, tw - 8.0 * u, th - 8.0 * u, 7.0 * u, Rgba([0.0, 0.0, 0.0, 0.06]));
                            }
                        } else {
                            o.push(rect(tx, cy2 + 8.0 * u, 1.0, th - 16.0 * u, 0.0, solid(c.line)));
                        }
                        let st = ts(Face::Sans, 12.0 * u, 500, if i == 0 { c.ink } else { c.muted });
                        let lw = a.measure(tab, &st).min(tw - 60.0 * u);
                        let gx = tx + tw * 0.5 - (lw + 22.0 * u) * 0.5;
                        ui::logo(&mut o, a, if i == 0 { fav_name } else { tab }, if i == 0 { acc } else { ui::hue(i + 2) }, gx, cy2 + th * 0.5 - 7.0 * u, 14.0 * u);
                        ui::fit(&mut o, a, tab, &st, gx + 22.0 * u, ui::mid(cy2 + th * 0.5, 12.0 * u), tw - 60.0 * u, 0.0);
                    }
                    cy2 += th;
                }
                cy2
            }
        };
        o.push(rect(x, content_y - 1.0, w, 1.0, 0.0, solid(c.line)));
        // Content: rounded only at the bottom (clip with the window).
        push_clip(&mut o, x, y, w, h, r);
        screen(&mut o, ctx, &self.screen, Host::Browser, (t - self.delay).max(0.0), x, content_y, w, y + h - content_y, 0.0, 0.0);
        o.push(Op::PopLayer);
        // Window edge: hairline plus a top highlight in dark mode.
        border(&mut o, x, y, w, h, r, if dark { Rgba([0.0, 0.0, 0.0, 0.7]) } else { Rgba([0.0, 0.0, 0.0, 0.14]) });
        if dark {
            o.push(Op::Stroke { path: rrect_path(x + 1.5, y + 1.5, w - 3.0, h - 3.0, r - 1.5), paint: Paint::Linear { p0: [x, y], p1: [x, y + 60.0 * u], stops: vec![(0.0, Rgba([1.0, 1.0, 1.0, 0.16])), (1.0, Rgba([1.0, 1.0, 1.0, 0.0]))] }, stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
        }
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
        assert!(build("phone", &json!({"finish": "titanum"})).err().unwrap().0.contains("titanium"));
        assert!(build("laptop", &json!({"density": "compakt"})).err().unwrap().0.contains("compact"));
        assert!(build("browser", &json!({"style": "safary"})).err().unwrap().0.contains("safari"));
        // Looks: every option builds and renders.
        let mut ops = Vec::new();
        ops.extend(build("phone", &json!({"x": 0, "y": 0, "width": 300, "height": 540, "screen": "settings", "finish": "white", "ui": "light", "tint": "warm", "font": "serif", "motion": "still"})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        ops.extend(build("browser", &json!({"x": 300, "y": 20, "width": 660, "height": 420, "screen": "feed", "style": "chrome", "ui": "light", "density": "compact", "radius": 4, "motion": "lively", "tint": "cool"})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        preview::validate(&ops).unwrap();
        // Product copy instead of invented data.
        let items = json!(["Offline mode on every device you own", "Handwriting search in sketches", "Offline mode", "Shared notebooks with live cursors", "Templates", "Version history"]);
        let mut ops = Vec::new();
        ops.extend(build("phone", &json!({"x": 0, "y": 0, "width": 300, "height": 540, "screen": "app", "title": "Quill", "items": items})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        ops.extend(build("browser", &json!({"x": 300, "y": 20, "width": 660, "height": 420, "screen": "app", "title": "Quill", "items": items})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a));
        preview::validate(&ops).unwrap();
        preview::snapshot("screen-quill", 960, 540, th.background, ops, a);
        let ops = build("laptop", &json!({"screen": "dashboard", "title": "Quill", "items": items, "kpis": [{"label": "Notes captured this week", "value": "12,400"}]})).unwrap().eval(2.5, 960.0, 540.0, 30.0, &th, a);
        preview::validate(&ops).unwrap();
        preview::snapshot("screen-quill-kpi", 960, 540, th.background, ops, a);
        // Contrast: a light orange brand gets dark labels.
        let mut ops = Vec::new();
        for (i, v) in ["primary", "gradient"].iter().enumerate() {
            ops.extend(build("button", &json!({"x": 40 + 300 * i, "y": 40, "width": 280, "height": 80, "label": "Start free", "variant": v, "color": "#E8C23C"})).unwrap().eval(2.0, 960.0, 540.0, 30.0, &th, a));
        }
        ops.extend(build("badge", &json!({"x": 40, "y": 140, "width": 120, "height": 40, "label": "New", "solid": true, "color": "#7CF0A0"})).unwrap().eval(2.0, 960.0, 540.0, 30.0, &th, a));
        preview::snapshot("contrast", 960, 540, th.background, ops, a);
    }
}
