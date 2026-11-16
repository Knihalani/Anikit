//! Kinetic typography: per-unit (char, word, line) animations over a laid-out
//! [`TextLayout`], emitting display-list ops for a time `t`.
//!
//! Build a [`KineticPlan`] once at preprocessing (it resolves easing, stagger
//! delays and scramble glyph tables), then call [`KineticPlan::ops`] per frame.
//! Every style is a pure function of `t`.
//!
//! ```ignore
//! let layout = fonts.layout(&spec);
//! let plan = KineticPlan::new(&fonts, layout, &Kinetic { style: KineticStyle::Rise { distance: 40.0 }, ..Default::default() });
//! plan.ops(t, transform, &mut ops);
//! ```

use std::sync::Arc;

use anikit_core::easing::Easing;
use anikit_core::stagger::{stagger, Stagger, StaggerFrom};
use serde::{Deserialize, Serialize};

use super::color::Color;
use super::fonts::FontStore;
use super::layout::{rect_op, TextLayout, TextSpec, TextStyle, Unit, UnitKind};
use crate::dl::{self, Affine, Filter, FontId, Op, Paint, Rgba};

/// Which animation each unit performs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum KineticStyle {
    /// Slide up from `distance` px below while fading in.
    Rise {
        #[serde(default = "d_rise")]
        distance: f32,
    },
    /// Fade in.
    Fade,
    /// Unblur from `radius` px while fading in.
    BlurIn {
        #[serde(default = "d_blur")]
        radius: f32,
    },
    /// Scale up from `from` around the unit's centre (overshoots with a back/spring ease).
    ScalePop {
        #[serde(default)]
        from: f32,
    },
    /// Characters appear one by one; optional blinking caret.
    Typewriter {
        #[serde(default = "d_true")]
        caret: bool,
        #[serde(default)]
        caret_color: Option<Color>,
        /// Blinks per second once typing stops (0 = steady).
        #[serde(default = "d_blink")]
        blink: f32,
    },
    /// Random glyphs that resolve into the text (decode effect).
    Scramble {
        #[serde(default = "d_charset")]
        charset: String,
        /// Glyph changes per second.
        #[serde(default = "d_rate")]
        rate: f32,
        #[serde(default)]
        seed: u32,
    },
    /// Continuous sine wave through the units (amplitude ramps in over `duration`).
    Wave {
        #[serde(default = "d_amp")]
        amplitude: f32,
        /// Units per full wave.
        #[serde(default = "d_wavelength")]
        wavelength: f32,
        /// Waves per second.
        #[serde(default = "d_speed")]
        speed: f32,
    },
    /// A marker highlight sweeps in behind each unit.
    HighlightSweep {
        #[serde(default = "d_highlight")]
        color: Color,
        /// Padding around the unit's box, px.
        #[serde(default = "d_pad")]
        padding: f32,
        #[serde(default = "d_radius")]
        radius: f32,
    },
    /// Each unit slides into view from behind a mask at its own box.
    MaskReveal {
        #[serde(default)]
        direction: Direction,
    },
}

fn d_rise() -> f32 {
    40.0
}
fn d_blur() -> f32 {
    12.0
}
fn d_true() -> bool {
    true
}
fn d_blink() -> f32 {
    1.0
}
fn d_charset() -> String {
    "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&*+=?".into()
}
fn d_rate() -> f32 {
    20.0
}
fn d_amp() -> f32 {
    12.0
}
fn d_wavelength() -> f32 {
    8.0
}
fn d_speed() -> f32 {
    0.8
}
fn d_highlight() -> Color {
    Color::rgba(1.0, 0.85, 0.2, 0.45)
}
fn d_pad() -> f32 {
    4.0
}
fn d_radius() -> f32 {
    6.0
}

/// Direction a masked unit enters from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// From below, moving up.
    #[default]
    Up,
    Down,
    Left,
    Right,
}

/// Stagger origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum From {
    #[default]
    First,
    Last,
    Center,
    Edges,
    Random,
}

/// A kinetic text animation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Kinetic {
    pub style: KineticStyle,
    pub unit: UnitKind,
    /// Seconds between consecutive units starting.
    pub stagger: f32,
    /// Seconds each unit takes.
    pub duration: f32,
    /// Seconds before the first unit starts.
    pub delay: f32,
    /// Easing (any `anikit-core` syntax: `outCubic`, `back.out(1.7)`, `spring(bounce: 0.3)`…).
    /// Defaults per style.
    pub ease: Option<String>,
    pub from: From,
    /// Play as an exit: units disappear in stagger order.
    pub exit: bool,
}

impl Default for Kinetic {
    fn default() -> Self {
        Kinetic {
            style: KineticStyle::Rise { distance: d_rise() },
            unit: UnitKind::Word,
            stagger: 0.06,
            duration: 0.6,
            delay: 0.0,
            ease: None,
            from: From::First,
            exit: false,
        }
    }
}

/// Precomputed kinetic animation for one layout.
pub struct KineticPlan {
    pub layout: Arc<TextLayout>,
    k: Kinetic,
    unit: UnitKind,
    ease: Easing,
    /// Start time of each unit.
    starts: Vec<f32>,
    /// Scramble: per run, the charset's glyph ids and advances (same face and size).
    scramble: Vec<Vec<(u16, f32)>>,
    caret: Rgba,
}

impl KineticPlan {
    pub fn new(fonts: &FontStore, layout: Arc<TextLayout>, k: &Kinetic) -> Self {
        let unit = match k.style {
            KineticStyle::Typewriter { .. } | KineticStyle::Scramble { .. } => UnitKind::Char,
            _ => k.unit,
        };
        let default_ease = match k.style {
            KineticStyle::ScalePop { .. } => "back.out(1.7)",
            KineticStyle::Typewriter { .. } | KineticStyle::Wave { .. } => "linear",
            KineticStyle::HighlightSweep { .. } => "inOutCubic",
            KineticStyle::MaskReveal { .. } => "outExpo",
            _ => "outCubic",
        };
        let ease = k
            .ease
            .as_deref()
            .and_then(Easing::parse)
            .unwrap_or_else(|| Easing::parse(default_ease).unwrap_or_default());
        let n = layout.units(unit).len();
        let from = match k.from {
            From::First => StaggerFrom::First,
            From::Last => StaggerFrom::Last,
            From::Center => StaggerFrom::Center,
            From::Edges => StaggerFrom::Edges,
            From::Random => StaggerFrom::Random(7),
        };
        let starts = stagger(n, &Stagger { each: k.stagger as f64, from, ..Default::default() })
            .into_iter()
            .map(|d| k.delay + d as f32)
            .collect();
        let scramble = match &k.style {
            KineticStyle::Scramble { charset, .. } => layout
                .runs
                .iter()
                .map(|r| {
                    charset
                        .chars()
                        .map(|c| {
                            let g = fonts.glyph_for_char(r.font, c);
                            (g, fonts.advance(r.font, g, r.size))
                        })
                        .filter(|(g, _)| *g != 0)
                        .collect()
                })
                .collect(),
            _ => Vec::new(),
        };
        let caret = match &k.style {
            KineticStyle::Typewriter { caret_color: Some(c), .. } => c.0,
            _ => layout.runs.first().map_or(Rgba([1.0; 4]), |r| r.color),
        };
        KineticPlan { layout, k: k.clone(), unit, ease, starts, scramble, caret }
    }

    /// Time at which every unit has finished its entrance.
    pub fn duration(&self) -> f32 {
        self.starts.iter().cloned().fold(self.k.delay, f32::max) + self.k.duration
    }

    /// Linear progress of unit `i` at time `t` (0..1, before easing).
    #[inline]
    pub fn progress(&self, i: usize, t: f32) -> f32 {
        let d = self.k.duration.max(1e-6);
        let p = ((t - self.starts[i]) / d).clamp(0.0, 1.0);
        if self.k.exit {
            1.0 - p
        } else {
            p
        }
    }

    /// Emit the ops for time `t` (seconds since the animation started).
    pub fn ops(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        let l = &*self.layout;
        let units = l.units(self.unit);
        // Units at rest are batched into one op per glyph run.
        let mut rest: Vec<Vec<dl::Glyph>> = vec![Vec::new(); l.runs.len()];
        let mut any_rest = false;
        let mut decos_rest = true;
        match &self.k.style {
            KineticStyle::HighlightSweep { color, padding, radius } => {
                for (i, u) in units.iter().enumerate() {
                    let e = self.ease.ease(self.progress(i, t) as f64) as f32;
                    if e <= 0.0 {
                        continue;
                    }
                    let r = u.rect;
                    out.push(Op::RoundRect {
                        x: r[0] - padding,
                        y: r[1] - padding * 0.5,
                        w: (r[2] + 2.0 * padding) * e,
                        h: r[3] + *padding,
                        radius: *radius,
                        paint: Paint::Solid(color.0),
                        transform,
                    });
                }
                l.ops(transform, out);
                return;
            }
            KineticStyle::Wave { amplitude, wavelength, speed } => {
                for (i, u) in units.iter().enumerate() {
                    let ramp = if self.k.duration > 0.0 { self.ease.ease(self.progress(i, t) as f64) as f32 } else { 1.0 };
                    let ph = std::f32::consts::TAU * (speed * t - i as f32 / wavelength.max(0.1));
                    let dy = -amplitude * ph.sin() * ramp;
                    self.unit_ops(u, 1.0, dl::mul(&transform, &translate(0.0, dy)), out);
                }
                self.decorations(transform, 1.0, out);
                return;
            }
            _ => {}
        }
        let mut caret_at: Option<[f32; 4]> = None;
        for (i, u) in units.iter().enumerate() {
            let p = self.progress(i, t);
            if p <= 0.0 {
                decos_rest = false;
                continue;
            }
            if let KineticStyle::Typewriter { .. } = self.k.style {
                caret_at = Some(u.rect);
            }
            if p >= 1.0 || matches!(self.k.style, KineticStyle::Typewriter { .. }) {
                for gi in u.glyphs.clone() {
                    let g = l.glyphs[gi as usize];
                    rest[g.run as usize].push(l.runs[g.run as usize].glyphs[g.index as usize]);
                }
                any_rest = true;
                continue;
            }
            decos_rest = false;
            let e = self.ease.ease(p as f64) as f32;
            match &self.k.style {
                KineticStyle::Rise { distance } => {
                    let a = p.min(1.0) * (2.0 - p.min(1.0)); // fast fade (outQuad)
                    self.unit_ops(u, a, dl::mul(&transform, &translate(0.0, (1.0 - e) * distance)), out);
                }
                KineticStyle::Fade => self.unit_ops(u, e.clamp(0.0, 1.0), transform, out),
                KineticStyle::BlurIn { radius } => {
                    let b = (1.0 - e).max(0.0) * radius;
                    out.push(Op::PushLayer {
                        opacity: e.clamp(0.0, 1.0),
                        blend: Default::default(),
                        filters: if b > 0.05 { vec![Filter::Blur(b)] } else { Vec::new() },
                        clip: None,
                        cache_key: None,
                    });
                    self.unit_ops(u, 1.0, transform, out);
                    out.push(Op::PopLayer);
                }
                KineticStyle::ScalePop { from } => {
                    let s = from + (1.0 - from) * e;
                    let cx = u.rect[0] + u.rect[2] * 0.5;
                    let cy = u.rect[1] + u.rect[3] * 0.5;
                    let m = [s, 0.0, 0.0, s, cx - s * cx, cy - s * cy];
                    self.unit_ops(u, (p * 3.0).min(1.0), dl::mul(&transform, &m), out);
                }
                KineticStyle::Typewriter { .. } => unreachable!(),
                KineticStyle::Scramble { rate, seed, .. } => {
                    let tick = (t * rate).floor() as u32;
                    self.scrambled(u, i as u32, tick ^ seed.wrapping_mul(0x9e37_79b9), transform, out);
                }
                KineticStyle::MaskReveal { direction } => {
                    let r = u.rect;
                    let pad = r[3] * 0.25;
                    let (dx, dy) = match direction {
                        Direction::Up => (0.0, (1.0 - e) * r[3]),
                        Direction::Down => (0.0, -(1.0 - e) * r[3]),
                        Direction::Left => ((1.0 - e) * r[2], 0.0),
                        Direction::Right => (-(1.0 - e) * r[2], 0.0),
                    };
                    let mut clip = dl::Path::new();
                    let (x0, y0, x1, y1) = ((r[0] - pad) as f64, r[1] as f64, (r[0] + r[2] + pad) as f64, (r[1] + r[3]) as f64);
                    clip.move_to((x0, y0));
                    clip.line_to((x1, y0));
                    clip.line_to((x1, y1));
                    clip.line_to((x0, y1));
                    clip.close_path();
                    out.push(Op::PushLayer {
                        opacity: 1.0,
                        blend: Default::default(),
                        filters: Vec::new(),
                        clip: Some((clip, transform)),
                        cache_key: None,
                    });
                    self.unit_ops(u, 1.0, dl::mul(&transform, &translate(dx, dy)), out);
                    out.push(Op::PopLayer);
                }
                KineticStyle::HighlightSweep { .. } | KineticStyle::Wave { .. } => unreachable!(),
            }
        }
        if any_rest {
            for (ri, g) in rest.into_iter().enumerate() {
                if !g.is_empty() {
                    let r = &l.runs[ri];
                    out.push(Op::Glyphs { font: r.font, size: r.size, glyphs: g, paint: Paint::Solid(r.color), transform });
                }
            }
        }
        if decos_rest {
            self.decorations(transform, 1.0, out);
        }
        if let KineticStyle::Typewriter { caret: true, blink, .. } = self.k.style {
            let typing = t < self.duration();
            let on = typing || blink <= 0.0 || ((t - self.duration()) * blink * 2.0).floor() as i64 % 2 == 0;
            let started = t >= self.k.delay;
            if on && started {
                let size = l.font_size;
                let r = caret_at.unwrap_or_else(|| {
                    l.chars.first().map_or([0.0, 0.0, 0.0, size * 1.2], |c| [c.rect[0], c.rect[1], 0.0, c.rect[3]])
                });
                let w = (size * 0.07).max(1.5);
                out.push(Op::RoundRect {
                    x: r[0] + r[2] + w * 0.5,
                    y: r[1] + r[3] * 0.08,
                    w,
                    h: r[3] * 0.84,
                    radius: w * 0.5,
                    paint: Paint::Solid(self.caret),
                    transform,
                });
            }
        }
    }

    fn decorations(&self, transform: Affine, alpha: f32, out: &mut Vec<Op>) {
        for d in &self.layout.decorations {
            out.push(rect_op(d.rect, with_alpha(d.color, alpha), transform));
        }
    }

    /// One op per glyph-run segment of a unit.
    fn unit_ops(&self, u: &Unit, alpha: f32, transform: Affine, out: &mut Vec<Op>) {
        if alpha <= 0.0 {
            return;
        }
        let l = &*self.layout;
        let mut gi = u.glyphs.start;
        while gi < u.glyphs.end {
            let run = l.glyphs[gi as usize].run;
            let mut glyphs = Vec::new();
            while gi < u.glyphs.end && l.glyphs[gi as usize].run == run {
                let g = l.glyphs[gi as usize];
                glyphs.push(l.runs[run as usize].glyphs[g.index as usize]);
                gi += 1;
            }
            let r = &l.runs[run as usize];
            out.push(Op::Glyphs { font: r.font, size: r.size, glyphs, paint: Paint::Solid(with_alpha(r.color, alpha)), transform });
        }
        // Decorations under this unit.
        for d in &l.decorations {
            let ux0 = u.rect[0];
            let ux1 = u.rect[0] + u.rect[2];
            let x0 = d.rect[0].max(ux0);
            let x1 = (d.rect[0] + d.rect[2]).min(ux1);
            let line = l.glyphs.get(u.glyphs.start as usize).map(|g| g.line);
            if x1 > x0 && line == Some(d.line) {
                out.push(rect_op([x0, d.rect[1], x1 - x0, d.rect[3]], with_alpha(d.color, alpha), transform));
            }
        }
    }

    fn scrambled(&self, u: &Unit, idx: u32, tick: u32, transform: Affine, out: &mut Vec<Op>) {
        let l = &*self.layout;
        let Some(g) = l.glyphs.get(u.glyphs.start as usize) else { return };
        let run = g.run as usize;
        let table = &self.scramble[run];
        if table.is_empty() {
            return;
        }
        // Prefer glyphs that roughly fit the cell so narrow letters don't collide.
        let mut h = hash(idx.wrapping_mul(0x85eb_ca6b) ^ tick);
        let mut pick = table[(h as usize) % table.len()];
        for _ in 0..6 {
            if pick.1 <= u.rect[2] * 1.25 {
                break;
            }
            h = hash(h);
            pick = table[(h as usize) % table.len()];
        }
        let (gid, adv) = pick;
        let r = &l.runs[run];
        let base = r.glyphs[g.index as usize];
        let x = u.rect[0] + (u.rect[2] - adv) * 0.5;
        out.push(Op::Glyphs {
            font: r.font,
            size: r.size,
            glyphs: vec![dl::Glyph { id: gid, x, y: base.y }],
            paint: Paint::Solid(with_alpha(r.color, 0.75)),
            transform,
        });
    }
}

#[inline]
fn translate(x: f32, y: f32) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

#[inline]
fn with_alpha(c: Rgba, a: f32) -> Rgba {
    let mut v = c.0;
    v[3] *= a;
    Rgba(v)
}

#[inline]
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// An animated number (`$1,234.5k`), drawn from pre-shaped tabular digits so
/// each frame is one glyph op with no layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Counter {
    pub from: f64,
    pub to: f64,
    pub decimals: u32,
    pub prefix: String,
    pub suffix: String,
    /// Thousands separator (empty for none).
    pub separator: String,
    pub style: TextStyle,
    pub align: super::layout::Align,
    pub duration: f32,
    pub delay: f32,
    pub ease: Option<String>,
}

impl Default for Counter {
    fn default() -> Self {
        Counter {
            from: 0.0,
            to: 100.0,
            decimals: 0,
            prefix: String::new(),
            suffix: String::new(),
            separator: ",".into(),
            style: TextStyle { weight: 700.0, size: 96.0, ..Default::default() },
            align: super::layout::Align::Left,
            duration: 1.5,
            delay: 0.0,
            ease: None,
        }
    }
}

/// Precomputed [`Counter`].
pub struct CounterPlan {
    c: Counter,
    ease: Easing,
    font: FontId,
    size: f32,
    color: Rgba,
    baseline: f32,
    /// Glyph and advance per char (digits, sign, point, separator, affixes).
    table: Vec<(char, u16, f32)>,
}

impl CounterPlan {
    pub fn new(fonts: &FontStore, c: &Counter) -> Self {
        let mut style = c.style.clone();
        let f = style.features.take().unwrap_or_default();
        style.features = Some(if f.is_empty() { "\"tnum\" on".into() } else { format!("{f}, \"tnum\" on") });
        let mut chars: Vec<char> = "0123456789-.".chars().collect();
        chars.extend(c.separator.chars());
        chars.extend(c.prefix.chars());
        chars.extend(c.suffix.chars());
        chars.sort_unstable();
        chars.dedup();
        let text: String = chars.iter().collect();
        let l = fonts.layout(&TextSpec::plain(text, style));
        let mut table = Vec::new();
        for (i, ch) in chars.iter().enumerate() {
            // Map each char unit back by its text offset.
            if let Some(u) = l.chars.iter().find(|u| l.text[u.text.clone()].starts_with(*ch)) {
                if let Some(gi) = l.glyphs.get(u.glyphs.start as usize) {
                    let g = l.runs[gi.run as usize].glyphs[gi.index as usize];
                    table.push((*ch, g.id, u.rect[2]));
                }
            }
            let _ = i;
        }
        let r = l.runs.first();
        CounterPlan {
            c: c.clone(),
            ease: c.ease.as_deref().and_then(Easing::parse).unwrap_or_else(|| Easing::parse("outExpo").unwrap_or_default()),
            font: r.map_or(FontId(0), |r| r.font),
            size: r.map_or(c.style.size, |r| r.size),
            color: r.map_or(c.style.color.0, |r| r.color),
            baseline: l.lines.first().map_or(c.style.size, |x| x.baseline),
            table,
        }
    }

    /// Value shown at time `t`.
    pub fn value(&self, t: f32) -> f64 {
        let p = ((t - self.c.delay) / self.c.duration.max(1e-6)).clamp(0.0, 1.0);
        let e = self.ease.ease(p as f64);
        self.c.from + (self.c.to - self.c.from) * e
    }

    /// The formatted string at time `t`.
    pub fn text(&self, t: f32) -> String {
        format_number(self.value(t), self.c.decimals, &self.c.separator, &self.c.prefix, &self.c.suffix)
    }

    /// Emit the counter at time `t`; `(0, 0)` is the top-left (left align),
    /// top-centre or top-right of the text.
    pub fn ops(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        let s = self.text(t);
        let mut glyphs = Vec::with_capacity(s.len());
        let mut x = 0.0;
        for ch in s.chars() {
            if let Some((_, g, a)) = self.table.iter().find(|e| e.0 == ch) {
                glyphs.push(dl::Glyph { id: *g, x, y: self.baseline });
                x += a;
            }
        }
        let shift = match self.c.align {
            super::layout::Align::Center => -x * 0.5,
            super::layout::Align::Right => -x,
            _ => 0.0,
        };
        for g in &mut glyphs {
            g.x += shift;
        }
        out.push(Op::Glyphs { font: self.font, size: self.size, glyphs, paint: Paint::Solid(self.color), transform });
    }
}

/// Format `v` with fixed decimals and a thousands separator.
pub fn format_number(v: f64, decimals: u32, sep: &str, prefix: &str, suffix: &str) -> String {
    let s = format!("{:.*}", decimals as usize, v.abs());
    let (int, frac) = s.split_once('.').map_or((s.as_str(), ""), |(a, b)| (a, b));
    let mut out = String::with_capacity(s.len() + 8 + prefix.len() + suffix.len());
    let neg = v < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0');
    if neg {
        out.push('-');
    }
    out.push_str(prefix);
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(ch);
    }
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    out.push_str(suffix);
    out
}
