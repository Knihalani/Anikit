//! The text interface the evaluator needs, plus a metrics-only stand-in.
//!
//! The real implementation (fonts, shaping, wrapping) lives in `crate::text`
//! and implements [`TextEngine`]. The evaluator only needs positioned glyphs
//! with their character / word / line indices (for `split`, `reveal` and
//! per-unit animation), and the block size (for anchoring and layout).

use crate::dl::FontId;

/// What to lay out.
#[derive(Clone, Debug, PartialEq)]
pub struct TextReq<'a> {
    pub text: &'a str,
    /// Resolved family name (`"Inter"`).
    pub family: &'a str,
    pub size: f32,
    pub weight: f32,
    pub italic: bool,
    /// 0 left, 1 centre, 2 right.
    pub align: u8,
    /// Multiple of `size`.
    pub line_height: f32,
    /// Extra px between characters.
    pub letter_spacing: f32,
    /// Wrap width in px.
    pub max_width: Option<f32>,
    pub max_lines: Option<u32>,
}

/// One positioned glyph, in the block's local space (origin top-left; `y` is the baseline).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PGlyph {
    pub font: FontId,
    pub id: u16,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    /// Index of the source character (in chars, not bytes).
    pub char_index: u32,
    pub word: u32,
    pub line: u32,
}

/// A laid-out block of text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextBlock {
    /// Width and height in px.
    pub size: [f32; 2],
    pub glyphs: Vec<PGlyph>,
    /// Total characters, words and lines (for `split` / `reveal`).
    pub chars: u32,
    pub words: u32,
    pub lines: u32,
    /// Font size used (glyph ops carry it).
    pub font_size: f32,
}

/// Something that can lay out text. Shared read-only across render workers.
pub trait TextEngine: Send + Sync {
    fn layout(&self, req: &TextReq) -> TextBlock;
}

/// Metrics-only stand-in: monospaced-ish advances, greedy word wrap. Glyph ids
/// are the low 16 bits of the char and the font is `FontId(u32::MAX)` so a
/// renderer can recognise placeholder text.
pub struct ApproxText;

pub const PLACEHOLDER_FONT: FontId = FontId(u32::MAX);

impl TextEngine for ApproxText {
    fn layout(&self, r: &TextReq) -> TextBlock {
        let mono = r.family.to_ascii_lowercase().contains("mono");
        let adv_of = |c: char| -> f32 {
            let k = if mono {
                0.6
            } else if c == ' ' {
                0.28
            } else if "iljt.,:;'!|".contains(c) {
                0.3
            } else if "mwMW".contains(c) {
                0.85
            } else if c.is_uppercase() {
                0.66
            } else {
                0.54
            };
            k * r.size * (0.92 + r.weight / 5000.0) + r.letter_spacing
        };
        let lh = r.size * r.line_height;
        // Break into lines of words.
        let mut lines: Vec<Vec<(usize, char)>> = vec![Vec::new()];
        let chars: Vec<char> = r.text.chars().collect();
        let mut i = 0;
        let mut line_w = 0.0f32;
        while i < chars.len() {
            let c = chars[i];
            if c == '\n' {
                lines.push(Vec::new());
                line_w = 0.0;
                i += 1;
                continue;
            }
            // Measure the next word (including leading spaces).
            let mut j = i;
            let mut w = 0.0;
            while j < chars.len() && chars[j] == ' ' {
                w += adv_of(' ');
                j += 1;
            }
            while j < chars.len() && chars[j] != ' ' && chars[j] != '\n' {
                w += adv_of(chars[j]);
                j += 1;
            }
            if let Some(mw) = r.max_width {
                if line_w > 0.0 && line_w + w > mw {
                    lines.push(Vec::new());
                    line_w = 0.0;
                    while i < chars.len() && chars[i] == ' ' {
                        i += 1;
                    }
                    continue;
                }
            }
            for (k, &c) in chars.iter().enumerate().take(j).skip(i) {
                lines.last_mut().unwrap().push((k, c));
            }
            line_w += w;
            i = j;
        }
        if let Some(m) = r.max_lines {
            lines.truncate(m.max(1) as usize);
        }
        let widths: Vec<f32> = lines.iter().map(|l| l.iter().map(|&(_, c)| adv_of(c)).sum()).collect();
        let bw = r.max_width.unwrap_or(0.0).max(widths.iter().cloned().fold(0.0, f32::max));
        let bw = if r.max_width.is_some() { widths.iter().cloned().fold(0.0, f32::max) } else { bw };
        let mut glyphs = Vec::new();
        let mut word = 0u32;
        let mut prev_space = true;
        for (li, l) in lines.iter().enumerate() {
            let mut x = match r.align {
                0 => 0.0,
                2 => bw - widths[li],
                _ => (bw - widths[li]) * 0.5,
            };
            let y = li as f32 * lh + (lh - r.size) * 0.5 + r.size * 0.8;
            prev_space = true;
            for &(k, c) in l {
                let a = adv_of(c);
                if c == ' ' {
                    prev_space = true;
                } else {
                    if prev_space && !glyphs.is_empty() {
                        word += 1;
                    }
                    prev_space = false;
                    glyphs.push(PGlyph {
                        font: PLACEHOLDER_FONT,
                        id: c as u32 as u16,
                        x,
                        y,
                        advance: a,
                        char_index: k as u32,
                        word,
                        line: li as u32,
                    });
                }
                x += a;
            }
        }
        let _ = prev_space;
        TextBlock {
            size: [bw, lines.len() as f32 * lh],
            chars: chars.len() as u32,
            words: if glyphs.is_empty() { 0 } else { word + 1 },
            lines: lines.len() as u32,
            glyphs,
            font_size: r.size,
        }
    }
}
