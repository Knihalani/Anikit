//! Rich text layout (parley): spans in, positioned glyph runs, decorations and
//! addressable units (chars, words, lines) out.
//!
//! Layout runs once at preprocessing and is cached by the spec's content
//! (`(text, style, width)`), so frames only read the result.

use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use parley::{
    Alignment, AlignmentOptions, FontFamily, FontFeatures, FontStyle, FontVariations, FontWeight,
    Layout, LineHeight, StyleProperty,
};
use serde::{Deserialize, Serialize};

use super::color::Color;
use super::fonts::FontStore;
use crate::dl::{self, Affine, FontId, Op, Paint, Rgba};

/// Horizontal alignment of lines within the wrapping width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Base style of a text block (every field has a sensible default).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextStyle {
    /// Family name or alias: `Inter`, `Inter Display`, `JetBrains Mono`,
    /// `Instrument Serif`, `sans`, `serif`, `mono`, or any loaded family.
    /// A comma-separated CSS list works too.
    pub font: String,
    /// 100..900 (any value on variable fonts).
    pub weight: f32,
    pub italic: bool,
    /// Font size in px.
    pub size: f32,
    pub color: Color,
    /// Extra space between letters, in em (CSS-like tracking, e.g. `-0.02`).
    pub letter_spacing: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
    pub underline: bool,
    pub strikethrough: bool,
    /// OpenType features in CSS syntax, e.g. `"tnum" on, "ss01" on`.
    pub features: Option<String>,
    /// Variable-font axis coordinates, e.g. `[("wdth", 125.0), ("SOFT", 100.0)]`.
    /// Without an explicit `opsz`, faces with an optical-size axis get
    /// `opsz` = the font size in pt (px × 0.75), except Inter.
    pub axes: Vec<(String, f32)>,
    /// Outline the glyphs (`{ "width": 0.03, "hollow": true }`; width in em).
    pub outline: Option<Outline>,
    /// Colour for `==accent==` markup.
    pub accent: Color,
    /// A rounded box behind each line (captions, labels).
    pub background: Option<LineBackground>,
    /// Drop shadow: `{ "x": 0, "y": 8, "blur": 24, "color": "#0008" }`.
    pub shadow: Option<Shadow>,
    /// Soft glow: `{ "radius": 16, "intensity": 0.8, "color": "#8b7bff" }`.
    pub glow: Option<Glow>,
    /// Gradient fill across the whole block (overrides span colours):
    /// `["#7c5cff", "#ff6ad5"]` or `{ "colors": [...], "angle": 135 }`.
    pub gradient: Option<Gradient>,
}

/// Box behind each line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LineBackground {
    pub color: Color,
    /// Horizontal and vertical padding in em.
    pub padding: [f32; 2],
    /// Corner radius in em.
    pub radius: f32,
}

impl Default for LineBackground {
    fn default() -> Self {
        LineBackground { color: Color::rgba(0.0, 0.0, 0.0, 0.7), padding: [0.3, 0.08], radius: 0.18 }
    }
}

/// Drop shadow (px).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Shadow {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub color: Color,
}

impl Default for Shadow {
    fn default() -> Self {
        Shadow { x: 0.0, y: 6.0, blur: 18.0, color: Color::rgba(0.0, 0.0, 0.0, 0.5) }
    }
}

/// Glow (px).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Glow {
    pub radius: f32,
    pub intensity: f32,
    pub color: Option<Color>,
}

impl Default for Glow {
    fn default() -> Self {
        Glow { radius: 16.0, intensity: 0.8, color: None }
    }
}

/// Glyph outline stroke.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Outline {
    /// Stroke width in em.
    pub width: f32,
    /// Stroke colour (defaults to the text colour).
    pub color: Option<Color>,
    /// Draw only the outline, no fill.
    pub hollow: bool,
}

impl Default for Outline {
    fn default() -> Self {
        Outline { width: 0.025, color: None, hollow: true }
    }
}

/// A linear gradient: CSS angle convention (0 = upwards, 90 = to the right).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gradient {
    pub colors: Vec<Color>,
    pub angle: f32,
}

impl<'de> Deserialize<'de> for Gradient {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Colors(Vec<Color>),
            Full {
                colors: Vec<Color>,
                #[serde(default = "deg90")]
                angle: f32,
            },
        }
        fn deg90() -> f32 {
            90.0
        }
        Ok(match Raw::deserialize(d)? {
            Raw::Colors(colors) => Gradient { colors, angle: 90.0 },
            Raw::Full { colors, angle } => Gradient { colors, angle },
        })
    }
}

impl Gradient {
    /// The paint over a `w × h` box at the origin.
    pub fn paint(&self, w: f32, h: f32, alpha: f32) -> Paint {
        let a = self.angle.to_radians();
        let (dx, dy) = (a.sin(), -a.cos());
        let half = (w * 0.5 * dx).abs() + (h * 0.5 * dy).abs();
        let (cx, cy) = (w * 0.5, h * 0.5);
        let n = self.colors.len().max(2) - 1;
        Paint::Linear {
            p0: [cx - dx * half, cy - dy * half],
            p1: [cx + dx * half, cy + dy * half],
            stops: self
                .colors
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut v = c.0 .0;
                    v[3] *= alpha;
                    (i as f32 / n as f32, Rgba(v))
                })
                .collect(),
        }
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            font: "Inter".into(),
            weight: 400.0,
            italic: false,
            size: 48.0,
            color: Color(Rgba([1.0, 1.0, 1.0, 1.0])),
            letter_spacing: 0.0,
            line_height: 1.2,
            underline: false,
            strikethrough: false,
            features: None,
            axes: Vec::new(),
            accent: Color(Rgba([0.55, 0.48, 1.0, 1.0])),
            outline: None,
            background: None,
            shadow: None,
            glow: None,
            gradient: None,
        }
    }
}

/// A run of text with optional style overrides.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Span {
    pub text: String,
    pub font: Option<String>,
    pub weight: Option<f32>,
    pub italic: Option<bool>,
    pub size: Option<f32>,
    pub color: Option<Color>,
    pub letter_spacing: Option<f32>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub features: Option<String>,
}

impl Span {
    pub fn new(text: impl Into<String>) -> Self {
        Span { text: text.into(), ..Default::default() }
    }
}

/// Shrink-to-fit box: the font size is binary-searched so the text fits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Fit {
    pub width: f32,
    pub height: f32,
    /// Smallest allowed base size in px.
    pub min_size: f32,
    /// Largest allowed base size in px (defaults to the style's size).
    pub max_size: Option<f32>,
}

impl Default for Fit {
    fn default() -> Self {
        Fit { width: 1000.0, height: 200.0, min_size: 8.0, max_size: None }
    }
}

/// A block of rich text. Deserialises from an object or just a string.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", remote = "Self")]
pub struct TextSpec {
    /// Plain text (used when `spans` is empty).
    pub text: String,
    pub spans: Vec<Span>,
    pub style: TextStyle,
    /// Wrapping width in px; `None` = no wrapping.
    pub width: Option<f32>,
    pub align: Align,
    /// Truncate after this many lines.
    pub max_lines: Option<u32>,
    /// Append `…` when truncated (default true when `max_lines` is set).
    pub ellipsis: Option<bool>,
    /// Shrink to fit a box (wraps at `fit.width` unless `width` is set).
    pub fit: Option<Fit>,
    /// Inline markup in `text`/spans: `**bold**`, `*italic*`, `` `code` ``,
    /// `==accent==` (in `style.accent`). On by default; only paired markers
    /// count, and `\*` escapes.
    pub markup: Option<bool>,
}

impl Serialize for TextSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TextSpec::serialize(self, s)
    }
}

impl<'de> Deserialize<'de> for TextSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        if let Some(s) = v.as_str() {
            return Ok(TextSpec { text: s.to_string(), ..Default::default() });
        }
        TextSpec::deserialize(v).map_err(serde::de::Error::custom)
    }
}

impl TextSpec {
    /// Literal text in one style (markup off: safe for code and labels).
    pub fn plain(text: impl Into<String>, style: TextStyle) -> Self {
        TextSpec { text: text.into(), style, markup: Some(false), ..Default::default() }
    }

    /// Text with inline markup (`**bold**`, `*italic*`, `` `code` ``, `==accent==`).
    pub fn rich(text: impl Into<String>, style: TextStyle) -> Self {
        TextSpec { text: text.into(), style, markup: Some(true), ..Default::default() }
    }

    /// Expand CSS `font` shorthands (`"italic 800 72px/1.1 Inter Display"`) into fields.
    pub fn normalized(&self) -> TextSpec {
        let mut s = self.clone();
        if let Some(f) = parse_font(&s.style.font) {
            s.style.font = f.family;
            s.style.weight = f.weight.unwrap_or(s.style.weight);
            s.style.size = f.size.unwrap_or(s.style.size);
            s.style.line_height = f.line_height.unwrap_or(s.style.line_height);
            s.style.italic |= f.italic;
        }
        if s.markup != Some(false) {
            let src: Vec<Span> = if s.spans.is_empty() { vec![Span::new(s.text.clone())] } else { s.spans.clone() };
            if src.iter().any(|sp| has_markup(&sp.text)) {
                s.spans = src.iter().flat_map(|sp| markup_spans(sp, &s.style)).collect();
                s.text.clear();
            }
        }
        for sp in &mut s.spans {
            if let Some(f) = sp.font.as_deref().and_then(parse_font) {
                sp.font = Some(f.family);
                sp.weight = sp.weight.or(f.weight);
                sp.size = sp.size.or(f.size);
                if f.italic {
                    sp.italic = Some(true);
                }
            }
        }
        s
    }

    fn cache_key(&self) -> u64 {
        // Stream the serialisation into the hasher: no allocation.
        struct W<'a>(&'a mut std::collections::hash_map::DefaultHasher);
        impl std::io::Write for W<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.write(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let _ = serde_json::to_writer(W(&mut h), self);
        h.finish()
    }

    fn joined(&self) -> (String, Vec<(Range<usize>, &Span)>) {
        if self.spans.is_empty() {
            return (self.text.clone(), Vec::new());
        }
        let mut s = String::new();
        let mut v = Vec::with_capacity(self.spans.len());
        for sp in &self.spans {
            let a = s.len();
            s.push_str(&sp.text);
            v.push((a..s.len(), sp));
        }
        (s, v)
    }
}

/// A run of glyphs sharing one face, size and colour (maps 1:1 to `Op::Glyphs`).
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontId,
    pub size: f32,
    pub color: Rgba,
    pub glyphs: Vec<dl::Glyph>,
}

/// Where a glyph sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphInfo {
    /// Index of its [`GlyphRun`] and position inside it.
    pub run: u32,
    pub index: u32,
    /// Char unit, word and line it belongs to.
    pub char: u32,
    pub word: u32,
    pub line: u32,
}

/// An addressable unit (char, word or line): its box and its glyphs.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    /// `[x, y, w, h]` in layout coordinates (origin top-left of the block).
    pub rect: [f32; 4],
    /// Baseline y.
    pub baseline: f32,
    /// Range into [`TextLayout::glyphs`].
    pub glyphs: Range<u32>,
    /// Byte range in [`TextLayout::text`].
    pub text: Range<usize>,
}

/// A filled rectangle (underline, strikethrough).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoration {
    pub rect: [f32; 4],
    pub color: Rgba,
    pub line: u32,
}

/// The result of laying out a [`TextSpec`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    /// The laid-out text (after truncation).
    pub text: String,
    pub runs: Vec<GlyphRun>,
    /// Every visible glyph, in visual order (spaces and breaks are dropped).
    pub glyphs: Vec<GlyphInfo>,
    /// Grapheme clusters that draw something.
    pub chars: Vec<Unit>,
    pub words: Vec<Unit>,
    pub lines: Vec<Unit>,
    pub decorations: Vec<Decoration>,
    pub width: f32,
    pub height: f32,
    /// Base font size actually used (differs from the style's after auto-fit).
    pub font_size: f32,
    /// Cap-height line of the first line and the last line's baseline (layout y): the box
    /// `textBoxTrim` aligns (CSS `text-box: cap alphabetic`).
    pub cap_top: f32,
    pub last_baseline: f32,
    /// True when `max_lines` cut the text.
    pub truncated: bool,
    /// Block-wide fill overriding run colours (gradient text).
    pub fill: Option<Gradient>,
    /// Outline stroke: one path per run in layout coordinates, plus style.
    pub outline: Option<(Vec<dl::Path>, dl::Stroke, Option<Rgba>, bool)>,
    /// Content hash of the spec (stable cache key for layer caching).
    pub key: u64,
    /// Line background boxes `([x, y, w, h], radius, colour)`.
    pub backgrounds: Vec<([f32; 4], f32, Rgba)>,
    /// Layer filters (shadow, glow) wrapping the glyphs.
    pub filters: Vec<dl::Filter>,
    /// The style's accent colour (highlight/underline defaults).
    pub accent: Rgba,
}

/// Unit granularity for per-unit addressing and animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitKind {
    #[default]
    Char,
    Word,
    Line,
}

impl TextLayout {
    pub fn units(&self, kind: UnitKind) -> &[Unit] {
        match kind {
            UnitKind::Char => &self.chars,
            UnitKind::Word => &self.words,
            UnitKind::Line => &self.lines,
        }
    }

    /// Glyph position (pen origin on the baseline) for glyph `i`.
    #[inline]
    pub fn glyph(&self, i: u32) -> (&GlyphRun, dl::Glyph) {
        let g = self.glyphs[i as usize];
        let r = &self.runs[g.run as usize];
        (r, r.glyphs[g.index as usize])
    }

    /// Unit indices sorted by reading (logical text) order. Units are stored
    /// in visual order, which differs inside right-to-left runs.
    pub fn logical_order(&self, kind: UnitKind) -> Vec<usize> {
        let u = self.units(kind);
        let mut o: Vec<usize> = (0..u.len()).collect();
        o.sort_by_key(|&i| (u[i].text.start, i));
        o
    }

    /// Box `[x, y, w, h]` around all lines in layout coordinates (aligned
    /// lines can start right of 0 when a wrap width is set).
    pub fn bounds(&self) -> [f32; 4] {
        if self.lines.is_empty() {
            return [0.0, 0.0, self.width, self.height];
        }
        let x0 = self.lines.iter().map(|l| l.rect[0]).fold(f32::MAX, f32::min);
        let x1 = self.lines.iter().map(|l| l.rect[0] + l.rect[2]).fold(f32::MIN, f32::max);
        [x0, 0.0, (x1 - x0).max(0.0), self.height]
    }

    /// The paint for a run's glyphs with an extra alpha factor.
    #[inline]
    pub fn paint(&self, run: &GlyphRun, alpha: f32) -> Paint {
        match &self.fill {
            Some(g) => g.paint(self.width, self.height, alpha),
            None => {
                let mut c = run.color.0;
                c[3] *= alpha;
                Paint::Solid(Rgba(c))
            }
        }
    }

    /// Emit line backgrounds and open the effects layer (if any).
    pub fn begin_effects(&self, transform: Affine, out: &mut Vec<Op>) {
        for (r, rad, c) in &self.backgrounds {
            out.push(Op::RoundRect { x: r[0], y: r[1], w: r[2], h: r[3], radius: *rad, paint: Paint::Solid(*c), transform });
        }
        if !self.filters.is_empty() {
            out.push(Op::PushLayer { opacity: 1.0, blend: dl::Blend::Normal, filters: self.filters.clone(), clip: None, cache_key: None });
        }
    }

    /// Line background boxes (from `style.background`) with an alpha factor.
    /// For callers that emit glyphs themselves; draw before the glyphs.
    pub fn background_ops(&self, alpha: f32, transform: Affine, out: &mut Vec<Op>) {
        for (r, rad, c) in &self.backgrounds {
            let mut c = c.0;
            c[3] *= alpha;
            out.push(Op::RoundRect { x: r[0], y: r[1], w: r[2], h: r[3], radius: *rad, paint: Paint::Solid(Rgba(c)), transform });
        }
    }

    /// Whether glyph fills should be skipped (`outline.hollow`).
    pub fn hollow(&self) -> bool {
        matches!(&self.outline, Some((_, _, _, true)))
    }

    /// Outline strokes (from `style.outline`) with an alpha factor. For callers
    /// that emit glyphs themselves; draw after the glyph fills (or instead of
    /// them when [`Self::hollow`]).
    pub fn outline_ops(&self, alpha: f32, transform: Affine, out: &mut Vec<Op>) {
        let Some((paths, stroke, color, _)) = &self.outline else { return };
        for (r, p) in self.runs.iter().zip(paths) {
            let paint = match color {
                Some(c) => {
                    let mut c = c.0;
                    c[3] *= alpha;
                    Paint::Solid(Rgba(c))
                }
                None => self.paint(r, alpha),
            };
            out.push(Op::Stroke { path: p.clone(), paint, stroke: stroke.clone(), transform });
        }
    }

    /// Close the effects layer opened by [`Self::begin_effects`].
    pub fn end_effects(&self, out: &mut Vec<Op>) {
        if !self.filters.is_empty() {
            out.push(Op::PopLayer);
        }
    }

    /// Static ops for the whole block: backgrounds, decorations, glyph runs, effects.
    pub fn ops(&self, transform: Affine, out: &mut Vec<Op>) {
        self.begin_effects(transform, out);
        self.body_ops(transform, out);
        self.end_effects(out);
    }

    pub(crate) fn body_ops(&self, transform: Affine, out: &mut Vec<Op>) {
        for d in &self.decorations {
            out.push(rect_op(d.rect, d.color, transform));
        }
        if let Some((paths, stroke, color, hollow)) = &self.outline {
            for (r, p) in self.runs.iter().zip(paths) {
                if !hollow {
                    out.push(Op::Glyphs { font: r.font, size: r.size, glyphs: super::pool::copy(&r.glyphs), paint: self.paint(r, 1.0), transform });
                }
                let paint = color.map_or_else(|| self.paint(r, 1.0), Paint::Solid);
                out.push(Op::Stroke { path: p.clone(), paint, stroke: stroke.clone(), transform });
            }
            return;
        }
        for r in &self.runs {
            out.push(Op::Glyphs {
                font: r.font,
                size: r.size,
                glyphs: super::pool::copy(&r.glyphs),
                paint: self.paint(r, 1.0),
                transform,
            });
        }
    }

    /// Convenience: a fresh op list.
    pub fn to_ops(&self, transform: Affine) -> Vec<Op> {
        let mut v = Vec::with_capacity(self.runs.len() + self.decorations.len());
        self.ops(transform, &mut v);
        v
    }
}

/// A round-joined stroke for glyph outlines.
pub fn text_stroke(width: f32) -> dl::Stroke {
    dl::Stroke { width, cap: 1, join: 1, miter_limit: 4.0, dash: Vec::new(), dash_offset: 0.0, trim: [0.0, 1.0] }
}

pub(crate) fn rect_op(r: [f32; 4], color: Rgba, transform: Affine) -> Op {
    Op::RoundRect { x: r[0], y: r[1], w: r[2], h: r[3], radius: 0.0, paint: Paint::Solid(color), transform }
}

impl FontStore {
    /// Lay out a text block (cached by content; cheap to call again).
    pub fn layout(&self, spec: &TextSpec) -> Arc<TextLayout> {
        let key = spec.cache_key();
        if let Some(l) = self.cache.read().unwrap().get(&key) {
            return l.clone();
        }
        let mut l = self.layout_uncached(spec);
        l.key = key;
        let l = Arc::new(l);
        let mut c = self.cache.write().unwrap();
        // Bounded: per-frame text (e.g. counters laid out per value) must not grow it forever.
        if c.len() >= 8192 {
            c.clear();
        }
        c.insert(key, l.clone());
        l
    }

    /// Lay out without touching the cache.
    pub fn layout_uncached(&self, spec: &TextSpec) -> TextLayout {
        let spec = &spec.normalized();
        let (text, spans) = spec.joined();
        let width = spec.width.or(spec.fit.as_ref().map(|f| f.width));
        let max_lines = spec.max_lines.map(|m| m.max(1) as usize);
        let base = spec.style.size.max(0.1);
        let mut scale = 1.0;
        if let Some(fit) = &spec.fit {
            let hi = fit.max_size.unwrap_or(base) / base;
            let lo = (fit.min_size / base).min(hi);
            // Shape once at scale 1. Scaling all sizes by `s` and wrapping at `W`
            // breaks lines exactly like scale 1 wrapped at `W / s`, so each probe
            // is a cheap re-break instead of a re-shape.
            let mut l1 = self.parley_layout(spec, &text, &spans, 1.0, None);
            let mut fits = |s: f32| {
                l1.break_all_lines(Some(fit.width / s));
                let lines = l1.len();
                l1.height() * s <= fit.height + 0.01
                    && l1.lines().all(|ln| (ln.metrics().advance - ln.metrics().hanging_advance) * s <= fit.width + 0.01)
                    && max_lines.is_none_or(|m| lines <= m)
            };
            scale = if fits(hi) {
                hi
            } else {
                let (mut a, mut b) = (lo, hi);
                for _ in 0..20 {
                    let m = 0.5 * (a + b);
                    if fits(m) {
                        a = m
                    } else {
                        b = m
                    }
                    if (b - a) * base < 0.05 {
                        break;
                    }
                }
                a
            };
            // Verify at the real size (rounding and hinting are not perfectly linear).
            for _ in 0..4 {
                let l = self.parley_layout(spec, &text, &spans, scale, width);
                let ok = l.height() <= fit.height + 0.5
                    && l.lines().all(|ln| ln.metrics().advance - ln.metrics().hanging_advance <= fit.width + 0.5)
                    && max_lines.is_none_or(|m| l.len() <= m);
                if ok || scale <= lo {
                    break;
                }
                scale = (scale * 0.985).max(lo);
            }
        }
        let mut layout = self.parley_layout(spec, &text, &spans, scale, width);
        let mut text = text;
        let mut truncated = false;
        if let Some(m) = max_lines {
            if layout.len() > m {
                truncated = true;
                let ellipsis = spec.ellipsis.unwrap_or(true);
                let last = layout.get(m - 1);
                let cut_end = last.as_ref().map_or(text.len(), |l| l.text_range().end);
                let cut_start = last.as_ref().map_or(0, |l| l.text_range().start);
                // Candidate cut points: char boundaries on the last kept line.
                let cands: Vec<usize> =
                    text[cut_start..cut_end].char_indices().map(|(i, _)| cut_start + i).chain([cut_end]).collect();
                let make = |k: usize| -> String {
                    let mut t = text[..k].trim_end().to_string();
                    if ellipsis {
                        t.push('…');
                    }
                    t
                };
                let (mut lo, mut hi) = (0usize, cands.len() - 1);
                // Fast path: measure the last line's clusters and the ellipsis once,
                // and cut where both fit; verified by one layout below.
                let avail = width.unwrap_or(f32::MAX);
                if let Some(line) = last.as_ref() {
                    let ell_w = if ellipsis {
                        let sp: Vec<(Range<usize>, &Span)> = spans
                            .iter()
                            .rfind(|(r, _)| r.start < cut_end)
                            .map(|(_, s)| vec![(0..'…'.len_utf8(), *s)])
                            .unwrap_or_default();
                        self.parley_layout(spec, "…", &sp, scale, None).width()
                    } else {
                        0.0
                    };
                    let mut x = 0.0;
                    let mut best = cut_start;
                    for run in line.runs() {
                        for cl in run.visual_clusters() {
                            x += cl.advance();
                            let end = cl.text_range().end;
                            if x + ell_w <= avail && !cl.is_space_or_nbsp() {
                                best = best.max(end);
                            }
                        }
                    }
                    if let Ok(i) = cands.binary_search(&best) {
                        // Bracket the search tightly around the estimate.
                        let t = make(cands[i]);
                        let sp = clip_spans(&spans, t.len(), cands[i]);
                        if self.parley_layout(spec, &t, &sp, scale, width).len() <= m {
                            lo = i;
                            hi = (i + 1).min(cands.len() - 1);
                            let t2 = make(cands[hi]);
                            let sp2 = clip_spans(&spans, t2.len(), cands[hi]);
                            if hi == i || self.parley_layout(spec, &t2, &sp2, scale, width).len() > m {
                                hi = i;
                            } else {
                                lo = hi;
                                hi = cands.len() - 1;
                            }
                        } else {
                            hi = i.saturating_sub(1);
                        }
                    }
                }
                // Largest k whose truncated text fits in m lines.
                while lo < hi {
                    let mid = (lo + hi).div_ceil(2);
                    let t = make(cands[mid]);
                    let sp = clip_spans(&spans, t.len(), cands[mid]);
                    let l = self.parley_layout(spec, &t, &sp, scale, width);
                    if l.len() <= m {
                        lo = mid
                    } else {
                        hi = mid - 1
                    }
                }
                let t = make(cands[lo]);
                let sp = clip_spans(&spans, t.len(), cands[lo]);
                layout = self.parley_layout(spec, &t, &sp, scale, width);
                text = t;
            }
        }
        let mut out = self.extract(&layout, &text);
        out.accent = spec.style.accent.0;
        out.fill = spec.style.gradient.clone().filter(|g| !g.colors.is_empty());
        let em = base * scale;
        if let Some(bg) = &spec.style.background {
            let (px, py, rad) = (bg.padding[0] * em, bg.padding[1] * em, bg.radius * em);
            out.backgrounds = out
                .lines
                .iter()
                .filter(|l| l.rect[2] > 0.0)
                .map(|l| ([l.rect[0] - px, l.rect[1] - py, l.rect[2] + 2.0 * px, l.rect[3] + 2.0 * py], rad, bg.color.0))
                .collect();
        }
        if let Some(sh) = &spec.style.shadow {
            out.filters.push(dl::Filter::Shadow { dx: sh.x, dy: sh.y, blur: sh.blur, color: sh.color.0 });
        }
        if let Some(g) = &spec.style.glow {
            let c = g.color.unwrap_or(spec.style.color).0;
            out.filters.push(dl::Filter::Glow { radius: g.radius, intensity: g.intensity, color: c });
        }
        if let Some(o) = &spec.style.outline {
            let paths = out.runs.iter().map(|r| self.run_path(r, &r.glyphs)).collect();
            out.outline = Some((paths, text_stroke(o.width * base * scale), o.color.map(|c| c.0), o.hollow));
        }
        out.font_size = base * scale;
        out.truncated = truncated;
        out
    }

    /// The outlines of `glyphs` (from `run`) as one path in layout coordinates.
    pub fn run_path(&self, run: &GlyphRun, glyphs: &[dl::Glyph]) -> dl::Path {
        let mut out = dl::Path::new();
        for g in glyphs {
            let p = self.glyph_path(run.font, g.id, run.size);
            let t = kurbo::Affine::translate((g.x as f64, g.y as f64));
            for el in p.elements() {
                out.push(t * *el);
            }
        }
        out
    }

    fn parley_layout(
        &self,
        spec: &TextSpec,
        text: &str,
        spans: &[(Range<usize>, &Span)],
        scale: f32,
        width: Option<f32>,
    ) -> Layout<Rgba> {
        let mut guard = self.shaper.lock().unwrap();
        let sh = &mut *guard;
        let st = &spec.style;
        let fam_base = family_css(sh, &st.font);
        let fams: Vec<(String, bool)> = spans
            .iter()
            .map(|(_, s)| s.font.as_deref().map(|f| family_css(sh, f)).unwrap_or_else(|| fam_base.clone()))
            .collect();
        let mut b = sh.lcx.ranged_builder(&mut sh.fcx, text, 1.0, true);
        let size = st.size * scale;
        b.push_default(StyleProperty::FontFamily(FontFamily::Source(Cow::Borrowed(&fam_base.0))));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(StyleProperty::FontWeight(FontWeight::new(st.weight)));
        if st.italic {
            b.push_default(StyleProperty::FontStyle(FontStyle::Italic));
        }
        b.push_default(StyleProperty::Brush(st.color.0));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(st.line_height)));
        if st.letter_spacing != 0.0 {
            b.push_default(StyleProperty::LetterSpacing(st.letter_spacing * size));
        }
        let base_vars = variations(&fam_base, &st.axes, size);
        b.push_default(StyleProperty::FontVariations(FontVariations::List(Cow::Borrowed(&base_vars))));
        if st.underline {
            b.push_default(StyleProperty::Underline(true));
        }
        if st.strikethrough {
            b.push_default(StyleProperty::Strikethrough(true));
        }
        if let Some(f) = &st.features {
            b.push_default(StyleProperty::FontFeatures(FontFeatures::Source(Cow::Borrowed(f))));
        }
        for ((r, s), fam) in spans.iter().zip(&fams) {
            let r = r.clone();
            if r.is_empty() {
                continue;
            }
            let ssize = s.size.map(|v| v * scale).unwrap_or(size);
            if s.font.is_some() || s.size.is_some() {
                if s.font.is_some() {
                    b.push(StyleProperty::FontFamily(FontFamily::Source(Cow::Borrowed(&fam.0))), r.clone());
                }
                b.push(StyleProperty::FontVariations(FontVariations::List(Cow::Owned(variations(fam, &st.axes, ssize)))), r.clone());
            }
            if s.size.is_some() {
                b.push(StyleProperty::FontSize(ssize), r.clone());
            }
            if let Some(w) = s.weight {
                b.push(StyleProperty::FontWeight(FontWeight::new(w)), r.clone());
            }
            if let Some(i) = s.italic {
                b.push(StyleProperty::FontStyle(if i { FontStyle::Italic } else { FontStyle::Normal }), r.clone());
            }
            if let Some(c) = s.color {
                b.push(StyleProperty::Brush(c.0), r.clone());
            }
            if let Some(ls) = s.letter_spacing {
                b.push(StyleProperty::LetterSpacing(ls * ssize), r.clone());
            } else if s.size.is_some() && st.letter_spacing != 0.0 {
                b.push(StyleProperty::LetterSpacing(st.letter_spacing * ssize), r.clone());
            }
            if let Some(u) = s.underline {
                b.push(StyleProperty::Underline(u), r.clone());
            }
            if let Some(u) = s.strikethrough {
                b.push(StyleProperty::Strikethrough(u), r.clone());
            }
            if let Some(f) = &s.features {
                b.push(StyleProperty::FontFeatures(FontFeatures::Source(Cow::Borrowed(f))), r.clone());
            }
        }
        let mut layout: Layout<Rgba> = b.build(text);
        let align = match spec.align {
            Align::Left => Alignment::Left,
            Align::Center => Alignment::Center,
            Align::Right => Alignment::Right,
            Align::Justify => Alignment::Justify,
        };
        layout.set_alignment(align, AlignmentOptions { align_when_overflowing: true, ..Default::default() });
        layout.break_all_lines(width);
        layout
    }

    fn extract(&self, layout: &Layout<Rgba>, text: &str) -> TextLayout {
        let mut out = TextLayout { text: text.to_string(), ..Default::default() };
        // Interned face per parley run, memoised by (blob, index, coords ptr).
        let mut last_face: Option<(u64, u32, usize, FontId)> = None;
        // One run per (face, size, colour) across the whole block: syntax-highlighted
        // code alternates colours, and merging cuts its ops ~20x.
        let mut run_of: std::collections::HashMap<(u32, u32, [u32; 4]), usize> = std::collections::HashMap::new();
        for (li, line) in layout.lines().enumerate() {
            let m = *line.metrics();
            let li = li as u32;
            let line_g0 = out.glyphs.len() as u32;
            let mut pen = m.offset + m.inline_min_coord;
            let mut word_open = false;
            let mut line_text = Range { start: usize::MAX, end: 0 };
            for run in line.runs() {
                let fd = run.font();
                let coords = run.normalized_coords();
                let syn = run.synthesis();
                let fid = match last_face {
                    Some((b, i, p, f)) if b == fd.data.id() && i == fd.index && p == coords.as_ptr() as usize => f,
                    _ => {
                        let c: Vec<i16> = coords.iter().map(|c| c.to_bits()).collect();
                        let f = self.intern(fd, &c, syn.embolden(), syn.skew());
                        last_face = Some((fd.data.id(), fd.index, coords.as_ptr() as usize, f));
                        f
                    }
                };
                let size = run.font_size();
                let fm = *run.font_metrics();
                if li == 0 && out.cap_top == 0.0 {
                    out.cap_top = m.baseline - fm.cap_height.filter(|c| *c > 0.0).unwrap_or(size * 0.7);
                }
                let top = m.baseline - fm.ascent;
                let h = fm.ascent + fm.descent;
                for cl in run.visual_clusters() {
                    let adv = cl.advance();
                    let tr = cl.text_range();
                    let space = cl.is_space_or_nbsp() || cl.is_hard_line_break();
                    let style = cl.style();
                    if !space {
                        line_text.start = line_text.start.min(tr.start);
                        line_text.end = line_text.end.max(tr.end);
                        let color = style.brush;
                        let ci = out.chars.len() as u32;
                        if !word_open {
                            out.words.push(Unit {
                                rect: [pen, top, 0.0, h],
                                baseline: m.baseline,
                                glyphs: out.glyphs.len() as u32..out.glyphs.len() as u32,
                                text: tr.clone(),
                            });
                            word_open = true;
                        }
                        let wi = out.words.len() as u32 - 1;
                        let g0 = out.glyphs.len() as u32;
                        let mut gx = pen;
                        for g in cl.glyphs() {
                            let key = (fid.0, size.to_bits(), color.0.map(f32::to_bits));
                            let run_i = match out.runs.last() {
                                Some(r) if r.font == fid && r.size == size && r.color == color => out.runs.len() - 1,
                                _ => *run_of.entry(key).or_insert_with(|| {
                                    out.runs.push(GlyphRun { font: fid, size, color, glyphs: Vec::new() });
                                    out.runs.len() - 1
                                }),
                            };
                            let r = &mut out.runs[run_i];
                            r.glyphs.push(dl::Glyph { id: g.id as u16, x: gx + g.x, y: m.baseline + g.y });
                            out.glyphs.push(GlyphInfo {
                                run: run_i as u32,
                                index: r.glyphs.len() as u32 - 1,
                                char: ci,
                                word: wi,
                                line: li,
                            });
                            gx += g.advance;
                        }
                        let g1 = out.glyphs.len() as u32;
                        out.chars.push(Unit { rect: [pen, top, adv, h], baseline: m.baseline, glyphs: g0..g1, text: tr.clone() });
                        let w = out.words.last_mut().unwrap();
                        let x1 = (pen + adv).max(w.rect[0] + w.rect[2]);
                        let y0 = w.rect[1].min(top);
                        let y1 = (w.rect[1] + w.rect[3]).max(top + h);
                        w.rect = [w.rect[0].min(pen), y0, x1 - w.rect[0].min(pen), y1 - y0];
                        w.glyphs.end = g1;
                        w.text.start = w.text.start.min(tr.start);
                        w.text.end = w.text.end.max(tr.end);
                    } else {
                        word_open = false;
                    }
                    for (dec, is_under) in [(&style.underline, true), (&style.strikethrough, false)] {
                        let Some(d) = dec else { continue };
                        let (off, sz) = if is_under {
                            (d.offset.unwrap_or(fm.underline_offset), d.size.unwrap_or(fm.underline_size))
                        } else {
                            (d.offset.unwrap_or(fm.strikethrough_offset), d.size.unwrap_or(fm.strikethrough_size))
                        };
                        let y = m.baseline - off;
                        let color = d.brush;
                        match out.decorations.last_mut() {
                            Some(p) if p.line == li && p.color == color && (p.rect[1] - y).abs() < 0.01
                                && (p.rect[0] + p.rect[2] - pen).abs() < 0.5 =>
                            {
                                p.rect[2] = pen + adv - p.rect[0];
                            }
                            _ => out.decorations.push(Decoration { rect: [pen, y, adv, sz.max(1.0)], color, line: li }),
                        }
                    }
                    pen += adv;
                }
            }
            let g1 = out.glyphs.len() as u32;
            if line_text.start == usize::MAX {
                line_text = line.text_range().start..line.text_range().start;
            }
            out.last_baseline = m.baseline;
            out.lines.push(Unit {
                rect: [m.offset + m.inline_min_coord, m.block_min_coord, m.advance - m.hanging_advance, m.block_max_coord - m.block_min_coord],
                baseline: m.baseline,
                glyphs: line_g0..g1,
                text: line_text,
            });
        }
        // Trailing decorations over spaces at line ends look wrong; trim to the line's ink.
        for d in &mut out.decorations {
            let l = &out.lines[d.line as usize];
            let end = l.rect[0] + l.rect[2];
            if d.rect[0] + d.rect[2] > end {
                d.rect[2] = (end - d.rect[0]).max(0.0);
            }
        }
        out.decorations.retain(|d| d.rect[2] > 0.0);
        out.width = layout.width();
        out.height = layout.height();
        out
    }
}

/// Typesetting pass for display text: curly quotes and apostrophes, `...` → …,
/// ` -- ` → en dash, `---` → em dash, and a no-break space before the last
/// word of each paragraph of 4+ words (no one-word last line).
pub fn typeset(src: &str) -> String {
    let src = src.replace("...", "\u{2026}").replace("---", "\u{2014}").replace(" -- ", " \u{2013} ");
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        let opens = prev.is_none_or(|p| p.is_whitespace() || "([{\u{2014}\u{2013}-/".contains(p));
        match c {
            '"' => out.push(if opens { '\u{201C}' } else { '\u{201D}' }),
            '\'' => {
                let next_digit = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
                // ’90s and rock ’n’ roll: an apostrophe, not an opening quote.
                out.push(if opens && !next_digit && chars.get(i + 1).is_some_and(|n| *n != 'n') { '\u{2018}' } else { '\u{2019}' });
            }
            _ => out.push(c),
        }
    }
    out.split('\n')
        .map(|para| {
            let words = para.split(' ').filter(|w| !w.is_empty()).count();
            match para.trim_end().rfind(' ') {
                Some(k) if words >= 4 && para.trim_end().len() - k <= 12 => format!("{}\u{00A0}{}", &para[..k], &para[k + 1..]),
                _ => para.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// OpenType features from a friendly list (`"ss01 tnum -liga"`, commas
/// optional) or CSS (`"\"ss01\" on"`, passed through) to CSS syntax.
pub fn features_css(src: &str) -> String {
    if src.contains('"') || src.contains('\'') {
        return src.to_string();
    }
    src.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| match t.strip_prefix('-') {
            Some(t) => format!("\"{t}\" 0"),
            None => match t.split_once('=') {
                Some((t, v)) => format!("\"{t}\" {v}"),
                None => format!("\"{t}\" 1"),
            },
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Variation settings for a family list at `size` px: the explicit axes,
/// plus an optical size (Inter Display = opsz 32; other faces: opsz = size).
fn variations(fam: &(String, bool), axes: &[(String, f32)], size: f32) -> Vec<parley::FontVariation> {
    use std::fmt::Write;
    let mut css = String::new();
    for (t, x) in axes {
        if t.len() == 4 && t.bytes().all(|b| b.is_ascii_alphanumeric()) {
            let _ = write!(css, "\"{t}\" {x},");
        }
    }
    if !axes.iter().any(|(t, _)| t == "opsz") {
        if fam.1 {
            css.push_str("\"opsz\" 32");
        } else if !(fam.0.starts_with("\"Inter") || fam.0.starts_with("sans-serif") || fam.0.starts_with("system-ui")) {
            let _ = write!(css, "\"opsz\" {}", size * 0.75);
        }
    }
    parley::FontVariation::parse_css_list(&css).filter_map(Result::ok).collect()
}

/// Resolve a user family list to CSS (with emoji fallback) and whether it asks
/// for Inter Display (Inter at optical size 32).
fn family_css(sh: &mut super::fonts::Shaper, names: &str) -> (String, bool) {
    let mut out = Vec::new();
    let mut display = false;
    for n in names.split(',') {
        let n = n.trim().trim_matches(|c| c == '"' || c == '\'');
        if n.is_empty() {
            continue;
        }
        let lower = n.to_lowercase();
        if lower == "inter display" && out.is_empty() {
            display = true;
        }
        match lower.as_str() {
            "serif" | "sans-serif" | "monospace" | "system-ui" | "cursive" | "fantasy" => out.push(lower.clone()),
            _ => {
                if !sh.families.contains_key(&lower) {
                    // A library family the compile pre-pass didn't see: fetch it now.
                    let _ = super::fonts::load_library_locked(sh, n);
                }
                match sh.families.get(&lower) {
                    Some(f) => out.push(format!("\"{f}\"")),
                    // Unknown families are dropped so the default sans takes over.
                    None if sh.fcx.collection.family_id(n).is_some() => out.push(format!("\"{n}\"")),
                    None => {}
                }
            }
        }
    }
    if out.is_empty() {
        out.push("sans-serif".into());
    }
    if let Some(e) = &sh.emoji {
        out.push(format!("\"{e}\""));
    }
    for f in &sh.fallbacks {
        out.push(format!("\"{f}\""));
    }
    // Per-glyph fallback: characters the chosen face lacks (₂, →, ½…) come from Inter, not tofu.
    if out.first().is_none_or(|f| f != "sans-serif" && !f.starts_with("\"Inter")) {
        out.push("sans-serif".into());
    }
    (out.join(", "), display)
}

/// Clip span ranges to a truncated text: bytes before `cut` keep their span;
/// the appended ellipsis inherits the last kept span.
fn clip_spans<'a>(spans: &[(Range<usize>, &'a Span)], new_len: usize, cut: usize) -> Vec<(Range<usize>, &'a Span)> {
    let mut v: Vec<(Range<usize>, &Span)> = Vec::new();
    for (r, s) in spans {
        if r.start >= cut {
            break;
        }
        v.push((r.start..r.end.min(cut), s));
    }
    if let Some(last) = v.last_mut() {
        last.0.end = new_len;
    }
    v
}

/// A parsed CSS `font` shorthand.
#[derive(Debug, PartialEq)]
pub struct FontShorthand {
    pub family: String,
    pub weight: Option<f32>,
    pub size: Option<f32>,
    pub line_height: Option<f32>,
    pub italic: bool,
}

/// Parse `[italic] [weight] [size[/line-height]] family` (each part optional;
/// weights may be numbers or names like `bold`, `semibold`, `black`).
/// Returns `None` when the string is just a family name.
pub fn parse_font(s: &str) -> Option<FontShorthand> {
    let mut out = FontShorthand { family: String::new(), weight: None, size: None, line_height: None, italic: false };
    let mut fam: Vec<&str> = Vec::new();
    let mut any = false;
    for t in s.split_whitespace() {
        if !fam.is_empty() {
            fam.push(t);
            continue;
        }
        let l = t.to_lowercase();
        let w = match l.as_str() {
            "thin" | "hairline" => Some(100.0),
            "extralight" | "ultralight" => Some(200.0),
            "light" => Some(300.0),
            "regular" | "normal" | "book" => Some(400.0),
            "medium" => Some(500.0),
            "semibold" | "demibold" => Some(600.0),
            "bold" => Some(700.0),
            "extrabold" | "ultrabold" | "heavy" => Some(800.0),
            "black" => Some(900.0),
            _ => None,
        };
        if l == "italic" || l == "oblique" {
            out.italic = true;
        } else if let Some(w) = w {
            out.weight = Some(w);
        } else if let Ok(n) = l.parse::<f32>() {
            out.weight = Some(n);
        } else if l.chars().next().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            let (sz, lh) = l.split_once('/').map_or((l.as_str(), None), |(a, b)| (a, Some(b)));
            out.size = Some(sz.trim_end_matches("px").parse().ok()?);
            if let Some(lh) = lh {
                out.line_height = Some(lh.parse().ok()?);
            }
        } else {
            fam.push(t);
            continue;
        }
        any = true;
    }
    if !any {
        return None;
    }
    out.family = if fam.is_empty() { "Inter".into() } else { fam.join(" ") };
    Some(out)
}

fn has_markup(t: &str) -> bool {
    t.contains("**") || t.contains("==") || t.contains('`') || t.contains('*') || t.contains('\\')
}

/// Split one span on inline markup into styled spans.
fn markup_spans(base: &Span, style: &TextStyle) -> Vec<Span> {
    const MARKS: [&str; 4] = ["**", "==", "`", "*"];
    let t = &base.text;
    let mut out: Vec<Span> = Vec::new();
    let mut cur = String::new();
    // Active flags: bold, accent, code, italic.
    let mut on = [false; 4];
    let flush = |cur: &mut String, on: &[bool; 4], out: &mut Vec<Span>| {
        if cur.is_empty() {
            return;
        }
        let mut sp = Span { text: std::mem::take(cur), ..base.clone() };
        if on[0] {
            sp.weight = Some((sp.weight.unwrap_or(style.weight) + 300.0).clamp(700.0, 900.0));
        }
        if on[1] {
            sp.color = Some(style.accent);
        }
        if on[2] {
            sp.font = Some("JetBrains Mono".into());
        }
        if on[3] {
            sp.italic = Some(true);
        }
        out.push(sp);
    };
    let mut i = 0;
    let b = t.as_bytes();
    while i < t.len() {
        if b[i] == b'\\' && i + 1 < t.len() {
            let c = t[i + 1..].chars().next().unwrap();
            cur.push(c);
            i += 1 + c.len_utf8();
            continue;
        }
        let mut matched = false;
        for (k, m) in MARKS.iter().enumerate() {
            if !t[i..].starts_with(m) || (on[2] && k != 2) {
                continue;
            }
            // `*` must not be the first half of `**`.
            if k == 3 && t[i..].starts_with("**") {
                continue;
            }
            if on[k] {
                flush(&mut cur, &on, &mut out);
                on[k] = false;
                i += m.len();
                matched = true;
                break;
            }
            // Opening: only if a closing marker follows (and content is non-empty).
            let rest = &t[i + m.len()..];
            let close = if k == 3 {
                rest.char_indices().find(|(j, c)| *c == '*' && !rest[*j..].starts_with("**") && (*j == 0 || !rest[..*j].ends_with('*'))).map(|x| x.0)
            } else {
                rest.find(m)
            };
            if let Some(j) = close {
                if j > 0 && !rest.starts_with(' ') {
                    flush(&mut cur, &on, &mut out);
                    on[k] = true;
                    i += m.len();
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            let c = t[i..].chars().next().unwrap();
            cur.push(c);
            i += c.len_utf8();
        }
    }
    flush(&mut cur, &on, &mut out);
    out
}

#[cfg(test)]
mod typeset_tests {
    #[test]
    fn quotes_dashes_widows() {
        assert_eq!(super::typeset("\"Hi,\" she said. It's 'fine'..."), "\u{201C}Hi,\u{201D} she said. It\u{2019}s \u{2018}fine\u{2019}\u{2026}");
        assert_eq!(super::typeset("in '90s -- ok"), "in \u{2019}90s \u{2013}\u{00A0}ok");
        assert_eq!(super::typeset("one two three four"), "one two three\u{00A0}four");
        assert_eq!(super::typeset("two words"), "two words");
    }
}
