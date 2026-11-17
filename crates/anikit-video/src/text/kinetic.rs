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
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase", remote = "Self")]
pub enum KineticStyle {
    /// Slide up from `distance` em below while fading in.
    Rise {
        #[serde(default = "d_rise")]
        distance: f32,
    },
    /// Fade in.
    Fade,
    /// Unblur from `radius` em while fading in.
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
    /// Continuous sine wave through the units (amplitude in em; ramps in over `duration`).
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
        /// Defaults to the text style's `accent` at 30% opacity.
        #[serde(default)]
        color: Option<Color>,
        /// Padding around the unit's box, em.
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
    /// Flip up around the baseline (a 3D-ish card flip faked with a y-scale).
    Flip,
    /// Letters converge from wide tracking (`spread` em per unit from centre).
    Tracking {
        #[serde(default = "d_spread")]
        spread: f32,
    },
    /// Fall from `distance` em above with a springy landing.
    Drop {
        #[serde(default = "d_drop")]
        distance: f32,
    },
    /// Slide in horizontally from `distance` em (negative = from the right).
    Slide {
        #[serde(default = "d_slide")]
        distance: f32,
    },
    /// Digital glitch: jittering RGB-split copies that settle.
    Glitch {
        #[serde(default = "d_glitch")]
        amount: f32,
        #[serde(default)]
        seed: u32,
    },
    /// An underline draws on under each unit.
    Underline {
        #[serde(default)]
        color: Option<Color>,
        /// Thickness in em.
        #[serde(default = "d_thick")]
        thickness: f32,
    },
    /// Outlines trace on (stroke trim 0 → 1), then the fill fades in. Stroke width in em.
    Draw {
        #[serde(default = "d_draw")]
        width: f32,
    },
    /// Captions: every unit is visible at `dim` alpha; the current unit takes
    /// `active` colour, spoken units return to full colour. Pair with `times`.
    Karaoke {
        #[serde(default)]
        active: Option<Color>,
        #[serde(default = "d_dim")]
        dim: f32,
    },
}

fn d_dim() -> f32 {
    0.35
}
fn d_draw() -> f32 {
    0.022
}
fn d_spread() -> f32 {
    0.6
}
fn d_drop() -> f32 {
    1.2
}
fn d_slide() -> f32 {
    0.8
}
fn d_glitch() -> f32 {
    0.12
}
fn d_thick() -> f32 {
    0.07
}

fn d_rise() -> f32 {
    0.6
}
fn d_blur() -> f32 {
    0.25
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
    0.15
}
fn d_wavelength() -> f32 {
    8.0
}
fn d_speed() -> f32 {
    0.8
}
fn d_pad() -> f32 {
    0.08
}
fn d_radius() -> f32 {
    0.12
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

/// A kinetic text animation. Deserialises from an object or a shorthand
/// string (see [`Kinetic::parse`]): `"rise"`, `"pop by char from center"`,
/// `"fade words 0.8s 0.1s exit"`. Unset fields take per-style defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", remote = "Self")]
pub struct Kinetic {
    pub style: KineticStyle,
    pub unit: Option<UnitKind>,
    /// Seconds between consecutive units starting.
    pub stagger: Option<f32>,
    /// Seconds each unit takes.
    pub duration: Option<f32>,
    /// Seconds before the first unit starts.
    pub delay: f32,
    /// Easing (any `anikit-core` syntax: `outCubic`, `back.out(1.7)`, `spring(bounce: 0.3)`…).
    /// Defaults per style.
    pub ease: Option<String>,
    pub from: From,
    /// Play as an exit: units disappear in stagger order.
    pub exit: bool,
    /// Explicit start time per unit in seconds (e.g. word timings from speech),
    /// overriding `stagger`; units past the end continue at the stagger step.
    pub times: Option<Vec<f32>>,
}

impl Default for Kinetic {
    fn default() -> Self {
        Kinetic {
            style: KineticStyle::Rise { distance: d_rise() },
            unit: None,
            stagger: None,
            duration: None,
            delay: 0.0,
            ease: None,
            from: From::First,
            exit: false,
            times: None,
        }
    }
}

/// Style names accepted in shorthand (and as `"style": "<name>"`).
pub const STYLE_NAMES: &[&str] =
    &[
    "rise", "fade", "blur", "pop", "typewriter", "scramble", "wave", "highlight", "mask", "flip", "tracking", "drop",
    "slide", "glitch", "underline", "draw", "karaoke",
];

impl KineticStyle {
    /// A style with default parameters by name (`rise`, `blur-in`, `scalePop`, `decode`…).
    pub fn by_name(name: &str) -> Option<Self> {
        let n: String = name.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        Some(match n.as_str() {
            "rise" | "slideup" | "up" => KineticStyle::Rise { distance: d_rise() },
            "fade" | "fadein" => KineticStyle::Fade,
            "blur" | "blurin" => KineticStyle::BlurIn { radius: d_blur() },
            "pop" | "scale" | "scalepop" => KineticStyle::ScalePop { from: 0.0 },
            "type" | "typewriter" | "typing" => KineticStyle::Typewriter { caret: true, caret_color: None, blink: d_blink() },
            "scramble" | "decode" => KineticStyle::Scramble { charset: d_charset(), rate: d_rate(), seed: 0 },
            "wave" => KineticStyle::Wave { amplitude: d_amp(), wavelength: d_wavelength(), speed: d_speed() },
            "highlight" | "highlightsweep" | "marker" => {
                KineticStyle::HighlightSweep { color: None, padding: d_pad(), radius: d_radius() }
            }
            "mask" | "maskreveal" | "reveal" => KineticStyle::MaskReveal { direction: Direction::Up },
            "flip" | "flipup" => KineticStyle::Flip,
            "tracking" | "trackingin" | "spread" | "converge" => KineticStyle::Tracking { spread: d_spread() },
            "drop" | "fall" | "bounce" => KineticStyle::Drop { distance: d_drop() },
            "slide" | "slidein" | "slideright" => KineticStyle::Slide { distance: d_slide() },
            "slideleft" => KineticStyle::Slide { distance: -d_slide() },
            "glitch" => KineticStyle::Glitch { amount: d_glitch(), seed: 0 },
            "underline" => KineticStyle::Underline { color: None, thickness: d_thick() },
            "draw" | "trace" | "handwrite" | "sketch" => KineticStyle::Draw { width: d_draw() },
            "karaoke" | "caption" | "captions" | "sing" => KineticStyle::Karaoke { active: None, dim: d_dim() },
            _ => return None,
        })
    }
}

fn unknown_style(name: &str) -> String {
    format!("unknown text animation {name:?}; use one of: {}", STYLE_NAMES.join(", "))
}

impl Serialize for KineticStyle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        KineticStyle::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for KineticStyle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        if let Some(name) = v.as_str() {
            return KineticStyle::by_name(name).ok_or_else(|| serde::de::Error::custom(unknown_style(name)));
        }
        let mut v = v;
        if let Some(t) = v.get("type").and_then(|t| t.as_str()).map(str::to_string) {
            // Accept any spelling of the name in objects too.
            if let Some(def) = KineticStyle::by_name(&t) {
                let canon = KineticStyle::serialize(&def, serde_json::value::Serializer).map_err(serde::de::Error::custom)?;
                v["type"] = canon["type"].clone();
            }
        }
        KineticStyle::deserialize(v).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Kinetic {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Kinetic::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for Kinetic {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        if let Some(s) = v.as_str() {
            return Kinetic::parse(s).map_err(serde::de::Error::custom);
        }
        Kinetic::deserialize(v).map_err(serde::de::Error::custom)
    }
}

impl std::str::FromStr for Kinetic {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Kinetic::parse(s)
    }
}

impl Kinetic {
    /// Parse shorthand: a style name, then any of: a unit (`char(s)`, `word(s)`,
    /// `line(s)`), durations (`0.6s`, `400ms`: the first is the per-unit
    /// duration, the second the stagger), `stagger 0.05`, `delay 1s`,
    /// `from center|last|edges|random`, `exit`, a direction (`up`, `down`,
    /// `left`, `right` for mask), or an easing (`outBack`, `spring(bounce: 0.3)`).
    pub fn parse(src: &str) -> Result<Kinetic, String> {
        let toks: Vec<String> = split_tokens(src);
        let mut it = toks.iter().map(|t| t.as_str()).peekable();
        let first = it.next().ok_or("empty text animation")?;
        let mut k = Kinetic { style: KineticStyle::by_name(first).ok_or_else(|| unknown_style(first))?, ..Default::default() };
        let mut times = 0;
        let secs = |t: &str| -> Option<f32> {
            if let Some(v) = t.strip_suffix("ms") {
                v.parse::<f32>().ok().map(|x| x / 1000.0)
            } else {
                t.strip_suffix('s').unwrap_or(t).parse::<f32>().ok()
            }
        };
        while let Some(t) = it.next() {
            let tl = t.to_lowercase();
            match tl.as_str() {
                "by" | "per" | "each" | "in" | "with" => {}
                "char" | "chars" | "letter" | "letters" | "character" | "characters" => k.unit = Some(UnitKind::Char),
                "word" | "words" => k.unit = Some(UnitKind::Word),
                "line" | "lines" => k.unit = Some(UnitKind::Line),
                "exit" | "out" => k.exit = true,
                "stagger" => k.stagger = Some(it.next().and_then(secs).ok_or("`stagger` needs a time")?),
                "delay" => k.delay = it.next().and_then(secs).ok_or("`delay` needs a time")?,
                "duration" => k.duration = Some(it.next().and_then(secs).ok_or("`duration` needs a time")?),
                "from" => {
                    k.from = match it.next().map(|x| x.to_lowercase()).as_deref() {
                        Some("first") | Some("start") => From::First,
                        Some("last") | Some("end") => From::Last,
                        Some("center") | Some("centre") | Some("middle") => From::Center,
                        Some("edges") => From::Edges,
                        Some("random") => From::Random,
                        o => return Err(format!("`from` takes first|last|center|edges|random, got {o:?}")),
                    }
                }
                "up" | "down" | "left" | "right" => {
                    let d = match tl.as_str() {
                        "up" => Direction::Up,
                        "down" => Direction::Down,
                        "left" => Direction::Left,
                        _ => Direction::Right,
                    };
                    if let KineticStyle::MaskReveal { direction } = &mut k.style {
                        *direction = d;
                    }
                }
                _ => {
                    if let Some(v) = secs(&tl).filter(|_| tl.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.')) {
                        if times == 0 {
                            k.duration = Some(v)
                        } else {
                            k.stagger = Some(v)
                        }
                        times += 1;
                    } else if Easing::parse(t).is_some() {
                        k.ease = Some(t.to_string());
                    } else {
                        return Err(format!("text animation {src:?}: don't understand {t:?}"));
                    }
                }
            }
        }
        Ok(k)
    }
}

/// Split on whitespace and commas, keeping parenthesised groups together.
fn split_tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if (c.is_whitespace() || c == ',') && depth == 0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl TextLayout {
    /// Ops for one unit (its glyphs, one op per run segment, and the
    /// decorations under it), with an alpha factor and optional colour
    /// override. Building block for custom per-unit animation.
    pub fn unit_ops(&self, u: &Unit, alpha: f32, color: Option<Rgba>, transform: Affine, out: &mut Vec<Op>) {
        if alpha <= 0.0 {
            return;
        }
        let mut gi = u.glyphs.start;
        while gi < u.glyphs.end {
            let run = self.glyphs[gi as usize].run;
            let mut glyphs = super::pool::take((u.glyphs.end - gi) as usize);
            while gi < u.glyphs.end && self.glyphs[gi as usize].run == run {
                let g = self.glyphs[gi as usize];
                glyphs.push(self.runs[run as usize].glyphs[g.index as usize]);
                gi += 1;
            }
            let r = &self.runs[run as usize];
            let paint = match color {
                Some(c) => Paint::Solid(with_alpha(c, alpha)),
                None => self.paint(r, alpha),
            };
            out.push(Op::Glyphs { font: r.font, size: r.size, glyphs, paint, transform });
        }
        let line = self.glyphs.get(u.glyphs.start as usize).map(|g| g.line);
        for d in &self.decorations {
            let x0 = d.rect[0].max(u.rect[0]);
            let x1 = (d.rect[0] + d.rect[2]).min(u.rect[0] + u.rect[2]);
            if x1 > x0 && line == Some(d.line) {
                out.push(rect_op([x0, d.rect[1], x1 - x0, d.rect[3]], with_alpha(color.unwrap_or(d.color), alpha), transform));
            }
        }
    }
}

/// Precomputed kinetic animation for one layout.
pub struct KineticPlan {
    pub layout: Arc<TextLayout>,
    k: Kinetic,
    unit: UnitKind,
    duration: f32,
    em: f32,
    ease: Easing,
    /// Start time of each unit.
    starts: Vec<f32>,
    /// Scramble: per run, the charset's glyph ids and advances (same face and size).
    scramble: Vec<Vec<(u16, f32)>>,
    caret: Rgba,
    /// Draw style: outline path and stroke colour per unit.
    paths: Vec<(dl::Path, Rgba)>,
}

impl KineticPlan {
    pub fn new(fonts: &FontStore, layout: Arc<TextLayout>, k: &Kinetic) -> Self {
        // Per-style defaults: (unit, stagger, duration).
        let (du, ds, dd) = match k.style {
            KineticStyle::Rise { .. } => (UnitKind::Word, 0.06, 0.6),
            KineticStyle::Fade => (UnitKind::Char, 0.03, 0.4),
            KineticStyle::BlurIn { .. } => (UnitKind::Word, 0.08, 0.7),
            KineticStyle::ScalePop { .. } => (UnitKind::Char, 0.03, 0.5),
            KineticStyle::Typewriter { .. } => (UnitKind::Char, 0.045, 0.0),
            KineticStyle::Scramble { .. } => (UnitKind::Char, 0.03, 0.5),
            KineticStyle::Wave { .. } => (UnitKind::Char, 0.03, 0.4),
            KineticStyle::HighlightSweep { .. } => (UnitKind::Line, 0.3, 0.6),
            KineticStyle::MaskReveal { .. } => (UnitKind::Line, 0.12, 0.8),
            KineticStyle::Karaoke { .. } => (UnitKind::Word, 0.35, 0.12),
            KineticStyle::Flip => (UnitKind::Char, 0.035, 0.6),
            KineticStyle::Tracking { .. } => (UnitKind::Char, 0.0, 1.2),
            KineticStyle::Drop { .. } => (UnitKind::Char, 0.04, 0.8),
            KineticStyle::Slide { .. } => (UnitKind::Word, 0.07, 0.7),
            KineticStyle::Glitch { .. } => (UnitKind::Word, 0.08, 0.6),
            KineticStyle::Underline { .. } => (UnitKind::Line, 0.25, 0.6),
            KineticStyle::Draw { .. } => (UnitKind::Word, 0.15, 1.4),
        };
        let unit = match k.style {
            KineticStyle::Typewriter { .. } | KineticStyle::Scramble { .. } => UnitKind::Char,
            _ => k.unit.unwrap_or(du),
        };
        let stagger_s = k.stagger.unwrap_or(ds);
        let duration = k.duration.unwrap_or(dd);
        let default_ease = match k.style {
            KineticStyle::ScalePop { .. } => "back.out(1.7)",
            KineticStyle::Typewriter { .. } | KineticStyle::Wave { .. } => "linear",
            KineticStyle::HighlightSweep { .. } => "inOutCubic",
            KineticStyle::MaskReveal { .. } => "outExpo",
            KineticStyle::Flip => "back.out(1.4)",
            KineticStyle::Tracking { .. } => "outExpo",
            KineticStyle::Drop { .. } => "spring(bounce: 0.35)",
            KineticStyle::Slide { .. } => "outQuart",
            KineticStyle::Underline { .. } => "inOutCubic",
            KineticStyle::Draw { .. } => "inOutSine",
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
        let by_order: Vec<f32> = match &k.times {
            Some(ts) if !ts.is_empty() => (0..n)
                .map(|i| {
                    k.delay + ts.get(i).copied().unwrap_or_else(|| ts[ts.len() - 1] + (i + 1 - ts.len()) as f32 * stagger_s)
                })
                .collect(),
            _ => stagger(n, &Stagger { each: stagger_s as f64, from, ..Default::default() })
                .into_iter()
                .map(|d| k.delay + d as f32)
                .collect(),
        };
        // Units are stored in visual order; sequence them in reading (logical)
        // order so right-to-left text animates from its start.
        let order = layout.logical_order(unit);
        let mut starts = vec![0.0; n];
        for (rank, &ui) in order.iter().enumerate() {
            starts[ui] = by_order[rank];
        }
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
        let em = layout.font_size;
        let paths = match k.style {
            KineticStyle::Draw { .. } => layout
                .units(unit)
                .iter()
                .map(|u| {
                    let mut path = dl::Path::new();
                    let mut color = Rgba([1.0; 4]);
                    let mut gi = u.glyphs.start;
                    while gi < u.glyphs.end {
                        let run = layout.glyphs[gi as usize].run as usize;
                        let mut gl = Vec::new();
                        while gi < u.glyphs.end && layout.glyphs[gi as usize].run as usize == run {
                            gl.push(layout.runs[run].glyphs[layout.glyphs[gi as usize].index as usize]);
                            gi += 1;
                        }
                        color = layout.runs[run].color;
                        for el in fonts.run_path(&layout.runs[run], &gl).elements() {
                            path.push(*el);
                        }
                    }
                    (path, color)
                })
                .collect(),
            _ => Vec::new(),
        };
        KineticPlan { layout, k: k.clone(), unit, duration, em, ease, starts, scramble, caret, paths }
    }

    /// Time at which every unit has finished its entrance.
    pub fn duration(&self) -> f32 {
        self.starts.iter().cloned().fold(self.k.delay, f32::max) + self.duration
    }

    /// Linear progress of unit `i` at time `t` (0..1, before easing).
    #[inline]
    pub fn progress(&self, i: usize, t: f32) -> f32 {
        let d = self.duration.max(1e-6);
        let p = ((t - self.starts[i]) / d).clamp(0.0, 1.0);
        if self.k.exit {
            1.0 - p
        } else {
            p
        }
    }

    /// Emit the ops for time `t` (seconds since the animation started).
    pub fn ops(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        self.layout.begin_effects(transform, out);
        self.body(t, transform, out);
        self.layout.end_effects(out);
    }

    fn body(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        let l = &*self.layout;
        let units = l.units(self.unit);
        let mut any_rest = false;
        let mut decos_rest = true;
        match &self.k.style {
            KineticStyle::HighlightSweep { color, padding, radius } => {
                let (padding, radius) = (&(padding * self.em), radius * self.em);
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
                        radius,
                        paint: Paint::Solid(color.map_or(with_alpha(l.accent, 0.3), |c| c.0)),
                        transform,
                    });
                }
                l.body_ops(transform, out);
                return;
            }
            KineticStyle::Underline { color, thickness } => {
                l.body_ops(transform, out);
                let th = thickness * self.em;
                for (i, u) in units.iter().enumerate() {
                    let e = self.ease.ease(self.progress(i, t) as f64) as f32;
                    if e <= 0.0 {
                        continue;
                    }
                    let c = color.map(|c| c.0).unwrap_or(l.accent);
                    let y = u.baseline + self.em * 0.14;
                    out.push(Op::RoundRect { x: u.rect[0], y, w: u.rect[2] * e.min(1.0), h: th, radius: th * 0.5, paint: Paint::Solid(c), transform });
                }
                return;
            }
            KineticStyle::Karaoke { active, dim } => {
                let act = active.map_or_else(|| l.runs.first().map_or(Rgba([1.0; 4]), |_| l.accent), |c| c.0);
                for (i, u) in units.iter().enumerate() {
                    let st = self.starts[i];
                    let next = self.starts.get(i + 1).copied().unwrap_or(f32::INFINITY).max(st + self.duration);
                    let fade = |x: f32| (x / self.duration.max(1e-3)).clamp(0.0, 1.0);
                    if t < st {
                        self.unit_ops(u, *dim, transform, out);
                    } else if t < next {
                        // Current: fade from dim to the active colour.
                        let e = self.ease.ease(fade(t - st) as f64) as f32;
                        let base = l.glyphs.get(u.glyphs.start as usize).map_or(act, |g| l.runs[g.run as usize].color);
                        let c = lerp_rgba(with_alpha(base, *dim), act, e);
                        l.unit_ops(u, 1.0, Some(c), transform, out);
                    } else {
                        let e = fade(t - next);
                        let base = l.glyphs.get(u.glyphs.start as usize).map_or(act, |g| l.runs[g.run as usize].color);
                        l.unit_ops(u, 1.0, Some(lerp_rgba(act, base, e)), transform, out);
                    }
                }
                return;
            }
            KineticStyle::Wave { amplitude, wavelength, speed } => {
                for (i, u) in units.iter().enumerate() {
                    let ramp = if self.duration > 0.0 { self.ease.ease(self.progress(i, t) as f64) as f32 } else { 1.0 };
                    let ph = std::f32::consts::TAU * (speed * t - i as f32 / wavelength.max(0.1));
                    let dy = -amplitude * self.em * ph.sin() * ramp;
                    self.unit_ops(u, 1.0, dl::mul(&transform, &translate(0.0, dy)), out);
                }
                self.decorations(transform, 1.0, out);
                return;
            }
            _ => {}
        }
        // Units at rest are batched into one op per glyph run (buffers from the pool).
        let mut rest: Vec<Vec<dl::Glyph>> = REST.with(|r| std::mem::take(&mut *r.borrow_mut()));
        rest.resize_with(l.runs.len(), Vec::new);
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
                    let slot = &mut rest[g.run as usize];
                    if slot.capacity() == 0 {
                        *slot = super::pool::take(l.runs[g.run as usize].glyphs.len());
                    }
                    slot.push(l.runs[g.run as usize].glyphs[g.index as usize]);
                }
                any_rest = true;
                continue;
            }
            decos_rest = false;
            let e = self.ease.ease(p as f64) as f32;
            match &self.k.style {
                KineticStyle::Rise { distance } => {
                    let a = p.min(1.0) * (2.0 - p.min(1.0)); // fast fade (outQuad)
                    self.unit_ops(u, a, dl::mul(&transform, &translate(0.0, (1.0 - e) * distance * self.em)), out);
                }
                KineticStyle::Fade => self.unit_ops(u, e.clamp(0.0, 1.0), transform, out),
                KineticStyle::BlurIn { radius } => {
                    let b = (1.0 - e).max(0.0) * radius * self.em;
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
                KineticStyle::Flip => {
                    let sy = e.max(0.0);
                    let by = u.baseline;
                    let m = [1.0, 0.0, 0.0, sy, 0.0, by - sy * by];
                    self.unit_ops(u, (p * 2.5).min(1.0), dl::mul(&transform, &m), out);
                }
                KineticStyle::Tracking { spread } => {
                    // Each unit sits `spread` em further from the centre per index.
                    let rel = i as f32 - (units.len() as f32 - 1.0) * 0.5;
                    let dx = (1.0 - e) * spread * self.em * rel;
                    self.unit_ops(u, p.min(1.0).powf(0.6), dl::mul(&transform, &translate(dx, 0.0)), out);
                }
                KineticStyle::Drop { distance } => {
                    let dy = -(1.0 - e) * distance * self.em;
                    self.unit_ops(u, (p * 4.0).min(1.0), dl::mul(&transform, &translate(0.0, dy)), out);
                }
                KineticStyle::Slide { distance } => {
                    let dx = -(1.0 - e) * distance * self.em;
                    self.unit_ops(u, (p * 2.0).min(1.0), dl::mul(&transform, &translate(dx, 0.0)), out);
                }
                KineticStyle::Glitch { amount, seed } => {
                    // Jitter changes ~30 times a second and fades out with progress.
                    let tick = (t * 30.0) as u32;
                    let h = hash(tick ^ (i as u32).wrapping_mul(0x9e37_79b9) ^ seed);
                    let j = (1.0 - e).max(0.0) * amount * self.em;
                    let r1 = ((h & 0xff) as f32 / 127.5 - 1.0) * j;
                    let r2 = (((h >> 8) & 0xff) as f32 / 127.5 - 1.0) * j * 0.4;
                    let vis = if p < 0.5 && (h >> 16) & 3 == 0 { 0.0 } else { 1.0 };
                    let a = (p * 3.0).min(1.0) * vis;
                    l.unit_ops(u, a * 0.8, Some(Rgba([1.0, 0.1, 0.35, 1.0])), dl::mul(&transform, &translate(r1 - j, r2)), out);
                    l.unit_ops(u, a * 0.8, Some(Rgba([0.1, 0.9, 1.0, 1.0])), dl::mul(&transform, &translate(-r1 + j, -r2)), out);
                    self.unit_ops(u, a, dl::mul(&transform, &translate(r1 * 0.3, 0.0)), out);
                }
                KineticStyle::Draw { width } => {
                    let (path, color) = &self.paths[i];
                    let trace = (e / 0.75).clamp(0.0, 1.0);
                    let fill = ((e - 0.55) / 0.45).clamp(0.0, 1.0);
                    if fill > 0.0 {
                        self.unit_ops(u, fill, transform, out);
                    }
                    let mut st = super::layout::text_stroke(width * self.em);
                    st.trim = [0.0, trace];
                    out.push(Op::Stroke { path: path.clone(), paint: Paint::Solid(with_alpha(*color, 1.0 - 0.6 * fill)), stroke: st, transform });
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
                KineticStyle::HighlightSweep { .. }
                | KineticStyle::Wave { .. }
                | KineticStyle::Karaoke { .. }
                | KineticStyle::Underline { .. } => unreachable!(),
            }
        }
        if any_rest {
            for (ri, g) in rest.iter_mut().enumerate() {
                if !g.is_empty() {
                    let r = &l.runs[ri];
                    out.push(Op::Glyphs { font: r.font, size: r.size, glyphs: std::mem::take(g), paint: l.paint(r, 1.0), transform });
                }
            }
        }
        for g in rest.iter_mut() {
            if g.capacity() > 0 {
                super::pool::give(std::mem::take(g));
            }
        }
        rest.clear();
        REST.with(|r| *r.borrow_mut() = rest);
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
    #[inline]
    fn unit_ops(&self, u: &Unit, alpha: f32, transform: Affine, out: &mut Vec<Op>) {
        self.layout.unit_ops(u, alpha, None, transform, out);
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
            glyphs: {
                let mut v = super::pool::take(1);
                v.push(dl::Glyph { id: gid, x, y: base.y });
                v
            },
            paint: l.paint(r, 0.75),
            transform,
        });
    }
}

thread_local! {
    static REST: std::cell::RefCell<Vec<Vec<dl::Glyph>>> = const { std::cell::RefCell::new(Vec::new()) };
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
fn lerp_rgba(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let mut o = [0.0; 4];
    for (i, v) in o.iter_mut().enumerate() {
        *v = a.0[i] + (b.0[i] - a.0[i]) * t;
    }
    Rgba(o)
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
    gradient: Option<super::layout::Gradient>,
    height: f32,
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
            gradient: c.style.gradient.clone().filter(|g| !g.colors.is_empty()),
            height: l.height,
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
        thread_local! {
            static BUF: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
        }
        let s = BUF.with(|b| {
            let mut b = b.borrow_mut();
            format_number_into(&mut b, self.value(t), self.c.decimals, &self.c.separator, &self.c.prefix, &self.c.suffix);
            std::mem::take(&mut *b)
        });
        let mut glyphs = super::pool::take(s.len());
        let mut x = 0.0;
        for ch in s.chars() {
            if let Some((_, g, a)) = self.table.iter().find(|e| e.0 == ch) {
                glyphs.push(dl::Glyph { id: *g, x, y: self.baseline });
                x += a;
            }
        }
        BUF.with(|b| *b.borrow_mut() = s);
        let shift = match self.c.align {
            super::layout::Align::Center => -x * 0.5,
            super::layout::Align::Right => -x,
            _ => 0.0,
        };
        for g in &mut glyphs {
            g.x += shift;
        }
        let paint = match &self.gradient {
            Some(g) => match g.paint(x, self.height, 1.0) {
                Paint::Linear { p0, p1, stops } => Paint::Linear { p0: [p0[0] + shift, p0[1]], p1: [p1[0] + shift, p1[1]], stops },
                p => p,
            },
            None => Paint::Solid(self.color),
        };
        out.push(Op::Glyphs { font: self.font, size: self.size, glyphs, paint, transform });
    }
}

/// Format `v` with fixed decimals and a thousands separator.
pub fn format_number(v: f64, decimals: u32, sep: &str, prefix: &str, suffix: &str) -> String {
    let mut out = String::new();
    format_number_into(&mut out, v, decimals, sep, prefix, suffix);
    out
}

/// [`format_number`] into a reused buffer (cleared first; no allocation once warm).
pub fn format_number_into(out: &mut String, v: f64, decimals: u32, sep: &str, prefix: &str, suffix: &str) {
    use std::fmt::Write;
    out.clear();
    // Digits first, at the end of the buffer, then assembled in front.
    let _ = write!(out, "{:.*}", decimals as usize, v.abs());
    let digits_len = out.len();
    let neg = v < 0.0 && out.bytes().any(|c| c.is_ascii_digit() && c != b'0');
    let int_len = out.find('.').unwrap_or(digits_len);
    if neg {
        out.push('-');
    }
    out.push_str(prefix);
    for i in 0..int_len {
        if i > 0 && (int_len - i) % 3 == 0 {
            out.push_str(sep);
        }
        let ch = out.as_bytes()[i] as char;
        out.push(ch);
    }
    if int_len < digits_len {
        for i in int_len..digits_len {
            let ch = out.as_bytes()[i] as char;
            out.push(ch);
        }
    }
    out.push_str(suffix);
    out.drain(..digits_len);
}

/// A text block ready for evaluation: its layout plus optional enter and exit
/// animations and an anchor. Build once at preprocessing; `ops(t)` per frame.
pub struct TextBlock {
    pub layout: Arc<TextLayout>,
    enter: Option<KineticPlan>,
    exit: Option<(f32, KineticPlan)>,
    /// Offset applied so the anchor point lands on the transform's origin.
    pub offset: [f32; 2],
}

impl TextBlock {
    /// `anchor` is a fraction of the block's size (`[0, 0]` top-left,
    /// `[0.5, 0.5]` centre); `exit_at` is when the exit animation starts.
    pub fn new(
        fonts: &FontStore,
        spec: &TextSpec,
        enter: Option<&Kinetic>,
        exit: Option<(&Kinetic, f32)>,
        anchor: [f32; 2],
    ) -> Self {
        let layout = fonts.layout(spec);
        let b = layout.bounds();
        let offset = [-(b[0] + b[2] * anchor[0]), -layout.height * anchor[1]];
        let enter = enter.map(|k| KineticPlan::new(fonts, layout.clone(), k));
        let exit = exit.map(|(k, at)| {
            let mut k = k.clone();
            k.exit = true;
            (at, KineticPlan::new(fonts, layout.clone(), &k))
        });
        TextBlock { layout, enter, exit, offset }
    }

    /// Time the enter animation finishes.
    pub fn enter_duration(&self) -> f32 {
        self.enter.as_ref().map_or(0.0, |p| p.duration())
    }

    /// Ops at local time `t` (seconds since the block appeared).
    pub fn ops(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        let tr = dl::mul(&transform, &translate(self.offset[0], self.offset[1]));
        if let Some((at, p)) = &self.exit {
            if t >= *at {
                return p.ops(t - at, tr, out);
            }
        }
        match &self.enter {
            Some(p) => p.ops(t, tr, out),
            None => self.layout.ops(tr, out),
        }
    }
}
