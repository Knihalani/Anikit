//! Shared look and motion: theme, colours, easing and shape helpers.

use crate::dl::{self, Affine, FillRule, Op, Paint, Path, Rgba, Stroke, IDENTITY};
use kurbo::Shape;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

// ---------------------------------------------------------------- colour

/// A colour parameter: any CSS colour string (`"#7c5cff"`, `"rgb(…)"`,
/// `"oklch(…)"`, named) or an `[r, g, b, a?]` array of 0..1 floats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub Rgba);

impl Color {
    pub fn hex(s: &str) -> Color {
        Color(parse_color(s).expect("valid colour literal"))
    }
    pub fn with_alpha(self, a: f32) -> Rgba {
        let c = self.0 .0;
        Rgba([c[0], c[1], c[2], c[3] * a])
    }
}

impl From<Color> for Rgba {
    fn from(c: Color) -> Rgba {
        c.0
    }
}

/// Parse a CSS colour.
pub fn parse_color(s: &str) -> Option<Rgba> {
    anikit_core::color::parse(s).map(|c| Rgba([(c.r / 255.0) as f32, (c.g / 255.0) as f32, (c.b / 255.0) as f32, c.a as f32]))
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        color_from_value(&v).map(Color).map_err(serde::de::Error::custom)
    }
}

fn color_from_value(v: &Value) -> Result<Rgba, String> {
    match v {
        Value::String(s) => parse_color(s).ok_or_else(|| format!("not a colour: {s:?} (use \"#rrggbb\", \"rgb(…)\", \"oklch(…)\" or a name)")),
        Value::Array(a) if (3..=4).contains(&a.len()) => {
            let mut c = [0.0, 0.0, 0.0, 1.0];
            for (i, x) in a.iter().enumerate() {
                c[i] = x.as_f64().ok_or("colour arrays hold numbers 0..1")? as f32;
            }
            Ok(Rgba(c))
        }
        _ => Err(format!("not a colour: {v}")),
    }
}

/// Mix two colours in (approximately linear) light.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let mut o = [0.0; 4];
    for i in 0..3 {
        let (x, y) = (a.0[i] * a.0[i], b.0[i] * b.0[i]);
        o[i] = (x + (y - x) * t).max(0.0).sqrt();
    }
    o[3] = a.0[3] + (b.0[3] - a.0[3]) * t;
    Rgba(o)
}

/// Same colour with alpha multiplied by `a`.
pub fn alpha(c: Rgba, a: f32) -> Rgba {
    Rgba([c.0[0], c.0[1], c.0[2], c.0[3] * a])
}

/// Lighten (`amt > 0`) or darken (`amt < 0`) towards white/black.
pub fn shade(c: Rgba, amt: f32) -> Rgba {
    if amt >= 0.0 {
        mix(c, Rgba([1.0, 1.0, 1.0, c.0[3]]), amt)
    } else {
        mix(c, Rgba([0.0, 0.0, 0.0, c.0[3]]), -amt)
    }
}

// ---------------------------------------------------------------- theme

/// Colours and type every component falls back to. The defaults are a dark,
/// launch-video look; `"light"` flips it. Brand kits override any field.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub background: Rgba,
    pub surface: Rgba,
    pub surface2: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub positive: Rgba,
    pub negative: Rgba,
    /// Categorical palette for charts and particles.
    pub palette: Vec<Rgba>,
    /// Corner radius of cards and windows.
    pub radius: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

fn hx(s: &str) -> Rgba {
    parse_color(s).unwrap()
}

impl Theme {
    pub fn dark() -> Theme {
        Theme {
            dark: true,
            background: hx("#08090c"),
            surface: hx("#14151b"),
            surface2: hx("#1c1d25"),
            border: Rgba([1.0, 1.0, 1.0, 0.09]),
            text: hx("#f2f2f5"),
            muted: hx("#8b8f9c"),
            accent: hx("#7c6cff"),
            positive: hx("#3ddc97"),
            negative: hx("#ff5d73"),
            palette: ["#7c6cff", "#2ad4e8", "#ff6fb1", "#3ddc97", "#ffb547", "#5a9bff", "#c08bff", "#ff8a5c"]
                .iter()
                .map(|s| hx(s))
                .collect(),
            radius: 18.0,
        }
    }

    pub fn light() -> Theme {
        Theme {
            dark: false,
            background: hx("#f6f6f8"),
            surface: hx("#ffffff"),
            surface2: hx("#f0f0f3"),
            border: Rgba([0.0, 0.0, 0.05, 0.10]),
            text: hx("#121318"),
            muted: hx("#6b6f7b"),
            accent: hx("#5b4cf0"),
            positive: hx("#12a66a"),
            negative: hx("#e23a52"),
            palette: ["#5b4cf0", "#0fb3c9", "#e8488f", "#12a66a", "#f09a1a", "#3a7bf0", "#9b5cf0", "#f0663a"]
                .iter()
                .map(|s| hx(s))
                .collect(),
            radius: 18.0,
        }
    }

    /// `"dark"`, `"light"`, or an object `{ "mode": "dark", "accent": "#…", "palette": [...] , ... }`.
    pub fn from_value(v: &Value) -> Result<Theme, String> {
        match v {
            Value::String(s) => match s.as_str() {
                "dark" => Ok(Theme::dark()),
                "light" => Ok(Theme::light()),
                _ => Err(format!("unknown theme {s:?}; use \"dark\", \"light\" or an object")),
            },
            Value::Object(m) => {
                let mut t = match m.get("mode").and_then(|m| m.as_str()) {
                    Some("light") => Theme::light(),
                    _ => Theme::dark(),
                };
                for (k, v) in m {
                    let slot = match k.as_str() {
                        "mode" => continue,
                        "radius" => {
                            t.radius = v.as_f64().ok_or("radius must be a number")? as f32;
                            continue;
                        }
                        "palette" => {
                            let a = v.as_array().ok_or("palette must be an array of colours")?;
                            t.palette = a.iter().map(color_from_value).collect::<Result<_, _>>()?;
                            if t.palette.is_empty() {
                                return Err("palette is empty".into());
                            }
                            continue;
                        }
                        "background" => &mut t.background,
                        "surface" => &mut t.surface,
                        "surface2" => &mut t.surface2,
                        "border" => &mut t.border,
                        "text" => &mut t.text,
                        "muted" => &mut t.muted,
                        "accent" => &mut t.accent,
                        "positive" => &mut t.positive,
                        "negative" => &mut t.negative,
                        _ => {
                            let keys = ["mode", "radius", "palette", "background", "surface", "surface2", "border", "text", "muted", "accent", "positive", "negative"];
                            return Err(match closest(k, &keys) {
                                Some(s) => format!("unknown theme key `{k}`; did you mean `{s}`?"),
                                None => format!("unknown theme key `{k}`"),
                            });
                        }
                    };
                    *slot = color_from_value(v)?;
                }
                Ok(t)
            }
            _ => Err("theme must be \"dark\", \"light\" or an object".into()),
        }
    }

    /// Palette colour `i` (wrapping).
    pub fn pal(&self, i: usize) -> Rgba {
        self.palette[i % self.palette.len()]
    }
}

// ---------------------------------------------------------------- motion

#[inline]
pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Progress 0..1 of a segment starting at `start` lasting `dur`.
#[inline]
pub fn seg(t: f32, start: f32, dur: f32) -> f32 {
    if dur <= 0.0 {
        return if t >= start { 1.0 } else { 0.0 };
    }
    clamp01((t - start) / dur)
}

/// Expo-ish ease out (the "Apple" curve: fast start, long soft landing).
#[inline]
pub fn ease_out(x: f32) -> f32 {
    let x = clamp01(x);
    1.0 - (1.0 - x).powi(4)
}

#[inline]
pub fn ease_in(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * x
}

#[inline]
pub fn ease_in_out(x: f32) -> f32 {
    let x = clamp01(x);
    if x < 0.5 {
        8.0 * x * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(4) / 2.0
    }
}

/// Critically-damped-ish spring from 0 to 1 with a little overshoot, as a
/// function of seconds since start. Settles in about `0.6 / speed` s.
#[inline]
pub fn spring(t: f32, speed: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    let w = 14.0 * speed;
    let z = 0.62f32;
    let wd = w * (1.0 - z * z).sqrt();
    let e = (-z * w * t).exp();
    // Settled: return exactly 1 so later frames are bit-identical (dedupe,
    // incremental render and P_Skip all key on identical ops).
    if e < 2e-4 {
        return 1.0;
    }
    1.0 - e * ((wd * t).cos() + (z * w / wd) * (wd * t).sin())
}

/// Smooth pseudo-random in `[0, 1)` from an integer seed.
#[inline]
pub fn hash01(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Small deterministic RNG (xorshift) for seeded layouts.
#[derive(Clone)]
pub struct Rng(pub u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

/// FNV-1a over bytes, for image cache keys.
pub fn fnv(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in *p {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------- shapes

pub fn translate(x: f32, y: f32) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

/// Scale by `s` about `(cx, cy)`.
pub fn scale_about(s: f32, cx: f32, cy: f32) -> Affine {
    [s, 0.0, 0.0, s, cx - s * cx, cy - s * cy]
}

/// Rotate by `a` radians about `(cx, cy)`.
pub fn rotate_about(a: f32, cx: f32, cy: f32) -> Affine {
    let (s, c) = a.sin_cos();
    [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
}

pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0) as f64;
    kurbo::RoundedRect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64, r).to_path(0.1)
}

pub fn circle_path(cx: f32, cy: f32, r: f32) -> Path {
    kurbo::Circle::new((cx as f64, cy as f64), r.max(0.0) as f64).to_path(0.1)
}

pub fn rect(x: f32, y: f32, w: f32, h: f32, r: f32, paint: Paint) -> Op {
    Op::RoundRect { x, y, w, h, radius: r, paint, transform: IDENTITY }
}

pub fn fill(path: Path, paint: Paint) -> Op {
    Op::Fill { path, paint, rule: FillRule::NonZero, transform: IDENTITY }
}

pub fn circle(cx: f32, cy: f32, r: f32, paint: Paint) -> Op {
    fill(circle_path(cx, cy, r), paint)
}

pub fn stroke_op(path: Path, paint: Paint, width: f32, trim: [f32; 2]) -> Op {
    Op::Stroke { path, paint, stroke: line(width, trim), transform: IDENTITY }
}

/// Round-capped, round-joined stroke.
pub fn line(width: f32, trim: [f32; 2]) -> Stroke {
    Stroke { width, cap: 1, join: 1, miter_limit: 4.0, dash: Vec::new(), dash_offset: 0.0, trim }
}

pub fn solid(c: Rgba) -> Paint {
    Paint::Solid(c)
}

pub fn vgrad(y0: f32, y1: f32, a: Rgba, b: Rgba) -> Paint {
    Paint::Linear { p0: [0.0, y0], p1: [0.0, y1], stops: vec![(0.0, a), (1.0, b)] }
}

pub fn radial(cx: f32, cy: f32, r: f32, stops: Vec<(f32, Rgba)>) -> Paint {
    Paint::Radial { c: [cx, cy], r, stops }
}

/// Soft drop shadow under a rounded rect. Emitted as a `Shadow` layer around
/// an opaque card silhouette (which the caller then paints over), the form
/// the renderer draws from cached blurred sprites.
pub fn shadow(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, dy: f32, blur: f32, a: f32) {
    if a <= 0.001 || w <= 0.0 || h <= 0.0 {
        return;
    }
    out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: vec![dl::Filter::Shadow { dx: 0.0, dy, blur, color: Rgba([0.0, 0.0, 0.0, a]) }], clip: None, cache_key: None });
    out.push(rect(x, y, w, h, r, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
    out.push(Op::PopLayer);
}

/// Coloured halo around a rounded rect (screen-blended), drawn through the
/// renderer's cached shadow sprites: cheap even for big, moving cards.
pub fn glow(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, blur: f32, c: Rgba) {
    if c.0[3] <= 0.001 || w <= 0.0 || h <= 0.0 {
        return;
    }
    out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Screen, filters: vec![dl::Filter::Shadow { dx: 0.0, dy: 0.0, blur, color: c }], clip: None, cache_key: None });
    out.push(rect(x, y, w, h, r, solid(Rgba([0.0, 0.0, 0.0, 1.0]))));
    out.push(Op::PopLayer);
}

/// A premium surface: shadow, fill, top highlight and a hairline border.
pub fn surface(out: &mut Vec<Op>, th: &Theme, x: f32, y: f32, w: f32, h: f32, r: f32, elevation: f32) {
    shadow(out, x, y, w, h, r, 10.0 * elevation, 24.0 * elevation, if th.dark { 0.55 } else { 0.16 } * elevation.min(1.0));
    let top = shade(th.surface, if th.dark { 0.035 } else { 0.0 });
    out.push(rect(x, y, w, h, r, vgrad(y, y + h, top, th.surface)));
    border(out, x, y, w, h, r, th.border);
}

/// Hairline inner border.
pub fn border(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
    out.push(Op::Stroke { path: rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, r - 0.5), paint: solid(c), stroke: line(1.0, [0.0, 1.0]), transform: IDENTITY });
}

/// Push a layer clipped to a rounded rect.
pub fn push_clip(out: &mut Vec<Op>, x: f32, y: f32, w: f32, h: f32, r: f32) {
    out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: Vec::new(), clip: Some((rrect_path(x, y, w, h, r), IDENTITY)), cache_key: None });
}

pub fn push_layer(out: &mut Vec<Op>, opacity: f32, blend: dl::Blend, filters: Vec<dl::Filter>) {
    out.push(Op::PushLayer { opacity, blend, filters, clip: None, cache_key: None });
}

/// Smooth curve through points (Catmull-Rom → cubic Bézier).
pub fn smooth_path(pts: &[(f32, f32)], tension: f32) -> Path {
    let mut p = Path::new();
    if pts.is_empty() {
        return p;
    }
    let pt = |i: usize| kurbo::Point::new(pts[i].0 as f64, pts[i].1 as f64);
    p.move_to(pt(0));
    let n = pts.len();
    let k = (tension / 6.0) as f64;
    for i in 0..n.saturating_sub(1) {
        let p0 = pt(i.saturating_sub(1));
        let p1 = pt(i);
        let p2 = pt(i + 1);
        let p3 = pt((i + 2).min(n - 1));
        let c1 = p1 + (p2 - p0) * k;
        let c2 = p2 - (p3 - p1) * k;
        // Keep monotone in y between points so the curve never overshoots data.
        let (lo, hi) = (p1.y.min(p2.y), p1.y.max(p2.y));
        let c1 = kurbo::Point::new(c1.x, c1.y.clamp(lo, hi));
        let c2 = kurbo::Point::new(c2.x, c2.y.clamp(lo, hi));
        p.curve_to(c1, c2, p2);
    }
    p
}

// ---------------------------------------------------------------- misc

/// Closest candidate by edit distance (for "did you mean").
pub fn closest<'a>(word: &str, options: &[&'a str]) -> Option<&'a str> {
    let mut best = None;
    let mut best_d = usize::MAX;
    for o in options {
        let d = levenshtein(&word.to_ascii_lowercase(), &o.to_ascii_lowercase());
        if d < best_d {
            best_d = d;
            best = Some(*o);
        }
    }
    best.filter(|o| best_d <= (o.len().max(word.len()) / 3).max(2))
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + (ca != *cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Format a number with thousands separators and `decimals` places.
pub fn format_number(v: f64, decimals: usize, sep: bool) -> String {
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = s.split_once('.').map(|(a, b)| (a.to_string(), Some(b.to_string()))).unwrap_or((s.clone(), None));
    let mut out = String::new();
    if v < 0.0 && v.abs() >= 0.5 * 10f64.powi(-(decimals as i32)) {
        out.push('-');
    }
    let n = int.len();
    for (i, c) in int.chars().enumerate() {
        if sep && i > 0 && (n - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(&f);
    }
    out
}

/// Compact number: 1.2K, 3.4M.
pub fn compact_number(v: f64) -> String {
    let a = v.abs();
    let (d, s) = if a >= 1e9 {
        (1e9, "B")
    } else if a >= 1e6 {
        (1e6, "M")
    } else if a >= 1e4 {
        (1e3, "K")
    } else {
        return format_number(v, if a.fract() == 0.0 { 0 } else { 1 }, true);
    };
    let x = v / d;
    format!("{}{}", format_number(x, if x.abs() < 10.0 { 1 } else { 0 }, false).trim_end_matches(".0"), s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers() {
        assert_eq!(format_number(1234567.891, 2, true), "1,234,567.89");
        assert_eq!(format_number(-12.0, 0, true), "-12");
        assert_eq!(compact_number(12600.0), "13K");
        assert_eq!(compact_number(1_260_000.0), "1.3M");
        assert_eq!(compact_number(950.0), "950");
    }
    #[test]
    fn spring_settles() {
        assert!((spring(2.0, 1.0) - 1.0).abs() < 1e-3);
        assert!(spring(0.12, 1.0) > 0.5);
    }
    #[test]
    fn theme_overrides() {
        let t = Theme::from_value(&serde_json::json!({"mode":"light","accent":"#ff0000"})).unwrap();
        assert!(!t.dark);
        assert_eq!(t.accent, Rgba([1.0, 0.0, 0.0, 1.0]));
        assert!(Theme::from_value(&serde_json::json!({"acent":"#ff0000"})).unwrap_err().contains("accent"));
    }
}
