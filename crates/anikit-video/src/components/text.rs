//! Text styles used by components, and an in-memory [`Assets`] store.
//!
//! Components never shape text themselves: they describe a [`TextStyle`] and
//! ask [`Assets::text`]. Until the TEXT module is wired in, [`StubAssets`]
//! "greeks" text as rounded bars with realistic advances, so layouts and
//! previews are already right.

use super::Assets;
use crate::dl::{ImageId, Op, Rgba};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Which bundled family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    /// Inter: UI text.
    #[default]
    Sans,
    /// Inter Display: big titles and numbers.
    Display,
    /// JetBrains Mono: code.
    Mono,
    /// Instrument Serif: editorial accents, quotes.
    Serif,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub face: Face,
    pub size: f32,
    /// 100..900.
    pub weight: u16,
    pub color: Rgba,
    /// Extra spacing between letters, in em (negative tightens titles).
    pub tracking: f32,
    /// Tabular (fixed-width) digits, for counters.
    pub tabular: bool,
    /// Linear gradient fill `(from, to, p0, p1)` with `p0`/`p1` in the same
    /// coordinates as the draw position (overrides `color`, keeps its alpha).
    pub gradient: Option<(Rgba, Rgba, [f32; 2], [f32; 2])>,
}

impl TextStyle {
    pub fn new(face: Face, size: f32, weight: u16, color: Rgba) -> TextStyle {
        TextStyle { face, size, weight, color, tracking: 0.0, tabular: false, gradient: None }
    }
    pub fn tracking(mut self, em: f32) -> TextStyle {
        self.tracking = em;
        self
    }
    pub fn tabular(mut self) -> TextStyle {
        self.tabular = true;
        self
    }
    pub fn gradient(mut self, a: Rgba, b: Rgba, p0: [f32; 2], p1: [f32; 2]) -> TextStyle {
        self.gradient = Some((a, b, p0, p1));
        self
    }
    /// The paint for a run drawn with local origin `(ox, oy)`.
    pub fn paint_at(&self, ox: f32, oy: f32) -> crate::dl::Paint {
        match self.gradient {
            Some((a, b, p0, p1)) => {
                let k = self.color.0[3];
                let al = |c: Rgba| Rgba([c.0[0], c.0[1], c.0[2], c.0[3] * k]);
                crate::dl::Paint::Linear { p0: [p0[0] - ox, p0[1] - oy], p1: [p1[0] - ox, p1[1] - oy], stops: vec![(0.0, al(a)), (1.0, al(b))] }
            }
            None => crate::dl::Paint::Solid(self.color),
        }
    }
    pub fn color(mut self, c: Rgba) -> TextStyle {
        self.color = c;
        self
    }
}

/// Approximate advance of one char in em, close to Inter / JetBrains Mono.
pub fn approx_advance(c: char, face: Face, weight: u16, tabular: bool) -> f32 {
    if face == Face::Mono {
        return 0.6;
    }
    let w = match c {
        ' ' => 0.26,
        'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' => 0.24,
        'f' | 't' | 'r' | 'I' | '(' | ')' | '[' | ']' => 0.34,
        'm' | 'w' => 0.82,
        'M' | 'W' => 0.9,
        '0'..='9' if tabular => 0.62,
        '0'..='9' => 0.58,
        c if c.is_uppercase() => 0.68,
        _ => 0.55,
    };
    let bold = (weight as f32 - 400.0) / 500.0 * 0.05;
    let display = if face == Face::Display { -0.02 } else { 0.0 };
    w + bold + display
}

/// Width of `s` under the approximate metrics.
pub fn approx_width(s: &str, st: &TextStyle) -> f32 {
    s.chars().map(|c| approx_advance(c, st.face, st.weight, st.tabular) + st.tracking).sum::<f32>() * st.size
}

/// One stored image.
pub struct StoredImage {
    pub w: u32,
    pub h: u32,
    /// Straight-alpha RGBA8.
    pub rgba: Vec<u8>,
}

/// In-memory asset store: content-keyed images plus approximate text.
/// Thread-safe; used by tests and as a reference for the pipeline.
#[derive(Default)]
pub struct StubAssets {
    by_key: RwLock<HashMap<u64, ImageId>>,
    images: RwLock<Vec<Arc<StoredImage>>>,
    media: RwLock<HashMap<String, ImageId>>,
}

impl StubAssets {
    pub fn get(&self, id: ImageId) -> Option<Arc<StoredImage>> {
        self.images.read().unwrap().get(id.0 as usize).cloned()
    }
    /// Register a user image under a `src` name.
    pub fn add_media(&self, src: &str, w: u32, h: u32, rgba: Vec<u8>) -> ImageId {
        let id = self.push(w, h, rgba);
        self.media.write().unwrap().insert(src.to_string(), id);
        id
    }
    fn push(&self, w: u32, h: u32, rgba: Vec<u8>) -> ImageId {
        let mut v = self.images.write().unwrap();
        v.push(Arc::new(StoredImage { w, h, rgba }));
        ImageId(v.len() as u32 - 1)
    }
    pub fn image_count(&self) -> usize {
        self.images.read().unwrap().len()
    }
}

impl Assets for StubAssets {
    fn image(&self, key: u64, w: u32, h: u32, make: &mut dyn FnMut() -> Vec<u8>) -> ImageId {
        if let Some(id) = self.by_key.read().unwrap().get(&key) {
            return *id;
        }
        let px = make();
        debug_assert_eq!(px.len(), (w * h * 4) as usize);
        let id = self.push(w, h, px);
        *self.by_key.write().unwrap().entry(key).or_insert(id)
    }

    fn media(&self, src: &str, _t: f32) -> Option<(ImageId, u32, u32)> {
        let id = *self.media.read().unwrap().get(src)?;
        let im = self.get(id)?;
        Some((id, im.w, im.h))
    }

    fn text(&self, s: &str, st: &TextStyle, x: f32, y: f32) -> (Vec<Op>, f32) {
        // Greeked text: one rounded bar per word at x-height.
        let mut ops = Vec::new();
        let mut cx = x;
        let xh = st.size * if st.face == Face::Mono { 0.55 } else { 0.52 };
        let mut word_start: Option<f32> = None;
        let flush = |ops: &mut Vec<Op>, from: f32, to: f32| {
            if to > from {
                ops.push(Op::RoundRect { x: from, y: y - xh, w: to - from, h: xh, radius: xh * 0.3, paint: st.paint_at(0.0, 0.0), transform: crate::dl::IDENTITY });
            }
        };
        for c in s.chars() {
            let adv = (approx_advance(c, st.face, st.weight, st.tabular) + st.tracking) * st.size;
            if c.is_whitespace() {
                if let Some(ws) = word_start.take() {
                    flush(&mut ops, ws, cx);
                }
            } else if word_start.is_none() {
                word_start = Some(cx);
            }
            cx += adv;
        }
        if let Some(ws) = word_start {
            flush(&mut ops, ws, cx - st.tracking * st.size);
        }
        (ops, cx - x)
    }

    fn measure(&self, s: &str, st: &TextStyle) -> f32 {
        approx_width(s, st)
    }
}

/// Draw `s` aligned at `x` (`align` 0 left, 0.5 centre, 1 right) on baseline `y`.
pub fn draw(out: &mut Vec<Op>, a: &dyn Assets, s: &str, st: &TextStyle, x: f32, y: f32, align: f32) -> f32 {
    if s.is_empty() || st.color.0[3] <= 0.0 {
        return 0.0;
    }
    let w = if align != 0.0 { a.measure(s, st) } else { 0.0 };
    let (ops, adv) = a.text(s, st, x - w * align, y);
    out.extend(ops);
    adv
}

/// Greedy word wrap of `s` into lines no wider than `max_w`.
pub fn wrap(a: &dyn Assets, s: &str, st: &TextStyle, max_w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut cur = String::new();
        for word in para.split_whitespace() {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if !cur.is_empty() && a.measure(&cand, st) > max_w {
                lines.push(std::mem::replace(&mut cur, word.to_string()));
            } else {
                cur = cand;
            }
        }
        lines.push(cur);
    }
    lines
}
