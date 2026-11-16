//! Word-synced captions: parse SRT/WebVTT (including VTT word timestamps) or
//! plain text with times, estimate missing word timings, group words into
//! short "pages" (TikTok style) and draw them with a per-word pop and
//! highlight. Built once at preprocessing; `ops(t)` per frame.
//!
//! ```ignore
//! let cues = captions::parse(&std::fs::read_to_string("talk.srt")?);
//! let track = CaptionTrack::new(&fonts, &cues, &CaptionStyle::default());
//! track.ops(t, transform_to_bottom_centre, &mut ops);
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::color::Color;
use super::fonts::FontStore;
use super::layout::{Align, TextLayout, TextSpec, TextStyle};
use crate::dl::{self, Affine, Op, Paint, Rgba};

/// One timed word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

/// One subtitle cue; `words` is filled by parsing (VTT word timestamps) or estimation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub text: String,
    pub start: f32,
    pub end: f32,
    #[serde(default)]
    pub words: Vec<Word>,
}

fn timestamp(s: &str) -> Option<f32> {
    let s = s.trim().replace(',', ".");
    let p: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match p.as_slice() {
        [h, m, s] => (h.parse::<f32>().ok()?, m.parse::<f32>().ok()?, s.parse::<f32>().ok()?),
        [m, s] => (0.0, m.parse::<f32>().ok()?, s.parse::<f32>().ok()?),
        [s] => (0.0, 0.0, s.parse::<f32>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// Parse SRT or WebVTT (auto-detected). VTT inline `<00:00:01.500>` word
/// timestamps become word timings; other tags (`<c>`, `<b>`, `<v Name>`) are dropped.
pub fn parse(src: &str) -> Vec<Cue> {
    let src = src.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    for block in src.split("\n\n") {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        let Some(k) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let mut it = lines[k].split("-->");
        let a = it.next().and_then(timestamp);
        // VTT cue settings follow the end time.
        let b = it.next().and_then(|x| timestamp(x.split_whitespace().next().unwrap_or("")));
        let (Some(a), Some(b)) = (a, b) else { continue };
        let raw = lines[k + 1..].join("\n");
        let (text, marks) = strip_tags(&raw);
        let mut cue = Cue { text, start: a, end: b.max(a), words: Vec::new() };
        if !marks.is_empty() {
            cue.words = words_from_marks(&cue, &marks);
        }
        out.push(cue);
    }
    out
}

/// Remove tags; return the text and `(byte offset in text, time)` for each timestamp tag.
fn strip_tags(s: &str) -> (String, Vec<(usize, f32)>) {
    let mut text = String::with_capacity(s.len());
    let mut marks = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        text.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else {
            text.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let tag = &rest[i + 1..i + j];
        if let Some(t) = timestamp(tag) {
            marks.push((text.len(), t));
        }
        rest = &rest[i + j + 1..];
    }
    text.push_str(rest);
    (text.trim().to_string(), marks)
}

fn words_from_marks(cue: &Cue, marks: &[(usize, f32)]) -> Vec<Word> {
    let mut words = Vec::new();
    let mut off = 0;
    for w in cue.text.split_whitespace() {
        let pos = cue.text[off..].find(w).map_or(off, |p| off + p);
        off = pos + w.len();
        // The latest mark at or before this word's start, else the cue start.
        let start = marks.iter().filter(|(o, _)| *o <= pos).map(|m| m.1).fold(cue.start, f32::max);
        words.push(Word { text: w.to_string(), start, end: cue.end });
    }
    for i in 0..words.len().saturating_sub(1) {
        words[i].end = words[i + 1].start.max(words[i].start);
    }
    words
}

/// Cues from plain text and per-word start times (e.g. from speech recognition).
pub fn from_words(text: &str, times: &[f32], end: f32) -> Vec<Cue> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<Word> = toks
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let start = times.get(i).copied().unwrap_or_else(|| times.last().copied().unwrap_or(0.0) + 0.3 * (i + 1 - times.len()) as f32);
            Word { text: w.to_string(), start, end: start }
        })
        .collect();
    let mut cue = Cue { text: text.to_string(), start: words.first().map_or(0.0, |w| w.start), end, words };
    for i in 0..cue.words.len() {
        cue.words[i].end = cue.words.get(i + 1).map_or(end, |n| n.start);
    }
    vec![cue]
}

/// Fill missing word timings by spreading each cue's duration over its words,
/// weighted by length (a cheap proxy for syllables).
pub fn estimate_words(cues: &mut [Cue]) {
    for c in cues {
        if !c.words.is_empty() {
            continue;
        }
        let toks: Vec<&str> = c.text.split_whitespace().collect();
        let weights: Vec<f32> = toks.iter().map(|w| w.chars().filter(|c| c.is_alphanumeric()).count() as f32 + 2.0).collect();
        let total: f32 = weights.iter().sum::<f32>().max(1.0);
        let dur = (c.end - c.start).max(0.0);
        let mut t = c.start;
        for (w, k) in toks.iter().zip(&weights) {
            let d = dur * k / total;
            c.words.push(Word { text: w.to_string(), start: t, end: t + d });
            t += d;
        }
    }
}

/// How words not yet spoken look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Upcoming {
    /// Visible at reduced opacity.
    #[default]
    Dim,
    /// Hidden until spoken (words build up).
    Hidden,
    /// Fully visible.
    Show,
}

/// Caption look and paging.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CaptionStyle {
    pub style: TextStyle,
    /// Wrapping width in px.
    pub width: f32,
    /// Words per page (1 = one word at a time).
    pub max_words: usize,
    pub max_lines: u32,
    /// Colour of the word being spoken.
    pub highlight: Option<Color>,
    /// Box behind the word being spoken (TikTok style).
    pub highlight_box: Option<Color>,
    pub upcoming: Upcoming,
    /// Extra scale on the word being spoken as it starts (0 = none).
    pub pop: f32,
    pub uppercase: bool,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        CaptionStyle {
            style: TextStyle {
                font: "Inter Display".into(),
                weight: 800.0,
                size: 64.0,
                line_height: 1.15,
                shadow: Some(super::layout::Shadow { x: 0.0, y: 3.0, blur: 10.0, color: Color::rgba(0.0, 0.0, 0.0, 0.75) }),
                ..Default::default()
            },
            width: 900.0,
            max_words: 4,
            max_lines: 2,
            highlight: Some(Color::rgba(1.0, 0.82, 0.4, 1.0)),
            highlight_box: None,
            upcoming: Upcoming::Dim,
            pop: 0.12,
            uppercase: false,
        }
    }
}

struct Page {
    start: f32,
    end: f32,
    layout: Arc<TextLayout>,
    /// Word timings, one per layout word unit.
    words: Vec<(f32, f32)>,
}

/// Captions ready to draw. `(0, 0)` of the transform is the bottom centre of
/// the caption block (pages grow upwards).
pub struct CaptionTrack {
    pages: Vec<Page>,
    st: CaptionStyle,
}

impl CaptionTrack {
    pub fn new(fonts: &FontStore, cues: &[Cue], st: &CaptionStyle) -> Self {
        let mut cues = cues.to_vec();
        estimate_words(&mut cues);
        let mut pages = Vec::new();
        for c in &cues {
            let mut cur: Vec<&Word> = Vec::new();
            let flush = |cur: &mut Vec<&Word>, pages: &mut Vec<Page>| {
                if cur.is_empty() {
                    return;
                }
                let mut text = cur.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
                if st.uppercase {
                    text = text.to_uppercase();
                }
                let spec = TextSpec {
                    text,
                    style: st.style.clone(),
                    width: Some(st.width),
                    align: Align::Center,
                    max_lines: Some(st.max_lines.max(1)),
                    markup: Some(false),
                    ..Default::default()
                };
                let layout = fonts.layout(&spec);
                // Timings are in reading order; layout words are visual (RTL differs).
                let mut words = vec![(c.end, c.end); layout.words.len()];
                for (rank, ui) in layout.logical_order(super::layout::UnitKind::Word).into_iter().enumerate() {
                    if let Some(w) = cur.get(rank) {
                        words[ui] = (w.start, w.end);
                    }
                }
                pages.push(Page { start: cur[0].start, end: cur.last().unwrap().end, layout, words });
                cur.clear();
            };
            for (i, w) in c.words.iter().enumerate() {
                let gap = i > 0 && w.start - c.words[i - 1].end > 0.6;
                let sentence = i > 0 && c.words[i - 1].text.ends_with(['.', '!', '?']) && cur.len() >= 2;
                if cur.len() >= st.max_words.max(1) || gap || sentence {
                    flush(&mut cur, &mut pages);
                }
                cur.push(w);
            }
            flush(&mut cur, &mut pages);
        }
        // A page stays up until the next starts (bridging short pauses) or 0.4 s after its last word.
        for i in 0..pages.len() {
            let next = pages.get(i + 1).map_or(f32::INFINITY, |p| p.start);
            pages[i].end = if next - pages[i].end < 0.8 { next } else { pages[i].end + 0.4 };
        }
        CaptionTrack { pages, st: st.clone() }
    }

    /// Number of pages (for tests and critique).
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Text of the page shown at `t`, if any.
    pub fn page_text(&self, t: f32) -> Option<&str> {
        self.page_at(t).map(|p| p.layout.text.as_str())
    }

    fn page_at(&self, t: f32) -> Option<&Page> {
        let i = self.pages.partition_point(|p| p.start <= t);
        let p = self.pages.get(i.checked_sub(1)?)?;
        (t < p.end).then_some(p)
    }

    pub fn ops(&self, t: f32, transform: Affine, out: &mut Vec<Op>) {
        let Some(p) = self.page_at(t) else { return };
        let l = &*p.layout;
        // Bottom-centre anchor.
        let b = l.bounds();
        let base = dl::mul(&transform, &[1.0, 0.0, 0.0, 1.0, -(b[0] + b[2] * 0.5), -l.height]);
        // Page entrance: quick fade and lift.
        let enter = ((t - p.start) / 0.12).clamp(0.0, 1.0);
        let base = dl::mul(&base, &[1.0, 0.0, 0.0, 1.0, 0.0, (1.0 - enter) * l.font_size * 0.15]);
        l.begin_effects(base, out);
        let em = l.font_size;
        for (i, u) in l.words.iter().enumerate() {
            let (ws, we) = p.words[i];
            let active = t >= ws && t < we.max(ws + 0.05);
            let spoken = t >= ws;
            let alpha = if spoken {
                1.0
            } else {
                match self.st.upcoming {
                    Upcoming::Dim => 0.5,
                    Upcoming::Hidden => 0.0,
                    Upcoming::Show => 1.0,
                }
            } * enter;
            let mut tr = base;
            if active && self.st.pop > 0.0 {
                // Rise to 1 in 80 ms, settle over 220 ms.
                let k = t - ws;
                let bump = if k < 0.08 { k / 0.08 } else { (1.0 - (k - 0.08) / 0.22).max(0.0) };
                let s = 1.0 + self.st.pop * bump * bump * (3.0 - 2.0 * bump);
                let cx = u.rect[0] + u.rect[2] * 0.5;
                let cy = u.rect[1] + u.rect[3] * 0.5;
                tr = dl::mul(&base, &[s, 0.0, 0.0, s, cx - s * cx, cy - s * cy]);
            }
            if active {
                if let Some(bx) = self.st.highlight_box {
                    let (px, py) = (0.14 * em, 0.04 * em);
                    out.push(Op::RoundRect {
                        x: u.rect[0] - px,
                        y: u.rect[1] - py,
                        w: u.rect[2] + 2.0 * px,
                        h: u.rect[3] + 2.0 * py,
                        radius: 0.18 * em,
                        paint: Paint::Solid(with_alpha(bx.0, enter)),
                        transform: tr,
                    });
                }
            }
            let color = if active { self.st.highlight.map(|c| c.0) } else { None };
            l.unit_ops(u, alpha, color, tr, out);
        }
        l.end_effects(out);
    }
}

fn with_alpha(c: Rgba, a: f32) -> Rgba {
    let mut v = c.0;
    v[3] *= a;
    Rgba(v)
}

/// Average adult silent reading speed for on-screen text, words per second.
pub const READ_WPS: f32 = 3.5;

/// Seconds a viewer needs to read `text` comfortably on screen
/// (`0.6 s` to notice it plus words at [`READ_WPS`]; CJK counts ~2 characters per word).
pub fn reading_seconds(text: &str) -> f32 {
    0.6 + word_count(text) / READ_WPS
}

/// Word count with CJK characters counted as half a word each.
pub fn word_count(text: &str) -> f32 {
    let mut n = 0.0;
    for w in text.split_whitespace() {
        let cjk = w.chars().filter(|c| is_cjk(*c)).count();
        n += if cjk > 0 { cjk as f32 * 0.5 + (w.chars().count() - cjk).min(1) as f32 } else { 1.0 };
    }
    n
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff)
}

/// Trim `text` at a word boundary so it reads in `seconds` (adds `…` if cut).
pub fn fit_to_seconds(text: &str, seconds: f32) -> String {
    if reading_seconds(text) <= seconds {
        return text.to_string();
    }
    let budget = ((seconds - 0.6).max(0.0) * READ_WPS).floor() as usize;
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut s = words[..budget.min(words.len()).max(1)].join(" ");
    s = s.trim_end_matches([',', ';', ':', '.']).to_string();
    s.push('…');
    s
}
